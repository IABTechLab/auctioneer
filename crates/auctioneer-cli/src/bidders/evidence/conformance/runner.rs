//! Fixed owner-local source tests, not a semantic or artifact qualification driver.

use super::Refusal;
use super::scratch::Cache;
use auctioneer_cli::native_control::Result as ControlResult;
use auctioneer_cli::native_control::io::{file_digest, parse_json, sha256};
use auctioneer_cli::native_control::process::{CommandOutput, ProcessSpec, Supervisor};
use futures::future::{Either, select};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env::var_os;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use std::time::Duration;
use tokio::time::sleep;

pub(super) const TESTS: [&str; 2] = [
    "tests::schema_selection_source_and_expanded_generated_membership_manifest",
    "tests::schema_selection_manifest_is_order_independent_and_detects_omissions",
];
const TARGET: &str = "schema_source_conformance";
const BUILD_CAP: usize = 16 << 20;
const TEST_CAP: usize = 8 << 20;
const EXECUTABLE_CAP: usize = 512 << 20;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Build {
    pub target: String,
    pub features: Vec<String>,
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub cargo_stdout_sha256: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Dispatch {
    pub test: String,
    pub dispatched: u8,
    pub passed: u8,
    pub failed: u8,
    pub ignored: u8,
    pub stdout: String,
    pub stdout_sha256: String,
}

#[derive(Default)]
pub(super) struct Execution {
    pub build: Option<Build>,
    pub operations: Vec<Dispatch>,
    pub error: Option<Refusal>,
}

pub(super) fn environment() -> BTreeMap<OsString, OsString> {
    let mut environment = BTreeMap::new();
    for name in [
        "PATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "CARGO_TARGET_DIR",
        "RUSTUP_TOOLCHAIN",
        "TMPDIR",
    ] {
        if let Some(value) = var_os(name).filter(|value| !value.is_empty()) {
            environment.insert(OsString::from(name), value);
        }
    }
    for (name, value) in [
        ("LC_ALL", "C"),
        ("LANG", "C"),
        ("CARGO_NET_OFFLINE", "true"),
        ("CARGO_TERM_COLOR", "never"),
        ("CARGO_BUILD_JOBS", "1"),
        ("CARGO_INCREMENTAL", "0"),
        ("CARGO_PROFILE_DEV_DEBUG", "0"),
        ("CARGO_PROFILE_TEST_DEBUG", "0"),
        ("CARGO_PROFILE_DEV_INCREMENTAL", "false"),
        ("CARGO_PROFILE_TEST_INCREMENTAL", "false"),
        ("CARGO_PROFILE_DEV_STRIP", "none"),
        ("CARGO_ENCODED_RUSTFLAGS", ""),
    ] {
        environment.insert(OsString::from(name), OsString::from(value));
    }
    environment
}

pub(super) fn test_environment(root: &Path) -> BTreeMap<OsString, OsString> {
    let mut environment = environment();
    environment.insert(
        OsString::from("CARGO_MANIFEST_DIR"),
        root.join("crates/auctioneer-bidder").into_os_string(),
    );
    environment
}

/// Opaque access to the existing bounded cache; other evidence producers do not
/// acquire an independent lock or implement a second directory walker.
pub(in crate::bidders::evidence) struct CacheGuard(Cache);

pub(in crate::bidders::evidence) struct ReadGuard {
    _lease: super::scratch::ReadLease,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::bidders::evidence) enum GuardError {
    Storage,
    Busy,
    Command,
    Cleanup,
}

impl From<Refusal> for GuardError {
    fn from(error: Refusal) -> Self {
        match error {
            Refusal::BuildBusy => Self::Busy,
            Refusal::Cleanup => Self::Cleanup,
            Refusal::Storage => Self::Storage,
            Refusal::ReferenceIntegrityFailed
            | Refusal::Build
            | Refusal::BuildOutput
            | Refusal::Command
            | Refusal::Dispatch => Self::Command,
        }
    }
}

impl CacheGuard {
    pub(in crate::bidders::evidence) fn claim(root: &Path) -> Result<Self, GuardError> {
        Cache::claim(root).map(Self).map_err(GuardError::from)
    }

    pub(in crate::bidders::evidence) fn discard(&self) -> Result<(), GuardError> {
        self.0.discard().map_err(GuardError::from)
    }

    /// Consume the lease after observing the supervisor's cleanup fence.
    pub(in crate::bidders::evidence) fn finalize(
        self,
        cleanup: ControlResult<()>,
        operation_failed: bool,
    ) -> Result<(), GuardError> {
        if cleanup.is_err() {
            self.0.quarantine().map_err(|_error| GuardError::Cleanup)?;
            return Err(GuardError::Cleanup);
        }
        if operation_failed {
            self.discard()?;
        }
        Ok(())
    }
}

impl ReadGuard {
    pub(in crate::bidders::evidence) fn acquire(root: &Path) -> Result<Self, GuardError> {
        super::scratch::ReadLease::acquire(root)
            .map(|lease| Self { _lease: lease })
            .map_err(GuardError::from)
    }
}

pub(in crate::bidders::evidence) fn bounded_environment() -> BTreeMap<OsString, OsString> {
    environment()
}

#[cfg(test)]
pub(in crate::bidders::evidence) fn cache_directory(managed_target: &Path) -> PathBuf {
    super::scratch::directory(managed_target)
}

pub(in crate::bidders::evidence) async fn supervised_process(
    supervisor: &Supervisor,
    spec: ProcessSpec,
    cache: &CacheGuard,
) -> Result<CommandOutput, GuardError> {
    bounded_run(supervisor, spec, &cache.0, Refusal::Command)
        .await
        .map_err(GuardError::from)
}

fn build_spec(root: &Path, target: &str, target_directory: &Path) -> ProcessSpec {
    let mut spec = ProcessSpec::new(env!("CARGO"));
    spec.cwd = Some(root.to_owned());
    spec.env = environment();
    spec.args = [
        "test",
        "--locked",
        "--offline",
        "--no-default-features",
        "-p",
        "auctioneer-bidder",
        "--features",
        "test-support",
        "--test",
        TARGET,
        "--no-run",
        "--message-format=json",
        "--target",
        target,
        "--target-dir",
    ]
    .map(OsString::from)
    .to_vec();
    spec.args.push(target_directory.as_os_str().to_owned());
    spec.timeout_ms = 600_000;
    spec.output_cap = BUILD_CAP;
    spec
}

fn test_spec(root: &Path, executable: &Path, test: &str) -> ProcessSpec {
    let mut spec = ProcessSpec::new(executable);
    spec.cwd = Some(root.to_owned());
    spec.env = test_environment(root);
    spec.args = [
        "--exact",
        test,
        "--test-threads=1",
        "--color=never",
        "--format=pretty",
    ]
    .map(OsString::from)
    .to_vec();
    spec.timeout_ms = 120_000;
    spec.output_cap = TEST_CAP;
    spec
}

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    manifest_path: Option<PathBuf>,
    target: Option<CargoTarget>,
    profile: Option<CargoProfile>,
    #[serde(default)]
    features: Vec<String>,
    executable: Option<PathBuf>,
    success: Option<bool>,
}

#[derive(Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
}

#[derive(Deserialize)]
struct CargoProfile {
    test: bool,
}

fn executable(
    bytes: &[u8],
    root: &Path,
    target: &str,
    target_directory: &Path,
) -> Result<PathBuf, Refusal> {
    if bytes.len() > BUILD_CAP {
        return Err(Refusal::BuildOutput);
    }
    let text = from_utf8(bytes).map_err(|_error| Refusal::BuildOutput)?;
    let mut found = None;
    let mut finished = false;
    for (index, line) in text.lines().enumerate() {
        if index >= 10_000 || finished {
            return Err(Refusal::BuildOutput);
        }
        let message: CargoMessage = parse_json(line.as_bytes(), super::super::CAP)
            .map_err(|_error| Refusal::BuildOutput)?;
        if message.reason == "build-finished" {
            if message.success != Some(true) {
                return Err(Refusal::BuildOutput);
            }
            finished = true;
            continue;
        }
        if message.reason == "compiler-artifact"
            && message
                .target
                .as_ref()
                .is_some_and(|row| row.name == TARGET)
        {
            let row = message.target.ok_or(Refusal::BuildOutput)?;
            if found.is_some()
                || message.manifest_path != Some(root.join("crates/auctioneer-bidder/Cargo.toml"))
                || row.kind != ["test"]
                || row.src_path
                    != root.join("crates/auctioneer-bidder/tests/schema_source_conformance.rs")
                || !message.profile.is_some_and(|profile| profile.test)
                || message.features != ["test-support"]
            {
                return Err(Refusal::BuildOutput);
            }
            let path = message.executable.ok_or(Refusal::BuildOutput)?;
            if !artifact_path(&path, target, target_directory) {
                return Err(Refusal::BuildOutput);
            }
            found = Some(path);
        }
    }
    if !finished {
        return Err(Refusal::BuildOutput);
    }
    found.ok_or(Refusal::BuildOutput)
}

fn artifact_path(path: &Path, target: &str, target_directory: &Path) -> bool {
    path.is_absolute()
        && path.parent() == Some(target_directory.join(target).join("debug/deps").as_path())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.strip_prefix("schema_source_conformance-")
                    .is_some_and(|suffix| {
                        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
            })
}

pub(super) fn verify_build(
    build: &Build,
    target_directory: &Path,
    target: &str,
) -> Result<(), Refusal> {
    if build.target != target
        || build.features != ["test-support"]
        || !artifact_path(&build.executable, target, target_directory)
        || file_digest(&build.executable, EXECUTABLE_CAP).map_err(|_error| Refusal::BuildOutput)?
            != build.executable_sha256
    {
        return Err(Refusal::BuildOutput);
    }
    Ok(())
}

pub(super) fn dispatch(test: &str, bytes: &[u8]) -> Result<Dispatch, Refusal> {
    if bytes.len() > super::super::CAP || !TESTS.contains(&test) {
        return Err(Refusal::Dispatch);
    }
    let transcript = from_utf8(bytes).map_err(|_error| Refusal::Dispatch)?;
    let mut lines = transcript.lines().filter(|line| !line.is_empty());
    if lines.next() != Some("running 1 test")
        || lines.next() != Some(format!("test {test} ... ok").as_str())
    {
        return Err(Refusal::Dispatch);
    }
    let summary = lines.next().ok_or(Refusal::Dispatch)?;
    let (filtered, duration) = summary
        .strip_prefix("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ")
        .and_then(|tail| tail.split_once(" filtered out; finished in "))
        .ok_or(Refusal::Dispatch)?;
    let _filtered: u32 = filtered.parse().map_err(|_error| Refusal::Dispatch)?;
    let seconds = duration.strip_suffix('s').ok_or(Refusal::Dispatch)?;
    let (whole, fraction) = seconds.split_once('.').ok_or(Refusal::Dispatch)?;
    let _whole: u32 = whole.parse().map_err(|_error| Refusal::Dispatch)?;
    if fraction.is_empty()
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || lines.next().is_some()
    {
        return Err(Refusal::Dispatch);
    }
    Ok(Dispatch {
        test: test.to_owned(),
        dispatched: 1,
        passed: 1,
        failed: 0,
        ignored: 0,
        stdout: transcript.to_owned(),
        stdout_sha256: sha256(bytes),
    })
}

pub(super) async fn execute(root: &Path, target: &str, target_directory: &Path) -> Execution {
    let supervisor = Supervisor::new();
    let mut execution = Execution::default();
    let cache = match Cache::claim(target_directory) {
        Ok(cache) => CacheGuard(cache),
        Err(error) => {
            return Execution {
                error: Some(error),
                ..execution
            };
        }
    };
    let result = execute_owned(
        &supervisor,
        root,
        target,
        target_directory,
        &cache.0,
        &mut execution,
    )
    .await;
    execution.error = result.err();
    if cache
        .finalize(supervisor.cleanup().await, execution.error.is_some())
        .is_err()
    {
        execution.error = Some(Refusal::Cleanup);
    }
    execution
}

async fn bounded_run(
    supervisor: &Supervisor,
    spec: ProcessSpec,
    cache: &Cache,
    refusal: Refusal,
) -> Result<CommandOutput, Refusal> {
    cache.sample()?;
    let mut command = Box::pin(supervisor.run(spec));
    loop {
        match select(command, Box::pin(sleep(Duration::from_secs(1)))).await {
            Either::Left((result, _sample)) => {
                cache.sample()?;
                return result.map_err(|_error| refusal);
            }
            Either::Right(((), pending)) => {
                command = pending;
                if let Err(error) = cache.sample() {
                    supervisor
                        .cleanup()
                        .await
                        .map_err(|_error| Refusal::Cleanup)?;
                    return Err(error);
                }
            }
        }
    }
}

async fn execute_owned(
    supervisor: &Supervisor,
    root: &Path,
    target: &str,
    target_directory: &Path,
    cache: &Cache,
    execution: &mut Execution,
) -> Result<(), Refusal> {
    supervisor
        .set_deadline(1_200_000)
        .await
        .map_err(|_error| Refusal::Build)?;
    let compiled = bounded_run(
        supervisor,
        build_spec(root, target, target_directory),
        cache,
        Refusal::Build,
    )
    .await?;
    let executable = executable(&compiled.stdout, root, target, target_directory)?;
    let executable_sha256 =
        file_digest(&executable, EXECUTABLE_CAP).map_err(|_error| Refusal::BuildOutput)?;
    execution.build = Some(Build {
        target: target.to_owned(),
        features: vec!["test-support".to_owned()],
        executable: executable.clone(),
        executable_sha256: executable_sha256.clone(),
        cargo_stdout_sha256: sha256(&compiled.stdout),
    });
    for test in TESTS {
        let output = bounded_run(
            supervisor,
            test_spec(root, &executable, test),
            cache,
            Refusal::Command,
        )
        .await?;
        execution
            .operations
            .push(completed_dispatch(test, Ok(output))?);
    }
    if file_digest(&executable, EXECUTABLE_CAP).map_err(|_error| Refusal::BuildOutput)?
        != executable_sha256
    {
        return Err(Refusal::BuildOutput);
    }
    supervisor
        .check_completion()
        .await
        .map_err(|_error| Refusal::Command)
}

fn completed_dispatch(
    test: &str,
    output: ControlResult<CommandOutput>,
) -> Result<Dispatch, Refusal> {
    let completed = output.map_err(|_error| Refusal::Command)?;
    dispatch(test, &completed.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_cli::native_control::HarnessError;
    use serde_json::json;
    use std::env::temp_dir;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use tempfile::tempdir_in;

    #[test]
    fn shared_finalization_quarantines_unproven_cleanup_and_preserves_its_files() {
        for (cleanup_failed, operation_failed) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
            let root = cache_directory(temporary.path());
            let cache = CacheGuard::claim(&root).unwrap();
            let artifact = root.join("retained-artifact");
            fs::write(&artifact, b"preserved").unwrap();
            let cleanup = if cleanup_failed {
                Err(HarnessError::new("cleanup_failed"))
            } else {
                Ok(())
            };
            let result = cache.finalize(cleanup, operation_failed);
            if cleanup_failed {
                assert_eq!(result, Err(GuardError::Cleanup));
                assert_eq!(fs::read(&artifact).unwrap(), b"preserved");
                assert_eq!(fs::read(root.with_extension("lock")).unwrap(), b"!");
                assert!(matches!(CacheGuard::claim(&root), Err(GuardError::Storage)));
                assert!(matches!(
                    ReadGuard::acquire(&root),
                    Err(GuardError::Storage)
                ));
                assert_eq!(fs::read(&artifact).unwrap(), b"preserved");
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(fs::read(root.with_extension("lock")).unwrap(), b"");
                assert_eq!(root.exists(), !operation_failed);
                if !operation_failed {
                    assert_eq!(fs::read(&artifact).unwrap(), b"preserved");
                }
                CacheGuard::claim(&root).expect("proven cleanup leaves a reusable empty lease");
            }
        }
    }

    fn stdout(test: &str) -> String {
        format!(
            "\nrunning 1 test\ntest {test} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 269 filtered out; finished in 0.01s\n\n"
        )
    }

    #[test]
    fn source_conformance_dispatch_requires_exact_nonzero_named_passes() {
        let test = TESTS[0];
        let good = stdout(test);
        let row = dispatch(test, good.as_bytes()).unwrap();
        assert_eq!(
            (row.dispatched, row.passed, row.failed, row.ignored),
            (1, 1, 0, 0)
        );
        for bad in [
            String::new(),
            good.replace("running 1 test", "running 0 tests"),
            good.replace("1 passed", "0 passed"),
            good.replace("0 failed", "1 failed"),
            good.replace("0 ignored", "1 ignored"),
            good.replace("0 measured", "1 measured"),
            good.replace("... ok", "... ignored"),
            good.replace(test, TESTS[1]),
            format!("{good}{good}"),
            format!("{good}unreviewed output\n"),
            good.replace("test result: ok.", "test result: FAILED."),
        ] {
            assert_eq!(
                dispatch(test, bad.as_bytes()),
                Err(Refusal::Dispatch),
                "{bad}"
            );
        }
        assert_eq!(
            dispatch("arbitrary::command", good.as_bytes()),
            Err(Refusal::Dispatch)
        );
    }

    #[test]
    fn source_conformance_failed_process_result_cannot_earn_dispatch_credit() {
        assert_eq!(
            completed_dispatch(TESTS[0], Err(HarnessError::new("child_failed"))),
            Err(Refusal::Command)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn source_conformance_unsafe_scratch_stops_the_supervised_child() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = super::super::scratch::directory(temporary.path());
        let cache = Cache::claim(&root).unwrap();
        let supervisor = Supervisor::new();
        let mut spec = ProcessSpec::new("/bin/sleep");
        spec.args = vec![OsString::from("30")];
        spec.timeout_ms = 5_000;
        spec.output_cap = 1024;
        let unsafe_storage = async {
            sleep(Duration::from_millis(50)).await;
            symlink(temporary.path(), root.join("escape")).unwrap();
        };
        let (result, ()) = futures::join!(
            bounded_run(&supervisor, spec, &cache, Refusal::Build),
            unsafe_storage
        );
        assert!(matches!(result, Err(Refusal::Storage)));
        supervisor.cleanup().await.unwrap();
        cache.discard().unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn source_conformance_build_binding_refuses_missing_changed_and_foreign_artifacts() {
        let directory = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory.path();
        let executable = root.join("native-fixture/debug/deps/schema_source_conformance-abc123");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"synthetic artifact binding fixture").unwrap();
        let mut build = Build {
            target: "native-fixture".to_owned(),
            features: vec!["test-support".to_owned()],
            executable: executable.clone(),
            executable_sha256: file_digest(&executable, EXECUTABLE_CAP).unwrap(),
            cargo_stdout_sha256: "a".repeat(64),
        };
        verify_build(&build, root, "native-fixture").unwrap();
        build.executable = root.join("foreign");
        assert_eq!(
            verify_build(&build, root, "native-fixture"),
            Err(Refusal::BuildOutput)
        );
        build.executable = executable.clone();
        fs::write(&executable, b"changed artifact fixture").unwrap();
        assert_eq!(
            verify_build(&build, root, "native-fixture"),
            Err(Refusal::BuildOutput)
        );
        fs::remove_file(executable).unwrap();
        assert_eq!(
            verify_build(&build, root, "native-fixture"),
            Err(Refusal::BuildOutput)
        );
    }

    fn artifact(root: &Path) -> serde_json::Value {
        json!({
            "reason":"compiler-artifact", "manifest_path":root.join("crates/auctioneer-bidder/Cargo.toml"),
            "target":{"name":TARGET,"kind":["test"],"src_path":root.join("crates/auctioneer-bidder/tests/schema_source_conformance.rs")},
            "profile":{"test":true},"features":["test-support"],
            "executable":root.join("target/native-fixture/debug/deps/schema_source_conformance-abc123")
        })
    }

    fn messages(rows: &[serde_json::Value]) -> Vec<u8> {
        rows.iter()
            .map(|row| serde_json::to_string(row).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes()
    }

    #[test]
    fn source_conformance_build_selects_one_exact_owner_artifact_after_success() {
        let root = Path::new("/fixture/workspace");
        let good = artifact(root);
        let finished = json!({"reason":"build-finished","success":true});
        let target_directory = root.join("target");
        assert_eq!(
            executable(
                &messages(&[good.clone(), finished.clone()]),
                root,
                "native-fixture",
                &target_directory
            )
            .unwrap(),
            target_directory.join("native-fixture/debug/deps/schema_source_conformance-abc123")
        );
        for rows in [
            vec![],
            vec![good.clone()],
            vec![finished.clone()],
            vec![good.clone(), good.clone(), finished.clone()],
            vec![
                good.clone(),
                json!({"reason":"build-finished","success":false}),
            ],
            vec![good.clone(), finished.clone(), good.clone()],
        ] {
            assert_eq!(
                executable(&messages(&rows), root, "native-fixture", &target_directory),
                Err(Refusal::BuildOutput)
            );
        }
        for (field, value) in [
            ("manifest_path", json!("/foreign/Cargo.toml")),
            ("features", json!(["test-support", "insecure-bidders"])),
            ("executable", json!("/foreign/test-binary")),
            (
                "executable",
                json!(
                    target_directory
                        .join("foreign-target/debug/deps/schema_source_conformance-abc123")
                ),
            ),
            (
                "executable",
                json!(target_directory.join("debug/deps/schema_source_conformance-abc123")),
            ),
            ("profile", json!({"test":false})),
            (
                "target",
                json!({"name":TARGET,"kind":["bin"],"src_path":"/foreign/source.rs"}),
            ),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            assert_eq!(
                executable(
                    &messages(&[bad, finished.clone()]),
                    root,
                    "native-fixture",
                    &target_directory
                ),
                Err(Refusal::BuildOutput),
                "{field}"
            );
        }
        let duplicate =
            b"{\"reason\":\"build-finished\",\"reason\":\"build-finished\",\"success\":true}";
        assert_eq!(
            executable(duplicate, root, "native-fixture", &target_directory),
            Err(Refusal::BuildOutput)
        );
    }

    #[test]
    fn source_conformance_build_spec_requires_explicit_compiled_target() {
        let root = Path::new("/fixture/workspace");
        let target = super::super::target().unwrap();
        let spec = build_spec(root, &target, &root.join("target"));
        assert!(
            spec.args
                .windows(2)
                .any(|pair| pair == [OsString::from("--target"), OsString::from(&target)]),
            "the actual build command must select the compiled target recorded in its receipt"
        );
    }

    #[test]
    fn source_conformance_specs_are_fixed_offline_and_environment_bounded() {
        let root = Path::new("/fixture/workspace");
        let spec = build_spec(root, "native-fixture", &root.join("target"));
        assert_eq!(spec.program, PathBuf::from(env!("CARGO")));
        assert_eq!(spec.cwd.as_deref(), Some(root));
        assert_eq!(spec.timeout_ms, 600_000);
        assert_eq!(spec.output_cap, BUILD_CAP);
        assert_eq!(
            spec.args,
            [
                "test",
                "--locked",
                "--offline",
                "--no-default-features",
                "-p",
                "auctioneer-bidder",
                "--features",
                "test-support",
                "--test",
                TARGET,
                "--no-run",
                "--message-format=json",
                "--target",
                "native-fixture",
                "--target-dir",
                "/fixture/workspace/target",
            ]
            .map(OsString::from)
        );
        assert!(!spec.stdin);
        assert_eq!(
            spec.env.get(&OsString::from("CARGO_NET_OFFLINE")),
            Some(&OsString::from("true"))
        );
        assert_eq!(
            spec.env.get(&OsString::from("CARGO_INCREMENTAL")),
            Some(&OsString::from("0"))
        );
        for (name, value) in [
            ("CARGO_PROFILE_DEV_DEBUG", "0"),
            ("CARGO_PROFILE_TEST_DEBUG", "0"),
            ("CARGO_PROFILE_DEV_INCREMENTAL", "false"),
            ("CARGO_PROFILE_TEST_INCREMENTAL", "false"),
            ("CARGO_PROFILE_DEV_STRIP", "none"),
        ] {
            assert_eq!(
                spec.env.get(&OsString::from(name)),
                Some(&OsString::from(value))
            );
        }
        for name in spec.env.keys() {
            assert!(
                [
                    "PATH",
                    "HOME",
                    "CARGO_HOME",
                    "RUSTUP_HOME",
                    "CARGO_TARGET_DIR",
                    "RUSTUP_TOOLCHAIN",
                    "TMPDIR",
                    "LC_ALL",
                    "LANG",
                    "CARGO_NET_OFFLINE",
                    "CARGO_TERM_COLOR",
                    "CARGO_BUILD_JOBS",
                    "CARGO_ENCODED_RUSTFLAGS",
                    "CARGO_INCREMENTAL",
                    "CARGO_PROFILE_DEV_DEBUG",
                    "CARGO_PROFILE_TEST_DEBUG",
                    "CARGO_PROFILE_DEV_INCREMENTAL",
                    "CARGO_PROFILE_TEST_INCREMENTAL",
                    "CARGO_PROFILE_DEV_STRIP",
                ]
                .map(OsString::from)
                .contains(name)
            );
        }
        let executable = Path::new("/fixture/test-binary");
        for test in TESTS {
            let validation = test_spec(root, executable, test);
            assert_eq!(validation.program, executable);
            assert_eq!(
                validation.env.get(&OsString::from("CARGO_MANIFEST_DIR")),
                Some(&root.join("crates/auctioneer-bidder").into_os_string()),
                "runtime expansion must resolve the owning bidder target, never the CLI manifest"
            );
            assert_eq!(
                validation.args,
                [
                    "--exact",
                    test,
                    "--test-threads=1",
                    "--color=never",
                    "--format=pretty"
                ]
                .map(OsString::from)
            );
            assert_eq!(validation.timeout_ms, 120_000);
            assert_eq!(validation.output_cap, TEST_CAP);
        }
    }
}
