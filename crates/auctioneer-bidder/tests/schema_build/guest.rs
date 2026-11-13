//! Local WASI observations, never a host memory, work or serving guarantee.

use super::{Catalog, RawVerdict};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::from_utf8;
use std::time::Duration;

const TOOL_VERSIONS: &str = include_str!("../../../../.tool-versions");
const MAX_RUNTIME_PROGRAM_BYTES: u64 = 128 << 20;

fn program_identity(path: &Path) -> Result<(u64, String), String> {
    file_identity(path, MAX_RUNTIME_PROGRAM_BYTES)
}

fn scalar_output(bytes: &[u8]) -> Result<u64, String> {
    let text = from_utf8(bytes)
        .map_err(|_error| "invalid bare scalar output")?
        .trim();
    if text == "-1" || text == "18446744073709551615" {
        return Err("bare probe refused".to_owned());
    }
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err("invalid bare scalar output".to_owned());
    }
    text.parse()
        .map_err(|_error| "invalid bare scalar output".to_owned())
}

pub struct Runtime {
    path: PathBuf,
    version: String,
    identity: (u64, String),
}

impl Runtime {
    pub fn bare_scalar(&self, catalog: &Catalog, call: super::bare::Call) -> Result<u64, String> {
        let artifact = catalog
            .target_dir
            .join("wasm32-unknown-unknown/wasm-release")
            .join(format!("{}.wasm", super::PACKAGE));
        let expected = catalog
            .reports
            .iter()
            .find(|row| row.target == "wasm32-unknown-unknown")
            .ok_or("guest artifact missing from linked membership")?;
        let identity = (expected.artifact_bytes, expected.sha256.clone());
        if file_identity(&artifact, super::MAX_ARTIFACT_BYTES)? != identity {
            return Err("guest artifact identity changed".to_owned());
        }
        let config = empty_config(catalog.directory.path())?;
        let (export, args) = call.invocation();
        let mut process = command_with_export(&self.path, &config, &artifact, &args, Some(export));
        process.current_dir(catalog.directory.path());
        let output = super::successful(&mut process, Duration::from_secs(10))?;
        if file_identity(&artifact, super::MAX_ARTIFACT_BYTES)? != identity {
            return Err("guest artifact identity changed".to_owned());
        }
        scalar_output(&output.stdout)
    }
    pub fn discover() -> Result<Self, String> {
        let expected = pinned_version(TOOL_VERSIONS)?;
        let path = env::split_paths(&env::var_os("PATH").ok_or("guest runtime PATH unavailable")?)
            .map(|entry| entry.join("wasmtime"))
            .find(|candidate| {
                fs::metadata(candidate).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
            })
            .ok_or("pinned guest runtime unavailable")?
            .canonicalize()
            .map_err(|_error| "guest runtime path unavailable")?;
        let identity = program_identity(&path)?;
        let mut version_command = Command::new(&path);
        version_command.env_clear().arg("--version");
        let output = super::successful(&mut version_command, Duration::from_secs(10))?;
        let version =
            String::from_utf8(output.stdout).map_err(|_error| "invalid runtime version output")?;
        validate_version(&version, expected)?;
        let runtime = Self {
            path,
            version: version.trim().to_owned(),
            identity,
        };
        runtime.verify()?;
        Ok(runtime)
    }

    pub fn verify(&self) -> Result<(), String> {
        if program_identity(&self.path)? != self.identity {
            return Err("guest runtime identity changed".to_owned());
        }
        Ok(())
    }

    pub fn identity(&self) -> Value {
        json!({"schema_version": 1_u8, "version": self.version,
            "expected_version": pinned_version(TOOL_VERSIONS).unwrap(),
            "invoked_program": self.path, "invoked_program_bytes": self.identity.0,
            "invoked_program_sha256": self.identity.1,
            "tool_versions_sha256": hex::encode(Sha256::digest(TOOL_VERSIONS.as_bytes())),
            "host_arch": env::consts::ARCH, "host_os": env::consts::OS,
            "code_cache": false, "filesystem_preopens": [], "inherited_guest_environment": false,
            "network": false, "debugger": false,
            "per_process_wall_timeout_ms": 10_000_u64, "per_stream_output_limit": super::MAX_STREAM_LOG_BYTES,
            "runtime_memory_limit": "not_configured", "runtime_fuel_limit": "not_configured",
            "application_or_resource_qualification": false})
    }

    pub fn inventory(&self, catalog: &Catalog, target: &str) -> Result<Value, String> {
        self.execute(catalog, target, "inventory", "", "")
    }

    pub fn initialize(&self, catalog: &Catalog, target: &str) -> Result<Value, String> {
        self.execute(catalog, target, "initialize", "", "")
    }

    pub fn cache_layouts(
        &self,
        catalog: &Catalog,
        target: &str,
    ) -> Result<super::CacheLayouts, String> {
        serde_json::from_value(self.execute(catalog, target, "cache-layouts", "", "")?)
            .map_err(|_error| "invalid guest cache layouts".to_owned())
    }

    pub fn check_raw(
        &self,
        catalog: &Catalog,
        target: &str,
        name: &str,
        input: &str,
    ) -> Result<RawVerdict, String> {
        serde_json::from_value(self.execute(catalog, target, "check-raw", name, input)?)
            .map_err(|_error| "invalid guest raw verdict".to_owned())
    }

    pub fn raw_table(
        &self,
        catalog: &Catalog,
        target: &str,
        name: &str,
        inputs: &[&str],
    ) -> Result<Vec<RawVerdict>, String> {
        let encoded =
            serde_json::to_string(inputs).map_err(|_error| "guest raw table encoding refused")?;
        if encoded.len() > 0x0001_0000 {
            return inputs
                .iter()
                .map(|input| self.check_raw(catalog, target, name, input))
                .collect();
        }
        let rows: Vec<RawVerdict> = serde_json::from_value(self.execute(
            catalog,
            target,
            "raw-sequence",
            name,
            &encoded,
        )?)
        .map_err(|_error| "invalid guest raw table output")?;
        if rows.len() != inputs.len() {
            return Err("guest raw table count mismatch".to_owned());
        }
        Ok(rows)
    }

    pub fn sequence(
        &self,
        catalog: &Catalog,
        target: &str,
        name: &str,
        inputs: &[Value],
    ) -> Result<Vec<bool>, String> {
        let encoded =
            serde_json::to_string(inputs).map_err(|_error| "guest sequence encoding refused")?;
        let rows: Vec<bool> =
            serde_json::from_value(self.execute(catalog, target, "sequence", name, &encoded)?)
                .map_err(|_error| "invalid guest sequence output")?;
        if rows.len() != inputs.len() {
            return Err("guest sequence count mismatch".to_owned());
        }
        Ok(rows)
    }

    fn execute(
        &self,
        catalog: &Catalog,
        target: &str,
        operation: &str,
        name: &str,
        input: &str,
    ) -> Result<Value, String> {
        let args = operation_args(operation, name, input)?;
        let artifact = artifact_path(&catalog.target_dir, target)?;
        let expected = catalog
            .reports
            .iter()
            .find(|row| row.target == target)
            .ok_or("guest artifact missing from linked membership")?;
        if file_identity(&artifact, super::MAX_ARTIFACT_BYTES)?
            != (expected.artifact_bytes, expected.sha256.clone())
        {
            return Err("guest artifact identity changed".to_owned());
        }
        let config = empty_config(catalog.directory.path())?;
        let mut process = command(&self.path, &config, &artifact, &args);
        process.current_dir(catalog.directory.path());
        let output = super::successful(&mut process, Duration::from_secs(10))?;
        let value =
            serde_json::from_slice(&output.stdout).map_err(|_error| "invalid guest output")?;
        if file_identity(&artifact, super::MAX_ARTIFACT_BYTES)?
            != (expected.artifact_bytes, expected.sha256.clone())
        {
            return Err("guest artifact identity changed".to_owned());
        }
        Ok(value)
    }
}

fn pinned_version(contents: &str) -> Result<&str, String> {
    let mut pin = None;
    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() != Some("wasmtime") {
            continue;
        }
        let version = fields.next().ok_or("missing Wasmtime pin")?;
        let parts: Vec<_> = version.split('.').collect();
        if fields.next().is_some()
            || pin.is_some()
            || parts.len() != 3
            || parts
                .iter()
                .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err("invalid Wasmtime pin".to_owned());
        }
        pin = Some(version);
    }
    pin.ok_or_else(|| "missing Wasmtime pin".to_owned())
}

fn validate_version(output: &str, expected: &str) -> Result<(), String> {
    let mut fields = output.split_whitespace();
    if output.lines().count() != 1
        || fields.next() != Some("wasmtime")
        || fields.next() != Some(expected)
    {
        return Err("Wasmtime version mismatch".to_owned());
    }
    Ok(())
}

fn artifact_path(root: &Path, target: &str) -> Result<PathBuf, String> {
    if !matches!(target, "wasm32-wasip1" | "wasm32-wasip2") {
        return Err("unsupported guest command target".to_owned());
    }
    Ok(root
        .join(target)
        .join("wasm-release")
        .join(format!("{}.wasm", super::PACKAGE)))
}

fn operation_args(operation: &str, name: &str, input: &str) -> Result<Vec<String>, String> {
    match operation {
        "inventory" | "initialize" | "cache-layouts" => Ok(vec![operation.to_owned()]),
        "check-raw" | "raw-sequence" | "sequence" => {
            if input.len() > 0x0001_0000 {
                return Err("guest payload limit".to_owned());
            }
            Ok(vec![
                operation.to_owned(),
                name.to_owned(),
                input.to_owned(),
            ])
        }
        _ => Err("invalid guest operation".to_owned()),
    }
}

fn command(runtime: &Path, config: &Path, artifact: &Path, args: &[String]) -> Command {
    command_with_export(runtime, config, artifact, args, None)
}

fn command_with_export(
    runtime: &Path,
    config: &Path,
    artifact: &Path,
    args: &[String],
    export: Option<&str>,
) -> Command {
    let mut built = Command::new(runtime);
    built
        .env_clear()
        .arg("run")
        .arg("--config")
        .arg(config)
        .args([
            "-C",
            "cache=n",
            "-S",
            "inherit-network=n,allow-ip-name-lookup=n,tcp=n,udp=n,inherit-env=n,inherit-stdin=n",
        ]);
    if let Some(name) = export {
        built.args(["-W", "unknown-imports-trap=y", "--invoke", name]);
    }
    built.arg("--").arg(artifact).args(args);
    built
}

fn file_identity(path: &Path, limit: u64) -> Result<(u64, String), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_error| "guest file identity unavailable")?;
    if !metadata.is_file() {
        return Err("guest identity requires a regular file".to_owned());
    }
    let mut file = File::open(path).map_err(|_error| "guest file identity unavailable")?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 8192];
    let mut bytes = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_error| "guest file identity unavailable")?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(count).map_err(|_error| "guest identity overflow")?)
            .ok_or("guest identity overflow")?;
        if bytes > limit {
            return Err("guest identity byte limit".to_owned());
        }
        hash.update(buffer.get(..count).ok_or("guest identity buffer limit")?);
    }
    if metadata.len() != bytes {
        return Err("guest identity changed while reading".to_owned());
    }
    Ok((bytes, hex::encode(hash.finalize())))
}

fn empty_config(root: &Path) -> Result<PathBuf, String> {
    let path = root.join("wasmtime-empty.toml");
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => drop(file),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(_error) => return Err("guest configuration unavailable".to_owned()),
    }
    let metadata =
        fs::symlink_metadata(&path).map_err(|_error| "guest configuration unavailable")?;
    if !metadata.is_file() || metadata.len() != 0 {
        return Err("guest configuration must remain empty".to_owned());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{
        artifact_path, command, empty_config, file_identity, operation_args, pinned_version,
        validate_version,
    };
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn runner_bare_scalar_protocol_refuses_extra_output_signed_errors_and_nonintegers() {
        for (bytes, expected) in [("0", 0_u64), ("1\n", 1), ("4294967295\n", 0xffff_ffff)] {
            assert_eq!(super::scalar_output(bytes.as_bytes()), Ok(expected));
        }
        for bytes in [
            "",
            "1 2",
            "1\n2",
            "\"1\"",
            "{}",
            "1.0",
            "+1",
            "01",
            "-2",
            "18446744073709551616",
        ] {
            assert_eq!(
                super::scalar_output(bytes.as_bytes()).unwrap_err(),
                "invalid bare scalar output"
            );
        }
        for bytes in ["-1", "18446744073709551615"] {
            assert_eq!(
                super::scalar_output(bytes.as_bytes()).unwrap_err(),
                "bare probe refused"
            );
        }
        assert_eq!(
            super::scalar_output(&[255]).unwrap_err(),
            "invalid bare scalar output"
        );
    }

    #[test]
    fn runner_bare_command_uses_only_a_fixed_integer_export_before_the_module_terminator() {
        let (export, args) = super::super::bare::Call::Replay {
            schema: 4,
            start: 16,
            reuse: true,
        }
        .invocation();
        let built = super::command_with_export(
            Path::new("/fake_runner"),
            Path::new("/fake_config"),
            Path::new("/fake_module"),
            &args,
            Some(export),
        );
        assert_eq!(
            built.get_args().collect::<Vec<_>>(),
            [
                "run",
                "--config",
                "/fake_config",
                "-C",
                "cache=n",
                "-S",
                "inherit-network=n,allow-ip-name-lookup=n,tcp=n,udp=n,inherit-env=n,inherit-stdin=n",
                "-W",
                "unknown-imports-trap=y",
                "--invoke",
                "auctioneer_probe_replay",
                "--",
                "/fake_module",
                "4",
                "16",
                "1"
            ]
        );
        assert_eq!(built.get_program(), "/fake_runner");
    }

    #[test]
    fn runner_pin_requires_one_exact_declared_version() {
        assert_eq!(pinned_version("rust 1.2.3\nwasmtime 4.5.6\n"), Ok("4.5.6"));
        for input in [
            "",
            "rust 4.5.6\n",
            "wasmtime\n",
            "wasmtime 4.5.6 extra\n",
            "wasmtime 4.5.6\nwasmtime 4.5.6\n",
            "wasmtime latest\n",
            "wasmtime 4.5\n",
        ] {
            pinned_version(input).unwrap_err();
        }
    }

    #[test]
    fn runner_version_checks_tool_and_version_independently() {
        assert_eq!(
            validate_version("wasmtime 4.5.6 (abc 2026-01-01)\n", "4.5.6"),
            Ok(())
        );
        for output in [
            "",
            "wasmtime",
            "fake_runner 4.5.6",
            "wasmtime 4.5.60",
            "wasmtime 4.5.6-dev",
            "wasmtime 4.5.6\nextra",
            "wasmtime 3.5.6",
        ] {
            assert_eq!(
                validate_version(output, "4.5.6"),
                Err("Wasmtime version mismatch".to_owned())
            );
        }
    }

    #[test]
    fn runner_paths_only_admit_existing_wasi_command_targets() {
        for target in ["wasm32-wasip1", "wasm32-wasip2"] {
            assert_eq!(
                artifact_path(Path::new("/fake_root"), target),
                Ok(PathBuf::from("/fake_root")
                    .join(target)
                    .join("wasm-release/auctioneer-schema-link-probe.wasm"))
            );
        }
        for target in [
            "native",
            "wasm32-unknown-unknown",
            "",
            "../fake_escape",
            "/fake_absolute",
        ] {
            assert_eq!(
                artifact_path(Path::new("/fake_root"), target),
                Err("unsupported guest command target".to_owned())
            );
        }
    }

    #[test]
    fn runner_protocol_bounds_raw_and_sequence_payloads_without_shell_interpretation() {
        for operation in ["inventory", "initialize", "cache-layouts"] {
            assert_eq!(
                operation_args(operation, "", ""),
                Ok(vec![operation.to_owned()])
            );
        }
        for operation in ["check-raw", "raw-sequence", "sequence"] {
            for bytes in [0_usize, 1, 0x0001_0000] {
                let input = "x".repeat(bytes);
                assert_eq!(
                    operation_args(operation, "fake_bidder", &input),
                    Ok(vec![operation.to_owned(), "fake_bidder".to_owned(), input])
                );
            }
            assert_eq!(
                operation_args(operation, "fake_bidder", &"x".repeat(0x0001_0001)),
                Err("guest payload limit".to_owned())
            );
            let mut unicode = "\u{00e9}".repeat(0x8000);
            assert_eq!(unicode.len(), 0x0001_0000);
            assert_eq!(
                operation_args(operation, "fake_bidder", &unicode),
                Ok(vec![
                    operation.to_owned(),
                    "fake_bidder".to_owned(),
                    unicode.clone()
                ])
            );
            unicode.push('x');
            assert_eq!(
                operation_args(operation, "fake_bidder", &unicode),
                Err("guest payload limit".to_owned())
            );
        }
        for operation in ["", "allocations", "check-formats-disabled", "fake_other"] {
            assert_eq!(
                operation_args(operation, "fake_bidder", "{}"),
                Err("invalid guest operation".to_owned())
            );
        }
        assert_eq!(
            operation_args("check-raw", "fake_bidder", "--dir=/fake_private"),
            Ok(vec![
                "check-raw".to_owned(),
                "fake_bidder".to_owned(),
                "--dir=/fake_private".to_owned()
            ])
        );
    }

    #[test]
    fn runner_command_isolates_host_settings_and_places_payload_after_option_terminator() {
        let args = operation_args("check-raw", "fake_bidder", "--env=FAKE_SECRET").unwrap();
        let built = command(
            Path::new("/fake_runtime"),
            Path::new("/fake_config"),
            Path::new("/fake_module.wasm"),
            &args,
        );
        assert_eq!(built.get_program(), "/fake_runtime");
        assert_eq!(
            built.get_args().collect::<Vec<_>>(),
            [
                "run",
                "--config",
                "/fake_config",
                "-C",
                "cache=n",
                "-S",
                "inherit-network=n,allow-ip-name-lookup=n,tcp=n,udp=n,inherit-env=n,inherit-stdin=n",
                "--",
                "/fake_module.wasm",
                "check-raw",
                "fake_bidder",
                "--env=FAKE_SECRET"
            ]
        );
        assert_eq!(built.get_envs().count(), 0);
    }

    #[test]
    fn runner_file_identity_checks_real_bytes_and_exact_one_over_limits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fake_artifact");
        fs::write(&path, b"abc").unwrap();
        let expected = (
            3_u64,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned(),
        );
        assert_eq!(file_identity(&path, 3), Ok(expected.clone()));
        assert_eq!(file_identity(&path, 4), Ok(expected.clone()));
        file_identity(&path, 2).unwrap_err();
        fs::write(&path, b"abd").unwrap();
        assert_ne!(file_identity(&path, 3).unwrap(), expected);
        file_identity(directory.path(), 3).unwrap_err();
        file_identity(&directory.path().join("fake_missing"), 3).unwrap_err();
    }

    #[test]
    fn runner_config_is_empty_idempotent_and_refuses_existing_unreviewed_settings() {
        let directory = tempfile::tempdir().unwrap();
        let path = empty_config(directory.path()).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"");
        assert_eq!(empty_config(directory.path()), Ok(path.clone()));
        fs::write(&path, "fake_setting = true\n").unwrap();
        empty_config(directory.path()).unwrap_err();
        assert_eq!(fs::read_to_string(path).unwrap(), "fake_setting = true\n");
    }

    #[test]
    fn runner_command_really_removes_inherited_host_environment() {
        // Cargo supplies this non-secret marker. The ordinary child is the
        // positive control; no process-global environment mutation is needed.
        assert!(env::var_os("CARGO_MANIFEST_DIR").is_some());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fake_runtime");
        fs::write(&path, "#!/bin/sh\n/usr/bin/env\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let inherited = Command::new(&path).output().unwrap();
        assert!(inherited.status.success());
        assert!(
            String::from_utf8(inherited.stdout)
                .unwrap()
                .contains("CARGO_MANIFEST_DIR=")
        );
        let isolated = command(
            &path,
            Path::new("/fake_config"),
            Path::new("/fake_module"),
            &[],
        )
        .output()
        .unwrap();
        assert!(isolated.status.success());
        assert!(
            !String::from_utf8(isolated.stdout)
                .unwrap()
                .contains("CARGO_MANIFEST_DIR=")
        );
    }

    #[test]
    fn runner_identity_rechecks_byte_count_and_digest_independently() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fake_program");
        fs::write(&path, b"abc").unwrap();
        let identity = (
            3_u64,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned(),
        );
        let mut runtime = super::Runtime {
            path,
            version: "fake_version".to_owned(),
            identity: identity.clone(),
        };
        assert_eq!(runtime.verify(), Ok(()));
        runtime.identity.0 = 4;
        assert_eq!(
            runtime.verify(),
            Err("guest runtime identity changed".to_owned())
        );
        runtime.identity.0 = 3;
        runtime.identity.1 = "fake_changed_digest".to_owned();
        assert_eq!(
            runtime.verify(),
            Err("guest runtime identity changed".to_owned())
        );
        runtime.identity = identity;
        assert_eq!(runtime.verify(), Ok(()));
    }

    fn fake_catalog(bytes: &[u8]) -> super::Catalog {
        use super::super::LinkReport;
        use sha2::{Digest as _, Sha256};
        let directory = tempfile::tempdir().unwrap();
        let target_dir = directory.path().join("fake_target");
        let path = artifact_path(&target_dir, "wasm32-wasip1").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        let native = directory.path().join("fake_native");
        fs::write(&native, "#!/bin/sh\n/bin/cat \"$0.data\"\n").unwrap();
        fs::write(
            native.with_extension("data"),
            serde_json::to_vec(&serde_json::json!({})).unwrap(),
        )
        .unwrap();
        fs::set_permissions(&native, fs::Permissions::from_mode(0o700)).unwrap();
        let lease = fs::File::create(directory.path().join("fake_lease")).unwrap();
        super::Catalog {
            reports: vec![LinkReport {
                target: "wasm32-wasip1".to_owned(),
                validators: 1,
                artifact_bytes: u64::try_from(bytes.len()).unwrap(),
                sha256: hex::encode(Sha256::digest(bytes)),
                names: vec!["fake_bidder".to_owned()],
                results: vec![false],
                wasm: None,
            }],
            directory,
            native,
            target_dir,
            _lease: lease,
            _inputs: None,
        }
    }

    fn fake_bare_catalog(bytes: &[u8]) -> super::Catalog {
        let mut catalog = fake_catalog(bytes);
        let path = catalog
            .target_dir
            .join("wasm32-unknown-unknown/wasm-release/auctioneer-schema-link-probe.wasm");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        let (artifact_bytes, sha256) =
            file_identity(&path, super::super::MAX_ARTIFACT_BYTES).unwrap();
        catalog.reports.push(super::super::LinkReport {
            target: "wasm32-unknown-unknown".to_owned(),
            validators: 1,
            artifact_bytes,
            sha256,
            names: vec!["fake_bidder".to_owned()],
            results: vec![false],
            wasm: None,
        });
        catalog
    }

    #[test]
    fn runner_bare_calls_use_the_unknown_artifact_and_reject_malformed_results_or_drift() {
        use super::super::bare::Call;
        let mut catalog = fake_bare_catalog(b"fake_module");
        let path = catalog.directory.path().join("fake_bare_runner");
        fs::write(&path, "#!/bin/sh\nconfig=\nexport=\nwhile [ \"$1\" != -- ]; do\n  if [ \"$1\" = --config ]; then config=$2; fi\n  if [ \"$1\" = --invoke ]; then export=$2; fi\n  shift\ndone\nshift\nmodule=$1\nshift\nprintf '%s|%s|%s\\n' \"$export\" \"$module\" \"$*\" >> \"$config.trace\"\n/bin/cat \"$config.scalar\"\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = super::Runtime {
            identity: super::program_identity(&path).unwrap(),
            path,
            version: "fake_runner".to_owned(),
        };
        let prefix = catalog.directory.path().join("wasmtime-empty.toml");
        let output = prefix.with_extension("toml.scalar");
        fs::write(&output, "1\n").unwrap();
        assert_eq!(runtime.bare_scalar(&catalog, Call::Version), Ok(1));
        assert_eq!(
            runtime.bare_scalar(
                &catalog,
                Call::Replay {
                    schema: 3,
                    start: 16,
                    reuse: true
                }
            ),
            Ok(1)
        );
        let module = catalog
            .target_dir
            .join("wasm32-unknown-unknown/wasm-release/auctioneer-schema-link-probe.wasm");
        assert_eq!(
            fs::read_to_string(prefix.with_extension("toml.trace")).unwrap(),
            format!(
                "auctioneer_probe_version|{}|\nauctioneer_probe_replay|{}|3 16 1\n",
                module.display(),
                module.display()
            )
        );
        fs::write(&output, "[]").unwrap();
        assert_eq!(
            runtime.bare_scalar(&catalog, Call::Version).unwrap_err(),
            "invalid bare scalar output"
        );
        fs::write(&output, "-1").unwrap();
        assert_eq!(
            runtime.bare_scalar(&catalog, Call::Version).unwrap_err(),
            "bare probe refused"
        );
        catalog
            .reports
            .iter_mut()
            .find(|row| row.target == "wasm32-unknown-unknown")
            .unwrap()
            .sha256 = "fake_changed".to_owned();
        assert_eq!(
            runtime.bare_scalar(&catalog, Call::Version).unwrap_err(),
            "guest artifact identity changed"
        );
        runtime.verify().unwrap();
    }

    #[test]
    fn runner_does_not_substitute_a_successful_native_program_for_a_failing_guest() {
        let runtime = super::Runtime::discover().unwrap();
        let mut catalog = fake_catalog(b"fake_invalid_wasm");
        let native = Command::new(&catalog.native).output().unwrap();
        assert!(native.status.success());
        assert_eq!(native.stdout, b"{}");
        let error = runtime.inventory(&catalog, "wasm32-wasip1").unwrap_err();
        assert!(error.starts_with("child failed:"), "{error}");
        catalog.reports[0].sha256 = "fake_changed_digest".to_owned();
        assert_eq!(
            runtime.inventory(&catalog, "wasm32-wasip1"),
            Err("guest artifact identity changed".to_owned())
        );
        assert_eq!(
            runtime.inventory(&catalog, "wasm32-unknown-unknown"),
            Err("unsupported guest command target".to_owned())
        );
        runtime.verify().unwrap();
    }

    #[test]
    fn runner_does_not_credit_native_success_when_the_integer_abi_module_is_invalid() {
        let runtime = super::Runtime::discover().unwrap();
        let catalog = fake_bare_catalog(b"fake_invalid_wasm");
        let native = Command::new(&catalog.native).output().unwrap();
        assert!(native.status.success());
        assert_eq!(native.stdout, b"{}");
        let error = runtime
            .bare_scalar(&catalog, super::super::bare::Call::Version)
            .unwrap_err();
        assert!(error.starts_with("child failed:"), "{error}");
        runtime.verify().unwrap();
    }

    fn fake_binding_import_module(
        import: (&str, &str),
        params: &[wasm_encoder::ValType],
        results: &[wasm_encoder::ValType],
        call_import: bool,
    ) -> Vec<u8> {
        use wasm_encoder::{
            CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection,
            ImportSection, Instruction, Module, TypeSection, ValType,
        };

        // Two function types, one unavailable binding import and an integer
        // export. Only the negative control calls the imported function.
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types
            .ty()
            .function(params.iter().copied(), results.iter().copied());
        types.ty().function([], [ValType::I64]);
        module.section(&types);
        let mut imports = ImportSection::new();
        imports.import(import.0, import.1, EntityType::Function(0));
        module.section(&imports);
        let mut functions = FunctionSection::new();
        functions.function(1);
        module.section(&functions);
        let mut exports = ExportSection::new();
        exports.export("auctioneer_probe_version", ExportKind::Func, 1);
        module.section(&exports);
        let mut function = Function::new([]);
        if call_import {
            for _ in params {
                function.instruction(&Instruction::I32Const(0));
            }
            function.instruction(&Instruction::Call(0));
            for _ in results {
                function.instruction(&Instruction::Drop);
            }
        }
        function.instruction(&Instruction::I64Const(1));
        function.instruction(&Instruction::End);
        let mut code = CodeSection::new();
        code.function(&function);
        module.section(&code);
        module.finish()
    }

    #[test]
    fn runner_bare_unavailable_imports_trap_instead_of_returning_default_values() {
        use wasm_encoder::ValType::I32;

        let runtime = super::Runtime::discover().unwrap();
        for (import, params, results) in [
            (
                ("__wbindgen_placeholder__", "__wbindgen_describe"),
                &[I32][..],
                &[][..],
            ),
            (
                (
                    "__wbindgen_externref_xform__",
                    "__wbindgen_externref_table_set_null",
                ),
                &[I32][..],
                &[][..],
            ),
            (
                (
                    "__wbindgen_externref_xform__",
                    "__wbindgen_externref_table_grow",
                ),
                &[I32][..],
                &[I32][..],
            ),
            (
                (
                    "__wbindgen_externref_xform__",
                    "__wbindgen_externref_statics_end",
                ),
                &[][..],
                &[I32][..],
            ),
        ] {
            for calls_import in [false, true] {
                let bytes = fake_binding_import_module(import, params, results, calls_import);
                wasmparser::Validator::new().validate_all(&bytes).unwrap();
                let imports = super::super::bare::trapped_imports(&bytes).unwrap();
                assert_eq!(imports, [(import.0.to_owned(), import.1.to_owned())]);
                let catalog = fake_bare_catalog(&bytes);
                let result = runtime.bare_scalar(&catalog, super::super::bare::Call::Version);
                if calls_import {
                    let error = result.unwrap_err();
                    assert!(error.contains("wasm backtrace"), "{import:?}: {error}");
                } else {
                    assert_eq!(result, Ok(1), "{import:?}");
                }
            }
        }
        runtime.verify().unwrap();
    }

    #[test]
    fn runner_raw_tables_preserve_each_verdict_and_use_scalar_fallback_without_shrinking_inputs() {
        use super::{RawVerdict, Runtime};
        use serde_json::json;
        let catalog = fake_catalog(b"fake_module");
        let path = catalog.directory.path().join("fake_runner");
        fs::write(&path, "#!/bin/sh\nconfig=\nwhile [ \"$1\" != -- ]; do\n  if [ \"$1\" = --config ]; then config=$2; fi\n  shift\ndone\nshift\nshift\nprintf '%s\\n' \"$1\" >> \"$config.trace\"\ncase \"$1\" in\n  raw-sequence) /bin/cat \"$config.batch\" ;;\n  check-raw) /bin/cat \"$config.scalar\" ;;\n  *) exit 2 ;;\nesac\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = Runtime {
            identity: file_identity(&path, super::super::MAX_ARTIFACT_BYTES).unwrap(),
            path,
            version: "fake_runner".to_owned(),
        };
        let prefix = catalog.directory.path().join("wasmtime-empty.toml");
        let batch_path = prefix.with_extension("toml.batch");
        let scalar_path = prefix.with_extension("toml.scalar");
        let trace = prefix.with_extension("toml.trace");
        fs::write(&batch_path, serde_json::to_vec(&json!([
            {"kind": "schema", "valid": true}, {"kind": "invalid_json"}, {"kind": "schema", "valid": false}])).unwrap()).unwrap();
        fs::write(
            &scalar_path,
            serde_json::to_vec(&json!({"kind": "invalid_json"})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            runtime
                .raw_table(
                    &catalog,
                    "wasm32-wasip1",
                    "fake_bidder",
                    &["{}", "[", "null"]
                )
                .unwrap(),
            vec![
                RawVerdict::Schema { valid: true },
                RawVerdict::InvalidJson,
                RawVerdict::Schema { valid: false }
            ]
        );
        assert_eq!(fs::read_to_string(&trace).unwrap(), "raw-sequence\n");
        let oversized = "x".repeat(0x0001_0000);
        assert_eq!(
            runtime
                .raw_table(&catalog, "wasm32-wasip1", "fake_bidder", &[&oversized, "["])
                .unwrap(),
            vec![RawVerdict::InvalidJson, RawVerdict::InvalidJson]
        );
        assert_eq!(
            fs::read_to_string(&trace).unwrap(),
            "raw-sequence\ncheck-raw\ncheck-raw\n"
        );
        let too_large = "x".repeat(0x0001_0001);
        assert_eq!(
            runtime.raw_table(&catalog, "wasm32-wasip1", "fake_bidder", &[&too_large]),
            Err("guest payload limit".to_owned())
        );
        fs::write(&batch_path, serde_json::to_vec(&json!([])).unwrap()).unwrap();
        assert_eq!(
            runtime.raw_table(&catalog, "wasm32-wasip1", "fake_bidder", &["["]),
            Err("guest raw table count mismatch".to_owned())
        );
    }

    #[test]
    fn runner_program_file_limit_is_independent_of_the_smaller_wasm_artifact_limit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fake_program");
        let file = fs::File::create(&path).unwrap();
        file.set_len(70_741_896).unwrap();
        assert_eq!(super::program_identity(&path).unwrap().0, 70_741_896);
        assert_eq!(
            file_identity(&path, super::super::MAX_ARTIFACT_BYTES),
            Err("guest identity byte limit".to_owned())
        );
        file.set_len(0x0800_0001).unwrap();
        assert_eq!(
            super::program_identity(&path),
            Err("guest identity byte limit".to_owned())
        );
    }

    #[test]
    fn runner_protocol_methods_check_typed_sequences_and_report_the_actual_program_witness() {
        use super::Runtime;
        use serde_json::json;
        let catalog = fake_catalog(b"fake_module");
        let path = catalog.directory.path().join("fake_runner");
        fs::write(&path, "#!/bin/sh\nconfig=\nwhile [ \"$1\" != -- ]; do\n  if [ \"$1\" = --config ]; then config=$2; fi\n  shift\ndone\nshift\nshift\ncase \"$1\" in\n  inventory|initialize|sequence|cache-layouts) /bin/cat \"$config.$1\" ;;\n  *) exit 2 ;;\nesac\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let identity = file_identity(&path, super::super::MAX_ARTIFACT_BYTES).unwrap();
        let runtime = Runtime {
            path,
            identity: identity.clone(),
            version: "fake_version_for_witness".to_owned(),
        };
        let prefix = catalog.directory.path().join("wasmtime-empty.toml");
        for (operation, output) in [
            ("inventory", json!({"fake_inventory": true})),
            ("initialize", json!({"fake_initializer": true})),
            ("sequence", json!([true, false])),
            (
                "cache-layouts",
                json!({"schema_version": 2_u8, "word_bytes": 4_usize,
                "cache_bytes": 20_usize, "cache_alignment": 4_usize,
                "rows": [{"name": "fake_bidder", "inline_bytes": 20_usize, "alignment": 4_usize}]}),
            ),
        ] {
            fs::write(
                prefix.with_extension(format!("toml.{operation}")),
                serde_json::to_vec(&output).unwrap(),
            )
            .unwrap();
        }
        assert_eq!(
            runtime.inventory(&catalog, "wasm32-wasip1"),
            Ok(json!({"fake_inventory": true}))
        );
        assert_eq!(
            runtime.initialize(&catalog, "wasm32-wasip1"),
            Ok(json!({"fake_initializer": true}))
        );
        let layouts = runtime.cache_layouts(&catalog, "wasm32-wasip1").unwrap();
        layouts.validate(4, &[("fake_bidder", 1)]).unwrap();
        assert_eq!(layouts.cache_bytes, 20);
        let layout_output = prefix.with_extension("toml.cache-layouts");
        for output in [json!({}), json!({"schema_version": "fake_not_a_version"})] {
            fs::write(&layout_output, serde_json::to_vec(&output).unwrap()).unwrap();
            assert_eq!(
                runtime
                    .cache_layouts(&catalog, "wasm32-wasip1")
                    .unwrap_err(),
                "invalid guest cache layouts"
            );
        }
        assert_eq!(
            runtime.sequence(
                &catalog,
                "wasm32-wasip1",
                "fake_bidder",
                &[json!(true), json!(false)]
            ),
            Ok(vec![true, false])
        );
        assert_eq!(
            runtime.sequence(&catalog, "wasm32-wasip1", "fake_bidder", &[json!(true)]),
            Err("guest sequence count mismatch".to_owned())
        );
        fs::write(
            prefix.with_extension("toml.sequence"),
            serde_json::to_vec(&json!(["fake_not_boolean"])).unwrap(),
        )
        .unwrap();
        assert_eq!(
            runtime.sequence(&catalog, "wasm32-wasip1", "fake_bidder", &[json!(true)]),
            Err("invalid guest sequence output".to_owned())
        );
        let witness = runtime.identity();
        assert_eq!(witness["version"], "fake_version_for_witness");
        assert_eq!(witness["invoked_program_bytes"], identity.0);
        assert_eq!(witness["invoked_program_sha256"], identity.1);
        for field in [
            "application_or_resource_qualification",
            "code_cache",
            "inherited_guest_environment",
            "network",
            "debugger",
        ] {
            assert_eq!(witness[field], false, "{field}");
        }
        assert_eq!(witness["filesystem_preopens"], json!([]));
        assert_eq!(witness["runtime_memory_limit"], "not_configured");
        assert_eq!(witness["runtime_fuel_limit"], "not_configured");
    }
}
