//! Executed acceptance for the frozen controlled-native priority cohort only.
//! These local receipts are tested reservations, not formal or guest qualification.

mod runner;

use super::conformance::runner as controls;
use super::{
    BUILT, CAP, Error, Failure, Manifest, Outcome, Producer, check, current, files, load,
    run_members,
};
use auctioneer_cli::BuildIdentity;
use auctioneer_cli::native_control::io::{canonical_digest, parse_json};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub(super) const PRODUCER: &str = "controlled_native_acceptance";
const OPERATION: &str = "controlled_native_tests";
const OPERATION_COUNT: u8 = 4;

pub(super) fn requirement() -> Producer {
    Producer {
        id: PRODUCER.to_owned(),
        operation: OPERATION.to_owned(),
        operation_count: OPERATION_COUNT,
    }
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

impl From<controls::GuardError> for Refusal {
    fn from(error: controls::GuardError) -> Self {
        match error {
            controls::GuardError::Storage => Self::Storage,
            controls::GuardError::Busy => Self::BuildBusy,
            controls::GuardError::Command => Self::Command,
            controls::GuardError::Cleanup => Self::Cleanup,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Scope {
    NativeOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    TestedOnly,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DispatchUnit {
    RustTestCases,
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
    scope: Scope,
    status: Status,
    dispatch_unit: DispatchUnit,
    formal_qualified: bool,
    wasm_ready: bool,
    outcome: Outcome,
    error: Option<Refusal>,
    builds: Vec<runner::Build>,
    operations: Vec<runner::Dispatch>,
}

fn target() -> Result<String, Error> {
    let built: BuildIdentity = parse_json(BUILT, CAP).map_err(|_error| Failure::Binding)?;
    if built.compiled_target.starts_with("wasm") {
        return Err(Failure::Binding.into());
    }
    Ok(built.compiled_target)
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    // Claim before prerequisite checks or child work. No retry can reuse a stale receipt.
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
    let operation_count =
        u8::try_from(execution.operations.len()).map_err(|_error| Failure::Membership)?;
    let success = execution.error.is_none()
        && operation_count == OPERATION_COUNT
        && execution.builds.len() == runner::PLANS.len();
    if current(inventory)?.binding != context.binding {
        return Err(Failure::Binding.into());
    }
    let (after, _) = load(inventory, root, id)?;
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
        scope: Scope::NativeOnly,
        status: if success {
            Status::TestedOnly
        } else {
            Status::Incomplete
        },
        dispatch_unit: DispatchUnit::RustTestCases,
        formal_qualified: false,
        wasm_ready: false,
        outcome: if success {
            Outcome::Completed
        } else {
            Outcome::Failed
        },
        error: execution.error,
        builds: execution.builds,
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
        || receipt.scope != Scope::NativeOnly
        || receipt.formal_qualified
        || receipt.wasm_ready
        || receipt.dispatch_unit != DispatchUnit::RustTestCases
    {
        return Err(Failure::Membership.into());
    }
    if receipt.manifest_sha256 != canonical_digest(manifest).map_err(|_error| Failure::Input)? {
        return Err(Failure::Binding.into());
    }
    if receipt.outcome != Outcome::Completed
        || receipt.error.is_some()
        || receipt.status != Status::TestedOnly
    {
        return Err(Failure::OperationFailed.into());
    }
    if receipt.operation_count != OPERATION_COUNT
        || receipt.operations.len() != runner::PLANS.len()
        || receipt.builds.len() != runner::PLANS.len()
    {
        return Err(Failure::Membership.into());
    }
    for ((build, row), plan) in receipt
        .builds
        .iter()
        .zip(&receipt.operations)
        .zip(runner::PLANS)
    {
        runner::verify_build_metadata(
            build,
            &manifest.inputs.evidence_target_directory,
            target,
            plan,
        )
        .map_err(|_error| Failure::Binding)?;
        let observed = runner::dispatch(plan, row.stdout.as_bytes())
            .map_err(|_error| Failure::OperationFailed)?;
        if *row != observed {
            return Err(Failure::Membership.into());
        }
    }
    Ok(())
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
    // Consumption never creates or discards a cache, and never invokes native tests.
    let _lease = controls::ReadGuard::acquire(&manifest.inputs.evidence_target_directory)
        .map_err(|_error| Failure::Binding)?;
    for (build, plan) in receipt.builds.iter().zip(runner::PLANS) {
        runner::verify_build(
            build,
            &manifest.inputs.evidence_target_directory,
            &target,
            plan,
        )
        .map_err(|_error| Failure::Binding)?;
    }
    let reference = check::run_bytes(source, &context.inventory, false, None, None).await?;
    if reference.exit_code() != 0_i32 {
        return Err(Failure::OperationFailed.into());
    }
    if current(inventory)?.binding != context.binding {
        return Err(Failure::Binding.into());
    }
    let (after, _) = load(inventory, root, id)?;
    if manifest != after {
        return Err(Failure::Binding.into());
    }
    for (build, plan) in receipt.builds.iter().zip(runner::PLANS) {
        runner::verify_build(
            build,
            &manifest.inputs.evidence_target_directory,
            &target,
            plan,
        )
        .map_err(|_error| Failure::Binding)?;
    }
    run_members(root, &["manifest.json", PRODUCER])?;
    files::members(&slot, &["receipt.json"])
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RemainingProof {
    FormalWorstCaseResourceCertification,
    GuestProductionReadiness,
    HostedProviderAndPublisherExecution,
}

#[derive(Serialize)]
pub(super) struct Report {
    producer: &'static str,
    scope: Scope,
    status: Status,
    operation_count: u8,
    formal_qualified: bool,
    wasm_ready: bool,
    full_migration: bool,
    remaining_proof: [RemainingProof; 3],
    error: Option<&'static str>,
}

impl Report {
    pub(super) fn complete(&self) -> bool {
        self.status == Status::TestedOnly && self.error.is_none()
    }
}

pub(super) async fn consume(source: &Path, inventory: &Path, root: &Path, id: &str) -> Report {
    let error = validate(source, inventory, root, id).await.err();
    Report {
        producer: PRODUCER,
        scope: Scope::NativeOnly,
        status: if error.is_none() {
            Status::TestedOnly
        } else {
            Status::Incomplete
        },
        operation_count: if error.is_none() { OPERATION_COUNT } else { 0 },
        formal_qualified: false,
        wasm_ready: false,
        full_migration: false,
        remaining_proof: [
            RemainingProof::FormalWorstCaseResourceCertification,
            RemainingProof::GuestProductionReadiness,
            RemainingProof::HostedProviderAndPublisherExecution,
        ],
        error: error.map(Error::code),
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
    fn exclusive_producer_slot_never_reuses_or_overwrites_an_old_receipt() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let slot = temporary.path().join(PRODUCER);
        files::fresh_dir(&slot).unwrap();
        fs::write(slot.join("receipt.json"), b"unchanged stale receipt").unwrap();
        assert_eq!(files::fresh_dir(&slot), Err(Failure::Output.into()));
        assert_eq!(
            fs::read(slot.join("receipt.json")).unwrap(),
            b"unchanged stale receipt"
        );
    }

    fn fixture() -> (Manifest, Receipt) {
        let manifest = Manifest {
            schema_version: super::super::VERSION,
            scope: "controlled_native_acceptance".to_owned(),
            run_id: "native-fixture".to_owned(),
            requirements: super::super::requirements(),
            inputs: super::super::Binding {
                build_identity_sha256: "a".repeat(64),
                executable_sha256: "b".repeat(64),
                workspace_sha256: "c".repeat(64),
                lock_sha256: "d".repeat(64),
                features: vec!["application".to_owned()],
                source_revision: super::super::REVISION.to_owned(),
                inventory_sha256: "e".repeat(64),
                evidence_target_directory: "/fixture/target".into(),
                evidence_tool_environment_sha256: "f".repeat(64),
            },
        };
        // Verifier-only data. No fake executable is run or published as evidence.
        let rows = [
            (
                runner::Plan::OnboardingSecure,
                "native_bidder_onboarding",
                &["conformance-suite", "test-support"][..],
                "native_bidder_onboarding::cases::all_frozen_native_flows_use_generated_app_dispatch_and_fresh_activation_snapshots",
            ),
            (
                runner::Plan::OnboardingInsecure,
                "native_bidder_onboarding",
                &["conformance-suite", "insecure-bidders", "test-support"][..],
                "native_bidder_onboarding::cases::all_frozen_native_flows_use_generated_app_dispatch_and_fresh_activation_snapshots",
            ),
            (
                runner::Plan::SchemaResources,
                "native_schema_resources",
                &["test-support"][..],
                "tests::cold_native_schema_reservations_cover_discovered_bindings_and_fresh_evaluations",
            ),
            (
                runner::Plan::NativeHeadroom,
                "auctioneer_adapter_axum",
                &[][..],
                "envelope::tests::shipped_baked_budget_leaves_256_mib_headroom",
            ),
        ];
        let operations = rows
            .into_iter()
            .map(|(plan, _, _, test)| {
                assert_eq!(plan.test_name(), test);
                let stdout = format!("\nrunning 1 test\ntest {test} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.01s\n");
                runner::Dispatch {
                    plan,
                    test: test.to_owned(),
                    dispatched: 1,
                    passed: 1,
                    failed: 0,
                    ignored: 0,
                    stdout_sha256: sha256(stdout.as_bytes()),
                    stdout,
                }
            })
            .collect();
        let builds = rows
            .into_iter()
            .map(|(plan, name, features, _)| runner::Build {
                plan,
                target: "native-fixture".to_owned(),
                features: features
                    .iter()
                    .map(|feature| (*feature).to_owned())
                    .collect(),
                executable: format!("/fixture/target/native-fixture/debug/deps/{name}-abc123")
                    .into(),
                executable_sha256: "a".repeat(64),
                cargo_stdout_sha256: "b".repeat(64),
            })
            .collect();
        let receipt = Receipt {
            schema_version: 3,
            run_id: "native-fixture".to_owned(),
            manifest_sha256: canonical_digest(&manifest).unwrap(),
            producer: "controlled_native_acceptance".to_owned(),
            operation: "controlled_native_tests".to_owned(),
            operation_count: 4,
            scope: Scope::NativeOnly,
            status: Status::TestedOnly,
            dispatch_unit: DispatchUnit::RustTestCases,
            formal_qualified: false,
            wasm_ready: false,
            outcome: Outcome::Completed,
            error: None,
            builds,
            operations,
        };
        (manifest, receipt)
    }

    #[test]
    fn each_native_receipt_guard_independently_refuses_missing_or_changed_evidence() {
        let (manifest, good) = fixture();
        verify_receipt(&good, &manifest, &manifest.run_id, "native-fixture").unwrap();
        for (field, value, failure) in [
            ("schema_version", json!(0_u8), Failure::Membership),
            ("run_id", json!("foreign"), Failure::RunMismatch),
            ("manifest_sha256", json!("0".repeat(64)), Failure::Binding),
            ("producer", json!("foreign"), Failure::Membership),
            ("operation", json!("foreign"), Failure::Membership),
            ("operation_count", json!(0_u8), Failure::Membership),
            ("formal_qualified", json!(true), Failure::Membership),
            ("wasm_ready", json!(true), Failure::Membership),
            ("status", json!("incomplete"), Failure::OperationFailed),
            ("outcome", json!("failed"), Failure::OperationFailed),
            ("error", json!("command"), Failure::OperationFailed),
            ("operations", json!([]), Failure::Membership),
            ("builds", json!([]), Failure::Membership),
        ] {
            let mut mutated_json = serde_json::to_value(&good).unwrap();
            assert_ne!(mutated_json[field], value, "{field}");
            mutated_json[field] = value;
            let bad: Receipt = serde_json::from_value(mutated_json).unwrap();
            assert_eq!(
                verify_receipt(&bad, &manifest, &manifest.run_id, "native-fixture"),
                Err(failure.into()),
                "{field}"
            );
        }
        let encoded = serde_json::to_value(&good).unwrap();
        for field in encoded.as_object().unwrap().keys() {
            let mut bad = encoded.clone();
            assert!(bad.as_object_mut().unwrap().remove(field).is_some());
            // An omitted optional error is equivalent to None; all other omissions refuse.
            if field == "error" {
                let receipt = serde_json::from_value(bad).unwrap();
                assert_eq!(
                    verify_receipt(&receipt, &manifest, &manifest.run_id, "native-fixture"),
                    Ok(())
                );
                continue;
            }
            assert!(serde_json::from_value::<Receipt>(bad).is_err(), "{field}");
        }
    }

    #[test]
    fn transcript_counts_hashes_order_build_target_and_scope_cannot_be_forged() {
        let (manifest, good) = fixture();
        assert_eq!(
            verify_receipt(&good, &manifest, &manifest.run_id, "native-fixture"),
            Ok(())
        );
        for (field, value, failure) in [
            ("passed", json!(0_u8), Failure::Membership),
            ("failed", json!(1_u8), Failure::Membership),
            ("ignored", json!(1_u8), Failure::Membership),
            ("dispatched", json!(0_u8), Failure::Membership),
            ("stdout_sha256", json!("0".repeat(64)), Failure::Membership),
            ("test", json!("foreign"), Failure::Membership),
            (
                "stdout",
                json!("running 0 tests\n"),
                Failure::OperationFailed,
            ),
        ] {
            let mut mutated_json = serde_json::to_value(&good).unwrap();
            assert_ne!(mutated_json["operations"][0][field], value, "{field}");
            mutated_json["operations"][0][field] = value;
            let bad: Receipt = serde_json::from_value(mutated_json).unwrap();
            assert_eq!(
                verify_receipt(&bad, &manifest, &manifest.run_id, "native-fixture"),
                Err(failure.into()),
                "{field}"
            );
        }
        let mut changed_receipt: Receipt =
            serde_json::from_value(serde_json::to_value(&good).unwrap()).unwrap();
        changed_receipt.operations.swap(0, 1);
        assert_eq!(
            verify_receipt(
                &changed_receipt,
                &manifest,
                &manifest.run_id,
                "native-fixture"
            ),
            Err(Failure::Membership.into())
        );
        changed_receipt.operations.swap(0, 1);
        changed_receipt.builds[0].target = "foreign".to_owned();
        assert_eq!(
            verify_receipt(
                &changed_receipt,
                &manifest,
                &manifest.run_id,
                "native-fixture"
            ),
            Err(Failure::Binding.into())
        );
        for (field, value) in [
            ("scope", json!("whole_catalog")),
            ("dispatch_unit", json!("fixture_estimate")),
            ("unreviewed", Value::Bool(true)),
        ] {
            let mut bad = serde_json::to_value(&good).unwrap();
            bad[field] = value;
            assert!(serde_json::from_value::<Receipt>(bad).is_err(), "{field}");
        }
    }

    #[test]
    fn every_current_binding_component_is_bound_to_the_native_receipt() {
        let (manifest, receipt) = fixture();
        assert_eq!(
            verify_receipt(&receipt, &manifest, &manifest.run_id, "native-fixture"),
            Ok(())
        );
        let encoded = serde_json::to_value(&manifest).unwrap();
        for field in encoded["inputs"].as_object().unwrap().keys() {
            let mut mutated_json = encoded.clone();
            let value = &mut mutated_json["inputs"][field];
            match value {
                Value::String(text) => text.push('0'),
                Value::Array(features) => features.push(json!("unreviewed-feature")),
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => {
                    panic!("binding fixture must explicitly cover every field shape")
                }
            }
            let changed: Manifest = serde_json::from_value(mutated_json).unwrap();
            assert_eq!(
                verify_receipt(&receipt, &changed, &manifest.run_id, "native-fixture"),
                Err(Failure::Binding.into()),
                "{field}"
            );
        }
    }

    #[test]
    fn report_and_requirement_never_promote_tested_native_results_to_formal_or_guest_readiness() {
        assert_eq!(
            usize::from(requirement().operation_count),
            runner::PLANS.len()
        );
        let report = Report {
            producer: PRODUCER,
            scope: Scope::NativeOnly,
            status: Status::TestedOnly,
            operation_count: OPERATION_COUNT,
            formal_qualified: false,
            wasm_ready: false,
            full_migration: false,
            remaining_proof: [
                RemainingProof::FormalWorstCaseResourceCertification,
                RemainingProof::GuestProductionReadiness,
                RemainingProof::HostedProviderAndPublisherExecution,
            ],
            error: None,
        };
        assert!(report.complete());
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["status"], "tested_only");
        assert_eq!(value["formal_qualified"], false);
        assert_eq!(value["wasm_ready"], false);
        assert_eq!(value["scope"], "native_only");
        assert_eq!(value["full_migration"], false);
        assert_eq!(
            value["remaining_proof"],
            json!([
                "formal_worst_case_resource_certification",
                "guest_production_readiness",
                "hosted_provider_and_publisher_execution"
            ])
        );
    }
}
