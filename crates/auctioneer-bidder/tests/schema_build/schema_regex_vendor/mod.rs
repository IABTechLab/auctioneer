use flate2::read::GzDecoder;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::from_utf8;
use std::time::Duration;

const ARCHIVE_PATH: &str = "vendor/regex-automata-0.4.18.crate";
const VENDOR_PATH: &str = "vendor/regex-automata";
const PATCH_PATH: &str = "vendor/patches/regex-automata-resource-layout.patch";
const PATCH_SHA256: &str = "5d853f05430eda9b9e80fb1f359cddce730dcf86a61be32efa2605a46702e020";
const ARCHIVE_SHA256: &str = "ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2";
const MAX_ARCHIVE_BYTES: u64 = 4 << 20;
const MAX_EXPANDED_BYTES: u64 = 16 << 20;
const MAX_MEMBER_BYTES: u64 = 2 << 20;
const MEMBERS: usize = 107;
const CHANGED_OWNERS: [&str; 3] = [
    "src/meta/mod.rs",
    "src/meta/regex.rs",
    "src/util/prefilter/mod.rs",
];

pub const FEATURES: [&str; 25] = [
    "alloc",
    "dfa-onepass",
    "dfa-search",
    "hybrid",
    "meta",
    "nfa",
    "nfa-backtrack",
    "nfa-pikevm",
    "nfa-thompson",
    "perf",
    "perf-inline",
    "perf-literal",
    "perf-literal-multisubstring",
    "perf-literal-substring",
    "std",
    "syntax",
    "unicode",
    "unicode-age",
    "unicode-bool",
    "unicode-case",
    "unicode-gencat",
    "unicode-perl",
    "unicode-script",
    "unicode-segment",
    "unicode-word-boundary",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifiedSource {
    pub canonical_root: PathBuf,
    pub archive_sha256: String,
    pub patch_sha256: String,
    pub tree_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifyError {
    Archive,
    Patch,
    Tree,
    Replay,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Archive => "regex archive identity refused",
            Self::Patch => "regex patch identity refused",
            Self::Tree => "regex source tree refused",
            Self::Replay => "regex patch replay refused",
        })
    }
}

type SourceFiles = BTreeMap<PathBuf, Vec<u8>>;

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn directory(path: &Path) -> Result<(), ()> {
    fs::symlink_metadata(path)
        .map_err(|_error| ())?
        .is_dir()
        .then_some(())
        .ok_or(())
}

fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>, ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_error| ())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(());
    }
    let mut bytes = Vec::new();
    let bounded = limit.checked_add(1).ok_or(())?;
    File::open(path)
        .and_then(|file| file.take(bounded).read_to_end(&mut bytes))
        .map_err(|_error| ())?;
    if u64::try_from(bytes.len()).map_err(|_error| ())? > limit {
        return Err(());
    }
    Ok(bytes)
}

fn published_files(raw: &[u8]) -> Result<SourceFiles, VerifyError> {
    let mut decoded = Vec::new();
    GzDecoder::new(raw)
        .take(MAX_EXPANDED_BYTES + 1)
        .read_to_end(&mut decoded)
        .map_err(|_error| VerifyError::Archive)?;
    if u64::try_from(decoded.len()).map_err(|_error| VerifyError::Archive)? > MAX_EXPANDED_BYTES {
        return Err(VerifyError::Archive);
    }
    let mut archive = tar::Archive::new(decoded.as_slice());
    let mut files = BTreeMap::new();
    for (index, entry_result) in archive
        .entries()
        .map_err(|_error| VerifyError::Archive)?
        .enumerate()
    {
        let mut entry = entry_result.map_err(|_error| VerifyError::Archive)?;
        if index >= MEMBERS
            || !entry.header().entry_type().is_file()
            || entry.size() > MAX_MEMBER_BYTES
        {
            return Err(VerifyError::Archive);
        }
        let encoded = entry.path_bytes();
        let name = from_utf8(&encoded).map_err(|_error| VerifyError::Archive)?;
        let relative = name
            .strip_prefix("regex-automata-0.4.18/")
            .ok_or(VerifyError::Archive)?;
        if relative
            .split('/')
            .any(|part| matches!(part, "" | "." | "..") || part.contains(['\\', ':']))
        {
            return Err(VerifyError::Archive);
        }
        let relative_path = PathBuf::from(relative);
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|_error| VerifyError::Archive)?;
        if files.insert(relative_path, bytes).is_some() {
            return Err(VerifyError::Archive);
        }
    }
    if files.len() != MEMBERS {
        return Err(VerifyError::Archive);
    }
    let vcs: Value = serde_json::from_slice(
        files
            .get(Path::new(".cargo_vcs_info.json"))
            .ok_or(VerifyError::Archive)?,
    )
    .map_err(|_error| VerifyError::Archive)?;
    if vcs["git"]["sha1"] != "e6ad938198e68339a593f85d228f2d44cb54718f"
        || vcs["path_in_vcs"] != "regex-automata"
    {
        return Err(VerifyError::Archive);
    }
    Ok(files)
}

fn source_tree(vendor: &Path) -> Result<SourceFiles, VerifyError> {
    directory(vendor).map_err(|()| VerifyError::Tree)?;
    let mut files = BTreeMap::new();
    let mut directories = BTreeSet::new();
    let mut pending = vec![PathBuf::new()];
    let mut total = 0_u64;
    while let Some(parent) = pending.pop() {
        for entry_result in
            fs::read_dir(vendor.join(&parent)).map_err(|_error| VerifyError::Tree)?
        {
            let entry = entry_result.map_err(|_error| VerifyError::Tree)?;
            let relative = parent.join(entry.file_name());
            let name = relative.to_str().ok_or(VerifyError::Tree)?;
            if name.len() > 512 || relative.components().count() > 32 {
                return Err(VerifyError::Tree);
            }
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_error| VerifyError::Tree)?;
            if metadata.is_dir() {
                if directories.len() >= 256 {
                    return Err(VerifyError::Tree);
                }
                directories.insert(relative.clone());
                pending.push(relative);
                continue;
            }
            if files.len() >= MEMBERS {
                return Err(VerifyError::Tree);
            }
            let bytes =
                read_file(&entry.path(), MAX_MEMBER_BYTES).map_err(|()| VerifyError::Tree)?;
            total = total
                .checked_add(u64::try_from(bytes.len()).map_err(|_error| VerifyError::Tree)?)
                .ok_or(VerifyError::Tree)?;
            if total > MAX_EXPANDED_BYTES {
                return Err(VerifyError::Tree);
            }
            files.insert(relative, bytes);
        }
    }
    if directories
        .iter()
        .any(|directory| !files.keys().any(|path| path.starts_with(directory)))
    {
        return Err(VerifyError::Tree);
    }
    Ok(files)
}

fn replay(original: &SourceFiles, patch_bytes: &[u8]) -> Result<SourceFiles, VerifyError> {
    let temporary = tempfile::tempdir().map_err(|_error| VerifyError::Replay)?;
    let source = temporary.path().join("source");
    for (relative, bytes) in original {
        let destination = source.join(relative);
        fs::create_dir_all(destination.parent().ok_or(VerifyError::Replay)?)
            .map_err(|_error| VerifyError::Replay)?;
        fs::write(destination, bytes).map_err(|_error| VerifyError::Replay)?;
    }
    let patch_path = temporary.path().join("reviewed.patch");
    fs::write(&patch_path, patch_bytes).map_err(|_error| VerifyError::Replay)?;
    for arguments in [&["apply", "--check"][..], &["apply"][..]] {
        let output = super::run(
            Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CEILING_DIRECTORIES", temporary.path())
                .current_dir(&source)
                .args(arguments)
                .arg(&patch_path),
            Duration::from_secs(10),
        )
        .map_err(|_error| VerifyError::Replay)?;
        if !output.status.success() || (arguments.len() == 2 && source_tree(&source)? != *original)
        {
            return Err(VerifyError::Replay);
        }
    }
    source_tree(&source)
}

#[expect(
    clippy::little_endian_bytes,
    reason = "tree identities frame lengths in a fixed host-independent byte order"
)]
fn tree_digest(files: &SourceFiles) -> Result<String, VerifyError> {
    let mut checksum = Sha256::new();
    for (path, bytes) in files {
        let name = path.to_str().ok_or(VerifyError::Tree)?.as_bytes();
        checksum.update(
            u64::try_from(name.len())
                .map_err(|_error| VerifyError::Tree)?
                .to_le_bytes(),
        );
        checksum.update(name);
        checksum.update(
            u64::try_from(bytes.len())
                .map_err(|_error| VerifyError::Tree)?
                .to_le_bytes(),
        );
        checksum.update(bytes);
    }
    Ok(hex::encode(checksum.finalize()))
}

pub fn verify(root: &Path) -> Result<VerifiedSource, VerifyError> {
    let workspace_root = root.canonicalize().map_err(|_error| VerifyError::Tree)?;
    directory(&workspace_root.join("vendor")).map_err(|()| VerifyError::Tree)?;
    let archive = read_file(&workspace_root.join(ARCHIVE_PATH), MAX_ARCHIVE_BYTES)
        .map_err(|()| VerifyError::Archive)?;
    if digest(&archive) != ARCHIVE_SHA256 {
        return Err(VerifyError::Archive);
    }
    let original = published_files(&archive)?;
    directory(&workspace_root.join("vendor/patches")).map_err(|()| VerifyError::Patch)?;
    let patch = read_file(&workspace_root.join(PATCH_PATH), MAX_MEMBER_BYTES)
        .map_err(|()| VerifyError::Patch)?;
    if digest(&patch) != PATCH_SHA256 {
        return Err(VerifyError::Patch);
    }
    let canonical_root = workspace_root.join(VENDOR_PATH);
    let actual = source_tree(&canonical_root)?;
    let reviewed = replay(&original, &patch)?;
    let changed = original
        .iter()
        .filter_map(|(path, bytes)| (reviewed.get(path) != Some(bytes)).then_some(path.to_str()))
        .collect::<Vec<_>>();
    if changed != CHANGED_OWNERS.map(Some) || reviewed.keys().ne(original.keys()) {
        return Err(VerifyError::Replay);
    }
    if actual != reviewed {
        return Err(VerifyError::Tree);
    }
    Ok(VerifiedSource {
        canonical_root: canonical_root
            .canonicalize()
            .map_err(|_error| VerifyError::Tree)?,
        archive_sha256: ARCHIVE_SHA256.into(),
        patch_sha256: PATCH_SHA256.into(),
        tree_sha256: tree_digest(&reviewed)?,
    })
}

pub fn verify_metadata(bytes: &[u8], vendor: &Path) -> Result<Vec<String>, String> {
    let refused = "engine metadata does not resolve the reviewed vendor";
    let document: Value = serde_json::from_slice(bytes).map_err(|_error| refused)?;
    let packages = document["packages"].as_array().ok_or(refused)?;
    let mut matching = packages
        .iter()
        .filter(|package| package["name"].as_str() == Some("regex-automata"));
    let package = matching.next().ok_or(refused)?;
    if matching.next().is_some()
        || package["version"].as_str() != Some("0.4.18")
        || !package.get("source").is_some_and(Value::is_null)
        || package["manifest_path"].as_str() != vendor.join("Cargo.toml").to_str()
    {
        return Err(refused.into());
    }
    let id = package["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or(refused)?;
    let feature_refusal = "engine resolved feature closure differs";
    let nodes = document["resolve"]["nodes"]
        .as_array()
        .ok_or(feature_refusal)?;
    let mut matching_nodes = nodes.iter().filter(|node| node["id"].as_str() == Some(id));
    let node = matching_nodes.next().ok_or(feature_refusal)?;
    if matching_nodes.next().is_some() {
        return Err(feature_refusal.into());
    }
    let features = node["features"].as_array().ok_or(feature_refusal)?;
    let actual = features
        .iter()
        .map(|feature| feature.as_str().ok_or(feature_refusal))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if actual.len() != features.len() || actual != BTreeSet::from(FEATURES) {
        return Err(feature_refusal.into());
    }
    Ok(actual.into_iter().map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

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

    fn fixture() -> tempfile::TempDir {
        let temporary = tempfile::tempdir().unwrap();
        fs::create_dir_all(temporary.path().join("vendor/patches")).unwrap();
        for relative in [ARCHIVE_PATH, PATCH_PATH] {
            fs::copy(workspace().join(relative), temporary.path().join(relative)).unwrap();
        }
        copy_tree(
            &workspace().join(VENDOR_PATH),
            &temporary.path().join(VENDOR_PATH),
        );
        verify(temporary.path()).unwrap();
        temporary
    }

    #[test]
    fn exact_replay_returns_a_canonical_source_identity() {
        let temporary = fixture();
        let verified = verify(&temporary.path().join("vendor/..")).unwrap();
        assert_eq!(
            verified.canonical_root,
            temporary.path().join(VENDOR_PATH).canonicalize().unwrap()
        );
        assert_eq!(
            verified.archive_sha256,
            "ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2"
        );
        assert_eq!(verified.patch_sha256, PATCH_SHA256);
        assert_eq!(verified.tree_sha256.len(), 64);
        assert_eq!(
            verify(&workspace()).unwrap().tree_sha256,
            verified.tree_sha256
        );
    }

    #[test]
    fn rust_archive_membership_and_changed_owners_are_exact() {
        let original = published_files(&fs::read(workspace().join(ARCHIVE_PATH)).unwrap()).unwrap();
        assert_eq!(original.len(), 107);
        let actual = source_tree(&workspace().join(VENDOR_PATH)).unwrap();
        assert_eq!(
            original.keys().collect::<Vec<_>>(),
            actual.keys().collect::<Vec<_>>()
        );
        let changed = original
            .iter()
            .filter_map(|(path, bytes)| (actual[path] != *bytes).then_some(path.to_str().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(
            changed,
            [
                "src/meta/mod.rs",
                "src/meta/regex.rs",
                "src/util/prefilter/mod.rs"
            ]
        );
        let vcs: serde_json::Value =
            serde_json::from_slice(&original[Path::new(".cargo_vcs_info.json")]).unwrap();
        assert_eq!(
            vcs["git"]["sha1"],
            "e6ad938198e68339a593f85d228f2d44cb54718f"
        );
        assert_eq!(vcs["path_in_vcs"], "regex-automata");
    }

    #[test]
    fn source_edits_and_original_or_partial_reversions_are_refused() {
        let original = published_files(&fs::read(workspace().join(ARCHIVE_PATH)).unwrap()).unwrap();
        for relative in [
            "src/meta/mod.rs",
            "src/meta/regex.rs",
            "src/util/prefilter/mod.rs",
            "README.md",
        ] {
            let temporary = fixture();
            let path = temporary.path().join(VENDOR_PATH).join(relative);
            fs::write(&path, b"unreviewed bytes").unwrap();
            assert_eq!(
                verify(temporary.path()),
                Err(VerifyError::Tree),
                "edited {relative}"
            );
            fs::write(&path, &original[Path::new(relative)]).unwrap();
            if relative == "README.md" {
                verify(temporary.path()).unwrap();
            } else {
                assert_eq!(
                    verify(temporary.path()),
                    Err(VerifyError::Tree),
                    "reverted {relative}"
                );
            }
        }
        let temporary = fixture();
        for (relative, bytes) in original {
            fs::write(temporary.path().join(VENDOR_PATH).join(relative), bytes).unwrap();
        }
        assert_eq!(verify(temporary.path()), Err(VerifyError::Tree));
    }

    #[test]
    fn omitted_added_and_nonregular_source_entries_are_refused() {
        for relative in [
            "src/meta/mod.rs",
            "README.md",
            "LICENSE-MIT",
            ".cargo_vcs_info.json",
        ] {
            let temporary = fixture();
            fs::remove_file(temporary.path().join(VENDOR_PATH).join(relative)).unwrap();
            assert_eq!(
                verify(temporary.path()),
                Err(VerifyError::Tree),
                "omitted {relative}"
            );
        }
        for relative in ["unexpected", "src/meta/unexpected.rs"] {
            let temporary = fixture();
            fs::write(temporary.path().join(VENDOR_PATH).join(relative), b"extra").unwrap();
            assert_eq!(
                verify(temporary.path()),
                Err(VerifyError::Tree),
                "added {relative}"
            );
        }
        let extra_directory = fixture();
        fs::create_dir_all(
            extra_directory
                .path()
                .join(VENDOR_PATH)
                .join("empty-extra-directory"),
        )
        .unwrap();
        assert_eq!(verify(extra_directory.path()), Err(VerifyError::Tree));
        let linked_file = fixture();
        let path = linked_file.path().join(VENDOR_PATH).join("README.md");
        fs::remove_file(&path).unwrap();
        symlink(workspace().join(VENDOR_PATH).join("README.md"), path).unwrap();
        assert_eq!(verify(linked_file.path()), Err(VerifyError::Tree));
        let linked_directory = fixture();
        fs::remove_dir_all(linked_directory.path().join(VENDOR_PATH).join("src/meta")).unwrap();
        symlink(
            workspace().join(VENDOR_PATH).join("src/meta"),
            linked_directory.path().join(VENDOR_PATH).join("src/meta"),
        )
        .unwrap();
        assert_eq!(verify(linked_directory.path()), Err(VerifyError::Tree));
    }

    #[test]
    fn archive_and_patch_tampering_refuse_before_tree_use() {
        for (relative, error) in [
            (ARCHIVE_PATH, VerifyError::Archive),
            (PATCH_PATH, VerifyError::Patch),
        ] {
            let temporary = fixture();
            fs::write(temporary.path().join(relative), b"not the approved input").unwrap();
            fs::remove_dir_all(temporary.path().join(VENDOR_PATH)).unwrap();
            assert_eq!(verify(temporary.path()), Err(error), "{relative}");
            assert!(!temporary.path().join(VENDOR_PATH).exists());
        }
        for (relative, error) in [
            (ARCHIVE_PATH, VerifyError::Archive),
            (PATCH_PATH, VerifyError::Patch),
        ] {
            let temporary = fixture();
            fs::remove_file(temporary.path().join(relative)).unwrap();
            symlink(workspace().join(relative), temporary.path().join(relative)).unwrap();
            assert_eq!(verify(temporary.path()), Err(error), "symlink {relative}");
        }
    }

    fn metadata() -> serde_json::Value {
        json!({"packages": [
            {"name": "jsonschema-macros-core", "version": "0.61.2", "source": null, "manifest_path": "/reviewed/compiler/Cargo.toml"},
            {"name": "regex-automata", "version": "0.4.18", "source": null, "manifest_path": "/reviewed/engine/Cargo.toml", "id": "path+file:///reviewed/engine#regex-automata@0.4.18"}
        ], "resolve": {"nodes": [{"id": "path+file:///reviewed/engine#regex-automata@0.4.18", "features": FEATURES}]}})
    }

    fn check_metadata(document: &serde_json::Value) -> Result<Vec<String>, String> {
        verify_metadata(
            &serde_json::to_vec(document).map_err(|error| error.to_string())?,
            Path::new("/reviewed/engine"),
        )
    }

    #[test]
    fn engine_metadata_requires_its_own_unique_local_package() {
        let original = metadata();
        assert_eq!(check_metadata(&original).unwrap(), FEATURES);
        for (field, value) in [
            ("version", json!("0.4.17")),
            (
                "source",
                json!("registry+https://github.com/rust-lang/crates.io-index"),
            ),
            ("manifest_path", json!("/unreviewed/engine/Cargo.toml")),
            (
                "manifest_path",
                json!("/reviewed/engine/../engine/Cargo.toml"),
            ),
            ("name", json!("regex")),
        ] {
            let mut changed = original.clone();
            changed["packages"][1][field] = value;
            assert_eq!(
                check_metadata(&changed),
                Err("engine metadata does not resolve the reviewed vendor".into()),
                "{field}"
            );
        }
        for field in ["version", "source", "manifest_path", "id"] {
            let mut changed = original.clone();
            assert!(
                changed["packages"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove(field)
                    .is_some()
            );
            assert_eq!(
                check_metadata(&changed),
                Err("engine metadata does not resolve the reviewed vendor".into()),
                "missing {field}"
            );
        }
        let mut duplicate_package = original.clone();
        duplicate_package["packages"]
            .as_array_mut()
            .unwrap()
            .push(original["packages"][1].clone());
        assert_eq!(
            check_metadata(&duplicate_package),
            Err("engine metadata does not resolve the reviewed vendor".into())
        );
        let mut missing_package = original;
        missing_package["packages"]
            .as_array_mut()
            .unwrap()
            .remove(1);
        assert_eq!(
            check_metadata(&missing_package),
            Err("engine metadata does not resolve the reviewed vendor".into())
        );
    }

    #[test]
    fn engine_features_join_the_exact_package_id_and_preserve_the_literal_closure() {
        let original = metadata();
        for id in [
            "unrelated#regex-automata@0.4.18",
            "registry+https://github.com/rust-lang/crates.io-index#regex-automata@0.4.18",
        ] {
            let mut changed = original.clone();
            changed["resolve"]["nodes"][0]["id"] = json!(id);
            assert_eq!(
                check_metadata(&changed),
                Err("engine resolved feature closure differs".into())
            );
        }
        let mut duplicate_node = original.clone();
        duplicate_node["resolve"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(original["resolve"]["nodes"][0].clone());
        assert_eq!(
            check_metadata(&duplicate_node),
            Err("engine resolved feature closure differs".into())
        );
        let mut missing_node = original.clone();
        missing_node["resolve"]["nodes"] = json!([]);
        assert_eq!(
            check_metadata(&missing_node),
            Err("engine resolved feature closure differs".into())
        );
        for features in [json!([]), json!(["alloc"]), json!(["unreviewed"]), {
            let mut features = json!(FEATURES);
            features.as_array_mut().unwrap().push(json!("alloc"));
            features
        }] {
            let mut changed = original.clone();
            changed["resolve"]["nodes"][0]["features"] = features;
            assert_eq!(
                check_metadata(&changed),
                Err("engine resolved feature closure differs".into())
            );
        }
    }
}
