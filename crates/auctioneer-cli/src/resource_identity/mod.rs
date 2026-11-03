//! Shared source snapshot for the build script and resource report.

pub mod git_metadata;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read as _};
use std::path::{Component, Path};
use std::process::Command;
use std::str::from_utf8;

/// Source files and their aggregate digest, including pending deletions.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub files: BTreeMap<String, Option<String>>,
    pub sha256: String,
}

/// Source and compiler identity captured by the application build script.
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildIdentity {
    pub snapshot: Snapshot,
    pub compiler_verbose: String,
    pub compiled_target: String,
    pub code_base_commit: String,
    pub source_tree_matches_commit: bool,
}

fn source_path(path: &Path) -> bool {
    // Only non-Rust documentation and approved resource qualification records are evidence.
    // Rust owners remain source even when placed inside either evidence tree.
    path.extension() == Some(OsStr::new("rs"))
        || (!path.starts_with("docs")
            && !path.starts_with("crates/auctioneer-core/tests/fixtures/resource_envelopes"))
}

fn file_digest(path: &Path) -> io::Result<Option<String>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Err(io::Error::other("source_identity_nonfile"));
    }
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 8_192];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        hash.update(
            buffer
                .get(..length)
                .ok_or_else(|| io::Error::other("source_identity_read"))?,
        );
    }
    Ok(Some(hex::encode(hash.finalize())))
}

/// Hash the complete source inventory rooted at a Git workspace.
///
/// # Errors
/// Returns an error for invalid Git listings, source paths or unreadable files.
pub fn snapshot(root: &Path) -> io::Result<Snapshot> {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "--no-replace-objects",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .output()?;
    let head = Command::new("git")
        .current_dir(root)
        .args([
            "--no-replace-objects",
            "ls-tree",
            "-r",
            "--name-only",
            "-z",
            "HEAD",
        ])
        .output()?;
    if !output.status.success() || !head.status.success() {
        return Err(io::Error::other("source_identity_listing"));
    }
    let mut paths = BTreeSet::new();
    // A staged deletion is absent from the index and untracked list, but must
    // remain visible until HEAD itself records the deletion.
    for bytes in [output.stdout, head.stdout]
        .iter()
        .flat_map(|bytes| bytes.split(|byte| *byte == 0))
        .filter(|value| !value.is_empty())
    {
        let path = from_utf8(bytes).map_err(io::Error::other)?;
        if path.chars().any(char::is_control)
            || Path::new(path)
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(io::Error::other("source_identity_path"));
        }
        if source_path(Path::new(path)) {
            paths.insert(path.to_owned());
        }
    }
    let files = paths
        .into_iter()
        .map(|path| {
            let digest = file_digest(&root.join(&path))?;
            Ok((path, digest))
        })
        .collect::<io::Result<BTreeMap<_, _>>>()?;
    let sha256 = hex::encode(Sha256::digest(
        serde_json::to_vec(&files).map_err(io::Error::other)?,
    ));
    Ok(Snapshot { files, sha256 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::var_os;

    #[test]
    fn workflow_reference_checkouts_are_ignored_without_hiding_other_sources() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workflow: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            &fs::read_to_string(workspace.join(".github/workflows/test.yml")).unwrap(),
        )
        .unwrap();
        let fixtures: Vec<_> = workflow["jobs"]["test"]["steps"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter(|step| {
                step["uses"]
                    .as_str()
                    .is_some_and(|uses| uses.starts_with("actions/checkout@"))
                    && step["with"]["repository"].as_str() == Some("prebid/prebid-server")
            })
            .map(|step| step["with"]["path"].as_str().unwrap())
            .collect();
        assert!(
            !fixtures.is_empty(),
            "exercise every declared reference checkout"
        );
        assert_eq!(
            fixtures.iter().collect::<BTreeSet<_>>().len(),
            fixtures.len(),
            "each reference checkout needs a distinct path"
        );
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(root)
                .env_clear()
                .env("PATH", var_os("PATH").unwrap())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .args([
                    "-c",
                    "user.name=Snapshot Test",
                    "-c",
                    "user.email=snapshot@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            output.stdout
        };
        git(&["init", "--quiet"]);
        fs::copy(workspace.join(".gitignore"), root.join(".gitignore")).unwrap();
        fs::create_dir_all(root.join("vendor")).unwrap();
        fs::write(root.join("vendor/owner.rs"), "fn owner() {}\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "baseline"]);
        let baseline = snapshot(root).unwrap();
        let nested_repo = |path: &str| {
            fs::create_dir_all(root.join(path)).unwrap();
            git(&["-C", path, "init", "--quiet"]);
            fs::write(root.join(path).join("fixture.rs"), "fn fixture() {}\n").unwrap();
            git(&["-C", path, "add", "."]);
            git(&["-C", path, "commit", "--quiet", "-m", "minimal reference"]);
        };
        for path in &fixtures {
            nested_repo(path);
        }
        assert_eq!(
            snapshot(root).unwrap(),
            baseline,
            "reference repos are not source inputs"
        );
        for path in &fixtures {
            fs::write(
                root.join(path).join("fixture.rs"),
                "changed reference bytes\n",
            )
            .unwrap();
        }
        assert_eq!(snapshot(root).unwrap(), baseline);
        fs::write(root.join("vendor/owner.rs"), "fn changed_owner() {}\n").unwrap();
        let tracked = snapshot(root).unwrap();
        assert_ne!(
            tracked.files["vendor/owner.rs"],
            baseline.files["vendor/owner.rs"]
        );
        assert_ne!(tracked.sha256, baseline.sha256);
        fs::write(root.join("vendor/untracked.rs"), "fn untracked() {}\n").unwrap();
        let untracked = snapshot(root).unwrap();
        assert!(untracked.files["vendor/untracked.rs"].is_some());
        assert_ne!(untracked.sha256, tracked.sha256);
        nested_repo("vendor/unknown-reference");
        assert_eq!(
            snapshot(root).unwrap_err().to_string(),
            "source_identity_nonfile"
        );
    }

    #[test]
    fn staged_deletions_remain_in_the_snapshot_and_cannot_look_clean() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(root)
                .env_clear()
                .env("PATH", var_os("PATH").unwrap())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .args([
                    "-c",
                    "user.name=Snapshot Test",
                    "-c",
                    "user.email=snapshot@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            output.stdout
        };
        git(&["init", "--quiet"]);
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(root.join("owner.rs"), "fn owner() {}\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "baseline"]);
        assert!(snapshot(root).unwrap().files["owner.rs"].is_some());
        git(&["rm", "--quiet", "owner.rs"]);
        let deleted = snapshot(root).unwrap();
        assert_eq!(deleted.files.get("owner.rs"), Some(&None));
        let mut args = vec!["status", "--porcelain=v1", "--"];
        args.extend(deleted.files.keys().map(String::as_str));
        assert!(
            !git(&args).is_empty(),
            "the build's restricted status query must see the deletion"
        );
        git(&["commit", "--quiet", "-m", "remove source"]);
        assert!(!snapshot(root).unwrap().files.contains_key("owner.rs"));
    }
}
