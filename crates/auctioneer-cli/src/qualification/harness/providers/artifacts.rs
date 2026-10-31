use super::super::{HarnessError, Result, io, process};
use super::preparation::{self, FILE_CAP, INPUT_CAP, Preparation, RECEIPT_CAP};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use std::time::{SystemTime, UNIX_EPOCH};

const CARGO_CAP: usize = 8 << 20;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureBuildRequest {
    pub preparation: Preparation,
    pub cargo_json_lines: String,
    pub started_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuildReceipt {
    pub artifact: PathBuf,
    pub artifact_sha256: String,
    pub source_sha256: String,
    pub fixture_metadata_sha256: String,
    pub started_ms: u64,
    pub target: String,
    pub profile: String,
    pub build_features: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageSpinRequest {
    pub preparation: Preparation,
    pub build: BuildReceipt,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageFastlyRequest {
    pub preparation: Preparation,
    pub build: BuildReceipt,
    pub bundle_root: PathBuf,
    pub attestation: ReleaseAttestation,
}

// Cargo messages contain evolving compiler-owned fields. Retain only the typed
// identity fields after checking each entire record's byte/node/depth bounds.
#[derive(Debug, Deserialize)]
struct CargoMessage {
    reason: String,
    target: Option<CargoTarget>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    filenames: Vec<PathBuf>,
    profile: Option<CargoProfile>,
    success: Option<bool>,
    fresh: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CargoProfile {
    opt_level: String,
    debug_assertions: bool,
    test: bool,
}

pub async fn capture_build(
    input: &Path,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    let cap = RECEIPT_CAP
        .checked_add(CARGO_CAP)
        .ok_or(HarnessError::new("input_limit"))?;
    let request: CaptureBuildRequest = io::read_json(input, cap)?;
    let receipt = capture(
        &request.preparation,
        &request.cargo_json_lines,
        request.started_ms,
        supervisor,
    )
    .await?;
    serde_json::to_value(receipt).map_err(|_error| HarnessError::new("invalid_input"))
}

fn cargo_messages(lines: &str) -> Result<Vec<CargoMessage>> {
    if lines.len() > CARGO_CAP {
        return Err(HarnessError::new("artifact_identity"));
    }
    let mut messages = Vec::new();
    for line in lines.trim().lines() {
        if messages.len() >= 10_000 {
            return Err(HarnessError::new("artifact_identity"));
        }
        auctioneer_runtime::count_json_nodes(line.as_bytes(), 1 << 16)
            .map_err(|_error| HarnessError::new("artifact_identity"))?;
        let _bounded_depth: serde_json::Value =
            serde_json::from_str(line).map_err(|_error| HarnessError::new("artifact_identity"))?;
        let message: CargoMessage =
            serde_json::from_str(line).map_err(|_error| HarnessError::new("artifact_identity"))?;
        if message.features.len() > 128 || message.filenames.len() > 128 {
            return Err(HarnessError::new("artifact_identity"));
        }
        messages.push(message);
    }
    if !messages
        .last()
        .is_some_and(|message| message.reason == "build-finished" && message.success == Some(true))
    {
        return Err(HarnessError::new("artifact_identity"));
    }
    Ok(messages)
}

pub(super) async fn capture(
    preparation: &Preparation,
    lines: &str,
    started_ms: u64,
    supervisor: &process::Supervisor,
) -> Result<BuildReceipt> {
    preparation::verify_frozen_inputs(preparation, supervisor).await?;
    let messages = cargo_messages(lines)?;
    let compiled: Vec<_> = messages
        .iter()
        .filter(|message| message.reason == "compiler-artifact")
        .collect();
    let config: Vec<_> = compiled
        .iter()
        .filter(|message| {
            message
                .target
                .as_ref()
                .is_some_and(|target| target.name == "auctioneer_config")
        })
        .collect();
    if config.is_empty()
        || !config
            .iter()
            .all(|message| message.features == ["test-harness-override"])
    {
        return Err(HarnessError::new("build_feature_missing"));
    }
    let underscore_name = format!("auctioneer_adapter_{}", preparation.provider);
    let hyphen_name = format!("auctioneer-adapter-{}", preparation.provider);
    let adapters: Vec<_> =
        compiled
            .iter()
            .filter(|message| {
                message.target.as_ref().is_some_and(|target| {
                    target.name == underscore_name || target.name == hyphen_name
                }) && message.filenames.iter().any(|file| {
                    file.extension()
                        .is_some_and(|extension| extension == "wasm")
                })
            })
            .collect();
    if adapters.len() != 1 {
        return Err(HarnessError::new("artifact_identity"));
    }
    let adapter = adapters
        .first()
        .ok_or(HarnessError::new("artifact_identity"))?;
    if adapter.features != [preparation.provider.clone()]
        || adapter.fresh == Some(true)
        || !adapter.profile.as_ref().is_some_and(|profile| {
            profile.opt_level == "3" && !profile.debug_assertions && !profile.test
        })
    {
        return Err(HarnessError::new("artifact_identity"));
    }
    let emitted: Vec<_> = adapter
        .filenames
        .iter()
        .filter(|file| {
            file.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .collect();
    if emitted.len() != 1 {
        return Err(HarnessError::new("artifact_identity"));
    }
    let artifact = (*emitted
        .first()
        .ok_or(HarnessError::new("artifact_identity"))?)
    .clone();
    let artifact_sha256 = io::file_digest(&artifact, FILE_CAP)
        .map_err(|_error| HarnessError::new("artifact_identity"))?;
    let result = BuildReceipt {
        artifact,
        artifact_sha256,
        source_sha256: preparation.source_sha256.clone(),
        fixture_metadata_sha256: preparation.fixture_metadata_sha256.clone(),
        started_ms,
        target: preparation.target.clone(),
        profile: preparation.profile.clone(),
        build_features: preparation.build_features.clone(),
    };
    verify_artifact(preparation, &result, supervisor).await?;
    Ok(result)
}

pub(super) async fn verify_artifact(
    preparation: &Preparation,
    build: &BuildReceipt,
    supervisor: &process::Supervisor,
) -> Result<()> {
    preparation::verify_frozen_inputs(preparation, supervisor).await?;
    let expected_parent = preparation
        .target_dir
        .join(&preparation.target)
        .join(&preparation.profile);
    let selected = preparation::provider(&preparation.provider)?;
    if build.target != preparation.target
        || build.profile != preparation.profile
        || build.source_sha256 != preparation.source_sha256
        || build.fixture_metadata_sha256 != preparation.fixture_metadata_sha256
        || build.build_features != preparation.build_features
        || !build.artifact.is_absolute()
        || build.artifact.parent() != Some(expected_parent.as_path())
        || !preparation::hex(&build.artifact_sha256, 64)
        || build.started_ms == 0
        || u128::from(build.started_ms) > now_ms()?
    {
        return Err(HarnessError::new("artifact_identity"));
    }
    preparation::check_features(selected, &build.build_features)?;
    let name = build
        .artifact
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(HarnessError::new("artifact_identity"))?;
    if name != format!("auctioneer_adapter_{}.wasm", selected.name())
        && name != format!("auctioneer-adapter-{}.wasm", selected.name())
    {
        return Err(HarnessError::new("artifact_identity"));
    }
    let metadata = fs::symlink_metadata(&build.artifact)
        .map_err(|_error| HarnessError::new("artifact_missing"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(HarnessError::new("artifact_identity"));
    }
    let relative = build
        .artifact
        .strip_prefix(&preparation.target_dir)
        .map_err(|_error| HarnessError::new("artifact_identity"))?;
    preparation::check_chain(&preparation.target_dir, relative)
        .map_err(|_error| HarnessError::new("artifact_identity"))?;
    let modified = metadata
        .modified()
        .and_then(|time| time.duration_since(UNIX_EPOCH).map_err(IoError::other))
        .map_err(|_error| HarnessError::new("artifact_identity"))?;
    if modified.as_millis() < u128::from(build.started_ms) {
        return Err(HarnessError::new("artifact_stale"));
    }
    let actual = io::file_digest(&build.artifact, FILE_CAP)
        .map_err(|_error| HarnessError::new("artifact_identity"))?;
    if actual != build.artifact_sha256 {
        return Err(HarnessError::new("artifact_digest"));
    }
    let bytes = preparation::strict_file(&preparation.target_dir, relative, FILE_CAP)
        .map_err(|_error| HarnessError::new("artifact_identity"))?;
    if bytes.len() < 8 || bytes.get(..4) != Some(b"\0asm".as_slice()) {
        return Err(HarnessError::new("artifact_identity"));
    }
    if io::sha256(&bytes) != build.artifact_sha256 {
        return Err(HarnessError::new("artifact_digest"));
    }
    preparation::verify_frozen_inputs(preparation, supervisor).await
}

fn now_ms() -> Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis())
        .map_err(|_error| HarnessError::new("artifact_identity"))
}

pub async fn stage_spin(
    input: &Path,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    let request: StageSpinRequest = io::read_json(input, RECEIPT_CAP)?;
    spin(&request.preparation, &request.build, supervisor).await
}

pub(super) async fn spin(
    preparation: &Preparation,
    build: &BuildReceipt,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    if preparation.provider != "spin" {
        return Err(HarnessError::new("artifact_identity"));
    }
    verify_artifact(preparation, build, supervisor).await?;
    let mut manifest = preparation::parse_toml(&preparation::strict_file(
        &preparation.repo_root,
        Path::new("crates/auctioneer-adapter-spin/spin.toml"),
        INPUT_CAP,
    )?)?;
    if !manifest
        .get("trigger")
        .and_then(|value| value.get("http"))
        .and_then(toml::Value::as_array)
        .is_some_and(|triggers| {
            triggers.iter().any(|trigger| {
                trigger.get("component").and_then(toml::Value::as_str)
                    == Some("auctioneer-adapter-spin")
            })
        })
    {
        return Err(HarnessError::new("artifact_identity"));
    }
    let component = manifest
        .get_mut("component")
        .and_then(|value| value.get_mut("auctioneer-adapter-spin"))
        .and_then(toml::Value::as_table_mut)
        .ok_or(HarnessError::new("artifact_identity"))?;
    component.insert(
        "source".to_owned(),
        toml::Value::String("./auctioneer_adapter_spin.wasm".to_owned()),
    );
    component.remove("build");
    let encoded = preparation::encode_toml(&manifest)?;
    let root = preparation.workspace.join("spin-handoff");
    supervisor.check_active().await?;
    preparation::private_directory(&root)?;
    let artifact_relative = build
        .artifact
        .strip_prefix(&preparation.target_dir)
        .map_err(|_error| HarnessError::new("artifact_identity"))?;
    let artifact_bytes =
        preparation::strict_file(&preparation.target_dir, artifact_relative, FILE_CAP)?;
    if io::sha256(&artifact_bytes) != build.artifact_sha256 {
        return Err(HarnessError::new("artifact_digest"));
    }
    preparation::write_private(
        &root.join("auctioneer_adapter_spin.wasm"),
        &artifact_bytes,
        FILE_CAP,
    )?;
    preparation::write_private(&root.join("spin.toml"), &encoded, INPUT_CAP)?;
    verify_artifact(preparation, build, supervisor).await?;
    if io::sha256(&preparation::strict_file(
        &root,
        Path::new("auctioneer_adapter_spin.wasm"),
        FILE_CAP,
    )?) != build.artifact_sha256
        || preparation::strict_file(&root, Path::new("spin.toml"), INPUT_CAP)? != encoded
    {
        return Err(HarnessError::new("artifact_digest"));
    }
    Ok(
        serde_json::json!({ "manifest": root.join("spin.toml"), "artifact_sha256": build.artifact_sha256,
        "build_features": build.build_features, "manifest_sha256": io::sha256(&encoded), "profile": preparation.profile, "target": preparation.target }),
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAttestation {
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub build_features: Vec<String>,
    pub package_sha256: String,
    pub app_cli_sha256: String,
    pub release_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseMember {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseManifests {
    pub edgezero: ReleaseMember,
    pub adapter: ReleaseMember,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseMetadata {
    pub format: u32,
    pub lifecycle_protocol: u32,
    pub source_revision: String,
    pub adapter: String,
    pub app_cli: ReleaseMember,
    pub package: ReleaseMember,
    pub manifests: ReleaseManifests,
}

impl ReleaseMetadata {
    pub(super) fn members(&self) -> [&ReleaseMember; 4] {
        [
            &self.app_cli,
            &self.package,
            &self.manifests.edgezero,
            &self.manifests.adapter,
        ]
    }

    pub(super) fn validate(&self, source_revision: &str, files: &BTreeSet<String>) -> Result<()> {
        let invalid = HarnessError::new("release_invalid");
        if self.format != 1
            || self.lifecycle_protocol != 1
            || self.adapter != "fastly"
            || self.source_revision != source_revision
        {
            return Err(invalid);
        }
        let mut expected = BTreeSet::from(["release.json".to_owned()]);
        for member in self.members() {
            release_path(Path::new("/release"), &member.path)?;
            if !preparation::hex(&member.sha256, 64) || !expected.insert(member.path.clone()) {
                return Err(invalid);
            }
        }
        if files != &expected {
            return Err(invalid);
        }
        Ok(())
    }
}

pub(super) fn release_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.is_empty()
        || relative.len() > 1024
        || relative.starts_with('/')
        || relative.split('/').count() > 8
        || !relative
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(HarnessError::new("release_invalid"));
    }
    Ok(root.join(relative))
}

fn release_files(root: &Path, prefix: &str, nodes: &mut usize) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    for entry in
        fs::read_dir(root.join(prefix)).map_err(|_error| HarnessError::new("release_invalid"))?
    {
        *nodes = nodes
            .checked_add(1)
            .ok_or(HarnessError::new("release_invalid"))?;
        if *nodes > 128 {
            return Err(HarnessError::new("release_invalid"));
        }
        let item = entry.map_err(|_error| HarnessError::new("release_invalid"))?;
        let basename = item
            .file_name()
            .into_string()
            .map_err(|_error| HarnessError::new("release_invalid"))?;
        let name = if prefix.is_empty() {
            basename
        } else {
            format!("{prefix}/{basename}")
        };
        let path = release_path(root, &name)?;
        let metadata =
            fs::symlink_metadata(path).map_err(|_error| HarnessError::new("release_invalid"))?;
        if metadata.file_type().is_symlink() {
            return Err(HarnessError::new("release_invalid"));
        }
        if metadata.is_dir() {
            let nested = release_files(root, &name, nodes)?;
            if nested.is_empty() {
                return Err(HarnessError::new("release_invalid"));
            }
            files.extend(nested);
        } else if metadata.is_file() {
            files.insert(name);
        } else {
            return Err(HarnessError::new("release_invalid"));
        }
        if files.len() > 5 {
            return Err(HarnessError::new("release_invalid"));
        }
    }
    Ok(files)
}

pub(super) fn verify_bundle(
    root: &Path,
    source_revision: &str,
) -> Result<(ReleaseMetadata, String)> {
    let canonical = preparation::canonical_directory(root, "release_invalid")?;
    if canonical != root {
        return Err(HarnessError::new("release_invalid"));
    }
    let bytes = preparation::strict_file(root, Path::new("release.json"), INPUT_CAP)?;
    let metadata: ReleaseMetadata =
        serde_json::from_slice(&bytes).map_err(|_error| HarnessError::new("release_invalid"))?;
    metadata.validate(source_revision, &release_files(root, "", &mut 0)?)?;
    for member in metadata.members() {
        preparation::check_chain(root, Path::new(&member.path))?;
        if io::file_digest(&root.join(&member.path), FILE_CAP)? != member.sha256 {
            return Err(HarnessError::new("release_invalid"));
        }
    }
    let application = preparation::parse_toml(&preparation::strict_file(
        root,
        Path::new(&metadata.manifests.edgezero.path),
        INPUT_CAP,
    )?)?;
    let adapters = application
        .get("adapters")
        .and_then(toml::Value::as_table)
        .ok_or(HarnessError::new("release_invalid"))?;
    let names: BTreeSet<_> = adapters
        .keys()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    if names.len() != adapters.len()
        || adapters
            .get("fastly")
            .and_then(|value| value.get("adapter"))
            .and_then(|value| value.get("manifest"))
            .and_then(toml::Value::as_str)
            != Some(metadata.manifests.adapter.path.as_str())
    {
        return Err(HarnessError::new("release_invalid"));
    }
    preparation::parse_toml(&preparation::strict_file(
        root,
        Path::new(&metadata.manifests.adapter.path),
        INPUT_CAP,
    )?)?;
    Ok((metadata, io::sha256(&bytes)))
}

pub(super) async fn tar(
    supervisor: &process::Supervisor,
    archive: &Path,
    mode: &str,
    member: Option<&str>,
    cap: usize,
) -> Result<process::CommandOutput> {
    if !["-tzf", "-tvzf", "-xOzf"].contains(&mode) {
        return Err(HarnessError::new("release_invalid"));
    }
    let mut spec = process::ProcessSpec::new("/usr/bin/tar");
    spec.args = vec![OsString::from(mode), archive.as_os_str().to_owned()];
    if let Some(name) = member {
        release_path(Path::new("/package"), name)?;
        spec.args
            .extend([OsString::from("--"), OsString::from(name)]);
    }
    spec.timeout_ms = 10_000;
    spec.output_cap = cap;
    spec.env = preparation::tool_environment();
    let output = supervisor.run(spec).await?;
    if !output.summary.closed {
        return Err(HarnessError::new("release_invalid"));
    }
    Ok(output)
}

#[derive(Debug, Serialize)]
struct PackageReceipt {
    package_sha256: String,
    artifact_member: String,
    manifest_member: String,
    artifact_sha256: String,
    manifest_sha256: String,
    listing: process::ProcessSummary,
    artifact: process::ProcessSummary,
    manifest: process::ProcessSummary,
}

fn package_members(bytes: &[u8]) -> Result<(Vec<String>, String, String)> {
    let text = from_utf8(bytes).map_err(|_error| HarnessError::new("release_invalid"))?;
    let mut names = Vec::new();
    let mut unique = BTreeSet::new();
    let mut artifacts = Vec::new();
    let mut manifests = Vec::new();
    for name in text.lines() {
        if names.len() >= 128 {
            return Err(HarnessError::new("release_invalid"));
        }
        let normalized = name.strip_suffix('/').unwrap_or(name);
        release_path(Path::new("/package"), normalized)?;
        if !unique.insert(normalized.to_owned()) {
            return Err(HarnessError::new("release_invalid"));
        }
        if name == "bin/main.wasm" || name.ends_with("/bin/main.wasm") {
            artifacts.push(name.to_owned());
        }
        if name == "fastly.toml" || name.ends_with("/fastly.toml") {
            manifests.push(name.to_owned());
        }
        names.push(name.to_owned());
    }
    if artifacts.len() != 1 || manifests.len() != 1 {
        return Err(HarnessError::new("release_invalid"));
    }
    Ok((
        names,
        artifacts
            .pop()
            .ok_or(HarnessError::new("release_invalid"))?,
        manifests
            .pop()
            .ok_or(HarnessError::new("release_invalid"))?,
    ))
}

async fn verify_package(
    supervisor: &process::Supervisor,
    package: &Path,
    artifact_sha256: &str,
    manifest_sha256: &str,
) -> Result<PackageReceipt> {
    let digest = io::file_digest(package, FILE_CAP)?;
    let listing = tar(supervisor, package, "-tzf", None, INPUT_CAP).await?;
    let (names, artifact_member, manifest_member) = package_members(&listing.stdout)?;
    let verbose = tar(supervisor, package, "-tvzf", None, INPUT_CAP).await?;
    let verbose_text =
        from_utf8(&verbose.stdout).map_err(|_error| HarnessError::new("release_invalid"))?;
    if verbose_text.lines().count() != names.len()
        || !verbose_text.lines().zip(&names).all(|(line, name)| {
            line.as_bytes().first() == Some(if name.ends_with('/') { &b'd' } else { &b'-' })
        })
    {
        return Err(HarnessError::new("release_invalid"));
    }
    let artifact = tar(
        supervisor,
        package,
        "-xOzf",
        Some(&artifact_member),
        FILE_CAP,
    )
    .await?;
    let manifest = tar(
        supervisor,
        package,
        "-xOzf",
        Some(&manifest_member),
        INPUT_CAP,
    )
    .await?;
    if io::sha256(&artifact.stdout) != artifact_sha256
        || io::sha256(&manifest.stdout) != manifest_sha256
        || io::file_digest(package, FILE_CAP)? != digest
    {
        return Err(HarnessError::new("release_invalid"));
    }
    Ok(PackageReceipt {
        package_sha256: digest,
        artifact_member,
        manifest_member,
        artifact_sha256: artifact_sha256.to_owned(),
        manifest_sha256: manifest_sha256.to_owned(),
        listing: listing.summary,
        artifact: artifact.summary,
        manifest: manifest.summary,
    })
}

pub async fn stage_fastly(
    input: &Path,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    let request: StageFastlyRequest = io::read_json(input, RECEIPT_CAP)?;
    fastly(
        &request.preparation,
        &request.build,
        &request.bundle_root,
        &request.attestation,
        supervisor,
    )
    .await
}

pub(super) async fn fastly(
    preparation: &Preparation,
    build: &BuildReceipt,
    bundle_root: &Path,
    attestation: &ReleaseAttestation,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    verify_artifact(preparation, build, supervisor).await?;
    if preparation.provider != "fastly"
        || attestation.source_sha256 != preparation.source_sha256
        || attestation.artifact_sha256 != build.artifact_sha256
        || attestation.build_features != preparation.build_features
    {
        return Err(HarnessError::new("release_identity"));
    }
    let bundle = preparation::canonical_directory(bundle_root, "release_invalid")?;
    let (metadata, release_sha256) = verify_bundle(&bundle, &preparation.auctioneer_commit)
        .map_err(|_error| HarnessError::new("release_invalid"))?;
    if attestation.package_sha256 != metadata.package.sha256
        || attestation.app_cli_sha256 != metadata.app_cli.sha256
        || attestation.release_sha256 != release_sha256
    {
        return Err(HarnessError::new("release_identity"));
    }
    // Inspect a private snapshot rather than repeatedly reopening an untrusted
    // package during tar listing/type/member checks. Never extract into source.
    let scratch = tempfile::Builder::new()
        .prefix("fastly-package-")
        .tempdir_in(&preparation.workspace)
        .map_err(|_error| HarnessError::new("output_io"))?;
    let package = scratch.path().join("package.tar.gz");
    let package_bytes =
        preparation::strict_file(&bundle, Path::new(&metadata.package.path), FILE_CAP)
            .map_err(|_error| HarnessError::new("release_invalid"))?;
    if io::sha256(&package_bytes) != metadata.package.sha256 {
        return Err(HarnessError::new("release_invalid"));
    }
    preparation::write_private(&package, &package_bytes, FILE_CAP)?;
    let package_receipt = verify_package(
        supervisor,
        &package,
        &build.artifact_sha256,
        &metadata.manifests.adapter.sha256,
    )
    .await
    .map_err(|_error| HarnessError::new("release_invalid"))?;
    let root = preparation.workspace.join("fastly-handoff");
    supervisor.check_active().await?;
    preparation::private_directory(&root)?;
    let mut members: BTreeMap<&str, &str> = metadata
        .members()
        .into_iter()
        .map(|member| (member.path.as_str(), member.sha256.as_str()))
        .collect();
    members.insert("release.json", &release_sha256);
    for (relative, expected) in members {
        supervisor.check_active().await?;
        let bytes = preparation::strict_file(&bundle, Path::new(relative), FILE_CAP)
            .map_err(|_error| HarnessError::new("release_invalid"))?;
        if io::sha256(&bytes) != expected {
            return Err(HarnessError::new("release_invalid"));
        }
        let dest = release_path(&root, relative)?;
        create_member_parents(&root, relative)?;
        preparation::write_private(&dest, &bytes, FILE_CAP)?;
    }
    let (_staged, staged_digest) = verify_bundle(&root, &preparation.auctioneer_commit)
        .map_err(|_error| HarnessError::new("release_invalid"))?;
    let (_original, original_digest) = verify_bundle(&bundle, &preparation.auctioneer_commit)
        .map_err(|_error| HarnessError::new("release_invalid"))?;
    if staged_digest != release_sha256 || original_digest != release_sha256 {
        return Err(HarnessError::new("release_invalid"));
    }
    verify_artifact(preparation, build, supervisor).await?;
    Ok(
        serde_json::json!({ "root": root, "sdk_verification": "required_at_deploy", "release_sha256": release_sha256,
        "artifact_sha256": build.artifact_sha256, "build_features": build.build_features, "package_receipt": package_receipt,
        "deploy_env": { "EDGEZERO_MANIFEST": release_path(&root, &metadata.manifests.edgezero.path)? },
        "deploy_args": ["deploy", "--adapter", "fastly", "--application-release", root.to_str().ok_or(HarnessError::new("release_invalid"))?, "--staging"] }),
    )
}

pub(super) fn create_member_parents(root: &Path, relative: &str) -> Result<()> {
    let path = release_path(root, relative)?;
    let parent = path.parent().ok_or(HarnessError::new("release_invalid"))?;
    let mut current = root.to_path_buf();
    for component in parent
        .strip_prefix(root)
        .map_err(|_error| HarnessError::new("release_invalid"))?
        .components()
    {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {
                preparation::private_directory(&current)?;
            }
            _ => return Err(HarnessError::new("release_invalid")),
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn minimal_bundle() -> (tempfile::TempDir, PathBuf, ReleaseMetadata) {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        for (name, bytes) in [
            ("auctioneer-cli", "synthetic CLI"),
            ("package.tar.gz", "synthetic package"),
            (
                "edgezero.toml",
                "[adapters.fastly.adapter]\nmanifest = 'fastly.toml'\n",
            ),
            ("fastly.toml", "name = 'synthetic'\n"),
        ] {
            fs::write(root.join(name), bytes).unwrap();
        }
        let member = |path: &str| ReleaseMember {
            path: path.to_owned(),
            sha256: io::file_digest(&root.join(path), FILE_CAP).unwrap(),
        };
        let metadata = ReleaseMetadata {
            format: 1,
            lifecycle_protocol: 1,
            source_revision: "a".repeat(40),
            adapter: "fastly".to_owned(),
            app_cli: member("auctioneer-cli"),
            package: member("package.tar.gz"),
            manifests: ReleaseManifests {
                edgezero: member("edgezero.toml"),
                adapter: member("fastly.toml"),
            },
        };
        fs::write(
            root.join("release.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        (directory, root, metadata)
    }

    #[test]
    fn extracted_bundle_checks_each_release_identity_without_archive_preflight() {
        let (_directory, root, metadata) = minimal_bundle();
        for (field, value) in [
            ("format", json!(2_u8)),
            ("lifecycle_protocol", json!(2_u8)),
            ("adapter", json!("spin")),
            ("source_revision", json!("b".repeat(40))),
        ] {
            let mut edited = serde_json::to_value(&metadata).unwrap();
            edited[field] = value;
            fs::write(
                root.join("release.json"),
                serde_json::to_vec(&edited).unwrap(),
            )
            .unwrap();
            assert_eq!(
                verify_bundle(&root, &metadata.source_revision).unwrap_err(),
                HarnessError::new("release_invalid"),
                "{field}"
            );
        }
        fs::write(
            root.join("release.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        assert_eq!(
            verify_bundle(&root, &metadata.source_revision)
                .unwrap()
                .0
                .source_revision,
            metadata.source_revision
        );
    }

    #[test]
    fn release_member_policy_requires_each_declared_name_and_no_undeclared_names() {
        let (_directory, _root, metadata) = minimal_bundle();
        let complete: BTreeSet<String> = [
            "release.json",
            "auctioneer-cli",
            "package.tar.gz",
            "edgezero.toml",
            "fastly.toml",
        ]
        .map(str::to_owned)
        .into();
        assert_eq!(
            metadata.validate(&metadata.source_revision, &complete),
            Ok(())
        );
        for name in &complete {
            let mut missing = complete.clone();
            assert!(missing.remove(name));
            assert_eq!(
                metadata.validate(&metadata.source_revision, &missing),
                Err(HarnessError::new("release_invalid")),
                "missing {name}"
            );
            missing.insert("undeclared".to_owned());
            assert_eq!(
                metadata.validate(&metadata.source_revision, &missing),
                Err(HarnessError::new("release_invalid")),
                "replaced {name}"
            );
        }
        let mut extra = complete;
        assert!(extra.insert("undeclared".to_owned()));
        assert_eq!(
            metadata.validate(&metadata.source_revision, &extra),
            Err(HarnessError::new("release_invalid"))
        );
    }

    #[test]
    fn release_member_policy_refuses_unsafe_paths_aliasing_and_invalid_digests() {
        let (_directory, _root, metadata) = minimal_bundle();
        let original = serde_json::to_value(&metadata).unwrap();
        let mut checked = 0_usize;
        for role in [
            "/app_cli",
            "/package",
            "/manifests/edgezero",
            "/manifests/adapter",
        ] {
            for (field, value) in [
                ("path", "../outside"),
                ("path", "release.json"),
                ("sha256", "not-a-digest"),
            ] {
                let mut edited = original.clone();
                *edited.pointer_mut(&format!("{role}/{field}")).unwrap() = json!(value);
                let changed: ReleaseMetadata = serde_json::from_value(edited).unwrap();
                // Matching the attacker's declared names must not authorize an
                // unsafe path, duplicate role, or malformed member digest.
                let mut files = BTreeSet::from(["release.json".to_owned()]);
                files.extend(changed.members().map(|member| member.path.clone()));
                assert_eq!(
                    changed.validate(&metadata.source_revision, &files),
                    Err(HarnessError::new("release_invalid")),
                    "{role}/{field} = {value}"
                );
                checked = checked.checked_add(1).unwrap();
            }
        }
        assert_eq!(checked, 12);
    }

    #[test]
    fn extracted_bundle_rejects_extra_files_without_archive_or_sdk_checks() {
        let (_directory, root, metadata) = minimal_bundle();
        assert_eq!(
            verify_bundle(&root, &metadata.source_revision)
                .unwrap()
                .0
                .adapter,
            "fastly"
        );
        fs::write(root.join("unexpected-sixth-member"), []).unwrap();
        assert_eq!(
            verify_bundle(&root, &metadata.source_revision).unwrap_err(),
            HarnessError::new("release_invalid")
        );
        fs::remove_file(root.join("unexpected-sixth-member")).unwrap();
        assert_eq!(
            verify_bundle(&root, &metadata.source_revision)
                .unwrap()
                .0
                .adapter,
            "fastly"
        );
    }

    #[test]
    fn extracted_walk_caps_regular_files_without_metadata_checks() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0_usize..5 {
            fs::write(directory.path().join(format!("member-{index}")), []).unwrap();
        }
        assert_eq!(
            release_files(directory.path(), "", &mut 0).unwrap().len(),
            5
        );
        fs::write(directory.path().join("member-5"), []).unwrap();
        assert_eq!(
            release_files(directory.path(), "", &mut 0).unwrap_err(),
            HarnessError::new("release_invalid")
        );
    }

    #[test]
    fn extracted_walk_caps_cumulative_entries_independently_of_regular_files() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("member"), []).unwrap();
        for initial in [126_usize, 127, 128] {
            let mut nodes = initial;
            let result = release_files(directory.path(), "", &mut nodes).map(|files| files.len());
            assert_eq!(
                result,
                if initial <= 127 {
                    Ok(1)
                } else {
                    Err(HarnessError::new("release_invalid"))
                }
            );
            assert_eq!(nodes, initial + 1);
        }
    }

    fn messages(plan: &Preparation, artifact: &Path) -> Vec<Value> {
        vec![
            json!({"reason":"compiler-artifact","target":{"name":"auctioneer_config"},"features":["test-harness-override"],"filenames":[]}),
            json!({"reason":"compiler-artifact","target":{"name":format!("auctioneer_adapter_{}",plan.provider)},
                "features":[plan.provider],"filenames":[artifact],"profile":{"opt_level":"3","debug_assertions":false,"test":false},"fresh":false}),
            json!({"reason":"build-finished","success":true}),
        ]
    }

    fn json_lines(rows: &[Value]) -> String {
        rows.iter()
            .map(|row| serde_json::to_string(row).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn artifact(plan: &Preparation) -> BuildReceipt {
        let parent = plan.target_dir.join(&plan.target).join(&plan.profile);
        fs::create_dir_all(&parent).unwrap();
        let file = parent.join(format!("auctioneer_adapter_{}.wasm", plan.provider));
        fs::write(&file, b"\0asm\x01\0\0\0").unwrap();
        BuildReceipt {
            artifact: file,
            artifact_sha256: io::sha256(b"\0asm\x01\0\0\0"),
            source_sha256: plan.source_sha256.clone(),
            fixture_metadata_sha256: plan.fixture_metadata_sha256.clone(),
            started_ms: 1,
            target: plan.target.clone(),
            profile: plan.profile.clone(),
            build_features: plan.build_features.clone(),
        }
    }

    #[test]
    fn cargo_message_parser_requires_bounded_complete_success_and_keeps_compiler_extensions() {
        cargo_messages(
            "{\"reason\":\"build-finished\",\"success\":true,\"future_compiler_field\":false}",
        )
        .unwrap();
        for source in [
            "",
            "SECRET_CANARY",
            "{}",
            "{\"reason\":\"build-finished\",\"success\":false}",
            "{\"reason\":\"build-finished\",\"success\":true}\n{\"reason\":\"compiler-artifact\"}",
        ] {
            assert_eq!(
                cargo_messages(source).unwrap_err(),
                HarnessError::new("artifact_identity")
            );
        }
        let mut too_many = "{\"reason\":\"compiler-artifact\"}\n".repeat(10_000);
        too_many.push_str("{\"reason\":\"build-finished\",\"success\":true}");
        assert_eq!(
            cargo_messages(&too_many).unwrap_err(),
            HarnessError::new("artifact_identity")
        );
        assert_eq!(
            cargo_messages(&" ".repeat((8 << 20) + 1)).unwrap_err(),
            HarnessError::new("artifact_identity")
        );
    }

    #[tokio::test]
    async fn capture_accepts_only_exact_compiler_features_target_profile_and_emitted_file() {
        let supervisor = process::Supervisor::new();
        let (_source, _work, plan) = preparation::tests::prepared("spin", &supervisor).await;
        let build = artifact(&plan);
        let valid = messages(&plan, &build.artifact);
        let capture_input = plan.workspace.join("capture-input.json");
        io::write_json(
            &capture_input,
            &json!({"preparation":plan,"cargo_json_lines":json_lines(&valid),"started_ms":1_u64}),
            RECEIPT_CAP,
        )
        .unwrap();
        let captured: BuildReceipt =
            serde_json::from_value(capture_build(&capture_input, &supervisor).await.unwrap())
                .unwrap();
        assert_eq!(captured.artifact, build.artifact);
        assert_eq!(captured.artifact_sha256, build.artifact_sha256);
        let mut checked = 0_usize;
        for (row, field, bad, code) in [
            (0_usize, "features", json!([]), "build_feature_missing"),
            (
                0,
                "features",
                json!(["test-harness-override", "insecure-bidders"]),
                "build_feature_missing",
            ),
            (
                0,
                "target",
                json!({"name":"wrong"}),
                "build_feature_missing",
            ),
            (1, "features", json!([]), "artifact_identity"),
            (
                1,
                "features",
                json!(["spin", "insecure-bidders"]),
                "artifact_identity",
            ),
            (1, "target", json!({"name":"wrong"}), "artifact_identity"),
            (
                1,
                "profile",
                json!({"opt_level":"s","debug_assertions":false,"test":false}),
                "artifact_identity",
            ),
            (
                1,
                "profile",
                json!({"opt_level":"3","debug_assertions":true,"test":false}),
                "artifact_identity",
            ),
            (
                1,
                "profile",
                json!({"opt_level":"3","debug_assertions":false,"test":true}),
                "artifact_identity",
            ),
            (1, "filenames", json!([]), "artifact_identity"),
            (
                1,
                "filenames",
                json!([build.artifact, build.artifact]),
                "artifact_identity",
            ),
            (1, "fresh", json!(true), "artifact_identity"),
            (2, "success", json!(false), "artifact_identity"),
        ] {
            let mut changed = valid.clone();
            changed[row][field] = bad;
            assert_eq!(
                capture(&plan, &json_lines(&changed), 1, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new(code),
                "{row}:{field}"
            );
            checked = checked.checked_add(1).unwrap();
        }
        let mut duplicated = valid.clone();
        duplicated.insert(1, valid[1].clone());
        assert_eq!(
            capture(&plan, &json_lines(&duplicated), 1, &supervisor)
                .await
                .unwrap_err(),
            HarnessError::new("artifact_identity")
        );
        assert_eq!(checked, 13);
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn spin_handoff_binds_receipt_identity_and_the_exact_staged_component() {
        let supervisor = process::Supervisor::new();
        let (_source, _work, plan) = preparation::tests::prepared("spin", &supervisor).await;
        let build = artifact(&plan);
        for field in [
            "target",
            "profile",
            "source_sha256",
            "fixture_metadata_sha256",
            "build_features",
            "artifact_sha256",
            "started_ms",
            "artifact",
        ] {
            let mut changed = serde_json::to_value(&build).unwrap();
            changed[field] = match field {
                "build_features" => json!(["spin"]),
                "started_ms" => json!(0_u64),
                "artifact_sha256" | "source_sha256" | "fixture_metadata_sha256" => {
                    json!("b".repeat(64))
                }
                "artifact" => {
                    json!("/repository/target/wasm32-wasip2/release/auctioneer_adapter_spin.wasm")
                }
                _ => json!("wrong"),
            };
            let bad: BuildReceipt = serde_json::from_value(changed).unwrap();
            assert_eq!(
                spin(&plan, &bad, &supervisor).await.unwrap_err(),
                HarnessError::new(if field == "artifact_sha256" {
                    "artifact_digest"
                } else {
                    "artifact_identity"
                })
            );
        }
        let input = plan.workspace.join("spin-input.json");
        io::write_json(
            &input,
            &json!({"preparation":plan,"build":build}),
            RECEIPT_CAP,
        )
        .unwrap();
        let result = stage_spin(&input, &supervisor).await.unwrap();
        let manifest_path = Path::new(result["manifest"].as_str().unwrap());
        let manifest = preparation::parse_toml(&fs::read(manifest_path).unwrap()).unwrap();
        let component = &manifest["component"]["auctioneer-adapter-spin"];
        assert_eq!(
            component["source"].as_str(),
            Some("./auctioneer_adapter_spin.wasm")
        );
        assert!(component.get("build").is_none());
        let staged = manifest_path
            .parent()
            .unwrap()
            .join(component["source"].as_str().unwrap());
        assert_eq!(
            io::file_digest(&staged, FILE_CAP).unwrap(),
            build.artifact_sha256
        );
        preparation::verify_production(&plan).unwrap();
        supervisor.cleanup().await.unwrap();
    }

    pub async fn release(
        supervisor: &process::Supervisor,
    ) -> (
        tempfile::TempDir,
        tempfile::TempDir,
        tempfile::TempDir,
        Preparation,
        BuildReceipt,
        ReleaseMetadata,
        ReleaseAttestation,
    ) {
        let (source, work, plan) = preparation::tests::prepared("fastly", supervisor).await;
        let build = artifact(&plan);
        let bundle = tempfile::tempdir().unwrap();
        let package = tempfile::tempdir().unwrap();
        fs::create_dir_all(package.path().join("bin")).unwrap();
        fs::copy(&build.artifact, package.path().join("bin/main.wasm")).unwrap();
        let adapter_bytes = fs::read(plan.repo_root.join("fastly.toml")).unwrap();
        fs::write(package.path().join("fastly.toml"), &adapter_bytes).unwrap();
        let archive = bundle.path().join("package.tar.gz");
        let mut tar_spec = process::ProcessSpec::new("/usr/bin/tar");
        tar_spec.args = vec![
            "-czf".into(),
            archive.as_os_str().to_owned(),
            "-C".into(),
            package.path().as_os_str().to_owned(),
            "bin/main.wasm".into(),
            "fastly.toml".into(),
        ];
        tar_spec.env = preparation::tool_environment();
        tar_spec.env.insert("COPYFILE_DISABLE".into(), "1".into());
        supervisor.run(tar_spec).await.unwrap();
        fs::write(
            bundle.path().join("auctioneer-cli"),
            b"synthetic offline CLI member",
        )
        .unwrap();
        fs::write(bundle.path().join("fastly.toml"), &adapter_bytes).unwrap();
        let mut application =
            preparation::parse_toml(&fs::read(plan.repo_root.join("edgezero.toml")).unwrap())
                .unwrap();
        application["adapters"]["fastly"]["adapter"]["manifest"] =
            toml::Value::String("fastly.toml".to_owned());
        fs::write(
            bundle.path().join("edgezero.toml"),
            preparation::encode_toml(&application).unwrap(),
        )
        .unwrap();
        let member = |path: &str| ReleaseMember {
            path: path.to_owned(),
            sha256: io::file_digest(&bundle.path().join(path), FILE_CAP).unwrap(),
        };
        let metadata = ReleaseMetadata {
            format: 1,
            lifecycle_protocol: 1,
            source_revision: plan.auctioneer_commit.clone(),
            adapter: "fastly".to_owned(),
            app_cli: member("auctioneer-cli"),
            package: member("package.tar.gz"),
            manifests: ReleaseManifests {
                edgezero: member("edgezero.toml"),
                adapter: member("fastly.toml"),
            },
        };
        fs::write(
            bundle.path().join("release.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        let attestation = ReleaseAttestation {
            source_sha256: plan.source_sha256.clone(),
            artifact_sha256: build.artifact_sha256.clone(),
            build_features: build.build_features.clone(),
            package_sha256: metadata.package.sha256.clone(),
            app_cli_sha256: metadata.app_cli.sha256.clone(),
            release_sha256: io::file_digest(&bundle.path().join("release.json"), FILE_CAP).unwrap(),
        };
        (source, work, bundle, plan, build, metadata, attestation)
    }

    #[tokio::test]
    async fn complete_fastly_bundle_handoff_requires_every_member_digest_identity_and_exact_set() {
        let supervisor = process::Supervisor::new();
        let (_source, _work, bundle, plan, build, metadata, attestation) =
            release(&supervisor).await;
        let mut checked = 0_usize;
        for member in [
            "release.json",
            "auctioneer-cli",
            "package.tar.gz",
            "edgezero.toml",
            "fastly.toml",
        ] {
            let file = bundle.path().join(member);
            let bytes = fs::read(&file).unwrap();
            fs::remove_file(&file).unwrap();
            assert_eq!(
                fastly(&plan, &build, bundle.path(), &attestation, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("release_invalid")
            );
            fs::write(&file, b"changed").unwrap();
            assert_eq!(
                fastly(&plan, &build, bundle.path(), &attestation, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("release_invalid")
            );
            fs::write(&file, bytes).unwrap();
            checked = checked.checked_add(2).unwrap();
        }
        for field in [
            "source_sha256",
            "artifact_sha256",
            "package_sha256",
            "app_cli_sha256",
            "release_sha256",
            "build_features",
        ] {
            let mut changed = serde_json::to_value(&attestation).unwrap();
            changed[field] = if field == "build_features" {
                json!(["fastly"])
            } else {
                json!("b".repeat(64))
            };
            let bad = serde_json::from_value(changed).unwrap();
            assert_eq!(
                fastly(&plan, &build, bundle.path(), &bad, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("release_identity")
            );
            checked = checked.checked_add(1).unwrap();
        }
        fs::write(bundle.path().join("extra"), b"extra").unwrap();
        assert_eq!(
            fastly(&plan, &build, bundle.path(), &attestation, &supervisor)
                .await
                .unwrap_err(),
            HarnessError::new("release_invalid")
        );
        fs::remove_file(bundle.path().join("extra")).unwrap();
        let input = plan.workspace.join("fastly-input.json");
        io::write_json(&input,&json!({"preparation":plan,"build":build,"bundle_root":bundle.path(),"attestation":attestation}),RECEIPT_CAP).unwrap();
        let result = stage_fastly(&input, &supervisor).await.unwrap();
        assert_eq!(result["sdk_verification"], "required_at_deploy");
        assert_eq!(result["deploy_args"][2], "fastly");
        assert_eq!(result["deploy_args"][3], "--application-release");
        assert_eq!(result["deploy_args"][5], "--staging");
        let staged_root = Path::new(result["root"].as_str().unwrap());
        let (staged, _) = verify_bundle(staged_root, &plan.auctioneer_commit).unwrap();
        assert_eq!(
            serde_json::to_value(staged).unwrap(),
            serde_json::to_value(metadata).unwrap()
        );
        assert_eq!(checked, 16);
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn package_member_names_reject_traversal_duplicates_and_ambiguous_components() {
        let (_, artifact_member, manifest_member) =
            package_members(b"bin/main.wasm\nfastly.toml\n").unwrap();
        assert_eq!(artifact_member, "bin/main.wasm");
        assert_eq!(manifest_member, "fastly.toml");
        for names in [
            "../bin/main.wasm\nfastly.toml\n",
            "/bin/main.wasm\nfastly.toml\n",
            "bin/main.wasm\nfastly.toml\nfastly.toml\n",
            "bin/main.wasm\na/bin/main.wasm\nfastly.toml\n",
            "bin/main.wasm\nfastly.toml\na/fastly.toml\n",
            "bin/main.wasm\n",
        ] {
            assert_eq!(
                package_members(names.as_bytes()).unwrap_err(),
                HarnessError::new("release_invalid")
            );
        }
    }

    #[tokio::test]
    async fn release_metadata_and_member_type_changes_cannot_be_reauthorized_by_rehashing() {
        let supervisor = process::Supervisor::new();
        let (_source, _work, bundle, plan, build, metadata, attestation) =
            release(&supervisor).await;
        for (field, value) in [
            ("adapter", json!("spin")),
            ("format", json!(2_u8)),
            ("lifecycle_protocol", json!(2_u8)),
            ("source_revision", json!("b".repeat(40))),
        ] {
            let mut edited = serde_json::to_value(&metadata).unwrap();
            edited[field] = value;
            let bytes = serde_json::to_vec(&edited).unwrap();
            fs::write(bundle.path().join("release.json"), &bytes).unwrap();
            let resigned = ReleaseAttestation {
                release_sha256: io::sha256(&bytes),
                ..attestation.clone()
            };
            assert_eq!(
                fastly(&plan, &build, bundle.path(), &resigned, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("release_invalid")
            );
        }
        fs::write(
            bundle.path().join("release.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        #[cfg(unix)]
        let links_removed = {
            use std::os::unix::fs::symlink;
            let path = bundle.path().join("auctioneer-cli");
            let outside = tempfile::NamedTempFile::new().unwrap();
            fs::write(outside.path(), fs::read(&path).unwrap()).unwrap();
            fs::remove_file(&path).unwrap();
            symlink(outside.path(), &path).unwrap();
            assert_eq!(
                fastly(&plan, &build, bundle.path(), &attestation, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("release_invalid")
            );
            fs::remove_file(&path).unwrap();
            fs::hard_link(outside.path(), &path).unwrap();
            assert_eq!(
                fastly(&plan, &build, bundle.path(), &attestation, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("release_invalid")
            );
            fs::remove_file(&path).unwrap();
            !path.exists()
        };
        #[cfg(unix)]
        assert!(links_removed);
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn capture_and_component_handoff_reject_modified_added_deleted_and_untracked_build_inputs()
     {
        let supervisor = process::Supervisor::new();
        let mut checked = 0_usize;
        for boundary in ["capture", "handoff"] {
            for mutation in ["modify", "add", "delete", "untracked_modify"] {
                let (_source, _work, plan) =
                    preparation::tests::prepared_with_source("spin", &supervisor, |root| {
                        fs::write(
                            root.join("crates/auctioneer-core/src/local_untracked.rs"),
                            b"A",
                        )
                        .unwrap();
                    })
                    .await;
                assert!(
                    preparation::git(
                        &supervisor,
                        &plan.repo_root,
                        &[
                            "ls-files",
                            "--",
                            "crates/auctioneer-core/src/local_untracked.rs"
                        ],
                        INPUT_CAP,
                    )
                    .await
                    .unwrap()
                    .is_empty()
                );
                let build = artifact(&plan);
                let file = plan.repo_root.join(if mutation == "untracked_modify" {
                    "crates/auctioneer-core/src/local_untracked.rs"
                } else if mutation == "add" {
                    "crates/auctioneer-core/src/new_source.rs"
                } else {
                    "crates/auctioneer-core/src/lib.rs"
                });
                if mutation == "delete" {
                    fs::remove_file(file).unwrap();
                } else {
                    fs::write(file, b"changed").unwrap();
                }
                let result = if boundary == "capture" {
                    capture(
                        &plan,
                        &json_lines(&messages(&plan, &build.artifact)),
                        1,
                        &supervisor,
                    )
                    .await
                    .map(|_receipt| ())
                } else {
                    spin(&plan, &build, &supervisor).await.map(|_receipt| ())
                };
                assert_eq!(
                    result.unwrap_err(),
                    HarnessError::new("source_changed"),
                    "{boundary}:{mutation}"
                );
                checked = checked.checked_add(1).unwrap();
            }
        }
        assert_eq!(checked, 8);
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn capture_and_component_handoff_reject_each_isolated_metadata_member_change() {
        let supervisor = process::Supervisor::new();
        let mut checked = 0_usize;
        for boundary in ["capture", "handoff"] {
            for mutation in ["modify", "add", "delete"] {
                let (_source, _work, plan) =
                    preparation::tests::prepared("spin", &supervisor).await;
                let build = artifact(&plan);
                let file = if mutation == "add" {
                    plan.bidders_dir.join("extra.toml")
                } else {
                    fs::read_dir(&plan.bidders_dir)
                        .unwrap()
                        .map(|entry| entry.unwrap().path().join("bidder.toml"))
                        .filter(|path| path.is_file())
                        .min()
                        .unwrap()
                };
                if mutation == "delete" {
                    fs::remove_file(file).unwrap();
                } else {
                    fs::write(file, b"changed").unwrap();
                }
                let result = if boundary == "capture" {
                    capture(
                        &plan,
                        &json_lines(&messages(&plan, &build.artifact)),
                        1,
                        &supervisor,
                    )
                    .await
                    .map(|_receipt| ())
                } else {
                    spin(&plan, &build, &supervisor).await.map(|_receipt| ())
                };
                assert_eq!(
                    result.unwrap_err(),
                    HarnessError::new("fixture_changed"),
                    "{boundary}:{mutation}"
                );
                checked = checked.checked_add(1).unwrap();
            }
        }
        assert_eq!(checked, 6);
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn component_identity_refuses_missing_stale_linked_and_rehashed_non_wasm_files() {
        use std::fs::{File, FileTimes};
        let supervisor = process::Supervisor::new();
        let (_source, _work, plan) = preparation::tests::prepared("spin", &supervisor).await;
        let build = artifact(&plan);
        fs::remove_file(&build.artifact).unwrap();
        assert_eq!(
            verify_artifact(&plan, &build, &supervisor)
                .await
                .unwrap_err(),
            HarnessError::new("artifact_missing")
        );
        fs::write(&build.artifact, b"\0asm\x01\0\0\0").unwrap();
        File::options()
            .write(true)
            .open(&build.artifact)
            .unwrap()
            .set_times(FileTimes::new().set_modified(UNIX_EPOCH))
            .unwrap();
        assert_eq!(
            verify_artifact(&plan, &build, &supervisor)
                .await
                .unwrap_err(),
            HarnessError::new("artifact_stale")
        );
        File::options()
            .write(true)
            .open(&build.artifact)
            .unwrap()
            .set_times(FileTimes::new().set_modified(SystemTime::now()))
            .unwrap();
        verify_artifact(&plan, &build, &supervisor).await.unwrap();
        fs::write(&build.artifact, b"NOT_WASM").unwrap();
        assert_eq!(
            verify_artifact(&plan, &build, &supervisor)
                .await
                .unwrap_err(),
            HarnessError::new("artifact_digest")
        );
        let rehashed = BuildReceipt {
            artifact_sha256: io::sha256(b"NOT_WASM"),
            ..build.clone()
        };
        assert_eq!(
            verify_artifact(&plan, &rehashed, &supervisor)
                .await
                .unwrap_err(),
            HarnessError::new("artifact_identity")
        );
        #[cfg(unix)]
        let removed_links = {
            use std::os::unix::fs::symlink;
            let outside = tempfile::NamedTempFile::new().unwrap();
            fs::write(outside.path(), b"\0asm\x01\0\0\0").unwrap();
            fs::remove_file(&build.artifact).unwrap();
            symlink(outside.path(), &build.artifact).unwrap();
            assert_eq!(
                verify_artifact(&plan, &build, &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("artifact_identity")
            );
            fs::remove_file(&build.artifact).unwrap();
            fs::hard_link(outside.path(), &build.artifact).unwrap();
            let hard_link_error = verify_artifact(&plan, &build, &supervisor)
                .await
                .unwrap_err();
            fs::remove_file(&build.artifact).unwrap();
            (hard_link_error, !build.artifact.exists())
        };
        #[cfg(unix)]
        assert_eq!(
            removed_links,
            (HarnessError::new("artifact_identity"), true)
        );
        supervisor.cleanup().await.unwrap();
    }
}
