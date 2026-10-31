//! Closed M7 native test plans. No caller-supplied executable, filter or feature set.

use super::Refusal;
use crate::bidders::evidence::conformance::runner as controls;
use auctioneer_cli::native_control::io::{file_digest, parse_json, sha256};
use auctioneer_cli::native_control::process::{CommandOutput, ProcessSpec, Supervisor};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::from_utf8;

const BUILD_CAP: usize = 16 << 20;
const TEST_CAP: usize = 1 << 20;
const EXECUTABLE_CAP: usize = 512 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Plan {
    OnboardingSecure,
    OnboardingInsecure,
    SchemaResources,
    NativeHeadroom,
}

pub(super) const PLANS: [Plan; 4] = [
    Plan::OnboardingSecure,
    Plan::OnboardingInsecure,
    Plan::SchemaResources,
    Plan::NativeHeadroom,
];

struct Selection {
    package: &'static str,
    target: &'static str,
    source: &'static str,
    kind: &'static str,
    requested_features: &'static str,
    compiled_features: &'static [&'static str],
    test: &'static str,
}

impl Plan {
    #[cfg(test)]
    pub(super) fn test_name(self) -> &'static str {
        self.selection().test
    }

    fn selection(self) -> Selection {
        match self {
            Self::OnboardingSecure | Self::OnboardingInsecure => Selection {
                package: "auctioneer-core",
                target: "native_bidder_onboarding",
                source: "tests/native_bidder_onboarding.rs",
                kind: "test",
                requested_features: if self == Self::OnboardingSecure {
                    "conformance-suite"
                } else {
                    "conformance-suite,insecure-bidders"
                },
                compiled_features: if self == Self::OnboardingSecure {
                    &["conformance-suite", "test-support"]
                } else {
                    &["conformance-suite", "insecure-bidders", "test-support"]
                },
                test: "native_bidder_onboarding::cases::all_frozen_native_flows_use_generated_app_dispatch_and_fresh_activation_snapshots",
            },
            Self::SchemaResources => Selection {
                package: "auctioneer-bidder",
                target: "native_schema_resources",
                source: "tests/native_schema_resources.rs",
                kind: "test",
                requested_features: "test-support",
                compiled_features: &["test-support"],
                test: "tests::cold_native_schema_reservations_cover_discovered_bindings_and_fresh_evaluations",
            },
            Self::NativeHeadroom => Selection {
                package: "auctioneer-adapter-axum",
                target: "auctioneer_adapter_axum",
                source: "src/lib.rs",
                kind: "lib",
                requested_features: "",
                compiled_features: &[],
                test: "envelope::tests::shipped_baked_budget_leaves_256_mib_headroom",
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Build {
    pub plan: Plan,
    pub target: String,
    pub features: Vec<String>,
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub cargo_stdout_sha256: String,
}

#[cfg(test)]
pub(super) fn build_fixture(plan: Plan, directory: &Path, target: &str) -> Build {
    Build {
        plan,
        target: target.to_owned(),
        features: plan
            .selection()
            .compiled_features
            .iter()
            .map(|feature| (*feature).to_owned())
            .collect(),
        executable: directory
            .join(target)
            .join("debug/deps")
            .join(format!("{}-abc123", plan.selection().target)),
        executable_sha256: "a".repeat(64),
        cargo_stdout_sha256: "b".repeat(64),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Dispatch {
    pub plan: Plan,
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
    pub builds: Vec<Build>,
    pub operations: Vec<Dispatch>,
    pub error: Option<Refusal>,
}

fn build_spec(root: &Path, target: &str, directory: &Path, plan: Plan) -> ProcessSpec {
    let selection = plan.selection();
    let mut spec = ProcessSpec::new(env!("CARGO"));
    spec.cwd = Some(root.to_owned());
    spec.env = controls::bounded_environment();
    spec.args = [
        "test",
        "--locked",
        "--offline",
        "--no-default-features",
        "-p",
        selection.package,
    ]
    .map(OsString::from)
    .to_vec();
    if !selection.requested_features.is_empty() {
        spec.args.extend([
            OsString::from("--features"),
            OsString::from(selection.requested_features),
        ]);
    }
    if selection.kind == "lib" {
        spec.args.push(OsString::from("--lib"));
    } else {
        spec.args
            .extend([OsString::from("--test"), OsString::from(selection.target)]);
    }
    spec.args.extend(
        [
            "--no-run",
            "--message-format=json",
            "--target",
            target,
            "--target-dir",
        ]
        .map(OsString::from),
    );
    spec.args.push(directory.as_os_str().to_owned());
    spec.timeout_ms = 600_000;
    spec.output_cap = BUILD_CAP;
    spec
}

fn test_spec(root: &Path, executable: &Path, plan: Plan) -> ProcessSpec {
    let selection = plan.selection();
    let mut spec = ProcessSpec::new(executable);
    spec.cwd = Some(root.to_owned());
    spec.env = controls::bounded_environment();
    spec.env.insert(
        OsString::from("CARGO_MANIFEST_DIR"),
        root.join("crates").join(selection.package).into_os_string(),
    );
    spec.args = [
        "--exact",
        selection.test,
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

fn artifact_path(path: &Path, target: &str, directory: &Path, plan: Plan) -> bool {
    path.is_absolute()
        && path.parent() == Some(directory.join(target).join("debug/deps").as_path())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.strip_prefix(plan.selection().target)
                    .and_then(|suffix| suffix.strip_prefix('-'))
                    .is_some_and(|suffix| {
                        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
            })
}

fn executable(
    bytes: &[u8],
    root: &Path,
    target: &str,
    directory: &Path,
    plan: Plan,
) -> Result<PathBuf, Refusal> {
    if bytes.len() > BUILD_CAP {
        return Err(Refusal::BuildOutput);
    }
    let selection = plan.selection();
    let mut found = None;
    let mut finished = false;
    for (index, line) in from_utf8(bytes)
        .map_err(|_error| Refusal::BuildOutput)?
        .lines()
        .enumerate()
    {
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
                .is_some_and(|row| row.name == selection.target)
        {
            let row = message.target.ok_or(Refusal::BuildOutput)?;
            if message.manifest_path
                != Some(
                    root.join("crates")
                        .join(selection.package)
                        .join("Cargo.toml"),
                )
                || row.kind != [selection.kind]
                || row.src_path
                    != root
                        .join("crates")
                        .join(selection.package)
                        .join(selection.source)
                || message.features != selection.compiled_features
            {
                return Err(Refusal::BuildOutput);
            }
            // Cargo may also report the owner's ordinary library. Only the
            // test-profile executable can supply the retained lib-test artifact.
            if selection.kind == "lib"
                && message
                    .profile
                    .as_ref()
                    .is_some_and(|profile| !profile.test)
                && message.executable.is_none()
            {
                continue;
            }
            if found.is_some() || !message.profile.is_some_and(|profile| profile.test) {
                return Err(Refusal::BuildOutput);
            }
            let path = message.executable.ok_or(Refusal::BuildOutput)?;
            if !artifact_path(&path, target, directory, plan) {
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

pub(super) fn verify_build_metadata(
    build: &Build,
    directory: &Path,
    target: &str,
    plan: Plan,
) -> Result<(), Refusal> {
    if build.plan != plan
        || build.target != target
        || build.features != plan.selection().compiled_features
        || !super::digest(&build.executable_sha256)
        || !super::digest(&build.cargo_stdout_sha256)
        || !artifact_path(&build.executable, target, directory, plan)
    {
        return Err(Refusal::BuildOutput);
    }
    Ok(())
}

pub(super) fn verify_build(
    build: &Build,
    directory: &Path,
    target: &str,
    plan: Plan,
) -> Result<(), Refusal> {
    verify_build_metadata(build, directory, target, plan)?;
    if file_digest(&build.executable, EXECUTABLE_CAP).map_err(|_error| Refusal::BuildOutput)?
        != build.executable_sha256
    {
        return Err(Refusal::BuildOutput);
    }
    Ok(())
}

pub(super) fn dispatch(plan: Plan, bytes: &[u8]) -> Result<Dispatch, Refusal> {
    if bytes.len() > TEST_CAP {
        return Err(Refusal::Dispatch);
    }
    let transcript = from_utf8(bytes).map_err(|_error| Refusal::Dispatch)?;
    let mut lines = transcript.lines().filter(|line| !line.is_empty());
    if lines.next() != Some("running 1 test")
        || lines.next() != Some(format!("test {} ... ok", plan.selection().test).as_str())
    {
        return Err(Refusal::Dispatch);
    }
    let (filtered, duration) = lines
        .next()
        .and_then(|summary| {
            let tail = summary
                .strip_prefix("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ")?;
            tail.split_once(" filtered out; finished in ")
        })
        .ok_or(Refusal::Dispatch)?;
    let _filtered: u32 = filtered.parse().map_err(|_error| Refusal::Dispatch)?;
    let (whole, fraction) = duration
        .strip_suffix('s')
        .and_then(|seconds| seconds.split_once('.'))
        .ok_or(Refusal::Dispatch)?;
    let _whole: u32 = whole.parse().map_err(|_error| Refusal::Dispatch)?;
    if fraction.is_empty()
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || lines.next().is_some()
    {
        return Err(Refusal::Dispatch);
    }
    Ok(Dispatch {
        plan,
        test: plan.selection().test.to_owned(),
        dispatched: 1,
        passed: 1,
        failed: 0,
        ignored: 0,
        stdout: transcript.to_owned(),
        stdout_sha256: sha256(bytes),
    })
}

fn completed(
    plan: Plan,
    result: Result<CommandOutput, controls::GuardError>,
) -> Result<Dispatch, Refusal> {
    let output = result.map_err(Refusal::from)?;
    if !output.summary.closed {
        return Err(Refusal::Command);
    }
    dispatch(plan, &output.stdout)
}

pub(super) async fn execute(root: &Path, target: &str, directory: &Path) -> Execution {
    let supervisor = Supervisor::new();
    let mut execution = Execution::default();
    let cache = match controls::CacheGuard::claim(directory) {
        Ok(cache) => cache,
        Err(error) => {
            return Execution {
                error: Some(error.into()),
                ..execution
            };
        }
    };
    execution.error = execute_owned(&supervisor, &cache, root, target, directory, &mut execution)
        .await
        .err();
    if cache
        .finalize(supervisor.cleanup().await, execution.error.is_some())
        .is_err()
    {
        execution.error = Some(Refusal::Cleanup);
    }
    execution
}

async fn execute_owned(
    supervisor: &Supervisor,
    cache: &controls::CacheGuard,
    root: &Path,
    target: &str,
    directory: &Path,
    execution: &mut Execution,
) -> Result<(), Refusal> {
    supervisor
        .set_deadline(1_200_000)
        .await
        .map_err(|_error| Refusal::Build)?;
    for plan in PLANS {
        let output = controls::supervised_process(
            supervisor,
            build_spec(root, target, directory, plan),
            cache,
        )
        .await
        .map_err(|error| {
            if error == controls::GuardError::Command {
                Refusal::Build
            } else {
                error.into()
            }
        })?;
        if !output.summary.closed {
            return Err(Refusal::Build);
        }
        let executable = executable(&output.stdout, root, target, directory, plan)?;
        let build = Build {
            plan,
            target: target.to_owned(),
            features: plan
                .selection()
                .compiled_features
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
            executable_sha256: file_digest(&executable, EXECUTABLE_CAP)
                .map_err(|_error| Refusal::BuildOutput)?,
            executable: executable.clone(),
            cargo_stdout_sha256: sha256(&output.stdout),
        };
        execution.builds.push(build);
        let test_output =
            controls::supervised_process(supervisor, test_spec(root, &executable, plan), cache)
                .await;
        execution.operations.push(completed(plan, test_output)?);
    }
    for (build, plan) in execution.builds.iter().zip(PLANS) {
        verify_build(build, directory, target, plan)?;
    }
    supervisor
        .check_completion()
        .await
        .map_err(|_error| Refusal::Command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_cli::native_control::process::ProcessSummary;
    use serde_json::json;
    use std::env::temp_dir;
    use std::fs;
    use tempfile::tempdir_in;

    #[test]
    fn retained_artifact_verification_is_readonly_and_refuses_every_changed_build_guard() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let directory = temporary.path();
        for plan in PLANS {
            // These regular byte fixtures are never executed or published as evidence.
            let mut good = build_fixture(plan, directory, "native-fixture");
            fs::create_dir_all(good.executable.parent().unwrap()).unwrap();
            fs::write(&good.executable, b"readonly artifact fixture").unwrap();
            good.executable_sha256 = file_digest(&good.executable, EXECUTABLE_CAP).unwrap();
            verify_build(&good, directory, "native-fixture", plan).unwrap();
            for (field, value) in [
                ("target", json!("foreign-target")),
                ("features", json!(["foreign-feature"])),
                ("executable", json!(directory.join("foreign-artifact"))),
                ("executable_sha256", json!("0".repeat(64))),
                ("cargo_stdout_sha256", json!("not-a-digest")),
            ] {
                let mut mutated_json = serde_json::to_value(&good).unwrap();
                mutated_json[field] = value;
                let bad: Build = serde_json::from_value(mutated_json).unwrap();
                assert_eq!(
                    verify_build(&bad, directory, "native-fixture", plan),
                    Err(Refusal::BuildOutput),
                    "{field}"
                );
            }
            assert_eq!(
                fs::read(&good.executable).unwrap(),
                b"readonly artifact fixture"
            );
            fs::write(&good.executable, b"changed artifact").unwrap();
            assert_eq!(
                verify_build(&good, directory, "native-fixture", plan),
                Err(Refusal::BuildOutput)
            );
            fs::remove_file(&good.executable).unwrap();
            assert_eq!(
                verify_build(&good, directory, "native-fixture", plan),
                Err(Refusal::BuildOutput)
            );
            assert!(
                !good.executable.exists(),
                "verification must not repair a missing artifact"
            );
        }
    }

    #[test]
    fn reused_cache_leases_exclude_builds_and_readonly_consumers_cannot_materialize_storage() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let directory = controls::cache_directory(temporary.path());
        assert!(matches!(
            controls::ReadGuard::acquire(&directory),
            Err(controls::GuardError::Storage)
        ));
        assert!(!directory.exists());
        let producer = controls::CacheGuard::claim(&directory).unwrap();
        fs::write(directory.join("retained"), b"readonly").unwrap();
        assert!(matches!(
            controls::ReadGuard::acquire(&directory),
            Err(controls::GuardError::Busy)
        ));
        drop(producer);
        let consumer = controls::ReadGuard::acquire(&directory).unwrap();
        assert!(matches!(
            controls::CacheGuard::claim(&directory),
            Err(controls::GuardError::Busy)
        ));
        assert_eq!(fs::read(directory.join("retained")).unwrap(), b"readonly");
        drop(consumer);
        controls::CacheGuard::claim(&directory)
            .unwrap()
            .discard()
            .unwrap();
        assert!(!directory.exists());
    }

    pub(super) fn transcript(plan: Plan) -> String {
        format!(
            "\nrunning 1 test\ntest {} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.01s\n",
            plan.selection().test
        )
    }

    #[test]
    fn every_plan_requires_one_exact_nonignored_success_without_unreviewed_output() {
        for plan in PLANS {
            let good = transcript(plan);
            assert_eq!(dispatch(plan, good.as_bytes()).unwrap().passed, 1);
            for bad in [
                String::new(),
                good.replace("running 1 test", "running 0 tests"),
                good.replace("1 passed", "0 passed"),
                good.replace("0 failed", "1 failed"),
                good.replace("0 ignored", "1 ignored"),
                good.replace("0 measured", "1 measured"),
                good.replace("... ok", "... ignored"),
                good.replace(plan.selection().test, "foreign::test"),
                format!("{good}{good}"),
                format!("{good}unreviewed output\n"),
                good.replace("test result: ok.", "test result: FAILED."),
                good.replace("0.01s", "NaNs"),
            ] {
                assert_eq!(
                    dispatch(plan, bad.as_bytes()),
                    Err(Refusal::Dispatch),
                    "{bad}"
                );
            }
        }
    }

    #[test]
    fn successful_looking_stdout_cannot_override_failed_or_unreaped_processes() {
        for plan in PLANS {
            assert_eq!(
                completed(plan, Err(controls::GuardError::Command)),
                Err(Refusal::Command)
            );
            let output = CommandOutput {
                stdout: transcript(plan).into_bytes(),
                summary: ProcessSummary {
                    output_bytes: 1,
                    output_sha256: "a".repeat(64),
                    closed: false,
                },
            };
            assert_eq!(completed(plan, Ok(output)), Err(Refusal::Command));
        }
    }

    fn artifact(root: &Path, directory: &Path, plan: Plan) -> serde_json::Value {
        let selection = plan.selection();
        json!({"reason":"compiler-artifact", "manifest_path":root.join("crates").join(selection.package).join("Cargo.toml"),
            "target":{"name":selection.target,"kind":[selection.kind],"src_path":root.join("crates").join(selection.package).join(selection.source)},
            "profile":{"test":true},"features":selection.compiled_features,
            "executable":directory.join("native-fixture/debug/deps").join(format!("{}-abc123", selection.target))})
    }

    fn messages(rows: &[serde_json::Value]) -> Vec<u8> {
        rows.iter()
            .map(|row| serde_json::to_string(row).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes()
    }

    #[test]
    fn every_build_requires_its_exact_owner_features_target_and_successful_cargo_finish() {
        let root = Path::new("/fixture/workspace");
        let directory = root.join("target");
        for plan in PLANS {
            let good = artifact(root, &directory, plan);
            let finished = json!({"reason":"build-finished","success":true});
            executable(
                &messages(&[good.clone(), finished.clone()]),
                root,
                "native-fixture",
                &directory,
                plan,
            )
            .unwrap();
            if plan == Plan::NativeHeadroom {
                let mut ordinary = good.clone();
                ordinary["profile"] = json!({"test":false});
                ordinary["executable"] = serde_json::Value::Null;
                executable(
                    &messages(&[ordinary.clone(), good.clone(), finished.clone()]),
                    root,
                    "native-fixture",
                    &directory,
                    plan,
                )
                .unwrap();
                assert_eq!(
                    executable(
                        &messages(&[ordinary, finished.clone()]),
                        root,
                        "native-fixture",
                        &directory,
                        plan
                    ),
                    Err(Refusal::BuildOutput)
                );
            }
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
                    executable(&messages(&rows), root, "native-fixture", &directory, plan),
                    Err(Refusal::BuildOutput)
                );
            }
            for (field, value) in [
                ("manifest_path", json!("/foreign/Cargo.toml")),
                ("features", json!(["unreviewed-feature"])),
                ("profile", json!({"test":false})),
                ("executable", json!("/foreign/test-binary")),
                (
                    "target",
                    json!({"name":plan.selection().target,"kind":["bin"],"src_path":"/foreign/source.rs"}),
                ),
            ] {
                let mut bad = good.clone();
                bad[field] = value;
                assert_eq!(
                    executable(
                        &messages(&[bad, finished.clone()]),
                        root,
                        "native-fixture",
                        &directory,
                        plan
                    ),
                    Err(Refusal::BuildOutput),
                    "{field}"
                );
            }
        }
        assert_eq!(
            executable(
                b"{\"reason\":\"build-finished\",\"reason\":\"build-finished\",\"success\":true}",
                root,
                "native-fixture",
                &directory,
                Plan::SchemaResources
            ),
            Err(Refusal::BuildOutput)
        );
    }

    #[test]
    fn fixed_specs_select_real_targets_and_only_reviewed_test_filters() {
        let root = Path::new("/fixture/workspace");
        let directory = root.join("target");
        for plan in PLANS {
            let build = build_spec(root, "native-fixture", &directory, plan);
            assert_eq!(build.program, PathBuf::from(env!("CARGO")));
            assert!(build.args.contains(&OsString::from("--offline")));
            assert!(build.args.contains(&OsString::from("--locked")));
            assert!(
                build
                    .args
                    .windows(2)
                    .any(|pair| pair == ["--target", "native-fixture"].map(OsString::from))
            );
            assert_eq!(
                (build.timeout_ms, build.output_cap, build.stdin),
                (600_000, BUILD_CAP, false)
            );
            let test = test_spec(root, Path::new("/fixture/binary"), plan);
            assert_eq!(
                test.args,
                [
                    "--exact",
                    plan.selection().test,
                    "--test-threads=1",
                    "--color=never",
                    "--format=pretty"
                ]
                .map(OsString::from)
            );
            assert_eq!(
                (test.timeout_ms, test.output_cap, test.stdin),
                (120_000, TEST_CAP, false)
            );
            assert_eq!(
                test.env.get(&OsString::from("CARGO_MANIFEST_DIR")),
                Some(
                    &root
                        .join("crates")
                        .join(plan.selection().package)
                        .into_os_string()
                )
            );
            assert!(
                test.env
                    .keys()
                    .all(|key| key.as_os_str() == "CARGO_MANIFEST_DIR"
                        || controls::bounded_environment().contains_key(key))
            );
        }
        let secure = build_spec(root, "native-fixture", &directory, Plan::OnboardingSecure);
        let insecure = build_spec(root, "native-fixture", &directory, Plan::OnboardingInsecure);
        assert!(secure.args.contains(&OsString::from("conformance-suite")));
        assert!(
            insecure
                .args
                .contains(&OsString::from("conformance-suite,insecure-bidders"))
        );
        assert!(
            build_spec(root, "native-fixture", &directory, Plan::NativeHeadroom)
                .args
                .contains(&OsString::from("--lib"))
        );
    }
}
