//! Reference conversion is independent of runtime promotion and endpoint approval.

use super::inventory::{self, Document};
use super::{Error, import_packages, import_projection};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions, symlink_metadata};
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};
use tempfile::Builder;

#[cfg(unix)]
use rustix::fs::OFlags;

const MAX_TREE_BYTES: usize = 16 << 20;
const MAX_FILE_BYTES: usize = 1 << 20;
pub(super) type Files = BTreeMap<PathBuf, Vec<u8>>;

pub(super) fn name(document: &Document, root: &str, extension: &str) -> Result<String, Error> {
    let result = document
        .entry
        .path
        .strip_prefix(root)
        .and_then(|path| path.strip_suffix(extension))
        .ok_or(Error::InvalidSource)?;
    if result.is_empty()
        || !result
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(Error::InvalidSource);
    }
    Ok(result.to_owned())
}

pub(super) fn insert(files: &mut Files, path: PathBuf, bytes: Vec<u8>) -> Result<(), Error> {
    let total = files
        .values()
        .try_fold(bytes.len(), |sum, value| sum.checked_add(value.len()))
        .ok_or(Error::SourceLimit)?;
    if bytes.len() > MAX_FILE_BYTES || total > MAX_TREE_BYTES {
        return Err(Error::SourceLimit);
    }
    if files.insert(path, bytes).is_some() {
        return Err(Error::InvalidSource);
    }
    Ok(())
}

async fn collect(source: &Path) -> Result<Files, Error> {
    import_packages::collect(inventory::capture(source).await?)
}

fn read_file(file: fs::File, length: u64) -> Result<Vec<u8>, Error> {
    if length > u64::try_from(MAX_FILE_BYTES).map_err(|_error| Error::ImportInput)? {
        return Err(Error::ImportInput);
    }
    let mut bytes = Vec::new();
    file.take(length.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_error| Error::ImportInput)?;
    if u64::try_from(bytes.len()).ok() != Some(length) {
        return Err(Error::ImportInput);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_directory(root: &Path) -> Result<Option<fs::File>, Error> {
    use rustix::fs::{CWD, Mode, openat};
    use rustix::io::Errno;
    if root.as_os_str().is_empty()
        || root
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(Error::ImportInput);
    }
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let mut directory = openat(
        CWD,
        if root.is_absolute() { "/" } else { "." },
        flags,
        Mode::empty(),
    )
    .map_err(|_error| Error::ImportInput)?;
    for part in root.components() {
        let Component::Normal(component) = part else {
            continue;
        };
        directory = match openat(&directory, component, flags, Mode::empty()) {
            Ok(next) => next,
            Err(Errno::NOENT) => return Ok(None),
            Err(_) => return Err(Error::ImportInput),
        };
    }
    Ok(Some(fs::File::from(directory)))
}

#[cfg(unix)]
fn check_tree(root: &Path, files: &Files) -> Result<bool, Error> {
    use rustix::fs::{Dir, Mode, openat};
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;
    let Some(directory) = open_directory(root)? else {
        return Ok(false);
    };
    let mut pending = vec![(directory, PathBuf::new())];
    let mut actual = Files::new();
    let mut directories = Vec::new();
    let mut members = 0_usize;
    while let Some((handle, prefix)) = pending.pop() {
        let stream = Dir::read_from(&handle).map_err(|_error| Error::ImportInput)?;
        for item in stream {
            let entry = item.map_err(|_error| Error::ImportInput)?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            let relative = prefix.join(OsStr::from_bytes(name.to_bytes()));
            members = members.checked_add(1).ok_or(Error::ImportInput)?;
            if members > 10_000 || relative.components().count() > 128 {
                return Err(Error::ImportInput);
            }
            let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
            let descriptor =
                openat(&handle, name, flags, Mode::empty()).map_err(|_error| Error::ImportInput)?;
            let file = fs::File::from(descriptor);
            let metadata = file.metadata().map_err(|_error| Error::ImportInput)?;
            if metadata.is_dir() {
                directories.push(relative.clone());
                pending.push((file, relative));
            } else if metadata.is_file() {
                insert(&mut actual, relative, read_file(file, metadata.len())?)
                    .map_err(|_error| Error::ImportInput)?;
            } else {
                return Err(Error::ImportInput);
            }
        }
    }
    if directories
        .iter()
        .any(|prefix| !actual.keys().any(|path| path.starts_with(prefix)))
    {
        return Ok(false);
    }
    import_projection::matches(&actual, files)
}

#[cfg(not(unix))]
fn check_tree(_root: &Path, _files: &Files) -> Result<bool, Error> {
    Err(Error::ImportInput)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn publish(source: &Path, target: &Path) -> Result<(), Error> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    renameat_with(CWD, source, CWD, target, RenameFlags::NOREPLACE)
        .map_err(|_error| Error::ImportOutput)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn publish(_source: &Path, _target: &Path) -> Result<(), Error> {
    Err(Error::ImportOutput)
}

fn write_tree(target: &Path, files: &Files) -> Result<(), Error> {
    if target.file_name().is_none() || symlink_metadata(target).is_ok() {
        return Err(Error::ImportOutput);
    }
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = Builder::new()
        .prefix(".auctioneer-reference-")
        .tempdir_in(parent)
        .map_err(|_error| Error::ImportOutput)?;
    for (path, bytes) in files {
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(Error::ImportOutput);
        }
        let destination = temporary.path().join(path);
        if let Some(directory) = destination.parent() {
            fs::create_dir_all(directory).map_err(|_error| Error::ImportOutput)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|_error| Error::ImportOutput)?;
        file.write_all(bytes)
            .map_err(|_error| Error::ImportOutput)?;
        file.sync_all().map_err(|_error| Error::ImportOutput)?;
    }
    publish(temporary.path(), target)
}

pub(super) async fn run(source: &Path, staging: &Path, check: bool) -> Result<i32, Error> {
    let files = collect(source).await?;
    if check {
        Ok(i32::from(!check_tree(staging, &files)?))
    } else {
        write_tree(staging, &files)?;
        Ok(0_i32)
    }
}

#[cfg(test)]
mod tests {
    use super::super::import_packages::{RoutingReview, route_review};
    use super::super::source::Entry;
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn public_names_preserve_hyphens_without_allowing_path_escape() {
        for (candidate, allowed) in [
            ("fake_one-with-hyphen", true),
            ("fake_two-with-hyphen", true),
            ("fake_Bidder", true),
            ("../escape", false),
            ("a/b", false),
            ("a\\b", false),
            ("", false),
            (".", false),
        ] {
            let document = Document {
                entry: Entry {
                    path: format!("static/bidder-info/{candidate}.yaml"),
                    oid: "0".repeat(40),
                    bytes: 0,
                },
                bytes: Vec::new(),
            };
            let result = name(&document, "static/bidder-info/", ".yaml");
            if allowed {
                assert_eq!(result, Ok(candidate.to_owned()));
            } else {
                assert_eq!(result, Err(Error::InvalidSource));
            }
        }
    }

    #[test]
    fn source_activation_and_https_never_create_routing_approval() {
        for (document, reason) in [
            (
                json!({"endpoint": "http://bidder.invalid", "enabled": true, "disabled": false}),
                RoutingReview::HttpsDefaultPending,
            ),
            (
                json!({"endpoint": "https://bidder.invalid", "enabled": true}),
                RoutingReview::EndpointPolicyPending,
            ),
            (json!({"endpoint": ""}), RoutingReview::MissingEndpoint),
            (json!({}), RoutingReview::MissingEndpoint),
        ] {
            assert_eq!(route_review(&document, None), reason);
        }
        assert_eq!(
            route_review(&json!({"whiteLabelOnly": true}), None),
            RoutingReview::AliasOnly
        );
        assert_eq!(
            route_review(
                &json!({"aliasOf": "fake_parent", "endpoint": "https://alias.invalid"}),
                Some(&json!({"whiteLabelOnly": true}))
            ),
            RoutingReview::EndpointPolicyPending
        );
    }

    #[test]
    fn failed_tree_writes_are_cleaned_up_without_partial_publication_or_escape() {
        for files in [
            Files::from([
                (PathBuf::from("same"), b"x".to_vec()),
                (PathBuf::from("same/nested"), b"y".to_vec()),
            ]),
            Files::from([(PathBuf::from("../outside"), b"do not write".to_vec())]),
        ] {
            let root = tempdir().unwrap();
            let target = root.path().join("published");
            assert_eq!(write_tree(&target, &files), Err(Error::ImportOutput));
            assert!(!target.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        }
    }

    #[cfg(unix)]
    #[test]
    fn checking_refuses_live_directory_links_even_with_dot_slash_or_linked_ancestors() {
        use std::os::unix::fs::symlink;
        let temporary = tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let staging = root.join("staging");
        let files = Files::from([(PathBuf::from("manifest.json"), b"fixture".to_vec())]);
        write_tree(&staging, &files).unwrap();
        assert_eq!(check_tree(&staging, &files), Ok(true));
        let linked = root.join("linked");
        symlink(&staging, &linked).unwrap();
        for path in [
            &linked,
            &linked.join("."),
            &PathBuf::from(format!("{}/", linked.display())),
        ] {
            assert_eq!(
                check_tree(path, &files),
                Err(Error::ImportInput),
                "{}",
                path.display()
            );
        }
        let ancestor = root.join("ancestor");
        symlink(&root, &ancestor).unwrap();
        assert_eq!(
            check_tree(&ancestor.join("staging"), &files),
            Err(Error::ImportInput)
        );
    }

    #[cfg(unix)]
    #[test]
    fn staged_members_cannot_be_links_even_when_target_bytes_match() {
        use std::os::unix::fs::symlink;
        let temporary = tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let staging = root.join("staging");
        let files = Files::from([(PathBuf::from("metadata/a.json"), b"fixture".to_vec())]);
        write_tree(&staging, &files).unwrap();
        assert_eq!(check_tree(&staging, &files), Ok(true));
        let member = staging.join("metadata/a.json");
        let original = root.join("original");
        fs::rename(&member, &original).unwrap();
        symlink(&original, &member).unwrap();
        assert_eq!(check_tree(&staging, &files), Err(Error::ImportInput));
        fs::remove_file(&member).unwrap();
        fs::rename(original, member).unwrap();
        assert_eq!(check_tree(&staging, &files), Ok(true));
        let directory = staging.join("metadata");
        let real_directory = root.join("real-metadata");
        fs::rename(&directory, &real_directory).unwrap();
        symlink(real_directory, directory).unwrap();
        assert_eq!(check_tree(&staging, &files), Err(Error::ImportInput));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn publication_does_not_replace_even_an_empty_existing_directory() {
        let root = tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        assert_eq!(publish(&source, &target), Err(Error::ImportOutput));
        assert!(source.is_dir());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        fs::remove_dir(&target).unwrap();
        publish(&source, &target).unwrap();
        assert!(target.is_dir());
        assert!(!source.exists());
    }
}
