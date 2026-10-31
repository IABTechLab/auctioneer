use super::super::{
    HarnessError, Result, io,
    process::{ProcessSpec, Supervisor},
    providers::preparation,
};
use super::inputs::{self, Candidate, Catalog, Export};
use crate::qualification::profile::{self, Digest, Hardware, Revision};
use auctioneer_cli::native_control::SOURCE_ROOT;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::env::{
    consts::{ARCH, OS},
    var_os,
};
use std::ffi::OsString;
#[cfg(unix)]
use std::fs::symlink_metadata;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use std::thread::available_parallelism;
use tokio::net::TcpListener;

const METADATA_CAP: usize = 4 << 20;
const PROC_MEMORY_CAP: usize = 64 << 10;
const PROC_CPU_CAP: usize = 4 << 20;
const ARTIFACT_CAP: usize = 512 << 20;
const CONSENT_CAP: usize = 16 << 10;
const GIT_OUTPUT_CAP: usize = 512 << 10;
const SOURCE_FILE_CAP: usize = 10_000;
const RAW_HASH_BATCH_CAP: usize = 128;
const RAW_HASH_ARG_CAP: usize = 16 << 10;
const RAW_HASH_OUTPUT_CAP: usize = 128 * 65;
const HARNESS_SOURCE: &str = "crates/auctioneer-cli/src/qualification/harness/";
const CONSENT_SOURCE: &str = "crates/auctioneer-privacy/tests/data/tcf_v2/valid_full_consent.txt";
const SOURCE_ROOTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "edgezero.toml",
    "auctioneer.toml",
    "crates",
    ".cargo",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifacts {
    pub axum: PathBuf,
    pub cli: PathBuf,
    pub runtime_version: profile::Version,
}

pub fn repository_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|_error| HarnessError::new("invalid_input"))
}

pub async fn cargo_artifacts(
    root: &Path,
    build: bool,
    supervisor: &Supervisor,
) -> Result<Artifacts> {
    let metadata = run_bytes(cargo_metadata_spec(root), supervisor, "metadata_failed").await?;
    let artifacts = parse_metadata(&metadata)?;
    if build {
        run_bytes(cargo_build_spec(root), supervisor, "subprocess_failed").await?;
    }
    Ok(artifacts)
}

pub async fn source_commit(
    root: &Path,
    requested: Option<&str>,
    supervisor: &Supervisor,
) -> Result<Revision> {
    let refusal = HarnessError::new("identity_mismatch");
    if let Some(value) = requested {
        Revision::try_from(value.to_owned()).map_err(|_error| refusal)?;
    }
    let commit = format!("{}^{{commit}}", requested.unwrap_or("HEAD"));
    let revision_output = run_bytes(
        git_spec(
            root,
            &["rev-parse", "--verify", "--end-of-options", &commit],
        ),
        supervisor,
        "identity_mismatch",
    )
    .await?;
    let revision = from_utf8(&revision_output)
        .map_err(|_error| refusal)?
        .trim();
    let identity = Revision::try_from(revision.to_owned()).map_err(|_error| refusal)?;
    if requested.is_some_and(|value| value != revision) {
        return Err(refusal);
    }
    let dirty = HarnessError::new("source_dirty");
    let mut diff_arguments = vec![
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-renames",
        "--ignore-submodules=none",
        "--name-only",
        "-z",
        revision,
        "--",
    ];
    diff_arguments.extend_from_slice(SOURCE_ROOTS);
    if !run_bytes(git_spec(root, &diff_arguments), supervisor, "source_dirty")
        .await?
        .is_empty()
    {
        return Err(dirty);
    }

    let mut tracked_arguments = vec!["ls-files", "--cached", "-v", "-z", "--"];
    tracked_arguments.extend_from_slice(SOURCE_ROOTS);
    let tracked_output = run_bytes(
        git_spec(root, &tracked_arguments),
        supervisor,
        "source_dirty",
    )
    .await?;
    let tracked = tracked_source_paths(&tracked_output)?;
    // Git excludes ignored inputs; the filesystem inventory deliberately does not.
    let inventory = preparation::source_inventory(root).map_err(|_error| dirty)?;
    let files: BTreeSet<_> = inventory
        .iter()
        .filter(|(_path, digest)| digest.is_some())
        .map(|(path, _digest)| path.as_str())
        .collect();
    if files != tracked {
        return Err(dirty);
    }
    let tree_output =
        run_bytes(source_tree_spec(root, revision), supervisor, "source_dirty").await?;
    let committed = parse_source_tree(&tree_output)?;
    let committed_paths: BTreeSet<_> = committed.keys().copied().collect();
    if files != committed_paths {
        return Err(dirty);
    }
    assert_source_modes(root, &committed)?;
    let paths: Vec<_> = committed.keys().copied().collect();
    for batch in raw_hash_batches(root, &paths)? {
        let hash_output =
            run_bytes(raw_hash_spec(root, &batch)?, supervisor, "source_dirty").await?;
        let hashes = parse_raw_hashes(&hash_output, batch.len())?;
        for (path, actual) in batch.iter().zip(hashes) {
            if committed.get(*path).ok_or(dirty)?.oid != actual {
                return Err(dirty);
            }
        }
    }
    // Fence the raw Git hashes with strict byte inventories and independent mode checks.
    if preparation::source_inventory(root).map_err(|_error| dirty)? != inventory {
        return Err(dirty);
    }
    assert_source_modes(root, &committed)?;
    supervisor.check_active().await?;
    Ok(identity)
}

fn tracked_source_paths(bytes: &[u8]) -> Result<BTreeSet<&str>> {
    let dirty = HarnessError::new("source_dirty");
    let mut files = BTreeSet::new();
    if bytes.is_empty() {
        return Ok(files);
    }
    let records = bytes.strip_suffix(&[0_u8]).ok_or(dirty)?;
    for record in records.split(|byte| *byte == 0) {
        // Lowercase tags and S hide changes through assume-unchanged/skip-worktree.
        if record.get(..2) != Some(b"H ".as_slice()) {
            return Err(dirty);
        }
        let path = from_utf8(record.get(2..).ok_or(dirty)?).map_err(|_error| dirty)?;
        if path.is_empty() || files.len() >= SOURCE_FILE_CAP || !files.insert(path) {
            return Err(dirty);
        }
    }
    Ok(files)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlobMode {
    Regular,
    Executable,
}

#[derive(Debug)]
struct SourceBlob<'a> {
    mode: BlobMode,
    oid: &'a str,
}

fn source_tree_spec(root: &Path, revision: &str) -> ProcessSpec {
    let mut arguments = vec!["ls-tree", "-r", "-z", "--full-tree", revision, "--"];
    arguments.extend_from_slice(SOURCE_ROOTS);
    git_spec(root, &arguments)
}

fn parse_source_tree(bytes: &[u8]) -> Result<BTreeMap<&str, SourceBlob<'_>>> {
    let dirty = HarnessError::new("source_dirty");
    if bytes.len() > GIT_OUTPUT_CAP {
        return Err(dirty);
    }
    let mut files = BTreeMap::new();
    if bytes.is_empty() {
        return Ok(files);
    }
    let records = bytes.strip_suffix(&[0_u8]).ok_or(dirty)?;
    for raw_record in records.split(|byte| *byte == 0) {
        let record = from_utf8(raw_record).map_err(|_error| dirty)?;
        let (header, path) = record.split_once('\t').ok_or(dirty)?;
        let mut fields = header.split(' ');
        let mode = match fields.next() {
            Some("100644") => BlobMode::Regular,
            Some("100755") => BlobMode::Executable,
            _ => return Err(dirty),
        };
        if fields.next() != Some("blob") {
            return Err(dirty);
        }
        let oid = fields.next().ok_or(dirty)?;
        if fields.next().is_some()
            || !valid_object_id(oid)
            || !source_path(path)
            || files.len() >= SOURCE_FILE_CAP
            || files.insert(path, SourceBlob { mode, oid }).is_some()
        {
            return Err(dirty);
        }
    }
    Ok(files)
}

fn source_path(path: &str) -> bool {
    !path.contains('\0')
        && !path.split('/').any(|part| matches!(part, "" | "." | ".."))
        && SOURCE_ROOTS.iter().any(|root| {
            path == *root
                || path
                    .strip_prefix(*root)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
}

fn valid_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_raw_hashes(bytes: &[u8], count: usize) -> Result<Vec<&str>> {
    let dirty = HarnessError::new("source_dirty");
    if count > RAW_HASH_BATCH_CAP || bytes.len() > RAW_HASH_OUTPUT_CAP {
        return Err(dirty);
    }
    if count == 0 {
        return if bytes.is_empty() {
            Ok(Vec::new())
        } else {
            Err(dirty)
        };
    }
    let text = from_utf8(bytes).map_err(|_error| dirty)?;
    let output = text.strip_suffix('\n').ok_or(dirty)?;
    let mut hashes = Vec::with_capacity(count);
    for oid in output.split('\n') {
        if hashes.len() >= count || !valid_object_id(oid) {
            return Err(dirty);
        }
        hashes.push(oid);
    }
    if hashes.len() != count {
        return Err(dirty);
    }
    Ok(hashes)
}

fn raw_hash_spec(root: &Path, paths: &[&str]) -> Result<ProcessSpec> {
    let dirty = HarnessError::new("source_dirty");
    if paths.is_empty()
        || paths.len() > RAW_HASH_BATCH_CAP
        || paths.iter().any(|path| !source_path(path))
    {
        return Err(dirty);
    }
    let mut spec = git_spec(root, &["hash-object", "--no-filters", "--"]);
    spec.args.extend(paths.iter().map(OsString::from));
    if argument_bytes(&spec)? > RAW_HASH_ARG_CAP {
        return Err(dirty);
    }
    spec.output_cap = RAW_HASH_OUTPUT_CAP;
    Ok(spec)
}

fn raw_hash_batches<'a>(root: &Path, paths: &[&'a str]) -> Result<Vec<Vec<&'a str>>> {
    let dirty = HarnessError::new("source_dirty");
    if paths.len() > SOURCE_FILE_CAP {
        return Err(dirty);
    }
    let base = argument_bytes(&git_spec(root, &["hash-object", "--no-filters", "--"]))?;
    if base > RAW_HASH_ARG_CAP {
        return Err(dirty);
    }
    let mut batches = Vec::new();
    let mut batch = Vec::new();
    let mut bytes = base;
    for path in paths {
        if !source_path(path) {
            return Err(dirty);
        }
        let size = path.len().checked_add(1).ok_or(dirty)?;
        let single = base.checked_add(size).ok_or(dirty)?;
        if single > RAW_HASH_ARG_CAP {
            return Err(dirty);
        }
        let next = bytes.checked_add(size).ok_or(dirty)?;
        if batch.len() >= RAW_HASH_BATCH_CAP || next > RAW_HASH_ARG_CAP {
            batches.push(batch);
            batch = Vec::new();
            bytes = base;
        }
        bytes = bytes.checked_add(size).ok_or(dirty)?;
        batch.push(*path);
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    Ok(batches)
}

fn argument_bytes(spec: &ProcessSpec) -> Result<usize> {
    let dirty = HarnessError::new("source_dirty");
    let program = spec
        .program
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .checked_add(1)
        .ok_or(dirty)?;
    spec.args.iter().try_fold(program, |total, argument| {
        total
            .checked_add(argument.as_encoded_bytes().len())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(dirty)
    })
}

#[cfg(unix)]
fn assert_source_modes(root: &Path, files: &BTreeMap<&str, SourceBlob<'_>>) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let dirty = HarnessError::new("source_dirty");
    for (path, blob) in files {
        let metadata = symlink_metadata(root.join(path)).map_err(|_error| dirty)?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || (metadata.permissions().mode() & 0o100 != 0) != (blob.mode == BlobMode::Executable)
        {
            return Err(dirty);
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn assert_source_modes(_root: &Path, _files: &BTreeMap<&str, SourceBlob<'_>>) -> Result<()> {
    Err(HarnessError::new("identity_unsupported"))
}

pub async fn hardware_identity(
    platform: &profile::Platform,
    supervisor: &Supervisor,
) -> Result<Hardware> {
    let os = OS;
    let architecture = ARCH;
    native_platform(os, architecture)?;
    supervisor.check_active().await?;
    let (cpu, logical_cores, memory_bytes) = if os == "macos" {
        let memory = probe(HostProbe::MacMemory, supervisor).await?;
        let cores = probe(HostProbe::MacCores, supervisor).await?;
        let model = probe(HostProbe::MacCpu, supervisor).await?;
        (
            parse_cpu_model(os_text(&model)?)?,
            usize::try_from(parse_unsigned(&cores)?)
                .map_err(|_error| HarnessError::new("identity_unsupported"))?,
            parse_unsigned(&memory)?,
        )
    } else {
        let memory = io::read_bytes(Path::new("/proc/meminfo"), PROC_MEMORY_CAP)
            .map_err(|_error| HarnessError::new("identity_unsupported"))?;
        let models = io::read_bytes(Path::new("/proc/cpuinfo"), PROC_CPU_CAP)
            .map_err(|_error| HarnessError::new("identity_unsupported"))?;
        let cores =
            available_parallelism().map_err(|_error| HarnessError::new("identity_unsupported"))?;
        (
            parse_linux_cpu(&models)?,
            cores.get(),
            parse_linux_memory(&memory)?,
        )
    };
    let kernel = probe(HostProbe::Kernel, supervisor).await?;
    let hostname = probe(HostProbe::Hostname, supervisor).await?;
    let page = probe(HostProbe::Page, supervisor).await?;
    hardware_from_readings(
        HardwareReadings {
            os,
            architecture,
            cpu,
            logical_cores,
            memory_bytes,
            page_bytes: &page,
            kernel_release: &kernel,
            hostname: &hostname,
        },
        platform,
    )
}

pub async fn inspect_export(
    binary: &Path,
    environment: &BTreeMap<OsString, OsString>,
    cwd: &Path,
    supervisor: &Supervisor,
) -> Result<Export> {
    let bytes = run_bytes(
        export_spec(binary, environment, cwd),
        supervisor,
        "invalid_child_output",
    )
    .await?;
    inputs::parse_export(&bytes)
}

pub fn assert_same_export(expected: &Export, actual: &Export) -> Result<()> {
    let refusal = HarnessError::new("candidate_mismatch");
    let expected_value = serde_json::to_value(expected).map_err(|_error| refusal)?;
    let actual_value = serde_json::to_value(actual).map_err(|_error| refusal)?;
    if expected_value != actual_value {
        return Err(refusal);
    }
    Ok(())
}

pub async fn reserve_port() -> Result<u16> {
    for _attempt in 0_u8..4 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_error| HarnessError::new("port_collision"))?;
        let port = listener
            .local_addr()
            .map_err(|_error| HarnessError::new("port_collision"))?
            .port();
        drop(listener);
        if usable_port(port) {
            return Ok(port);
        }
    }
    Err(HarnessError::new("port_collision"))
}

pub fn harness_digest(root: &Path, executable: &Path) -> Result<Digest> {
    let executable_sha256 = io::file_digest(executable, ARTIFACT_CAP)?;
    let mut harness_sources = preparation::source_inventory(root)?;
    harness_sources.retain(|(path, _digest)| {
        let source_path = Path::new(path);
        source_path.starts_with(HARNESS_SOURCE) || source_path.starts_with(SOURCE_ROOT)
    });
    Digest::canonical(&json!({
        "harness_sources": harness_sources,
        "executable_sha256": executable_sha256,
        "fixture_protocol_version": inputs::FIXTURE_PROTOCOL_VERSION,
    }))
    .map_err(|_error| HarnessError::new("identity_mismatch"))
}

pub fn fixtures_digest(candidate: &Candidate, catalog: &Catalog, root: &Path) -> Result<Digest> {
    let consent_seed_sha256 = io::file_digest(&root.join(CONSENT_SOURCE), CONSENT_CAP)?;
    Digest::canonical(&json!({
        "candidate": candidate,
        "catalog": catalog,
        "debug_geometry": {"dense_debug": true, "formats": 8_u8},
        "fixture_protocol_version": inputs::FIXTURE_PROTOCOL_VERSION,
        "consent_seed_sha256": consent_seed_sha256,
    }))
    .map_err(|_error| HarnessError::new("invalid_input"))
}

#[derive(Deserialize)]
struct CargoMetadata {
    target_directory: PathBuf,
    packages: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

fn parse_metadata(bytes: &[u8]) -> Result<Artifacts> {
    let refusal = HarnessError::new("metadata_failed");
    if bytes.len() > METADATA_CAP {
        return Err(refusal);
    }
    auctioneer_runtime::count_json_nodes(bytes, 1 << 16).map_err(|_error| refusal)?;
    // Bound depth even in ignored compiler fields, then reject duplicate retained fields.
    let _bounded: serde_json::Value = serde_json::from_slice(bytes).map_err(|_error| refusal)?;
    let metadata: CargoMetadata = serde_json::from_slice(bytes).map_err(|_error| refusal)?;
    if !metadata.target_directory.is_absolute()
        || metadata
            .target_directory
            .as_os_str()
            .as_encoded_bytes()
            .contains(&0_u8)
    {
        return Err(refusal);
    }
    let mut packages = metadata
        .packages
        .iter()
        .filter(|package| package.name == "auctioneer-adapter-axum");
    let adapter = packages.next().ok_or(refusal)?;
    if packages.next().is_some() {
        return Err(refusal);
    }
    let runtime_version = package_version(&adapter.version).map_err(|_error| refusal)?;
    let release = metadata.target_directory.join("release");
    Ok(Artifacts {
        axum: release.join("auctioneer-adapter-axum"),
        cli: release.join("auctioneer-cli"),
        runtime_version,
    })
}

fn package_version(value: &str) -> Result<profile::Version> {
    let refusal = HarnessError::new("metadata_failed");
    let mut components = value.split('.');
    let major = version_component(components.next().ok_or(refusal)?)?;
    let minor = version_component(components.next().ok_or(refusal)?)?;
    let patch = version_component(components.next().ok_or(refusal)?)?;
    if components.next().is_some() {
        return Err(refusal);
    }
    Ok(profile::Version {
        major,
        minor,
        patch,
    })
}

fn cargo_metadata_spec(root: &Path) -> ProcessSpec {
    let mut spec = cargo_spec(root);
    spec.args = [
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--locked",
        "--offline",
    ]
    .map(OsString::from)
    .to_vec();
    spec
}

fn cargo_build_spec(root: &Path) -> ProcessSpec {
    let mut spec = cargo_spec(root);
    spec.args = [
        "build",
        "--release",
        "--locked",
        "--offline",
        "-p",
        "auctioneer-adapter-axum",
        "-p",
        "auctioneer-cli",
        "--features",
        "auctioneer-adapter-axum/insecure-bidders",
    ]
    .map(OsString::from)
    .to_vec();
    spec
}

fn cargo_spec(root: &Path) -> ProcessSpec {
    let mut spec = ProcessSpec::new(env!("CARGO"));
    spec.cwd = Some(root.to_owned());
    spec.env = inputs::tool_environment();
    spec.output_cap = METADATA_CAP;
    spec.timeout_ms = 600_000;
    spec
}

fn git_spec(root: &Path, arguments: &[&str]) -> ProcessSpec {
    let mut spec = ProcessSpec::new("/usr/bin/git");
    spec.args = vec![
        OsString::from("--no-replace-objects"),
        OsString::from("-C"),
        root.as_os_str().to_owned(),
    ];
    spec.args.extend(arguments.iter().map(OsString::from));
    spec.env = BTreeMap::from([
        (
            OsString::from("PATH"),
            var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin")),
        ),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            OsString::from("/dev/null"),
        ),
    ]);
    spec.output_cap = GIT_OUTPUT_CAP;
    spec.timeout_ms = 10_000;
    spec
}

async fn run_bytes(
    spec: ProcessSpec,
    supervisor: &Supervisor,
    code: &'static str,
) -> Result<Vec<u8>> {
    let output = supervisor
        .run(spec)
        .await
        .map_err(|error| boundary_error(error, code))?;
    if !output.summary.closed {
        return Err(HarnessError::new(code));
    }
    Ok(output.stdout)
}

fn boundary_error(error: HarnessError, code: &'static str) -> HarnessError {
    match error.code {
        "run_deadline" | "interrupted" | "cleanup_failed" => error,
        _ => HarnessError::new(code),
    }
}

fn export_spec(
    binary: &Path,
    environment: &BTreeMap<OsString, OsString>,
    cwd: &Path,
) -> ProcessSpec {
    let mut spec = ProcessSpec::new(binary);
    spec.args = vec![OsString::from("--resource-profile")];
    spec.env = environment.clone();
    spec.cwd = Some(cwd.to_owned());
    spec.output_cap = 32 << 10_u32;
    spec.timeout_ms = 10_000;
    spec
}

#[derive(Clone, Copy)]
enum HostProbe {
    MacMemory,
    MacCores,
    MacCpu,
    Kernel,
    Hostname,
    Page,
}

fn host_probe_spec(probe: HostProbe) -> ProcessSpec {
    let (program, arguments): (&str, &[&str]) = match probe {
        HostProbe::MacMemory => ("/usr/sbin/sysctl", &["-n", "hw.memsize"]),
        HostProbe::MacCores => ("/usr/sbin/sysctl", &["-n", "hw.ncpu"]),
        HostProbe::MacCpu => ("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]),
        HostProbe::Kernel => ("/usr/bin/uname", &["-r"]),
        HostProbe::Hostname => ("/usr/bin/uname", &["-n"]),
        HostProbe::Page => ("/usr/bin/getconf", &["PAGESIZE"]),
    };
    let mut spec = ProcessSpec::new(program);
    spec.args = arguments.iter().map(OsString::from).collect();
    spec.env = BTreeMap::from([(OsString::from("LC_ALL"), OsString::from("C"))]);
    spec.output_cap = 1024;
    spec.timeout_ms = 10_000;
    spec
}

async fn probe(probe: HostProbe, supervisor: &Supervisor) -> Result<Vec<u8>> {
    run_bytes(host_probe_spec(probe), supervisor, "identity_unsupported").await
}

fn os_text(bytes: &[u8]) -> Result<&str> {
    let refusal = HarnessError::new("identity_unsupported");
    if bytes.len() > 1024 {
        return Err(refusal);
    }
    let value = from_utf8(bytes).map_err(|_error| refusal)?.trim();
    if value.is_empty() || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(refusal);
    }
    Ok(value)
}

fn parse_kernel_version(bytes: &[u8]) -> Result<profile::Version> {
    let refusal = HarnessError::new("identity_unsupported");
    let text = os_text(bytes)?;
    let mut parts = text.splitn(3, '.');
    let major = version_component(parts.next().ok_or(refusal)?)?;
    let minor = version_component(parts.next().ok_or(refusal)?)?;
    let tail = parts.next().ok_or(refusal)?;
    let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
    let patch = version_component(tail.get(..digits).ok_or(refusal)?)?;
    Ok(profile::Version {
        major,
        minor,
        patch,
    })
}

fn version_component(value: &str) -> Result<u16> {
    let refusal = HarnessError::new("identity_unsupported");
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(refusal);
    }
    value.parse().map_err(|_error| refusal)
}

fn parse_cpu_model(model: &str) -> Result<profile::CpuKind> {
    let model_text = os_text(model.as_bytes())?;
    if model_text.contains("Apple M") {
        Ok(profile::CpuKind::AppleMSeries)
    } else if model_text.contains("EPYC") {
        Ok(profile::CpuKind::AmdEpyc)
    } else if model_text.contains("Xeon") {
        Ok(profile::CpuKind::IntelXeon)
    } else {
        let lowercase = model_text.to_ascii_lowercase();
        if ["virtual", "qemu", "common kvm"]
            .into_iter()
            .any(|name| lowercase.contains(name))
        {
            Ok(profile::CpuKind::VirtualCpu)
        } else {
            Err(HarnessError::new("identity_unsupported"))
        }
    }
}

fn parse_linux_cpu(bytes: &[u8]) -> Result<profile::CpuKind> {
    let refusal = HarnessError::new("identity_unsupported");
    if bytes.len() > PROC_CPU_CAP {
        return Err(refusal);
    }
    let text = from_utf8(bytes).map_err(|_error| refusal)?;
    let has_model_name = text.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(key, _value)| key.trim() == "model name")
    });
    let field = if has_model_name {
        "model name"
    } else {
        "Hardware"
    };
    let mut model = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() != field {
            continue;
        }
        let cpu = parse_cpu_model(value)?;
        if model.is_some_and(|previous| previous != cpu) {
            return Err(refusal);
        }
        model = Some(cpu);
    }
    model.ok_or(refusal)
}

fn parse_linux_memory(bytes: &[u8]) -> Result<u64> {
    let refusal = HarnessError::new("identity_unsupported");
    if bytes.len() > PROC_MEMORY_CAP {
        return Err(refusal);
    }
    let text = from_utf8(bytes).map_err(|_error| refusal)?;
    let mut memory = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() != "MemTotal" {
            continue;
        }
        if memory.is_some() {
            return Err(refusal);
        }
        let mut fields = value.split_ascii_whitespace();
        let kibibytes = parse_unsigned(fields.next().ok_or(refusal)?.as_bytes())?;
        if fields.next() != Some("kB") || fields.next().is_some() {
            return Err(refusal);
        }
        memory = Some(
            kibibytes
                .checked_mul(1024)
                .filter(|total_bytes| *total_bytes > 0)
                .ok_or(refusal)?,
        );
    }
    memory.ok_or(refusal)
}

fn parse_unsigned(bytes: &[u8]) -> Result<u64> {
    let refusal = HarnessError::new("identity_unsupported");
    let value = os_text(bytes)?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(refusal);
    }
    value.parse().map_err(|_error| refusal)
}

fn parse_page_bytes(bytes: &[u8]) -> Result<u32> {
    let refusal = HarnessError::new("identity_unsupported");
    let page = u32::try_from(parse_unsigned(bytes)?).map_err(|_error| refusal)?;
    if !(1024..=0x0001_0000).contains(&page) || !page.is_power_of_two() {
        return Err(refusal);
    }
    Ok(page)
}

#[derive(Clone, Copy)]
struct HardwareReadings<'a> {
    os: &'a str,
    architecture: &'a str,
    cpu: profile::CpuKind,
    logical_cores: usize,
    memory_bytes: u64,
    page_bytes: &'a [u8],
    kernel_release: &'a [u8],
    hostname: &'a [u8],
}

fn hardware_from_readings(
    readings: HardwareReadings<'_>,
    platform: &profile::Platform,
) -> Result<Hardware> {
    let refusal = HarnessError::new("identity_unsupported");
    let (os, architecture) = native_platform(readings.os, readings.architecture)?;
    if readings.cpu == profile::CpuKind::ProviderManaged || readings.logical_cores == 0 {
        return Err(refusal);
    }
    let logical_cores = u16::try_from(readings.logical_cores).map_err(|_error| refusal)?;
    if readings.memory_bytes < platform.memory_bytes {
        return Err(HarnessError::new("hardware_budget"));
    }
    if readings.memory_bytes == 0 {
        return Err(refusal);
    }
    let page_bytes = parse_page_bytes(readings.page_bytes)?;
    let os_version = parse_kernel_version(readings.kernel_release)?;
    let host_sha256 = Digest::of(os_text(readings.hostname)?.as_bytes());
    let meter = profile::Meter::ProcessRss;
    let machine_sha256 = Digest::canonical(&json!({
        "architecture": architecture,
        "cpu": readings.cpu,
        "logical_cores": logical_cores,
        "memory_bytes": readings.memory_bytes,
        "page_bytes": page_bytes,
        "os": os,
        "os_version": os_version,
        "meter": meter,
        "meter_version": os_version,
        "host_sha256": host_sha256,
    }))
    .map_err(|_error| refusal)?;
    Ok(Hardware {
        architecture,
        cpu: readings.cpu,
        logical_cores,
        memory_bytes: readings.memory_bytes,
        page_bytes,
        os,
        os_version,
        meter,
        meter_version: os_version,
        machine_sha256,
    })
}

fn native_platform(
    os_name: &str,
    architecture_name: &str,
) -> Result<(profile::Os, profile::Architecture)> {
    let refusal = HarnessError::new("identity_unsupported");
    let os = match os_name {
        "linux" => profile::Os::Linux,
        "macos" => profile::Os::Macos,
        _ => return Err(refusal),
    };
    let architecture = match architecture_name {
        "aarch64" => profile::Architecture::Aarch64,
        "x86_64" => profile::Architecture::X86_64,
        _ => return Err(refusal),
    };
    Ok((os, architecture))
}

const fn usable_port(port: u16) -> bool {
    port >= 1024 && port != 8787
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::fs;
    use tempfile::TempDir;

    const CANDIDATE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/testing/fixtures/resource-qualification/native-axum.json"
    ));
    const CATALOG: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/testing/fixtures/resource-qualification/catalog.json"
    ));

    fn candidate() -> Candidate {
        serde_json::from_slice(CANDIDATE).unwrap()
    }

    fn catalog() -> Catalog {
        serde_json::from_slice(CATALOG).unwrap()
    }

    fn metadata() -> Value {
        json!({
            "target_directory": "/private/tmp/native-custom-target",
            "packages": [
                {"name": "auctioneer-cli", "version": "9.8.7", "targets": []},
                {"name": "auctioneer-adapter-axum", "version": "0.7.12", "targets": []}
            ],
            "workspace_members": [],
            "version": 1_u16,
            "compiler_added_field": {"ignored": [1_u16, 2_u16]}
        })
    }

    fn metadata_bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).unwrap()
    }

    const BLOB_OID: &str = "1111111111111111111111111111111111111111";

    fn tree_record(mode: &str, kind: &str, oid: &str, path: &str) -> Vec<u8> {
        format!("{mode} {kind} {oid}\t{path}\0").into_bytes()
    }

    #[test]
    fn committed_tree_parser_retains_blob_modes_oids_and_unquoted_paths() {
        let path = "crates/sample/src/a file\twith\nwhitespace.rs";
        let bytes = [
            tree_record("100644", "blob", BLOB_OID, "Cargo.toml"),
            tree_record("100755", "blob", BLOB_OID, path),
        ]
        .concat();
        let tree = parse_source_tree(&bytes).unwrap();
        assert_eq!(tree.len(), 2);
        assert_eq!(tree["Cargo.toml"].mode, BlobMode::Regular);
        assert_eq!(tree[path].mode, BlobMode::Executable);
        assert_eq!(tree[path].oid, BLOB_OID);
        let sha256 = "a".repeat(64);
        let sha256_record = tree_record("100644", "blob", &sha256, "Cargo.lock");
        assert_eq!(
            parse_source_tree(&sha256_record).unwrap()["Cargo.lock"].oid,
            sha256
        );
    }

    #[test]
    fn committed_tree_parser_refuses_nonregular_modes_types_and_invalid_oids() {
        for mode in [
            "120000",
            "160000",
            "040000",
            "100600",
            "100664",
            "100644 extra",
        ] {
            assert_eq!(
                parse_source_tree(&tree_record(mode, "blob", BLOB_OID, "Cargo.toml"))
                    .unwrap_err()
                    .code,
                "source_dirty"
            );
        }
        for kind in ["tree", "commit", "tag", "SECRET_KIND_CANARY"] {
            assert_eq!(
                parse_source_tree(&tree_record("100644", kind, BLOB_OID, "Cargo.toml"))
                    .unwrap_err()
                    .code,
                "source_dirty"
            );
        }
        for oid in [
            "a".repeat(39),
            "a".repeat(41),
            "a".repeat(63),
            "a".repeat(65),
            "A".repeat(40),
            "g".repeat(40),
            String::new(),
        ] {
            assert_eq!(
                parse_source_tree(&tree_record("100644", "blob", &oid, "Cargo.toml")).unwrap_err(),
                HarnessError::new("source_dirty")
            );
        }
    }

    #[test]
    fn committed_tree_parser_refuses_duplicate_escaping_or_malformed_records() {
        for path in [
            "",
            "/Cargo.toml",
            "../Cargo.toml",
            "crates/../escape",
            "crates/./file",
            "crates//file",
            "crates/",
            "docs/unowned.rs",
        ] {
            assert_eq!(
                parse_source_tree(&tree_record("100644", "blob", BLOB_OID, path))
                    .unwrap_err()
                    .code,
                "source_dirty"
            );
        }
        let record = tree_record("100644", "blob", BLOB_OID, "Cargo.toml");
        assert_eq!(
            parse_source_tree(&[record.clone(), record.clone()].concat())
                .unwrap_err()
                .code,
            "source_dirty"
        );
        let mut unterminated = record;
        unterminated.pop();
        assert_eq!(
            parse_source_tree(&unterminated).unwrap_err().code,
            "source_dirty"
        );
        for bytes in [
            b"\0".as_slice(),
            b"100644 blob\tCargo.toml\0".as_slice(),
            b"\xff\0".as_slice(),
        ] {
            assert_eq!(parse_source_tree(bytes).unwrap_err().code, "source_dirty");
        }
        let oversized = tree_record(
            "100644",
            "blob",
            BLOB_OID,
            &format!("crates/{}", "x".repeat(512 << 10)),
        );
        assert_eq!(
            parse_source_tree(&oversized).unwrap_err().code,
            "source_dirty"
        );
    }

    #[test]
    fn raw_hash_output_requires_exact_bounded_counts_and_lowercase_full_oids() {
        let one = format!("{BLOB_OID}\n");
        assert_eq!(parse_raw_hashes(one.as_bytes(), 1).unwrap(), vec![BLOB_OID]);
        let sha256 = format!("{}\n", "b".repeat(64));
        assert_eq!(
            parse_raw_hashes(sha256.as_bytes(), 1).unwrap(),
            vec!["b".repeat(64)]
        );
        assert_eq!(parse_raw_hashes(&[], 0).unwrap().len(), 0);
        for (bytes, count) in [
            (one.as_bytes().to_vec(), 2_usize),
            (one.repeat(2).into_bytes(), 1),
            (one.as_bytes().to_vec(), 0),
            (BLOB_OID.as_bytes().to_vec(), 1),
            (format!(" {BLOB_OID}\n").into_bytes(), 1),
            (format!("{BLOB_OID}\r\n").into_bytes(), 1),
            (format!("{}\n", "A".repeat(40)).into_bytes(), 1),
            (format!("{}\n", "a".repeat(39)).into_bytes(), 1),
            (b"SECRET_HASH_OUTPUT_CANARY\n".to_vec(), 1),
        ] {
            assert_eq!(
                parse_raw_hashes(&bytes, count).unwrap_err(),
                HarnessError::new("source_dirty")
            );
        }
        let exact = one.repeat(128);
        assert_eq!(parse_raw_hashes(exact.as_bytes(), 128).unwrap().len(), 128);
        let excessive = one.repeat(129);
        assert!(excessive.len() < 8320);
        assert_eq!(
            parse_raw_hashes(excessive.as_bytes(), 129)
                .unwrap_err()
                .code,
            "source_dirty"
        );
        let oversized = vec![b'a'; 8321];
        assert_eq!(
            parse_raw_hashes(&oversized, 1).unwrap_err().code,
            "source_dirty"
        );
    }

    #[test]
    fn raw_hash_batches_use_no_filters_and_at_most_128_paths_per_command() {
        let root = Path::new("/private/tmp/source repository");
        let names: Vec<_> = (0_u16..129)
            .map(|index| format!("crates/sample/src/file-{index}.rs"))
            .collect();
        let paths: Vec<_> = names.iter().map(String::as_str).collect();
        let batches = raw_hash_batches(root, &paths).unwrap();
        assert_eq!(
            batches.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![128, 1]
        );
        assert_eq!(batches.concat(), paths);
        let spec = raw_hash_spec(root, &batches[1]).unwrap();
        assert_eq!(spec.program, Path::new("/usr/bin/git"));
        assert_eq!(
            spec.args,
            [
                "--no-replace-objects",
                "-C",
                "/private/tmp/source repository",
                "hash-object",
                "--no-filters",
                "--",
                "crates/sample/src/file-128.rs"
            ]
            .map(OsString::from)
        );
        assert!(!spec.stdin);
        assert_eq!(spec.timeout_ms, 10_000);
        assert_eq!(spec.output_cap, 8320);
        assert_eq!(
            raw_hash_spec(root, &paths).err().unwrap().code,
            "source_dirty"
        );
    }

    #[test]
    fn raw_hash_batches_enforce_the_full_argv_byte_budget_including_root() {
        let root = Path::new("/private/tmp/source repository");
        let names: Vec<_> = (0_u8..3)
            .map(|index| format!("crates/{index}{}", "x".repeat(8300)))
            .collect();
        let paths: Vec<_> = names.iter().map(String::as_str).collect();
        let batches = raw_hash_batches(root, &paths).unwrap();
        assert_eq!(
            batches.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![1, 1, 1]
        );
        assert_eq!(batches.concat(), paths);
        for batch in &batches {
            let spec = raw_hash_spec(root, batch).unwrap();
            let bytes = spec.program.as_os_str().as_encoded_bytes().len()
                + 1
                + spec
                    .args
                    .iter()
                    .map(|argument| argument.as_encoded_bytes().len() + 1)
                    .sum::<usize>();
            assert!(bytes <= 0x4000);
        }
        let oversized = format!("crates/{}", "x".repeat(0x4000));
        assert_eq!(
            raw_hash_batches(root, &[oversized.as_str()])
                .unwrap_err()
                .code,
            "source_dirty"
        );
        let huge_root = PathBuf::from(format!("/{}", "r".repeat(0x4000)));
        assert_eq!(
            raw_hash_batches(&huge_root, &["Cargo.toml"])
                .unwrap_err()
                .code,
            "source_dirty"
        );
    }

    #[cfg(unix)]
    #[test]
    fn committed_modes_are_compared_to_filesystem_execute_bits_without_git_configuration() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let path = write(root.path(), "crates/sample/src/lib.rs", b"same bytes");
        let regular_record = tree_record("100644", "blob", BLOB_OID, "crates/sample/src/lib.rs");
        let regular = parse_source_tree(&regular_record).unwrap();
        let executable_record = tree_record("100755", "blob", BLOB_OID, "crates/sample/src/lib.rs");
        let executable = parse_source_tree(&executable_record).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_source_modes(root.path(), &regular).unwrap();
        assert_eq!(
            assert_source_modes(root.path(), &executable)
                .unwrap_err()
                .code,
            "source_dirty"
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert_source_modes(root.path(), &executable).unwrap();
        assert_eq!(
            assert_source_modes(root.path(), &regular).unwrap_err().code,
            "source_dirty"
        );
    }

    #[test]
    fn repository_root_is_canonical_and_comes_from_the_manifest_not_cwd() {
        let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        assert_eq!(repository_root().unwrap(), expected);
    }

    #[test]
    fn metadata_selects_exact_adapter_version_and_custom_absolute_target_directory() {
        let artifacts = parse_metadata(&metadata_bytes(&metadata())).unwrap();
        assert_eq!(
            artifacts.axum,
            Path::new("/private/tmp/native-custom-target/release/auctioneer-adapter-axum")
        );
        assert_eq!(
            artifacts.cli,
            Path::new("/private/tmp/native-custom-target/release/auctioneer-cli")
        );
        assert_eq!(
            artifacts.runtime_version,
            profile::Version {
                major: 0,
                minor: 7,
                patch: 12
            }
        );
        let mut moved = metadata();
        moved["target_directory"] = json!("/private/tmp/a different target");
        assert_eq!(
            parse_metadata(&metadata_bytes(&moved)).unwrap().cli,
            Path::new("/private/tmp/a different target/release/auctioneer-cli")
        );
    }

    #[test]
    fn metadata_refuses_missing_duplicate_or_inexact_adapter_packages() {
        for packages in [
            json!([]),
            json!([{"name":"auctioneer-adapter-axum-spoof", "version":"0.7.12"}]),
            json!([
                {"name":"auctioneer-adapter-axum", "version":"0.7.12"},
                {"name":"auctioneer-adapter-axum", "version":"0.7.12"}
            ]),
        ] {
            let mut value = metadata();
            value["packages"] = packages;
            assert_eq!(
                parse_metadata(&metadata_bytes(&value)).unwrap_err(),
                HarnessError::new("metadata_failed")
            );
        }
    }

    #[test]
    fn metadata_retained_fields_are_typed_and_versions_are_bounded() {
        for target in [json!(null), json!(1_u16), json!("target"), json!("")] {
            let mut value = metadata();
            value["target_directory"] = target;
            assert_eq!(
                parse_metadata(&metadata_bytes(&value)).unwrap_err().code,
                "metadata_failed"
            );
        }
        for version in [
            json!(null),
            json!(7_u16),
            json!("0.7"),
            json!("65536.0.0"),
            json!("0.65536.0"),
            json!("0.0.65536"),
            json!("0.-1.0"),
            json!("SECRET_VERSION_PATH"),
        ] {
            let mut value = metadata();
            value["packages"][1]["version"] = version;
            assert_eq!(
                parse_metadata(&metadata_bytes(&value)).unwrap_err().code,
                "metadata_failed"
            );
        }
        for (field, replacement) in [("name", json!(false)), ("version", json!({}))] {
            let mut value = metadata();
            value["packages"][1][field] = replacement;
            assert_eq!(
                parse_metadata(&metadata_bytes(&value)).unwrap_err().code,
                "metadata_failed"
            );
        }
        for packages in [
            json!(null),
            json!({}),
            json!([{"name":"auctioneer-adapter-axum"}]),
        ] {
            let mut value = metadata();
            value["packages"] = packages;
            assert_eq!(
                parse_metadata(&metadata_bytes(&value)).unwrap_err().code,
                "metadata_failed"
            );
        }
        assert_eq!(parse_metadata(br#"{"target_directory":"/a","target_directory":"/b","packages":[{"name":"auctioneer-adapter-axum","version":"1.2.3"}]}"#).unwrap_err().code, "metadata_failed");
        assert_eq!(parse_metadata(br#"{"target_directory":"/a","packages":[{"name":"auctioneer-adapter-axum","version":"1.2.3","version":"4.5.6"}]}"#).unwrap_err().code, "metadata_failed");
    }

    #[test]
    fn metadata_caps_apply_to_bytes_and_discarded_unknown_fields() {
        let mut exact = serde_json::to_vec(&metadata()).unwrap();
        exact.resize(METADATA_CAP, b' ');
        assert_eq!(parse_metadata(&exact).unwrap().runtime_version.patch, 12);
        exact.push(b' ');
        assert_eq!(parse_metadata(&exact).unwrap_err().code, "metadata_failed");
        let mut nested = json!(0_u16);
        for _depth in 0_u16..150 {
            nested = Value::Array(vec![nested]);
        }
        let mut value = metadata();
        value["compiler_added_field"] = nested;
        assert_eq!(
            parse_metadata(&metadata_bytes(&value)).unwrap_err().code,
            "metadata_failed"
        );
        assert_eq!(
            parse_metadata(b"SECRET_METADATA_CANARY").unwrap_err(),
            HarnessError::new("metadata_failed")
        );
    }

    #[test]
    fn cargo_specs_use_only_the_approved_offline_commands_and_environment() {
        let root = Path::new("/private/tmp/qualification source");
        let metadata = cargo_metadata_spec(root);
        let build = cargo_build_spec(root);
        assert_eq!(metadata.program, Path::new(env!("CARGO")));
        assert_eq!(build.program, metadata.program);
        assert_eq!(
            metadata.args,
            [
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--locked",
                "--offline"
            ]
            .map(OsString::from)
        );
        assert_eq!(
            build.args,
            [
                "build",
                "--release",
                "--locked",
                "--offline",
                "-p",
                "auctioneer-adapter-axum",
                "-p",
                "auctioneer-cli",
                "--features",
                "auctioneer-adapter-axum/insecure-bidders"
            ]
            .map(OsString::from)
        );
        let environment = super::super::inputs::tool_environment();
        for spec in [metadata, build] {
            assert_eq!(spec.cwd.as_deref(), Some(root));
            assert_eq!(spec.env, environment);
            assert_eq!(spec.output_cap, 4 << 20_u32);
            assert_eq!(spec.timeout_ms, 600_000);
            assert!(!spec.stdin);
        }
    }

    fn write(root: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    fn source_tree() -> TempDir {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "Cargo.toml",
            b"[workspace]\nmembers=[\"crates/sample\"]\nresolver=\"2\"\n",
        );
        write(root.path(), "Cargo.lock", b"# fixed fixture lock\n");
        write(root.path(), "edgezero.toml", b"[app]\nname=\"sample\"\n");
        write(
            root.path(),
            "auctioneer.toml",
            b"[auction]\ndefault_tmax_ms=500\n",
        );
        write(root.path(), ".cargo/config.toml", b"[build]\njobs=1\n");
        write(
            root.path(),
            "crates/sample/Cargo.toml",
            b"[package]\nname=\"sample\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        );
        write(
            root.path(),
            "crates/sample/src/lib.rs",
            b"pub const VALUE: u8 = 1;\n",
        );
        write(root.path(), ".gitignore", b"**/target/\n**/ignored.rs\n");
        root
    }

    // Fixture setup is a fixed local Git process, independent of the spec under test.
    async fn local_git(supervisor: &Supervisor, root: &Path, arguments: &[&str]) -> Vec<u8> {
        let mut spec = ProcessSpec::new("/usr/bin/git");
        spec.args = vec![
            OsString::from("--no-replace-objects"),
            OsString::from("-C"),
            root.as_os_str().to_owned(),
        ];
        spec.args.extend(
            [
                "-c",
                "user.name=Qualification Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ]
            .map(OsString::from),
        );
        spec.args.extend(arguments.iter().map(OsString::from));
        spec.env = BTreeMap::from([
            (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
            (OsString::from("LC_ALL"), OsString::from("C")),
            (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
            (
                OsString::from("GIT_CONFIG_GLOBAL"),
                OsString::from("/dev/null"),
            ),
        ]);
        spec.output_cap = 512 << 10_u32;
        spec.timeout_ms = 10_000;
        supervisor.run(spec).await.unwrap().stdout
    }

    async fn commit_all(supervisor: &Supervisor, root: &Path) -> String {
        local_git(supervisor, root, &["add", "--all", "--", "."]).await;
        local_git(supervisor, root, &["commit", "--quiet", "-m", "fixture"]).await;
        String::from_utf8(
            local_git(
                supervisor,
                root,
                &["rev-parse", "--verify", "HEAD^{commit}"],
            )
            .await,
        )
        .unwrap()
        .trim()
        .to_owned()
    }

    async fn repository(supervisor: &Supervisor) -> (TempDir, String) {
        let root = source_tree();
        local_git(
            supervisor,
            root.path(),
            &["init", "--quiet", "--initial-branch=main"],
        )
        .await;
        let revision = commit_all(supervisor, root.path()).await;
        (root, revision)
    }

    #[test]
    fn git_spec_disables_replacements_and_ambient_configuration_with_bounded_output() {
        let root = Path::new("/private/tmp/source repository");
        let spec = git_spec(root, &["rev-parse", "--verify", "HEAD^{commit}"]);
        assert_eq!(spec.program, Path::new("/usr/bin/git"));
        assert_eq!(
            spec.args,
            [
                "--no-replace-objects",
                "-C",
                "/private/tmp/source repository",
                "rev-parse",
                "--verify",
                "HEAD^{commit}"
            ]
            .map(OsString::from)
        );
        assert_eq!(spec.env.len(), 4);
        assert!(spec.env.contains_key(&OsString::from("PATH")));
        for (key, value) in [
            ("LC_ALL", "C"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ] {
            assert_eq!(
                spec.env.get(&OsString::from(key)),
                Some(&OsString::from(value))
            );
        }
        assert_eq!(spec.output_cap, 512 << 10_u32);
        assert_eq!(spec.timeout_ms, 10_000);
        assert!(!spec.stdin);
    }

    #[tokio::test]
    async fn source_commit_resolves_head_and_allows_later_docs_and_profile_commits() {
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        let expected = Revision::try_from(initial.clone()).unwrap();
        assert_eq!(
            source_commit(root.path(), None, &supervisor).await.unwrap(),
            expected
        );
        write(
            root.path(),
            "docs/testing/profile.json",
            b"{\"prepared\":true}\n",
        );
        let later = commit_all(&supervisor, root.path()).await;
        assert_ne!(initial, later);
        write(root.path(), "docs/notes.md", b"uncommitted docs\n");
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            source_commit(root.path(), None, &supervisor).await.unwrap(),
            Revision::try_from(later).unwrap()
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn source_commit_refuses_each_tracked_build_input_change_or_deletion() {
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        for relative in [
            "Cargo.toml",
            "Cargo.lock",
            "edgezero.toml",
            "auctioneer.toml",
            ".cargo/config.toml",
            "crates/sample/src/lib.rs",
        ] {
            let path = root.path().join(relative);
            let original = fs::read(&path).unwrap();
            fs::write(&path, b"# CHANGED_BUILD_INPUT\n").unwrap();
            assert_eq!(
                source_commit(root.path(), Some(&initial), &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("source_dirty")
            );
            fs::write(&path, original).unwrap();
        }
        fs::remove_file(root.path().join("crates/sample/src/lib.rs")).unwrap();
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn source_commit_refuses_staged_and_later_committed_source_changes() {
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        write(
            root.path(),
            "crates/sample/src/lib.rs",
            b"pub const VALUE: u8 = 2;\n",
        );
        local_git(
            &supervisor,
            root.path(),
            &["add", "--", "crates/sample/src/lib.rs"],
        )
        .await;
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        let later = commit_all(&supervisor, root.path()).await;
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        assert_eq!(
            source_commit(root.path(), None, &supervisor).await.unwrap(),
            Revision::try_from(later).unwrap()
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn source_inventory_refuses_untracked_ignored_and_src_target_inputs_independently() {
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        for relative in [
            "crates/sample/src/untracked.rs",
            "crates/sample/src/ignored.rs",
            "crates/sample/src/target/module.rs",
            "crates/sample/build.rs",
            "crates/sample/build-input.bin",
            ".cargo/ignored.rs",
        ] {
            let path = write(root.path(), relative, b"UNTRACKED_BUILD_INPUT");
            assert_eq!(
                source_commit(root.path(), Some(&initial), &supervisor)
                    .await
                    .unwrap_err()
                    .code,
                "source_dirty"
            );
            fs::remove_file(path).unwrap();
        }
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap(),
            Revision::try_from(initial).unwrap()
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn source_commit_excludes_actual_cargo_owned_target_outputs_not_target_named_sources() {
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        for relative in [
            "target/release/output.o",
            "crates/sample/target/debug/output.o",
        ] {
            write(root.path(), relative, b"GENERATED_OUTPUT");
        }
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap(),
            Revision::try_from(initial.clone()).unwrap()
        );
        let path = write(
            root.path(),
            "crates/sample/src/target/ignored.rs",
            b"SOURCE_NOT_CARGO_OUTPUT",
        );
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        fs::remove_file(path).unwrap();
        write(
            root.path(),
            "crates/not-a-package/target/input.rs",
            b"SOURCE_NOT_CARGO_OUTPUT",
        );
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn source_revision_requires_lowercase_full_hex_and_a_real_commit() {
        let supervisor = Supervisor::new();
        let (root, _initial) = repository(&supervisor).await;
        for requested in [
            "HEAD",
            "abc123",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "0000000000000000000000000000000000000000",
            "SECRET_REVISION_CANARY",
        ] {
            assert_eq!(
                source_commit(root.path(), Some(requested), &supervisor)
                    .await
                    .unwrap_err(),
                HarnessError::new("identity_mismatch")
            );
        }
        let blob = String::from_utf8(
            local_git(
                &supervisor,
                root.path(),
                &["rev-parse", "HEAD:crates/sample/src/lib.rs"],
            )
            .await,
        )
        .unwrap();
        assert_eq!(
            source_commit(root.path(), Some(blob.trim()), &supervisor)
                .await
                .unwrap_err()
                .code,
            "identity_mismatch"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn replacement_refs_cannot_relabel_changed_sources_as_the_requested_commit() {
        let supervisor = Supervisor::new();
        let (root, original) = repository(&supervisor).await;
        write(
            root.path(),
            "crates/sample/src/lib.rs",
            b"pub const VALUE: u8 = 2;\n",
        );
        let replacement = commit_all(&supervisor, root.path()).await;
        local_git(
            &supervisor,
            root.path(),
            &["replace", &original, &replacement],
        )
        .await;
        assert_eq!(
            source_commit(root.path(), Some(&original), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        write(
            root.path(),
            "crates/sample/src/lib.rs",
            b"pub const VALUE: u8 = 1;\n",
        );
        assert_eq!(
            source_commit(root.path(), Some(&original), &supervisor)
                .await
                .unwrap(),
            Revision::try_from(original).unwrap()
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn clean_filters_cannot_mask_changed_compiler_inputs() {
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        write(
            root.path(),
            ".git/info/attributes",
            b"crates/sample/src/lib.rs filter=qualification\n",
        );
        local_git(
            &supervisor,
            root.path(),
            &[
                "config",
                "filter.qualification.clean",
                "sed 's/VALUE: u8 = 2/VALUE: u8 = 1/'",
            ],
        )
        .await;
        write(
            root.path(),
            "crates/sample/src/lib.rs",
            b"pub const VALUE: u8 = 2;\n",
        );
        let filtered = local_git(
            &supervisor,
            root.path(),
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--name-only",
                &initial,
                "--",
                "crates/sample/src/lib.rs",
            ],
        )
        .await;
        assert!(
            filtered.is_empty(),
            "the adversarial filter must conceal the change from Git diff"
        );
        assert_eq!(
            fs::read(root.path().join("crates/sample/src/lib.rs")).unwrap(),
            b"pub const VALUE: u8 = 2;\n"
        );
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn source_commit_refuses_tracked_executable_mode_changes() {
        use std::os::unix::fs::PermissionsExt as _;
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        local_git(
            &supervisor,
            root.path(),
            &[
                "-c",
                "core.filemode=true",
                "update-index",
                "--chmod=-x",
                "crates/sample/src/lib.rs",
            ],
        )
        .await;
        let path = root.path().join("crates/sample/src/lib.rs");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_filemode_settings_cannot_hide_compiler_input_mode_changes() {
        use std::os::unix::fs::PermissionsExt as _;
        let supervisor = Supervisor::new();
        let (root, initial) = repository(&supervisor).await;
        local_git(
            &supervisor,
            root.path(),
            &["config", "core.filemode", "false"],
        )
        .await;
        fs::set_permissions(
            root.path().join("crates/sample/src/lib.rs"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(
            local_git(
                &supervisor,
                root.path(),
                &[
                    "diff",
                    "--name-only",
                    &initial,
                    "--",
                    "crates/sample/src/lib.rs"
                ]
            )
            .await
            .is_empty()
        );
        assert_eq!(
            source_commit(root.path(), Some(&initial), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn group_execution_cannot_stand_in_for_a_committed_owner_execute_bit() {
        use std::os::unix::fs::PermissionsExt as _;
        let supervisor = Supervisor::new();
        let (root, _initial) = repository(&supervisor).await;
        let path = root.path().join("crates/sample/src/lib.rs");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let executable = commit_all(&supervisor, root.path()).await;
        let tree = local_git(
            &supervisor,
            root.path(),
            &["ls-tree", &executable, "--", "crates/sample/src/lib.rs"],
        )
        .await;
        assert!(tree.starts_with(b"100755 blob "));
        local_git(
            &supervisor,
            root.path(),
            &["config", "core.filemode", "false"],
        )
        .await;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o655)).unwrap();
        assert!(
            local_git(
                &supervisor,
                root.path(),
                &[
                    "diff",
                    "--name-only",
                    &executable,
                    "--",
                    "crates/sample/src/lib.rs"
                ]
            )
            .await
            .is_empty()
        );
        assert_eq!(
            source_commit(root.path(), Some(&executable), &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_dirty"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn cpu_models_map_only_to_the_approved_closed_identities() {
        for (model, expected) in [
            ("Apple M4 Pro", profile::CpuKind::AppleMSeries),
            ("AMD EPYC 7B13", profile::CpuKind::AmdEpyc),
            (
                "Intel(R) Xeon(R) Platinum 8375C CPU",
                profile::CpuKind::IntelXeon,
            ),
            ("Virtual CPU", profile::CpuKind::VirtualCpu),
            (
                "QEMU Virtual CPU version 2.5+",
                profile::CpuKind::VirtualCpu,
            ),
            ("Common KVM processor", profile::CpuKind::VirtualCpu),
            ("common kvm processor", profile::CpuKind::VirtualCpu),
        ] {
            assert_eq!(parse_cpu_model(model).unwrap(), expected);
        }
        for model in [
            "",
            "Intel Core i9",
            "AMD Ryzen 9",
            "Apple Intel",
            "SECRET_MODEL_CANARY",
        ] {
            assert_eq!(
                parse_cpu_model(model).unwrap_err(),
                HarnessError::new("identity_unsupported")
            );
        }
        assert_eq!(
            parse_cpu_model(&format!("EPYC{}", " ".repeat(1024)))
                .unwrap_err()
                .code,
            "identity_unsupported"
        );
    }

    #[test]
    fn kernel_versions_accept_leading_three_u16_components_with_os_suffixes() {
        for (input, expected) in [
            (
                b"6.8.12-aws\n".as_slice(),
                profile::Version {
                    major: 6,
                    minor: 8,
                    patch: 12,
                },
            ),
            (
                b"23.6.0\n".as_slice(),
                profile::Version {
                    major: 23,
                    minor: 6,
                    patch: 0,
                },
            ),
            (
                b"65535.0.65535-custom\n".as_slice(),
                profile::Version {
                    major: 0xffff,
                    minor: 0,
                    patch: 0xffff,
                },
            ),
        ] {
            assert_eq!(parse_kernel_version(input).unwrap(), expected);
        }
        for input in [
            "6.8",
            "6..1",
            "65536.0.1",
            "6.65536.1",
            "6.1.65536",
            "release 6.8.12",
            "-1.2.3",
            "SECRET_KERNEL_CANARY",
            "",
        ] {
            assert_eq!(
                parse_kernel_version(input.as_bytes()).unwrap_err().code,
                "identity_unsupported"
            );
        }
    }

    #[test]
    fn linux_memory_uses_only_memtotal_in_kibibytes_with_checked_arithmetic() {
        assert_eq!(
            parse_linux_memory(
                b"MemAvailable: 1024 kB\nMemTotal:       8388608 kB\nSwapTotal: 0 kB\n"
            )
            .unwrap(),
            0x0002_0000_0000
        );
        for input in [
            "MemAvailable: 8388608 kB\n",
            "MemTotal: 0 kB\n",
            "MemTotal: -1 kB\n",
            "MemTotal: 8388608 MB\n",
            "MemTotal: 8388608 kB trailing\n",
            "MemTotal: 18446744073709551615 kB\n",
            "MemTotal: 8 kB\nMemTotal: 16 kB\n",
            "SECRET_MEMORY_CANARY",
        ] {
            assert_eq!(
                parse_linux_memory(input.as_bytes()).unwrap_err().code,
                "identity_unsupported"
            );
        }
        let mut oversized = b"MemTotal: 8388608 kB\n".to_vec();
        oversized.resize(PROC_MEMORY_CAP.checked_add(1).unwrap(), b' ');
        assert_eq!(
            parse_linux_memory(&oversized).unwrap_err().code,
            "identity_unsupported"
        );
    }

    #[test]
    fn linux_cpu_parsing_uses_structured_model_fields_not_arbitrary_mentions() {
        assert_eq!(parse_linux_cpu(b"processor : 0\nmodel name : AMD EPYC 7B13\n\nprocessor : 1\nmodel name : AMD EPYC 7B13\n").unwrap(), profile::CpuKind::AmdEpyc);
        assert_eq!(
            parse_linux_cpu(b"processor : 0\nmodel name : QEMU Virtual CPU\n").unwrap(),
            profile::CpuKind::VirtualCpu
        );
        for input in [
            "vendor_id : EPYC\nmodel name : Intel Core i9\n",
            "vendor_id : Xeon\n",
            "model name : AMD EPYC 7B13\nmodel name : Intel Core i9\n",
            "SECRET_CPU_CANARY",
        ] {
            assert_eq!(
                parse_linux_cpu(input.as_bytes()).unwrap_err().code,
                "identity_unsupported"
            );
        }
        assert_eq!(
            parse_linux_cpu(&[0xff_u8]).unwrap_err().code,
            "identity_unsupported"
        );
        let mut oversized = b"model name : AMD EPYC 7B13\n".to_vec();
        oversized.resize(PROC_CPU_CAP.checked_add(1).unwrap(), b' ');
        assert_eq!(
            parse_linux_cpu(&oversized).unwrap_err().code,
            "identity_unsupported"
        );
    }

    #[test]
    fn scalar_os_readings_and_page_sizes_are_typed_bounded_and_power_of_two() {
        assert_eq!(parse_unsigned(b"8589934592\n").unwrap(), 0x0002_0000_0000);
        assert_eq!(parse_unsigned(b"8\n").unwrap(), 8);
        for input in [
            "-1",
            "+8",
            "8 16",
            "8\n16",
            "18446744073709551616",
            "SECRET_SCALAR_CANARY",
        ] {
            assert_eq!(
                parse_unsigned(input.as_bytes()).unwrap_err().code,
                "identity_unsupported"
            );
        }
        for page in [1024_u32, 4096, 0x4000, 0x0001_0000] {
            assert_eq!(parse_page_bytes(page.to_string().as_bytes()).unwrap(), page);
        }
        for page in [
            "0",
            "512",
            "1025",
            "6144",
            "65537",
            "131072",
            "4294967296",
            "SECRET_PAGE_CANARY",
        ] {
            assert_eq!(
                parse_page_bytes(page.as_bytes()).unwrap_err().code,
                "identity_unsupported"
            );
        }
    }

    fn readings() -> HardwareReadings<'static> {
        HardwareReadings {
            os: "linux",
            architecture: "x86_64",
            cpu: profile::CpuKind::IntelXeon,
            logical_cores: 8,
            memory_bytes: 0x0002_0000_0000,
            page_bytes: b"4096\n",
            kernel_release: b"6.8.12-aws\n",
            hostname: b"SECRET_HOSTNAME_CANARY\n",
        }
    }

    #[test]
    fn hardware_retains_only_enums_numbers_and_a_machine_digest() {
        let platform = candidate().platform;
        let hardware = hardware_from_readings(readings(), &platform).unwrap();
        assert_eq!(hardware.os, profile::Os::Linux);
        assert_eq!(hardware.architecture, profile::Architecture::X86_64);
        assert_eq!(hardware.cpu, profile::CpuKind::IntelXeon);
        assert_eq!(hardware.logical_cores, 8);
        assert_eq!(hardware.memory_bytes, 0x0002_0000_0000);
        assert_eq!(hardware.page_bytes, 4096);
        assert_eq!(
            hardware.os_version,
            profile::Version {
                major: 6,
                minor: 8,
                patch: 12
            }
        );
        assert_eq!(hardware.meter, profile::Meter::ProcessRss);
        assert_eq!(
            hardware.meter_version,
            profile::Version {
                major: 6,
                minor: 8,
                patch: 12
            }
        );
        assert!(
            !serde_json::to_string(&hardware)
                .unwrap()
                .contains("SECRET_HOSTNAME_CANARY")
        );
        let mut changed_host = readings();
        changed_host.hostname = b"ANOTHER_PRIVATE_HOST\n";
        assert_ne!(
            hardware_from_readings(changed_host, &platform)
                .unwrap()
                .machine_sha256,
            hardware.machine_sha256
        );
        let mut changed_memory = readings();
        changed_memory.memory_bytes = 0x0004_0000_0000;
        assert_ne!(
            hardware_from_readings(changed_memory, &platform)
                .unwrap()
                .machine_sha256,
            hardware.machine_sha256
        );
        let mut mac_readings = readings();
        mac_readings.os = "macos";
        mac_readings.architecture = "aarch64";
        mac_readings.cpu = profile::CpuKind::AppleMSeries;
        let mac = hardware_from_readings(mac_readings, &platform).unwrap();
        assert_eq!(mac.os, profile::Os::Macos);
        assert_eq!(mac.architecture, profile::Architecture::Aarch64);
        assert_eq!(mac.cpu, profile::CpuKind::AppleMSeries);
    }

    #[test]
    fn hardware_budget_refuses_short_physical_memory_but_accepts_the_exact_budget() {
        let platform = candidate().platform;
        let mut short = readings();
        short.memory_bytes = platform.memory_bytes.checked_sub(1).unwrap();
        assert_eq!(
            hardware_from_readings(short, &platform).unwrap_err(),
            HarnessError::new("hardware_budget")
        );
        let mut exact = readings();
        exact.memory_bytes = platform.memory_bytes;
        assert_eq!(
            hardware_from_readings(exact, &platform)
                .unwrap()
                .memory_bytes,
            0x0001_0000_0000
        );
    }

    #[test]
    fn hardware_refuses_unsupported_os_architecture_and_out_of_range_core_counts() {
        let platform = candidate().platform;
        for os in ["windows", "freebsd", "SECRET_OS_CANARY"] {
            let mut value = readings();
            value.os = os;
            assert_eq!(
                hardware_from_readings(value, &platform).unwrap_err().code,
                "identity_unsupported"
            );
        }
        for architecture in ["wasm32", "riscv64", "SECRET_ARCH_CANARY"] {
            let mut value = readings();
            value.architecture = architecture;
            assert_eq!(
                hardware_from_readings(value, &platform).unwrap_err().code,
                "identity_unsupported"
            );
        }
        let mut provider_cpu = readings();
        provider_cpu.cpu = profile::CpuKind::ProviderManaged;
        assert_eq!(
            hardware_from_readings(provider_cpu, &platform)
                .unwrap_err()
                .code,
            "identity_unsupported"
        );
        for cores in [0_usize, 0x0001_0000] {
            let mut value = readings();
            value.logical_cores = cores;
            assert_eq!(
                hardware_from_readings(value, &platform).unwrap_err().code,
                "identity_unsupported"
            );
        }
        for cores in [1_usize, 0xffff] {
            let mut value = readings();
            value.logical_cores = cores;
            assert_eq!(
                usize::from(
                    hardware_from_readings(value, &platform)
                        .unwrap()
                        .logical_cores
                ),
                cores
            );
        }
    }

    #[test]
    fn hardware_probes_use_fixed_os_commands_and_small_output_limits() {
        for (probe, program, args) in [
            (
                HostProbe::MacMemory,
                "/usr/sbin/sysctl",
                vec!["-n", "hw.memsize"],
            ),
            (
                HostProbe::MacCores,
                "/usr/sbin/sysctl",
                vec!["-n", "hw.ncpu"],
            ),
            (
                HostProbe::MacCpu,
                "/usr/sbin/sysctl",
                vec!["-n", "machdep.cpu.brand_string"],
            ),
            (HostProbe::Kernel, "/usr/bin/uname", vec!["-r"]),
            (HostProbe::Hostname, "/usr/bin/uname", vec!["-n"]),
            (HostProbe::Page, "/usr/bin/getconf", vec!["PAGESIZE"]),
        ] {
            let spec = host_probe_spec(probe);
            assert_eq!(spec.program, Path::new(program));
            assert_eq!(
                spec.args,
                args.into_iter().map(OsString::from).collect::<Vec<_>>()
            );
            assert_eq!(
                spec.env,
                BTreeMap::from([(OsString::from("LC_ALL"), OsString::from("C"))])
            );
            assert_eq!(spec.timeout_ms, 10_000);
            assert_eq!(spec.output_cap, 1024);
            assert!(!spec.stdin);
        }
    }

    fn exported() -> Export {
        let profile = profile::tests::synthetic_profile(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/testing/fixtures/resource-qualification/native-axum-local-82075f1.json"
        )));
        serde_json::from_value(json!({
            "schema_version": 2_u16,
            "target": profile.identity.target,
            "backend": profile.backend,
            "edgezero_pin": profile.identity.edgezero_pin,
            "build_features": profile.identity.build_features,
            "baked_host_sha256": Digest::of(b"export-host"),
            "baked_account_sha256": Digest::of(b"export-account"),
            "baked_config_sha256": profile.identity.baked_config_sha256,
            "reference_data_sha256": profile.identity.reference_data_sha256,
            "effective_profile_sha256": profile.resources.digest().unwrap(),
            "resources": profile.resources,
            "derived": inputs::Derived::from(profile.derive().unwrap()),
        }))
        .unwrap()
    }

    #[test]
    fn export_inspection_uses_the_frozen_binary_and_exact_environment_cwd_and_cap() {
        let binary = Path::new("/private/tmp/frozen adapter");
        let cwd = Path::new("/private/tmp/isolated snapshot");
        let environment = BTreeMap::from([(OsString::from("PORT"), OsString::from("18080"))]);
        let spec = export_spec(binary, &environment, cwd);
        assert_eq!(spec.program, binary);
        assert_eq!(spec.args, [OsString::from("--resource-profile")]);
        assert_eq!(spec.cwd.as_deref(), Some(cwd));
        assert_eq!(spec.env, environment);
        assert_eq!(spec.output_cap, 32 << 10_u32);
        assert_eq!(spec.timeout_ms, 10_000);
        assert!(!spec.stdin);
    }

    #[test]
    fn same_export_is_structural_and_binds_both_wrapper_and_resource_values() {
        let expected = exported();
        let encoded = serde_json::to_string_pretty(&expected).unwrap();
        let actual: Export = serde_json::from_str(&encoded).unwrap();
        assert_same_export(&expected, &actual).unwrap();
        let mut wrapper = expected.clone();
        wrapper.baked_account_sha256 = Digest::of(b"different-account");
        assert_eq!(
            assert_same_export(&expected, &wrapper).unwrap_err(),
            HarnessError::new("candidate_mismatch")
        );
        let mut resources = expected.clone();
        resources.resources.baked_config_bytes = resources
            .resources
            .baked_config_bytes
            .checked_add(1)
            .unwrap();
        assert_eq!(
            assert_same_export(&expected, &resources).unwrap_err().code,
            "candidate_mismatch"
        );
        let mut derived = expected.clone();
        derived.derived.total_bytes = 2;
        assert_eq!(
            assert_same_export(&expected, &derived).unwrap_err().code,
            "candidate_mismatch"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn export_inspection_refuses_successful_children_without_a_typed_export() {
        let supervisor = Supervisor::new();
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            inspect_export(
                Path::new("/usr/bin/true"),
                &BTreeMap::new(),
                root.path(),
                &supervisor
            )
            .await
            .unwrap_err(),
            HarnessError::new("invalid_child_output")
        );
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn reserved_ports_exclude_privileged_and_default_service_ports() {
        for port in [0_u16, 1, 1023, 8787] {
            assert!(!usable_port(port));
        }
        for port in [1024_u16, 8786, 8788, 0xffff] {
            assert!(usable_port(port));
        }
    }

    #[tokio::test]
    async fn reserve_port_releases_the_loopback_listener_before_returning() {
        let port = reserve_port().await.unwrap();
        assert!(port >= 1024);
        assert_ne!(port, 8787);
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }

    #[test]
    fn harness_digest_binds_harness_sources_and_executable_not_absolute_roots() {
        let first = source_tree();
        let second = source_tree();
        let relative = format!("{HARNESS_SOURCE}native/system.rs");
        for root in [&first, &second] {
            write(root.path(), &relative, b"// harness alpha\n");
            write(root.path(), "frozen-cli", b"abc");
        }
        let executable = first.path().join("frozen-cli");
        let baseline = harness_digest(first.path(), &executable).unwrap();
        // Independent golden for {harness_sources, executable_sha256, fixture_protocol_version: 1}.
        assert_eq!(
            baseline,
            Digest::try_from(
                "9560ce5f3543f28ad9e37d447730e863e07ab493e256b144f26aee993ad7b2a4".to_owned()
            )
            .unwrap()
        );
        assert_ne!(baseline, Digest::of(b"abc"));
        assert_eq!(
            harness_digest(second.path(), &second.path().join("frozen-cli")).unwrap(),
            baseline
        );
        write(
            first.path(),
            "crates/sample/src/lib.rs",
            b"// not harness source\n",
        );
        assert_eq!(harness_digest(first.path(), &executable).unwrap(), baseline);
        write(first.path(), &relative, b"// harness beta\n");
        assert_ne!(harness_digest(first.path(), &executable).unwrap(), baseline);
        write(first.path(), &relative, b"// harness alpha\n");
        write(first.path(), "frozen-cli", b"abcd");
        assert_ne!(harness_digest(first.path(), &executable).unwrap(), baseline);
        write(first.path(), "frozen-cli", b"abc");
        fs::rename(
            first.path().join(&relative),
            first
                .path()
                .join(format!("{HARNESS_SOURCE}native/renamed.rs")),
        )
        .unwrap();
        assert_ne!(harness_digest(first.path(), &executable).unwrap(), baseline);
    }

    #[test]
    fn harness_digest_binds_each_native_control_source() {
        for relative in [
            "crates/auctioneer-cli/src/native_control/mod.rs",
            "crates/auctioneer-cli/src/native_control/io.rs",
            "crates/auctioneer-cli/src/native_control/process.rs",
        ] {
            let root = source_tree();
            let executable = write(root.path(), "frozen-cli", b"abc");
            write(
                root.path(),
                "crates/auctioneer-cli/src/qualification/harness/mod.rs",
                b"// existing harness\n",
            );
            let source = write(root.path(), relative, b"// native control alpha\n");
            let baseline = harness_digest(root.path(), &executable).unwrap();
            fs::write(&source, b"// native control beta\n").unwrap();
            assert_ne!(
                harness_digest(root.path(), &executable).unwrap(),
                baseline,
                "native harness digest omitted {relative}"
            );
            fs::write(&source, b"// native control alpha\n").unwrap();
            assert_eq!(harness_digest(root.path(), &executable).unwrap(), baseline);
        }
    }

    #[test]
    fn harness_digest_excludes_neighboring_unrelated_rust_sources() {
        let root = source_tree();
        let executable = write(root.path(), "frozen-cli", b"abc");
        write(
            root.path(),
            "crates/auctioneer-cli/src/native_control/mod.rs",
            b"// native control\n",
        );
        let baseline = harness_digest(root.path(), &executable).unwrap();
        for relative in [
            "crates/auctioneer-cli/src/native_control_extra/mod.rs",
            "crates/auctioneer-cli/src/native_controls/process.rs",
            "crates/auctioneer-cli/src/qualification/harness_extra/mod.rs",
            "crates/auctioneer-cli/src/resource_identity.rs",
        ] {
            let source = write(root.path(), relative, b"// unrelated alpha\n");
            assert_eq!(
                harness_digest(root.path(), &executable).unwrap(),
                baseline,
                "native harness digest admitted {relative}"
            );
            fs::write(&source, b"// unrelated beta\n").unwrap();
            assert_eq!(
                harness_digest(root.path(), &executable).unwrap(),
                baseline,
                "native harness digest admitted a change to {relative}"
            );
        }
    }

    #[test]
    fn harness_executable_hashing_refuses_oversized_files_before_reading_them() {
        let root = source_tree();
        let executable = root.path().join("huge-cli");
        let file = fs::File::create(&executable).unwrap();
        file.set_len(u64::try_from(ARTIFACT_CAP).unwrap().checked_add(1).unwrap())
            .unwrap();
        assert_eq!(
            harness_digest(root.path(), &executable).unwrap_err().code,
            "input_limit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn harness_executable_hashing_refuses_symlinks_and_hardlinks() {
        use std::os::unix::fs::symlink;
        let root = source_tree();
        let executable = write(root.path(), "cli", b"abc");
        let link = root.path().join("cli-link");
        symlink(&executable, &link).unwrap();
        assert_eq!(
            harness_digest(root.path(), &link).unwrap_err().code,
            "invalid_input"
        );
        fs::remove_file(&link).unwrap();
        fs::hard_link(&executable, &link).unwrap();
        assert_eq!(
            harness_digest(root.path(), &link).unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn fixture_digest_matches_an_independent_protocol_one_golden() {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), CONSENT_SOURCE, b"abc");
        let digest = fixtures_digest(&candidate(), &catalog(), root.path()).unwrap();
        // Independently calculated with sorted compact jq JSON and shasum, not this implementation.
        assert_eq!(
            digest,
            Digest::try_from(
                "d7757db6119dc0a75f85d2ec1669688f392bc303c0a9f8c48c7fc25162339f1e".to_owned()
            )
            .unwrap()
        );
    }

    #[test]
    fn fixture_digest_changes_with_candidate_catalog_and_raw_seed_independently() {
        let root = tempfile::tempdir().unwrap();
        let seed = write(root.path(), CONSENT_SOURCE, b"abc");
        let candidate = candidate();
        let catalog = catalog();
        let baseline = fixtures_digest(&candidate, &catalog, root.path()).unwrap();
        let mut changed_candidate = candidate.clone();
        changed_candidate.seed = 8;
        assert_ne!(
            fixtures_digest(&changed_candidate, &catalog, root.path()).unwrap(),
            baseline
        );
        let mut changed_catalog = catalog.clone();
        changed_catalog.cases[0].outcomes.min_offered = 5;
        assert_ne!(
            fixtures_digest(&candidate, &changed_catalog, root.path()).unwrap(),
            baseline
        );
        fs::write(&seed, b"abc\n").unwrap();
        assert_ne!(
            fixtures_digest(&candidate, &catalog, root.path()).unwrap(),
            baseline
        );
        fs::write(&seed, b"abd").unwrap();
        assert_ne!(
            fixtures_digest(&candidate, &catalog, root.path()).unwrap(),
            baseline
        );
    }

    #[test]
    fn fixture_seed_is_read_from_the_supplied_root_with_a_sixteen_kibibyte_cap() {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), CONSENT_SOURCE, &vec![b'a'; CONSENT_CAP]);
        fixtures_digest(&candidate(), &catalog(), root.path()).unwrap();
        write(
            root.path(),
            CONSENT_SOURCE,
            &vec![b'a'; CONSENT_CAP.checked_add(1).unwrap()],
        );
        assert_eq!(
            fixtures_digest(&candidate(), &catalog(), root.path())
                .unwrap_err()
                .code,
            "input_limit"
        );
    }
}
