use std::{
    collections::{BTreeMap, BTreeSet},
    env::var_os,
    fs::{self, File},
    io::Read as _,
    path::Path,
    process::Command,
    str::from_utf8,
};

use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::provenance;

const SOURCE_LOCK: &str =
    "crates/auctioneer-bidder/tests/conformance/fixtures/usersync/sources.json";
const PRIVACY_CAPTURE: &str = "crates/auctioneer-privacy/tests/data/privacy/reference.json";
const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    captures: Vec<Capture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    path: String,
    sha256: String,
    bytes: u64,
}

fn manifest() -> Result<Manifest> {
    // Compiled-in pins cannot be replaced by a staged tree's manifest.
    parse_manifest(include_str!("captures.json"))
}

fn parse_manifest(data: &str) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_str(data)?;
    ensure!(
        manifest.schema_version == 2 && manifest.captures.len() == 10,
        "capture manifest schema/count"
    );
    let mut paths = BTreeSet::new();
    for capture in &manifest.captures {
        safe_relative(&capture.path)?;
        provenance::sha256(&capture.sha256)?;
        ensure!(paths.insert(&capture.path), "duplicate capture path");
        ensure!(
            capture.bytes > 0 && capture.bytes <= MAX_SOURCE_BYTES,
            "capture size bound"
        );
    }
    Ok(manifest)
}

pub(crate) fn safe_relative(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && !path.contains(['\\', ':']) && !path.chars().any(char::is_control),
        "unsafe relative path"
    );
    ensure!(
        path.split('/').all(|part| !matches!(part, "" | "." | "..")),
        "unsafe relative path"
    );
    Ok(())
}

fn read_capture(root: &Path, capture: &Capture) -> Result<Vec<u8>> {
    let bytes = read_relative(root, &capture.path, capture.bytes)?;
    ensure!(
        u64::try_from(bytes.len())? == capture.bytes,
        "capture size mismatch"
    );
    ensure!(
        hex::encode(Sha256::digest(&bytes)) == capture.sha256,
        "capture SHA-256 mismatch"
    );
    Ok(bytes)
}

fn read_relative(root: &Path, path: &str, limit: u64) -> Result<Vec<u8>> {
    safe_relative(path)?;
    let mut full = root.to_path_buf();
    for part in path.split('/') {
        full.push(part);
        ensure!(
            !fs::symlink_metadata(&full)?.file_type().is_symlink(),
            "symlink in artifact path"
        );
    }
    ensure!(
        fs::metadata(&full)?.is_file(),
        "artifact is not a regular file"
    );
    let mut bytes = Vec::new();
    File::open(full)?
        .take(limit.checked_add(1).context("read limit overflow")?)
        .read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len())? <= limit,
        "artifact exceeds size bound"
    );
    Ok(bytes)
}

pub(crate) fn reject_go_sources(root: &Path) -> Result<()> {
    if root.join(".git").exists() {
        // Tracked paths remain owned even below an ignore rule. Ignored CI
        // reference checkouts are inputs, not repository-maintained tooling.
        let output = Command::new("git")
            .current_dir(root)
            .env_clear()
            .env("PATH", var_os("PATH").unwrap_or_default())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "--no-replace-objects",
                "--no-optional-locks",
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ])
            .output()?;
        ensure!(output.status.success(), "repository source inventory");
        for bytes in output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|bytes| !bytes.is_empty())
        {
            reject_go_path(Path::new(from_utf8(bytes)?))?;
        }
        return Ok(());
    }
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for result in fs::read_dir(directory)? {
            let entry = result?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if matches!(
                name.as_ref(),
                ".git" | ".claude" | "target" | "node_modules"
            ) {
                continue;
            }
            reject_go_path(entry.path().strip_prefix(root)?)?;
            if entry.file_type()?.is_dir() {
                directories.push(entry.path());
            }
        }
    }
    Ok(())
}

fn reject_go_path(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .context("source file name")?
        .to_string_lossy();
    ensure!(
        !name.ends_with(".go")
            && !name.ends_with(".go.txt")
            && !matches!(name.as_ref(), "go.mod" | "go.sum"),
        "repository-owned Go tooling: {}",
        path.display()
    );
    Ok(())
}

fn verify_sources(root: &Path, sources: &Value) -> Result<()> {
    for (path, digest) in provenance::object(provenance::field(sources, "source_sha256")?)? {
        safe_relative(path)?;
        let bytes = read_relative(root, path, MAX_SOURCE_BYTES)
            .with_context(|| format!("source {path}"))?;
        ensure!(
            Some(hex::encode(Sha256::digest(bytes)).as_str()) == digest.as_str(),
            "source {path}: SHA-256 mismatch"
        );
    }
    Ok(())
}

pub(crate) fn validate(root: &Path, source: Option<&Path>) -> Result<()> {
    let manifest = manifest()?;
    let mut files = BTreeMap::new();
    for capture in &manifest.captures {
        files.insert(
            capture.path.clone(),
            read_capture(root, capture).with_context(|| capture.path.clone())?,
        );
    }
    let sources: Value = parse_json(files.get(SOURCE_LOCK).context("missing source lock pin")?)?;
    provenance::source_lock(&sources).context(SOURCE_LOCK)?;
    for capture in &manifest.captures {
        if capture.path == SOURCE_LOCK
            || !Path::new(&capture.path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let bytes = files.get(&capture.path).context("missing capture")?;
        let data = parse_json(bytes)?;
        let result = if capture.path == PRIVACY_CAPTURE {
            provenance::privacy(&data)
        } else {
            provenance::source_cases(&data, &sources)
        };
        result.with_context(|| capture.path.clone())?;
    }
    verify_attribution(&manifest, &sources)?;
    reject_go_sources(root)?;
    if let Some(source_root) = source {
        verify_sources(source_root, &sources)?;
    }
    Ok(())
}

fn verify_attribution(manifest: &Manifest, sources: &Value) -> Result<()> {
    for (capture_name, source_name) in [
        ("LICENSE.prebid", "LICENSE"),
        ("pixel.png", "util/httputil/pixel.png"),
    ] {
        let capture = manifest
            .captures
            .iter()
            .find(|capture| capture.path.ends_with(capture_name))
            .context("missing attribution pin")?;
        ensure!(
            provenance::field(provenance::field(sources, "source_sha256")?, source_name)?.as_str()
                == Some(capture.sha256.as_str()),
            "source attribution identity mismatch"
        );
    }
    Ok(())
}

fn parse_json(bytes: &[u8]) -> Result<Value> {
    // Avoid including captured diagnostics or payloads in parse errors.
    serde_json::from_slice(bytes).map_err(|_error| anyhow::anyhow!("invalid capture JSON"))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::{
        env, fs,
        path::PathBuf,
        process,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "auctioneer-reference-{}-{}",
                process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn every_original_generated_capture_is_pinned_and_read() {
        let manifest = manifest().unwrap();
        assert_eq!(manifest.schema_version, 2);
        assert_eq!(manifest.captures.len(), 10);
        assert_eq!(
            manifest.captures.iter().map(|c| c.bytes).sum::<u64>(),
            734_948
        );
        for capture in &manifest.captures {
            assert_eq!(
                read_capture(&root(), capture).unwrap(),
                fs::read(root().join(&capture.path)).unwrap()
            );
        }
    }

    fn manifest_data() -> Value {
        serde_json::from_str(include_str!("captures.json")).unwrap()
    }

    #[test]
    fn manifest_rejects_wrong_capture_counts_without_other_pin_defects() {
        let valid = manifest_data();
        parse_manifest(&serde_json::to_string(&valid).unwrap()).unwrap();
        for count in [0_usize, 9, 11] {
            let mut data = valid.clone();
            let captures = data["captures"].as_array_mut().unwrap();
            if count < captures.len() {
                captures.truncate(count);
            } else {
                let mut extra = captures[0].clone();
                extra["path"] = serde_json::json!("fixtures/additional-capture.json");
                captures.push(extra);
            }
            assert_eq!(captures.len(), count);
            let error = parse_manifest(&serde_json::to_string(&data).unwrap())
                .err()
                .unwrap();
            assert_eq!(
                error.to_string(),
                "capture manifest schema/count",
                "count={count}"
            );
        }
    }

    #[test]
    fn manifest_version_is_checked_independently_of_capture_count() {
        for version in [0_u32, 1, 3] {
            let mut data = manifest_data();
            data["schema_version"] = serde_json::json!(version);
            assert_eq!(data["captures"].as_array().unwrap().len(), 10);
            let error = parse_manifest(&serde_json::to_string(&data).unwrap())
                .err()
                .unwrap();
            assert_eq!(error.to_string(), "capture manifest schema/count");
        }
    }

    #[test]
    fn each_opaque_capture_must_match_its_upstream_fingerprint() {
        let valid_sources: Value =
            serde_json::from_slice(&fs::read(root().join(SOURCE_LOCK)).unwrap()).unwrap();
        let pins = manifest().unwrap();
        provenance::source_lock(&valid_sources).unwrap();
        verify_attribution(&pins, &valid_sources).unwrap();
        for (capture_name, source_name) in [
            ("LICENSE.prebid", "LICENSE"),
            ("pixel.png", "util/httputil/pixel.png"),
        ] {
            let mut wrong_source = valid_sources.clone();
            wrong_source["source_sha256"][source_name] = serde_json::json!("0".repeat(64));
            provenance::source_lock(&wrong_source).unwrap();
            assert_eq!(
                verify_attribution(&pins, &wrong_source)
                    .unwrap_err()
                    .to_string(),
                "source attribution identity mismatch",
                "{source_name}"
            );

            let mut wrong_capture = manifest_data();
            let capture = wrong_capture["captures"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|entry| entry["path"].as_str().unwrap().ends_with(capture_name))
                .unwrap();
            capture["sha256"] = serde_json::json!("0".repeat(64));
            let wrong_pins =
                parse_manifest(&serde_json::to_string(&wrong_capture).unwrap()).unwrap();
            assert_eq!(
                verify_attribution(&wrong_pins, &valid_sources)
                    .unwrap_err()
                    .to_string(),
                "source attribution identity mismatch",
                "{capture_name}"
            );
        }
    }

    #[test]
    fn every_capture_rejects_same_size_corruption_and_missing_files() {
        let scratch = Scratch::new();
        for capture in &manifest().unwrap().captures {
            let path = scratch.0.join(&capture.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut bytes = fs::read(root().join(&capture.path)).unwrap();
            bytes[0] ^= 1;
            fs::write(&path, &bytes).unwrap();
            assert!(
                read_capture(&scratch.0, capture).is_err(),
                "{} accepted corruption",
                capture.path
            );
            fs::remove_file(path).unwrap();
            assert!(
                read_capture(&scratch.0, capture).is_err(),
                "{} accepted absence",
                capture.path
            );
        }
    }

    #[test]
    fn unsafe_paths_fail_without_accessing_files() {
        for path in [
            "",
            "../a",
            "a/../b",
            "/tmp/a",
            "a//b",
            "./a",
            "a/./b",
            "a\\b",
            "C:/a",
            "a/",
            "a\nsecret",
        ] {
            assert!(safe_relative(path).is_err(), "accepted {path:?}");
        }
        safe_relative("usersync/cookie.go").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn capture_symlink_escape_is_rejected_even_with_matching_bytes() {
        let scratch = Scratch::new();
        let outside = Scratch::new();
        let capture = manifest().unwrap().captures.remove(0);
        let destination = scratch.0.join(&capture.path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let source = outside.0.join("capture");
        fs::copy(root().join(&capture.path), &source).unwrap();
        symlink(source, destination).unwrap();
        read_capture(&scratch.0, &capture).unwrap_err();
    }

    #[test]
    fn all_go_owned_source_forms_are_rejected() {
        for name in ["main.go", "main.go.txt", "go.mod", "go.sum"] {
            let scratch = Scratch::new();
            fs::create_dir_all(scratch.0.join("crates/tool/reference")).unwrap();
            fs::write(
                scratch.0.join("crates/tool/reference").join(name),
                b"package main",
            )
            .unwrap();
            assert!(reject_go_sources(&scratch.0).is_err(), "accepted {name}");
        }
    }

    #[test]
    fn git_inventory_excludes_external_checkouts_but_not_owned_go_sources() {
        let scratch = Scratch::new();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&scratch.0)
                .env_clear()
                .env("PATH", env::var_os("PATH").unwrap())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
        };
        git(&["init", "--quiet"]);
        fs::write(scratch.0.join(".gitignore"), "/vendor/external/\n").unwrap();
        fs::create_dir_all(scratch.0.join("vendor/external")).unwrap();
        fs::write(scratch.0.join("vendor/external/main.go"), "package main").unwrap();
        reject_go_sources(&scratch.0).unwrap();
        for name in ["main.go", "main.go.txt", "go.mod", "go.sum"] {
            let relative = format!("vendor/owned/{name}");
            let path = scratch.0.join(&relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "package main").unwrap();
            assert_eq!(
                reject_go_sources(&scratch.0).unwrap_err().to_string(),
                format!("repository-owned Go tooling: {relative}")
            );
            fs::remove_file(path).unwrap();
        }
        git(&["add", "--force", "vendor/external/main.go"]);
        assert_eq!(
            reject_go_sources(&scratch.0).unwrap_err().to_string(),
            "repository-owned Go tooling: vendor/external/main.go"
        );
    }

    #[test]
    fn real_tree_has_no_repository_owned_go_programs_or_harnesses() {
        reject_go_sources(&root()).unwrap();
    }

    #[test]
    fn optional_source_check_rejects_missing_changed_and_traversing_sources() {
        let scratch = Scratch::new();
        let sources = serde_json::json!({"source_sha256":{"source.go":hex::encode(sha2::Sha256::digest(b"source"))}});
        assert!(verify_sources(&scratch.0, &sources).is_err());
        fs::write(scratch.0.join("source.go"), b"source").unwrap();
        verify_sources(&scratch.0, &sources).unwrap();
        fs::write(scratch.0.join("source.go"), b"changed").unwrap();
        assert!(verify_sources(&scratch.0, &sources).is_err());
        let traversing_sources =
            serde_json::json!({"source_sha256":{"../source.go":"a".repeat(64)}});
        assert!(verify_sources(&scratch.0, &traversing_sources).is_err());
    }
}
