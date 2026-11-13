//! Checks the approved crates.io payload from revision
//! 0e82636fde8b2afb5e712ede6c261166ea10fc17, with exact patched and added files.

use flate2::read::GzDecoder;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::str::from_utf8;

const ARCHIVE_PATH: &str = "vendor/jsonschema-macros-core-0.61.2.crate";
const VENDOR_PATH: &str = "vendor/jsonschema-macros-core";
const ARCHIVE_PREFIX: &str = "jsonschema-macros-core-0.61.2/";
const ARCHIVE_SHA256: &str = "75afa4b5e6986cc221f5339d7d6149360f0bf8e65bcb23617dc0f92900b576d2";
const PATCHED_FILES: [(&str, &str); 9] = [
    (
        "Cargo.toml",
        "7e20e3d63945bdeed04afb10a8763be91419577c510891e87a5bbbd1eaf559c3",
    ),
    (
        "Cargo.toml.orig",
        "467286fa79c0c70a4952e386a960af50874868ff40caaf1e44722daf97436d6d",
    ),
    (
        "src/lib.rs",
        "8fdaf9996475baf95ad42a820d2f9e1a8b7d3fe80e3bb9144b624507c150e5cd",
    ),
    (
        "src/context.rs",
        "ce580d3bcb085388ed8c11cd20e32c00789c972f461402d36233d799d93f6da5",
    ),
    (
        "src/codegen.rs",
        "68d9a7a692f6f9891ab9e19077ec24d22b45cb884a9281664a0c947c0c9ab35f",
    ),
    (
        "src/codegen/object_schema.rs",
        "c5fe64170d5154c5967280d28c2fcb269c24d49ef756322d269ac8eb8998d912",
    ),
    (
        "src/codegen/emit_root.rs",
        "5a757b0f4d360fcc9fb8dce7af0031b107067be9f5c451321ec94d49ff3fb2f8",
    ),
    (
        "src/codegen/regex.rs",
        "bb2103f10ab1f5a14ed490401dc74f24ddcb6a875146f35eaed581de25c4994b",
    ),
    (
        "src/tests.rs",
        "a16c557affa559362f19e5fd49823b0ed48314cee8d45a15fe072ac7aaef56c9",
    ),
];
const ADDED_FILES: [(&str, &str); 1] = [(
    "src/codegen/explicit_cache.rs",
    "d8f20ac8b4265b1a04b4cc9eb898f753c2abd9dc812784381b841893ebfdc159",
)];
const LICENSE_SHA256: &str = "117829c3ca21efb132d81a44b55363d395ab8eea18526873bc828da4c0e5f038";
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MEMBER_BYTES: u64 = 512 * 1024;
const MAX_MEMBERS: usize = 512;

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn directory(path: &Path) -> Result<(), String> {
    if !fs::symlink_metadata(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .is_dir()
    {
        return Err(format!("not a directory: {}", path.display()));
    }
    Ok(())
}

fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("not a regular file: {}", path.display()));
    }
    if metadata.len() > limit {
        return Err(format!("file size limit: {}", path.display()));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(limit).read_to_end(&mut bytes))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(bytes)
}

fn published_files(raw: &[u8]) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let mut decoded = GzDecoder::new(raw).take(MAX_BYTES);
    let mut bytes = Vec::new();
    decoded
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if decoded.limit() == 0 {
        return Err("expanded archive size limit".into());
    }
    let mut archive = tar::Archive::new(bytes.as_slice());
    let mut files = BTreeMap::new();
    for (index, entry_result) in archive
        .entries()
        .map_err(|error| error.to_string())?
        .enumerate()
    {
        if index >= MAX_MEMBERS {
            return Err("archive member limit".into());
        }
        let mut entry = entry_result.map_err(|error| error.to_string())?;
        if !entry.header().entry_type().is_file() || entry.size() > MAX_MEMBER_BYTES {
            return Err("nonregular or oversized archive member".into());
        }
        let encoded_path = entry.path_bytes();
        let name = from_utf8(&encoded_path).map_err(|error| error.to_string())?;
        let relative = name
            .strip_prefix(ARCHIVE_PREFIX)
            .ok_or("archive path escape")?;
        if relative
            .split('/')
            .any(|part| matches!(part, "" | "." | "..") || part.contains(['\\', ':']))
        {
            return Err("archive path escape".into());
        }
        let path = PathBuf::from(relative);
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|error| error.to_string())?;
        if files.insert(path, content).is_some() {
            return Err("duplicate archive member".into());
        }
    }
    if files.len() != 280 {
        return Err("expected 280 published files".into());
    }
    Ok(files)
}

pub fn verify(root: &Path) -> Result<PathBuf, String> {
    let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
    directory(&canonical_root.join("vendor"))?;
    let raw = read_file(&canonical_root.join(ARCHIVE_PATH), MAX_BYTES)?;
    if sha256(&raw) != ARCHIVE_SHA256 {
        return Err("archive SHA-256 differs from approved crates.io archive".into());
    }
    // Every published member must remain present; only individually pinned bytes may differ.
    let mut expected = published_files(&raw)?;
    let mut added: BTreeMap<_, _> = ADDED_FILES
        .iter()
        .map(|(name, checksum)| (PathBuf::from(*name), *checksum))
        .collect();
    let vendor = canonical_root.join(VENDOR_PATH);
    directory(&vendor)?;
    let mut pending = vec![PathBuf::new()];
    let mut license_seen = false;
    while let Some(parent) = pending.pop() {
        for entry_result in fs::read_dir(vendor.join(parent)).map_err(|error| error.to_string())? {
            let entry = entry_result.map_err(|error| error.to_string())?;
            let path = entry.path();
            let relative = path
                .strip_prefix(&vendor)
                .map_err(|error| error.to_string())?;
            let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if metadata.is_dir()
                && expected
                    .keys()
                    .chain(added.keys())
                    .any(|name| name.starts_with(relative))
            {
                pending.push(relative.to_owned());
                continue;
            }
            if !metadata.is_file() {
                return Err(format!("unexpected vendor entry: {}", relative.display()));
            }
            let original = expected.remove(relative);
            let addition = added.remove(relative);
            if original.is_none() && addition.is_none() && relative != Path::new("LICENSE") {
                return Err(format!("unexpected vendor entry: {}", relative.display()));
            }
            let actual = read_file(&path, MAX_MEMBER_BYTES)?;
            if let Some(expected_sha256) = addition {
                if sha256(&actual) != expected_sha256 {
                    return Err(format!(
                        "added SHA-256 differs from approved addition: {}",
                        relative.display()
                    ));
                }
                continue;
            }
            if let Some((_, expected_sha256)) = PATCHED_FILES
                .iter()
                .find(|(name, _)| relative == Path::new(name))
            {
                if sha256(&actual) != *expected_sha256 {
                    return Err(format!(
                        "patched SHA-256 differs from approved patch: {}",
                        relative.display()
                    ));
                }
                continue;
            }
            if relative == Path::new("LICENSE") {
                if sha256(&actual) != LICENSE_SHA256 {
                    return Err("LICENSE SHA-256 differs from approved license".into());
                }
                license_seen = true;
                continue;
            }
            if original.as_deref() != Some(actual.as_slice()) {
                return Err(format!("vendor bytes differ: {}", relative.display()));
            }
        }
    }
    if !expected.is_empty() {
        return Err("missing vendor files".into());
    }
    if !added.is_empty() {
        return Err(format!(
            "missing added vendor files: {}",
            added
                .keys()
                .map(|name| name.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !license_seen {
        return Err("missing LICENSE".into());
    }
    vendor.canonicalize().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ADDED_FILES, ARCHIVE_PATH, PATCHED_FILES, VENDOR_PATH, published_files, sha256, verify,
    };
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::Path;
    use tempfile::TempDir;

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry_result in fs::read_dir(source).unwrap() {
            let entry = entry_result.unwrap();
            let target = destination.join(entry.file_name());
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            if metadata.is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                assert!(metadata.is_file());
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn fixture() -> TempDir {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temporary = tempfile::tempdir().unwrap();
        fs::create_dir_all(temporary.path().join("vendor")).unwrap();
        fs::copy(root.join(ARCHIVE_PATH), temporary.path().join(ARCHIVE_PATH)).unwrap();
        copy_tree(&root.join(VENDOR_PATH), &temporary.path().join(VENDOR_PATH));
        assert_eq!(
            verify(temporary.path()).unwrap(),
            temporary.path().join(VENDOR_PATH).canonicalize().unwrap()
        );
        temporary
    }

    #[test]
    fn approved_fixture_returns_only_the_canonical_vendor_path() {
        let temporary = fixture();
        assert_eq!(
            verify(&temporary.path().join("vendor/..")).unwrap(),
            temporary.path().join(VENDOR_PATH).canonicalize().unwrap()
        );
    }

    #[test]
    fn rejects_wrong_archive_before_using_its_contents() {
        let temporary = fixture();
        fs::write(
            temporary.path().join(ARCHIVE_PATH),
            b"not the approved archive",
        )
        .unwrap();
        assert!(
            verify(temporary.path())
                .unwrap_err()
                .contains("archive SHA-256")
        );
    }

    #[test]
    fn rejects_edits_to_unpatched_source_lock_and_vcs_provenance() {
        let temporary = fixture();
        for name in [
            "src/codegen/dispatch.rs",
            "Cargo.lock",
            ".cargo_vcs_info.json",
        ] {
            let path = temporary.path().join(VENDOR_PATH).join(name);
            let original = fs::read(&path).unwrap();
            let mut changed = original.clone();
            changed.push(b'\n');
            fs::write(&path, changed).unwrap();
            let error = verify(temporary.path()).unwrap_err();
            assert!(error.contains("vendor bytes differ"), "{name}: {error}");
            assert!(error.contains(name), "{error}");
            fs::write(path, original).unwrap();
        }
    }

    #[test]
    fn rejects_reverting_each_approved_patch_to_the_published_source() {
        let temporary = fixture();
        let bytes = fs::read(temporary.path().join(ARCHIVE_PATH)).unwrap();
        let published = published_files(&bytes).unwrap();
        for (name, _) in PATCHED_FILES {
            let path = temporary.path().join(VENDOR_PATH).join(name);
            let approved = fs::read(&path).unwrap();
            fs::write(&path, &published[Path::new(name)]).unwrap();
            let result = verify(temporary.path());
            fs::write(&path, approved).unwrap();
            verify(temporary.path()).unwrap();
            let error = result.unwrap_err();
            assert!(error.contains("patched SHA-256"), "{name}: {error}");
            assert!(error.contains(name), "{error}");
        }
    }

    #[test]
    fn rejects_edits_to_each_approved_patch() {
        let temporary = fixture();
        for (name, _) in PATCHED_FILES {
            let path = temporary.path().join(VENDOR_PATH).join(name);
            let approved = fs::read(&path).unwrap();
            let mut changed = approved.clone();
            changed.push(b'\n');
            fs::write(&path, changed).unwrap();
            let result = verify(temporary.path());
            fs::write(&path, approved).unwrap();
            verify(temporary.path()).unwrap();
            let error = result.unwrap_err();
            assert!(error.contains("patched SHA-256"), "{name}: {error}");
            assert!(error.contains(name), "{error}");
        }
    }

    #[test]
    fn patch_and_addition_membership_preserves_all_280_published_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let published = published_files(&fs::read(root.join(ARCHIVE_PATH)).unwrap()).unwrap();
        assert_eq!(published.len(), 280);
        for (name, _) in PATCHED_FILES {
            assert!(published.contains_key(Path::new(name)), "{name}");
        }
        for (name, _) in ADDED_FILES {
            assert!(!published.contains_key(Path::new(name)), "{name}");
        }
        assert!(!published.contains_key(Path::new("LICENSE")));
    }

    #[test]
    fn rejects_deleting_each_patched_published_file() {
        let temporary = fixture();
        for (name, _) in PATCHED_FILES {
            let path = temporary.path().join(VENDOR_PATH).join(name);
            let approved = fs::read(&path).unwrap();
            fs::remove_file(&path).unwrap();
            let result = verify(temporary.path());
            fs::write(&path, approved).unwrap();
            verify(temporary.path()).unwrap();
            assert!(
                result.unwrap_err().contains("missing vendor files"),
                "{name}"
            );
        }
    }

    #[test]
    fn rejects_changed_deleted_or_renamed_added_files() {
        let temporary = fixture();
        for (name, _) in ADDED_FILES {
            let path = temporary.path().join(VENDOR_PATH).join(name);
            let approved = fs::read(&path).unwrap();
            let mut changed = approved.clone();
            changed.push(b'\n');
            fs::write(&path, changed).unwrap();
            let changed_error = verify(temporary.path()).unwrap_err();
            assert!(
                changed_error.contains("added SHA-256"),
                "{name}: {changed_error}"
            );
            assert!(changed_error.contains(name), "{changed_error}");

            fs::remove_file(&path).unwrap();
            let missing_error = verify(temporary.path()).unwrap_err();
            assert!(
                missing_error.contains("missing added vendor files"),
                "{missing_error}"
            );
            assert!(missing_error.contains(name), "{missing_error}");

            let renamed = path.with_extension("renamed");
            fs::write(&renamed, &approved).unwrap();
            assert!(verify(temporary.path()).is_err(), "renamed {name}");
            fs::remove_file(renamed).unwrap();
            fs::write(&path, approved).unwrap();
            verify(temporary.path()).unwrap();
        }
    }

    #[test]
    fn rejects_replacing_added_files_with_directories() {
        let temporary = fixture();
        for (name, _) in ADDED_FILES {
            let path = temporary.path().join(VENDOR_PATH).join(name);
            let approved = fs::read(&path).unwrap();
            fs::remove_file(&path).unwrap();
            fs::create_dir_all(&path).unwrap();
            assert!(verify(temporary.path()).is_err(), "{name}");
            fs::remove_dir(&path).unwrap();
            fs::write(&path, approved).unwrap();
            verify(temporary.path()).unwrap();
        }
    }

    #[test]
    fn rejects_reverting_to_the_two_patch_base_even_with_the_added_module_retained() {
        use std::process::{Command, Stdio};
        use std::thread;
        use std::time::{Duration, Instant};

        let temporary = fixture();
        let vendor = temporary.path().join(VENDOR_PATH);
        let additions: Vec<_> = ADDED_FILES
            .iter()
            .map(|(name, _)| (*name, fs::read(vendor.join(name)).unwrap()))
            .collect();
        let patch = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(
                "../../vendor/patches/jsonschema-macros-core-0.61.2-explicit-cache-all-modes.patch",
            )
            .canonicalize()
            .unwrap();
        let mut child = Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .current_dir(&vendor)
            .args(["apply", "--reverse"])
            .arg(patch)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "third patch must reverse to the two-patch base"
                );
                break;
            }
            if started.elapsed() >= Duration::from_secs(10) {
                let _killed = child.kill();
                let _reaped = child.wait();
                panic!("third-patch reversal exceeded its deadline");
            }
            thread::sleep(Duration::from_millis(10));
        }
        let published =
            published_files(&fs::read(temporary.path().join(ARCHIVE_PATH)).unwrap()).unwrap();
        for (name, original) in published {
            let actual = fs::read(vendor.join(&name)).unwrap();
            match name.to_str().unwrap() {
                "src/codegen/object_schema.rs" => assert_eq!(
                    sha256(&actual),
                    "c5fe64170d5154c5967280d28c2fcb269c24d49ef756322d269ac8eb8998d912"
                ),
                "src/codegen/emit_root.rs" => assert_eq!(
                    sha256(&actual),
                    "5a757b0f4d360fcc9fb8dce7af0031b107067be9f5c451321ec94d49ff3fb2f8"
                ),
                _ => assert_eq!(actual, original, "{}", name.display()),
            }
        }
        for (name, _) in &additions {
            assert!(!vendor.join(name).exists(), "{name}");
        }
        verify(temporary.path()).unwrap_err();
        // Restoring the added module must not hide reverted original members.
        for (name, bytes) in additions {
            fs::write(vendor.join(name), bytes).unwrap();
        }
        let error = verify(temporary.path()).unwrap_err();
        assert!(error.contains("patched SHA-256"), "{error}");
    }

    #[test]
    fn rejects_missing_published_files() {
        let temporary = fixture();
        fs::remove_file(temporary.path().join(VENDOR_PATH).join("src/lib.rs")).unwrap();
        assert!(
            verify(temporary.path())
                .unwrap_err()
                .contains("missing vendor files")
        );
    }

    #[test]
    fn rejects_extra_files_and_empty_directories() {
        let temporary = fixture();
        for name in ["unpublished", "src/codegen/explicit_cache_unreviewed.rs"] {
            let extra = temporary.path().join(VENDOR_PATH).join(name);
            fs::write(&extra, b"extra").unwrap();
            assert!(
                verify(temporary.path())
                    .unwrap_err()
                    .contains("unexpected vendor entry")
            );
            fs::remove_file(&extra).unwrap();
            fs::create_dir_all(&extra).unwrap();
            assert!(
                verify(temporary.path())
                    .unwrap_err()
                    .contains("unexpected vendor entry")
            );
            fs::remove_dir(extra).unwrap();
        }
    }

    #[test]
    fn rejects_modified_or_missing_license() {
        let temporary = fixture();
        let license = temporary.path().join(VENDOR_PATH).join("LICENSE");
        fs::write(&license, b"unapproved license").unwrap();
        assert!(
            verify(temporary.path())
                .unwrap_err()
                .contains("LICENSE SHA-256")
        );
        fs::remove_file(license).unwrap();
        assert!(
            verify(temporary.path())
                .unwrap_err()
                .contains("missing LICENSE")
        );
    }

    #[test]
    fn rejects_oversized_archive_and_vendor_member() {
        let temporary = fixture();
        let source = temporary.path().join(VENDOR_PATH).join("src/lib.rs");
        fs::File::create(source)
            .unwrap()
            .set_len(512 * 1024 + 1)
            .unwrap();
        assert!(
            verify(temporary.path())
                .unwrap_err()
                .contains("file size limit")
        );
        fs::File::create(temporary.path().join(ARCHIVE_PATH))
            .unwrap()
            .set_len(4 * 1024 * 1024 + 1)
            .unwrap();
        assert!(
            verify(temporary.path())
                .unwrap_err()
                .contains("file size limit")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_members_directories_archive_and_vendor_roots() {
        for relative in [
            "vendor/jsonschema-macros-core/src/lib.rs",
            "vendor/jsonschema-macros-core/src",
            ARCHIVE_PATH,
            VENDOR_PATH,
            "vendor",
        ] {
            let temporary = fixture();
            let path = temporary.path().join(relative);
            let saved = temporary.path().join("saved");
            fs::rename(&path, &saved).unwrap();
            symlink(saved, path).unwrap();
            assert!(verify(temporary.path()).is_err(), "{relative}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_for_every_patched_and_added_file() {
        let temporary = fixture();
        for name in PATCHED_FILES
            .iter()
            .map(|(name, _)| *name)
            .chain(ADDED_FILES.iter().map(|(name, _)| *name))
            .chain(["LICENSE"])
        {
            let path = temporary.path().join(VENDOR_PATH).join(name);
            let saved = temporary.path().join("saved");
            fs::rename(&path, &saved).unwrap();
            symlink(&saved, &path).unwrap();
            assert!(verify(temporary.path()).is_err(), "{name}");
            fs::remove_file(&path).unwrap();
            fs::rename(saved, &path).unwrap();
            verify(temporary.path()).unwrap();
        }
    }
}
