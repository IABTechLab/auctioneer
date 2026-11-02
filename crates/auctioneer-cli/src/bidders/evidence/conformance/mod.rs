//! Source/compiler-expansion checks only. No schema or serving qualification.

pub(super) mod runner;
mod scratch;

use self::scratch::ReadLease;
use super::{
    BUILT, CAP, Error, Failure, Manifest, Outcome, Producer, check, current, files, load,
    run_members,
};
use auctioneer_cli::BuildIdentity;
use auctioneer_cli::native_control::io::{canonical_digest, parse_json};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env::var_os;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const PRODUCER: &str = "selected_schema_source_conformance";
const OPERATION: &str = "selected_schema_source_tests";
const OPERATION_COUNT: u8 = 2;

pub(super) fn requirement() -> Producer {
    Producer {
        id: PRODUCER.to_owned(),
        operation: OPERATION.to_owned(),
        operation_count: OPERATION_COUNT,
    }
}

pub(super) fn build_binding(root: &Path) -> Result<(PathBuf, String), Error> {
    let selected = var_os("CARGO_TARGET_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let target_directory = managed_target(root, selected)?;
    let mut environment = BTreeMap::new();
    for (name, value) in runner::test_environment(root) {
        environment.insert(
            name.into_string().map_err(|_error| Failure::Binding)?,
            value.into_string().map_err(|_error| Failure::Binding)?,
        );
    }
    let environment_sha256 = canonical_digest(&environment).map_err(|_error| Failure::Binding)?;
    Ok((target_directory, environment_sha256))
}

fn managed_target(root: &Path, configured: Option<PathBuf>) -> Result<PathBuf, Error> {
    // One bounded child cache, not the invoking Cargo process's target lock.
    let selected = configured.unwrap_or_else(|| root.join("target"));
    let absolute = if selected.is_absolute() {
        selected
    } else {
        root.join(selected)
    };
    fs::create_dir_all(&absolute).map_err(|_error| Failure::Binding)?;
    let directory = fs::canonicalize(absolute).map_err(|_error| Failure::Binding)?;
    if !directory.is_dir() {
        return Err(Failure::Binding.into());
    }
    Ok(scratch::directory(&directory))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Refusal {
    ReferenceIntegrityFailed,
    Build,
    BuildOutput,
    Command,
    Dispatch,
    Cleanup,
    Storage,
    BuildBusy,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DispatchUnit {
    RustTestCases,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Qualification {
    Unqualified,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u8,
    run_id: String,
    manifest_sha256: String,
    producer: String,
    operation: String,
    operation_count: u8,
    dispatch_unit: DispatchUnit,
    qualification: Qualification,
    promotion_allowed: bool,
    outcome: Outcome,
    error: Option<Refusal>,
    build: Option<runner::Build>,
    operations: Vec<runner::Dispatch>,
}

fn target() -> Result<String, Error> {
    let built: BuildIdentity = parse_json(BUILT, CAP).map_err(|_error| Failure::Binding)?;
    Ok(built.compiled_target)
}

pub(super) async fn produce(
    source: &Path,
    inventory: &Path,
    root: &Path,
    id: &str,
) -> Result<(), Error> {
    let (manifest, context) = load(inventory, root, id)?;
    run_members(root, &["manifest.json"])?;
    let workspace = fs::canonicalize(super::ROOT).map_err(|_error| Failure::Binding)?;
    let target = target()?;
    let slot = root.join(PRODUCER);
    files::fresh_dir(&slot)?;
    let reference = check::run_bytes(source, &context.inventory, false, None, None).await;
    let execution = if reference.is_ok_and(|report| report.exit_code() == 0_i32) {
        runner::execute(
            &workspace,
            &target,
            &context.binding.evidence_target_directory,
        )
        .await
    } else {
        runner::Execution {
            error: Some(Refusal::ReferenceIntegrityFailed),
            ..runner::Execution::default()
        }
    };
    // A stopped, failed or skipped child can never inherit an older receipt.
    // Preserve only operations whose exact successful dispatch was observed.
    let operation_count =
        u8::try_from(execution.operations.len()).map_err(|_error| Failure::Membership)?;
    let success = execution.error.is_none()
        && operation_count == OPERATION_COUNT
        && execution.build.is_some();
    if current(inventory)?.binding != context.binding {
        return Err(Failure::Binding.into());
    }
    let (after, _context) = load(inventory, root, id)?;
    if manifest != after {
        return Err(Failure::Binding.into());
    }
    run_members(root, &["manifest.json", PRODUCER])?;
    files::members(&slot, &[])?;
    let receipt = Receipt {
        schema_version: super::VERSION,
        run_id: id.to_owned(),
        manifest_sha256: canonical_digest(&manifest).map_err(|_error| Failure::Input)?,
        producer: PRODUCER.to_owned(),
        operation: OPERATION.to_owned(),
        operation_count,
        dispatch_unit: DispatchUnit::RustTestCases,
        qualification: Qualification::Unqualified,
        promotion_allowed: false,
        outcome: if success {
            Outcome::Completed
        } else {
            Outcome::Failed
        },
        error: execution.error,
        build: execution.build,
        operations: execution.operations,
    };
    files::publish(&slot.join("receipt.json"), &receipt)?;
    if !success {
        return Err(Failure::OperationFailed.into());
    }
    Ok(())
}

fn verify_receipt(
    receipt: &Receipt,
    manifest: &Manifest,
    id: &str,
    target: &str,
) -> Result<(), Error> {
    if receipt.run_id != id {
        return Err(Failure::RunMismatch.into());
    }
    if receipt.schema_version != super::VERSION
        || receipt.producer != PRODUCER
        || receipt.operation != OPERATION
        || receipt.promotion_allowed
    {
        return Err(Failure::Membership.into());
    }
    if receipt.manifest_sha256 != canonical_digest(manifest).map_err(|_error| Failure::Input)? {
        return Err(Failure::Binding.into());
    }
    if receipt.outcome != Outcome::Completed || receipt.error.is_some() {
        return Err(Failure::OperationFailed.into());
    }
    if receipt.operation_count != OPERATION_COUNT || receipt.operations.len() != runner::TESTS.len()
    {
        return Err(Failure::Membership.into());
    }
    let build = receipt.build.as_ref().ok_or(Failure::Binding)?;
    if build.target != target
        || build.features != ["test-support"]
        || !digest(&build.executable_sha256)
        || !digest(&build.cargo_stdout_sha256)
    {
        return Err(Failure::Binding.into());
    }
    for (row, expected) in receipt.operations.iter().zip(runner::TESTS) {
        let observed = runner::dispatch(expected, row.stdout.as_bytes())
            .map_err(|_error| Failure::OperationFailed)?;
        if *row != observed {
            return Err(Failure::Membership.into());
        }
    }
    Ok(())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

async fn validate(source: &Path, inventory: &Path, root: &Path, id: &str) -> Result<(), Error> {
    let (manifest, context) = load(inventory, root, id)?;
    run_members(root, &["manifest.json", PRODUCER])?;
    let slot = root.join(PRODUCER);
    files::members(&slot, &["receipt.json"])?;
    let receipt: Receipt = parse_json(&files::read(&slot.join("receipt.json"))?, CAP)
        .map_err(|_error| Failure::Input)?;
    let target = target()?;
    verify_receipt(&receipt, &manifest, id, &target)?;
    // Retain the existing cache through artifact inspection, source awaits and
    // the final binding check; consumption must never create or discard it.
    let _lease = ReadLease::acquire(&manifest.inputs.evidence_target_directory)
        .map_err(|_error| Failure::Binding)?;
    runner::verify_build(
        receipt.build.as_ref().ok_or(Failure::Binding)?,
        &manifest.inputs.evidence_target_directory,
        &target,
    )
    .map_err(|_error| Failure::Binding)?;
    // Validate current source independently of the reference producer's receipt,
    // but never hide a failed or malformed producer behind a prerequisite error.
    let report = check::run_bytes(source, &context.inventory, false, None, None).await?;
    if report.exit_code() != 0_i32 {
        return Err(Failure::OperationFailed.into());
    }
    if current(inventory)?.binding != context.binding {
        return Err(Failure::Binding.into());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Complete,
    Incomplete,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum UnavailableStatus {
    Unavailable,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum UnavailableProducer {
    #[serde(rename = "selected_schema_semantics")]
    Semantics,
    #[serde(rename = "selected_schema_artifacts")]
    Artifacts,
    #[serde(rename = "selected_schema_fixture_counts")]
    FixtureCounts,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum UnavailableReason {
    DetailedSemanticReceiptContractMissing,
    LinkedArtifactReceiptContractMissing,
    InternalFixtureCountsNotExported,
}

#[derive(Clone, Copy, Serialize)]
struct Unavailable {
    producer: UnavailableProducer,
    status: UnavailableStatus,
    reason: UnavailableReason,
}

#[derive(Serialize)]
pub(super) struct Report {
    producer: &'static str,
    status: Status,
    error: Option<&'static str>,
    unavailable: [Unavailable; 3],
}

impl Report {
    pub(super) fn complete(&self) -> bool {
        matches!(self.status, Status::Complete) && self.error.is_none()
    }
}

pub(super) async fn consume(source: &Path, inventory: &Path, root: &Path, id: &str) -> Report {
    let error = validate(source, inventory, root, id).await.err();
    Report {
        producer: PRODUCER,
        status: if error.is_none() {
            Status::Complete
        } else {
            Status::Incomplete
        },
        error: error.map(Error::code),
        unavailable: [
            Unavailable {
                producer: UnavailableProducer::Semantics,
                status: UnavailableStatus::Unavailable,
                reason: UnavailableReason::DetailedSemanticReceiptContractMissing,
            },
            Unavailable {
                producer: UnavailableProducer::Artifacts,
                status: UnavailableStatus::Unavailable,
                reason: UnavailableReason::LinkedArtifactReceiptContractMissing,
            },
            Unavailable {
                producer: UnavailableProducer::FixtureCounts,
                status: UnavailableStatus::Unavailable,
                reason: UnavailableReason::InternalFixtureCountsNotExported,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_cli::native_control::io::sha256;
    use serde_json::{Value, json};
    use std::env::temp_dir;
    use tempfile::tempdir_in;

    #[test]
    fn source_conformance_fresh_managed_target_directory_is_supported() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = temporary.path();
        for (selected, parent) in [
            (None, root.join("target")),
            (
                Some(PathBuf::from("relative/nested")),
                root.join("relative/nested"),
            ),
            (
                Some(root.join("absolute/nested")),
                root.join("absolute/nested"),
            ),
        ] {
            assert!(!parent.exists());
            let directory = managed_target(root, selected.clone()).unwrap();
            assert_eq!(directory, parent.join("bidder-evidence-conformance-v2"));
            assert!(parent.is_dir());
            assert!(
                !directory.exists(),
                "binding must not materialize the child cache"
            );
            assert_eq!(managed_target(root, selected).unwrap(), directory);
        }
    }

    #[test]
    fn source_conformance_managed_target_refuses_non_directory_inputs() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = temporary.path();
        let file = root.join("not-a-directory");
        fs::write(&file, b"unchanged").unwrap();
        for selected in [file.clone(), file.join("nested")] {
            assert_eq!(
                managed_target(root, Some(selected)),
                Err(Failure::Binding.into())
            );
        }
        assert_eq!(fs::read(file).unwrap(), b"unchanged");
    }

    fn fixture() -> (Manifest, Receipt) {
        let manifest = Manifest {
            schema_version: super::super::VERSION,
            scope: "controlled_native_acceptance".to_owned(),
            run_id: "fixture-run".to_owned(),
            requirements: super::super::requirements(),
            inputs: super::super::Binding {
                build_identity_sha256: "b".repeat(64),
                executable_sha256: "c".repeat(64),
                workspace_sha256: "d".repeat(64),
                lock_sha256: "e".repeat(64),
                features: vec!["application".to_owned()],
                source_revision: super::super::REVISION.to_owned(),
                inventory_sha256: "f".repeat(64),
                evidence_target_directory: "/fixture/target".into(),
                evidence_tool_environment_sha256: "a".repeat(64),
            },
        };
        // Synthetic transcripts test the verifier only; they are never
        // published or presented as execution/qualification evidence.
        let operations = [
            "tests::schema_selection_source_and_expanded_generated_membership_manifest",
            "tests::schema_selection_manifest_is_order_independent_and_detects_omissions",
        ].into_iter().map(|test| {
            let stdout = format!("\nrunning 1 test\ntest {test} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 269 filtered out; finished in 0.01s\n\n");
            runner::Dispatch {
                test: test.to_owned(),
                dispatched: 1,
                passed: 1,
                failed: 0,
                ignored: 0,
                stdout_sha256: sha256(stdout.as_bytes()),
                stdout,
            }
        }).collect();
        let receipt = Receipt {
            schema_version: 3,
            run_id: "fixture-run".to_owned(),
            manifest_sha256: canonical_digest(&manifest).unwrap(),
            producer: "selected_schema_source_conformance".to_owned(),
            operation: "selected_schema_source_tests".to_owned(),
            operation_count: 2,
            dispatch_unit: DispatchUnit::RustTestCases,
            qualification: Qualification::Unqualified,
            promotion_allowed: false,
            outcome: Outcome::Completed,
            error: None,
            build: Some(runner::Build {
                target: "native-fixture".to_owned(),
                features: vec!["test-support".to_owned()],
                executable:
                    "/fixture/target/native-fixture/debug/deps/schema_source_conformance-abc123"
                        .into(),
                executable_sha256: "a".repeat(64),
                cargo_stdout_sha256: "b".repeat(64),
            }),
            operations,
        };
        (manifest, receipt)
    }

    fn verify(value: &Value, manifest: &Manifest) -> Result<(), Error> {
        let bytes = serde_json::to_vec(value).map_err(|_error| Failure::Input)?;
        let receipt: Receipt = parse_json(&bytes, CAP).map_err(|_error| Failure::Input)?;
        verify_receipt(&receipt, manifest, "fixture-run", "native-fixture")
    }

    #[test]
    fn source_conformance_receipt_requires_run_binding_and_exact_operation_membership() {
        let (manifest, receipt) = fixture();
        let original = serde_json::to_value(receipt).unwrap();
        verify(&original, &manifest).unwrap();
        for (field, replacement, expected) in [
            ("run_id", json!("earlier-run"), Failure::RunMismatch),
            ("schema_version", json!(1_u8), Failure::Membership),
            ("manifest_sha256", json!("0".repeat(64)), Failure::Binding),
            ("producer", json!("unreviewed"), Failure::Membership),
            ("operation", json!("arbitrary_command"), Failure::Membership),
            ("operation_count", json!(0_u8), Failure::Membership),
            ("operation_count", json!(1_u8), Failure::Membership),
            ("outcome", json!("failed"), Failure::OperationFailed),
            ("error", json!("command"), Failure::OperationFailed),
            ("promotion_allowed", json!(true), Failure::Membership),
            ("build", Value::Null, Failure::Binding),
            ("operations", json!([]), Failure::Membership),
        ] {
            assert_ne!(original[field], replacement, "{field}");
            let mut value = original.clone();
            value[field] = replacement;
            assert_eq!(verify(&value, &manifest), Err(expected.into()), "{field}");
        }
    }

    #[test]
    fn source_conformance_counters_cannot_replace_retained_nonzero_dispatch() {
        let (manifest, receipt) = fixture();
        let original = serde_json::to_value(receipt).unwrap();
        assert_eq!(verify(&original, &manifest), Ok(()));
        for index in 0..2 {
            for (field, value) in [
                ("dispatched", json!(0_u8)),
                ("passed", json!(0_u8)),
                ("failed", json!(1_u8)),
                ("ignored", json!(1_u8)),
                ("test", json!("foreign::test")),
                ("stdout_sha256", json!("0".repeat(64))),
            ] {
                let mut bad = original.clone();
                assert_ne!(bad["operations"][index][field], value, "{field}");
                bad["operations"][index][field] = value;
                assert_eq!(
                    verify(&bad, &manifest),
                    Err(Failure::Membership.into()),
                    "{field}"
                );
            }
            let mut bad = original.clone();
            bad["operations"][index]["stdout"] = json!("running 0 tests\n");
            assert_eq!(
                verify(&bad, &manifest),
                Err(Failure::OperationFailed.into())
            );
        }
        let mut duplicate = original.clone();
        duplicate["operations"][1] = duplicate["operations"][0].clone();
        assert_eq!(
            verify(&duplicate, &manifest),
            Err(Failure::OperationFailed.into())
        );
        for field in [
            "target",
            "features",
            "executable_sha256",
            "cargo_stdout_sha256",
        ] {
            let mut bad = original.clone();
            bad["build"][field] = if field == "features" {
                json!([])
            } else {
                json!("foreign")
            };
            assert_eq!(
                verify(&bad, &manifest),
                Err(Failure::Binding.into()),
                "{field}"
            );
        }
        for field in ["dispatch_unit", "qualification", "command"] {
            let mut bad = original.clone();
            bad[field] = json!("unreviewed");
            assert_eq!(
                verify(&bad, &manifest),
                Err(Failure::Input.into()),
                "{field}"
            );
        }
    }
}
