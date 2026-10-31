//! Real CLI evidence controls. Native acceptance is separate from full-catalog migration.

#[cfg(test)]
mod tests {
    use auctioneer_cli::native_control::io::{canonical_digest, sha256};
    use fs2::FileExt;
    use serde_json::{Value, json};
    use std::env::{temp_dir, var_os};
    use std::fs::{self, OpenOptions};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::str::from_utf8;
    use std::sync::{Mutex, PoisonError};
    use tempfile::{TempDir, tempdir_in};

    const RUN: &str = "workflow-123.attempt-2.commit-deadbeef";
    const SCHEMA_PRODUCER: &str = "selected_schema_source_conformance";
    const NATIVE_PRODUCER: &str = "controlled_native_acceptance";
    static SCHEMA_CACHE_TESTS: Mutex<()> = Mutex::new(());

    struct Fixture {
        directory: TempDir,
        inventory: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
            let inventory = directory.path().join("inventory.toml");
            fs::write(&inventory, include_bytes!(
            "../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml"
        )).unwrap();
            Self {
                directory,
                inventory,
            }
        }

        fn root(&self, name: &str) -> PathBuf {
            self.directory.path().join(name)
        }

        fn invoke(&self, action: &str, root: &Path, run: &str, extra: &[&str]) -> Output {
            self.invoke_with_output(action, root, run, extra, None)
        }

        fn invoke_with_output(
            &self,
            action: &str,
            root: &Path,
            run: &str,
            extra: &[&str],
            output: Option<&Path>,
        ) -> Output {
            let mut command = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"));
            command
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .arg("bidders");
            if action == "check" {
                command.arg("check");
            } else {
                command.args(["evidence", action]);
            }
            command
                .arg("--source")
                .arg(var_os("AUCTIONEER__BIDDER__SOURCE_DIR").expect("pinned source checkout"))
                .arg("--inventory")
                .arg(&self.inventory)
                .args(["--run-id", run]);
            match action {
                "check" => {
                    command.arg("--evidence-root").arg(root);
                }
                "produce" => {
                    command
                        .arg("--run-manifest")
                        .arg(root.join("manifest.json"))
                        .arg("--output")
                        .arg(
                            output.map_or_else(|| root.join("reference_integrity"), Path::to_owned),
                        );
                }
                _ => {
                    command.arg("--output").arg(root);
                }
            }
            if action == "produce" && !extra.contains(&"--producer") {
                command.args(["--producer", "reference_integrity"]);
            }
            command.args(extra).output().unwrap()
        }

        fn begin(&self, root: &Path, run: &str) {
            let result = self.invoke("begin", root, run, &[]);
            assert_eq!(result.status.code(), Some(0_i32), "{result:?}");
        }

        fn produce(&self, root: &Path, run: &str) {
            let result = self.invoke("produce", root, run, &[]);
            assert_eq!(result.status.code(), Some(0_i32), "{result:?}");
        }
    }

    fn read(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn write(path: &Path, value: &Value) {
        fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    }

    fn repin_manifest(root: &Path, manifest: &Value, original_receipt: &Value) {
        write(&root.join("manifest.json"), manifest);
        let mut receipt = original_receipt.clone();
        receipt["manifest_sha256"] = json!(canonical_digest(manifest).unwrap());
        write(&root.join("reference_integrity/receipt.json"), &receipt);
    }

    fn assert_incomplete(result: &Output, reason: &str) {
        let value = report(result, "incomplete");
        assert_eq!(value["preparation"]["error"], reason, "{result:?}");
    }

    fn report(result: &Output, preparation: &str) -> Value {
        acceptance_report(result, preparation, false)
    }

    fn acceptance_report(result: &Output, preparation: &str, complete: bool) -> Value {
        assert_eq!(
            result.status.code(),
            Some(i32::from(!complete)),
            "{result:?}"
        );
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["schema_version"], 3_u64);
        assert_acceptance_scope(&value);
        assert_eq!(
            value["status"],
            if complete {
                "native_acceptance_complete"
            } else {
                "native_acceptance_incomplete"
            }
        );
        assert_eq!(value["migrated"], if complete { 14_u64 } else { 0_u64 });
        assert_eq!(value["preparation"]["status"], preparation);
        if preparation == "complete" {
            assert_eq!(value["preparation"]["error"], Value::Null);
        }
        if complete {
            assert_eq!(value["conformance"]["status"], "complete");
            assert_eq!(value["conformance"]["error"], Value::Null);
            assert_eq!(value["native"]["status"], "tested_only");
            assert_eq!(value["native"]["operation_count"], 4_u64);
            assert_eq!(value["native"]["error"], Value::Null);
        }
        value
    }

    fn assert_acceptance_scope(value: &Value) {
        assert_eq!(value["scope"], "controlled_native_acceptance");
        assert_eq!(value["formal_qualified"], false);
        assert_eq!(value["wasm_ready"], false);
        assert_eq!(value["full_catalog_complete"], false);
        assert_eq!(value["native"]["scope"], "native_only");
        assert_eq!(value["native"]["formal_qualified"], false);
        assert_eq!(value["native"]["wasm_ready"], false);
        assert_eq!(value["native"]["full_migration"], false);
        assert_eq!(
            value["native"]["remaining_proof"],
            json!([
                "formal_worst_case_resource_certification",
                "guest_production_readiness",
                "hosted_provider_and_publisher_execution"
            ])
        );
        assert_eq!(
            value["full_catalog_unavailable"],
            json!([
                "foundation",
                "schema_qualification",
                "registry_lifecycle",
                "proprietary_decoder",
                "resource_qualification",
                "current_shipped_subset",
                "priority_contracts",
                "composed_onboarding"
            ])
        );
    }

    #[test]
    fn evidence_begin_reads_the_section_delimited_source_environment() {
        let fixture = Fixture::new();
        for present in [true, false] {
            let root = fixture.root(if present { "environment" } else { "absent" });
            let mut command = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"));
            command
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .env_clear()
                .env("PATH", var_os("PATH").unwrap())
                .args(["bidders", "evidence", "begin", "--inventory"])
                .arg(&fixture.inventory)
                .args(["--run-id", RUN, "--output"])
                .arg(&root);
            if present {
                command.env("AUCTIONEER__BIDDER__SOURCE_DIR", fixture.directory.path());
            }
            let result = command.output().unwrap();
            assert_eq!(
                result.status.code(),
                Some(if present { 0_i32 } else { 2_i32 }),
                "{result:?}"
            );
            assert_eq!(root.join("manifest.json").exists(), present);
            if !present {
                assert_eq!(
                    from_utf8(&result.stderr).unwrap().trim(),
                    "bidder_evidence_input"
                );
            }
        }
    }

    #[test]
    fn code_owned_source_and_native_producers_are_required_even_when_receipts_are_absent() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        let manifest = read(&root.join("manifest.json"));
        assert_eq!(manifest["schema_version"], 3_u64);
        assert_eq!(manifest["scope"], "controlled_native_acceptance");
        let child_target = Path::new(
            manifest["inputs"]["evidence_target_directory"]
                .as_str()
                .unwrap(),
        );
        assert_eq!(
            child_target.file_name().unwrap(),
            "bidder-evidence-conformance-v2"
        );
        for version in [1_u8, 2_u8] {
            let mut legacy = manifest.clone();
            legacy["schema_version"] = json!(version);
            write(&root.join("manifest.json"), &legacy);
            assert_incomplete(
                &fixture.invoke("check", &root, RUN, &[]),
                "bidder_evidence_binding",
            );
        }
        write(&root.join("manifest.json"), &manifest);
        assert!(
            manifest["requirements"]["producers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|producer| producer["id"] == SCHEMA_PRODUCER),
            "requirements come from code, not the receipts that happen to exist"
        );
        assert_eq!(
            manifest["requirements"]["producers"],
            json!([
                {"id":"reference_integrity", "operation":"inspect_reference_inventory", "operation_count":1_u64},
                {"id":SCHEMA_PRODUCER, "operation":"selected_schema_source_tests", "operation_count":2_u64},
                {"id":NATIVE_PRODUCER, "operation":"controlled_native_tests", "operation_count":4_u64}
            ])
        );
        fixture.produce(&root, RUN);
        let value = report(&fixture.invoke("check", &root, RUN, &[]), "complete");
        assert_eq!(value["conformance"]["status"], "incomplete");
        assert_eq!(value["conformance"]["error"], "bidder_evidence_membership");
        assert_eq!(value["native"]["status"], "incomplete");
        assert_eq!(value["native"]["error"], "bidder_evidence_membership");
    }

    fn assert_schema_dispatch_receipt(receipt: &Value) {
        assert_eq!(receipt["producer"], SCHEMA_PRODUCER);
        assert_eq!(receipt["schema_version"], 3_u64);
        assert_eq!(receipt["run_id"], RUN);
        assert_eq!(receipt["outcome"], "completed");
        assert_eq!(receipt["operation_count"], 2_u64);
        assert_eq!(receipt["dispatch_unit"], "rust_test_cases");
        assert_eq!(receipt["qualification"], "unqualified");
        assert_eq!(receipt["promotion_allowed"], false);
        let operations = receipt["operations"].as_array().unwrap();
        assert_eq!(operations.len(), 2);
        assert_eq!(
            operations
                .iter()
                .map(|operation| operation["test"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "tests::schema_selection_source_and_expanded_generated_membership_manifest",
                "tests::schema_selection_manifest_is_order_independent_and_detects_omissions"
            ]
        );
        for operation in operations {
            assert_eq!(operation["dispatched"], 1_u64);
            assert_eq!(operation["passed"], 1_u64);
            assert_eq!(operation["failed"], 0_u64);
            assert_eq!(operation["ignored"], 0_u64);
            assert_eq!(operation["stdout_sha256"].as_str().unwrap().len(), 64);
        }
    }

    fn assert_native_dispatch_receipt(receipt: &Value) {
        assert_eq!(receipt["schema_version"], 3_u64);
        assert_eq!(receipt["producer"], NATIVE_PRODUCER);
        assert_eq!(receipt["operation"], "controlled_native_tests");
        assert_eq!(receipt["run_id"], RUN);
        assert_eq!(receipt["scope"], "native_only");
        assert_eq!(receipt["status"], "tested_only");
        assert_eq!(receipt["outcome"], "completed");
        assert_eq!(receipt["error"], Value::Null);
        assert_eq!(receipt["operation_count"], 4_u64);
        assert_eq!(receipt["dispatch_unit"], "rust_test_cases");
        assert_eq!(receipt["formal_qualified"], false);
        assert_eq!(receipt["wasm_ready"], false);
        let operations = receipt["operations"].as_array().unwrap();
        let builds = receipt["builds"].as_array().unwrap();
        assert_eq!(operations.len(), 4);
        assert_eq!(builds.len(), operations.len());
        assert_eq!(
            operations
                .iter()
                .map(|row| row["plan"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "onboarding_secure",
                "onboarding_insecure",
                "schema_resources",
                "native_headroom"
            ]
        );
        assert_eq!(
            operations
                .iter()
                .map(|row| row["test"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "native_bidder_onboarding::cases::all_frozen_native_flows_use_generated_app_dispatch_and_fresh_activation_snapshots",
                "native_bidder_onboarding::cases::all_frozen_native_flows_use_generated_app_dispatch_and_fresh_activation_snapshots",
                "tests::cold_native_schema_reservations_cover_discovered_bindings_and_fresh_evaluations",
                "envelope::tests::shipped_baked_budget_leaves_256_mib_headroom",
            ]
        );
        for (operation, build) in operations.iter().zip(builds) {
            assert_native_operation_artifact(operation, build);
        }
    }

    fn assert_native_operation_artifact(operation: &Value, build: &Value) {
        assert_eq!(operation["plan"], build["plan"]);
        assert_eq!(operation["dispatched"], 1_u64);
        assert_eq!(operation["passed"], 1_u64);
        assert_eq!(operation["failed"], 0_u64);
        assert_eq!(operation["ignored"], 0_u64);
        assert_eq!(
            operation["stdout_sha256"],
            sha256(operation["stdout"].as_str().unwrap().as_bytes())
        );
        assert_eq!(build["executable_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(build["cargo_stdout_sha256"].as_str().unwrap().len(), 64);
        assert!(Path::new(build["executable"].as_str().unwrap()).is_file());
    }

    fn assert_each_completed_producer_is_required(fixture: &Fixture, root: &Path) {
        for (producer, report_field) in [
            ("reference_integrity", "preparation"),
            (SCHEMA_PRODUCER, "conformance"),
            (NATIVE_PRODUCER, "native"),
        ] {
            let path = root.join(producer).join("receipt.json");
            let original = fs::read(&path).unwrap();
            fs::remove_file(&path).unwrap();
            let preparation = if report_field == "preparation" {
                "incomplete"
            } else {
                "complete"
            };
            let missing = report(&fixture.invoke("check", root, RUN, &[]), preparation);
            assert_eq!(missing[report_field]["status"], "incomplete");
            assert_eq!(missing[report_field]["error"], "bidder_evidence_membership");
            let mut failed: Value = serde_json::from_slice(&original).unwrap();
            failed["outcome"] = json!("failed");
            write(&path, &failed);
            let refused = report(&fixture.invoke("check", root, RUN, &[]), preparation);
            assert_eq!(refused[report_field]["status"], "incomplete");
            assert_eq!(
                refused[report_field]["error"],
                "bidder_evidence_operation_failed"
            );
            fs::write(&path, original).unwrap();
        }
        acceptance_report(&fixture.invoke("check", root, RUN, &[]), "complete", true);
    }

    fn assert_native_receipt_guards(fixture: &Fixture, root: &Path) {
        let path = root.join(NATIVE_PRODUCER).join("receipt.json");
        let original = fs::read(&path).unwrap();
        let good: Value = serde_json::from_slice(&original).unwrap();
        for (field, value, error) in [
            (
                "operation_count",
                json!(0_u64),
                "bidder_evidence_membership",
            ),
            ("operations", json!([]), "bidder_evidence_membership"),
            (
                "formal_qualified",
                json!(true),
                "bidder_evidence_membership",
            ),
            ("wasm_ready", json!(true), "bidder_evidence_membership"),
            (
                "manifest_sha256",
                json!("0".repeat(64)),
                "bidder_evidence_binding",
            ),
        ] {
            let mut changed = good.clone();
            changed[field] = value;
            write(&path, &changed);
            let refused = report(&fixture.invoke("check", root, RUN, &[]), "complete");
            assert_eq!(refused["native"]["error"], error, "{field}");
        }
        for (field, value, error) in [
            ("dispatched", json!(0_u64), "bidder_evidence_membership"),
            ("passed", json!(0_u64), "bidder_evidence_membership"),
            (
                "stdout",
                json!("running 0 tests\n"),
                "bidder_evidence_operation_failed",
            ),
            (
                "stdout_sha256",
                json!("0".repeat(64)),
                "bidder_evidence_membership",
            ),
        ] {
            let mut changed = good.clone();
            changed["operations"][0][field] = value;
            write(&path, &changed);
            let refused = report(&fixture.invoke("check", root, RUN, &[]), "complete");
            assert_eq!(refused["native"]["error"], error, "{field}");
        }
        let mut changed = good.clone();
        changed["builds"][0]["executable_sha256"] = json!("0".repeat(64));
        write(&path, &changed);
        assert_eq!(
            report(&fixture.invoke("check", root, RUN, &[]), "complete")["native"]["error"],
            "bidder_evidence_binding"
        );
        for bytes in [
            b"{".to_vec(),
            vec![b' '; (1 << 20) + 1],
            format!(
                "{{\"run_id\":\"foreign\",{}",
                from_utf8(&original).unwrap().strip_prefix('{').unwrap()
            )
            .into_bytes(),
        ] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(
                report(&fixture.invoke("check", root, RUN, &[]), "complete")["native"]["error"],
                "bidder_evidence_input"
            );
        }
        fs::write(&path, &original).unwrap();
        let repeated = fixture.invoke_with_output(
            "produce",
            root,
            RUN,
            &["--producer", NATIVE_PRODUCER],
            Some(path.parent().unwrap()),
        );
        assert_eq!(repeated.status.code(), Some(2_i32));
        assert_eq!(fs::read(&path).unwrap(), original);
        acceptance_report(&fixture.invoke("check", root, RUN, &[]), "complete", true);
    }

    fn assert_native_consumer_requires_current_inputs(fixture: &Fixture, root: &Path) {
        let inventory = fs::read(&fixture.inventory).unwrap();
        fs::write(&fixture.inventory, "revision = 'drift'\n").unwrap();
        let stale = report(&fixture.invoke("check", root, RUN, &[]), "incomplete");
        assert_eq!(stale["native"]["error"], "bidder_evidence_binding");
        fs::write(&fixture.inventory, inventory).unwrap();
        let unavailable = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(["bidders", "check", "--source"])
            .arg(fixture.root("missing-source"))
            .arg("--inventory")
            .arg(&fixture.inventory)
            .arg("--evidence-root")
            .arg(root)
            .args(["--run-id", RUN])
            .output()
            .unwrap();
        let refused = report(&unavailable, "incomplete");
        assert_eq!(refused["native"]["error"], "bidder_source_unavailable");
        let manifest = read(&root.join("manifest.json"));
        let cache = Path::new(
            manifest["inputs"]["evidence_target_directory"]
                .as_str()
                .unwrap(),
        );
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .open(cache.with_extension("lock"))
            .unwrap();
        lease.try_lock_exclusive().unwrap();
        let blocked = report(&fixture.invoke("check", root, RUN, &[]), "complete");
        assert_eq!(blocked["native"]["error"], "bidder_evidence_binding");
        FileExt::unlock(&lease).unwrap();
        drop(lease);
        acceptance_report(&fixture.invoke("check", root, RUN, &[]), "complete", true);
    }

    fn assert_skipped_native_cannot_reuse_previous_run(fixture: &Fixture, root: &Path) {
        let current = fixture.root("native-next-run");
        let next_run = "workflow-123.attempt-3.commit-deadbeef";
        fixture.begin(&current, next_run);
        fixture.produce(&current, next_run);
        assert_eq!(
            read(&root.join("manifest.json"))["inputs"],
            read(&current.join("manifest.json"))["inputs"]
        );
        let skipped = report(
            &fixture.invoke("check", &current, next_run, &[]),
            "complete",
        );
        assert_eq!(skipped["native"]["error"], "bidder_evidence_membership");
        fs::create_dir_all(current.join(NATIVE_PRODUCER)).unwrap();
        fs::copy(
            root.join(NATIVE_PRODUCER).join("receipt.json"),
            current.join(NATIVE_PRODUCER).join("receipt.json"),
        )
        .unwrap();
        let replay = report(
            &fixture.invoke("check", &current, next_run, &[]),
            "complete",
        );
        assert_eq!(replay["native"]["error"], "bidder_evidence_run_mismatch");
    }

    fn assert_schema_requires_current_source(fixture: &Fixture, root: &Path) {
        for missing_source in [
            fixture.root("missing-source"),
            fixture.root("unpinned-source"),
        ] {
            if missing_source.file_name().unwrap() == "unpinned-source" {
                fs::create_dir_all(&missing_source).unwrap();
            }
            // Keep the real completed receipt and all bindings unchanged.
            // Only the current source prerequisite becomes unavailable.
            let checked = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .args(["bidders", "check", "--source"])
                .arg(&missing_source)
                .arg("--inventory")
                .arg(&fixture.inventory)
                .arg("--evidence-root")
                .arg(root)
                .args(["--run-id", RUN])
                .output()
                .unwrap();
            let refused = report(&checked, "incomplete");
            assert_eq!(refused["preparation"]["error"], "bidder_source_unavailable");
            assert_eq!(refused["conformance"]["status"], "incomplete");
            assert_eq!(refused["conformance"]["error"], "bidder_source_unavailable");
        }
    }

    fn assert_schema_consumer_requires_cache_lease(fixture: &Fixture, root: &Path) {
        let manifest = read(&root.join("manifest.json"));
        let cache = Path::new(
            manifest["inputs"]["evidence_target_directory"]
                .as_str()
                .unwrap(),
        );
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .open(cache.with_extension("lock"))
            .unwrap();
        lease.try_lock_exclusive().unwrap();
        let blocked = report(&fixture.invoke("check", root, RUN, &[]), "complete");
        assert_eq!(blocked["conformance"]["status"], "incomplete");
        assert_eq!(blocked["conformance"]["error"], "bidder_evidence_binding");
        FileExt::unlock(&lease).unwrap();
        drop(lease);
        let released = report(&fixture.invoke("check", root, RUN, &[]), "complete");
        assert_eq!(released["conformance"]["status"], "complete");
        assert_eq!(released["conformance"]["error"], Value::Null);
    }

    fn assert_skipped_schema_cannot_reuse_previous_run(
        fixture: &Fixture,
        root: &Path,
        original: &[u8],
    ) {
        let current = fixture.root("current");
        let next_run = "workflow-123.attempt-3.commit-deadbeef";
        fixture.begin(&current, next_run);
        fixture.produce(&current, next_run);
        assert_eq!(
            read(&root.join("manifest.json"))["inputs"],
            read(&current.join("manifest.json"))["inputs"]
        );
        let skipped = report(
            &fixture.invoke("check", &current, next_run, &[]),
            "complete",
        );
        assert_eq!(skipped["conformance"]["status"], "incomplete");
        assert_eq!(
            skipped["conformance"]["error"],
            "bidder_evidence_membership"
        );
        let replay_slot = current.join(SCHEMA_PRODUCER);
        fs::create_dir_all(&replay_slot).unwrap();
        fs::write(replay_slot.join("receipt.json"), original).unwrap();
        let replay = report(
            &fixture.invoke("check", &current, next_run, &[]),
            "complete",
        );
        assert_eq!(replay["conformance"]["status"], "incomplete");
        assert_eq!(
            replay["conformance"]["error"],
            "bidder_evidence_run_mismatch"
        );
    }

    #[test]
    fn selected_schema_source_producer_records_exact_real_dispatch_without_qualification() {
        let _cache = SCHEMA_CACHE_TESTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        let slot = root.join(SCHEMA_PRODUCER);
        let output = fixture.invoke_with_output(
            "produce",
            &root,
            RUN,
            &["--producer", SCHEMA_PRODUCER],
            Some(&slot),
        );
        assert_eq!(
            output.status.code(),
            Some(0_i32),
            "{output:?}; producer_receipt={:?}",
            fs::read_to_string(slot.join("receipt.json"))
        );
        assert_schema_dispatch_receipt(&read(&slot.join("receipt.json")));
        // A distributed schema producer may finish before reference preparation.
        let independent = report(&fixture.invoke("check", &root, RUN, &[]), "incomplete");
        assert_eq!(
            independent["preparation"]["error"],
            "bidder_evidence_membership"
        );
        assert_eq!(independent["conformance"]["status"], "complete");
        assert_eq!(independent["conformance"]["error"], Value::Null);
        fixture.produce(&root, RUN);
        let value = report(&fixture.invoke("check", &root, RUN, &[]), "complete");
        assert_eq!(value["conformance"]["status"], "complete");
        assert_eq!(value["conformance"]["error"], Value::Null);
        assert_schema_consumer_requires_cache_lease(&fixture, &root);
        assert_schema_requires_current_source(&fixture, &root);
        for missing in ["selected_schema_semantics", "selected_schema_artifacts"] {
            assert!(
                value["conformance"]["unavailable"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["producer"] == missing && row["status"] == "unavailable")
            );
        }
        let full = report(
            &fixture.invoke("check", &root, RUN, &["--require-complete"]),
            "complete",
        );
        assert_eq!(full["migrated"], 0_u64);
        let original = fs::read(slot.join("receipt.json")).unwrap();
        let repeated = fixture.invoke_with_output(
            "produce",
            &root,
            RUN,
            &["--producer", SCHEMA_PRODUCER],
            Some(&slot),
        );
        assert_eq!(repeated.status.code(), Some(2_i32), "{repeated:?}");
        assert_eq!(fs::read(slot.join("receipt.json")).unwrap(), original);
        assert_skipped_schema_cannot_reuse_previous_run(&fixture, &root, &original);
    }

    #[test]
    fn selected_schema_source_failed_guard_never_claims_unrun_operations() {
        let fixture = Fixture::new();
        fs::write(&fixture.inventory, "revision = 'drift'\n").unwrap();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        let slot = root.join(SCHEMA_PRODUCER);
        let output = fixture.invoke_with_output(
            "produce",
            &root,
            RUN,
            &["--producer", SCHEMA_PRODUCER],
            Some(&slot),
        );
        assert_eq!(output.status.code(), Some(2_i32), "{output:?}");
        assert_eq!(output.stderr, b"bidder_evidence_operation_failed\n");
        let receipt = read(&slot.join("receipt.json"));
        assert_eq!(receipt["outcome"], "failed");
        assert_eq!(receipt["error"], "reference_integrity_failed");
        assert_eq!(receipt["operation_count"], 0_u64);
        assert_eq!(receipt["operations"], json!([]));
        assert_eq!(receipt["build"], Value::Null);
        assert_eq!(receipt["promotion_allowed"], false);
        let value = report(&fixture.invoke("check", &root, RUN, &[]), "incomplete");
        assert_eq!(value["conformance"]["status"], "incomplete");
        assert_eq!(
            value["conformance"]["error"],
            "bidder_evidence_operation_failed"
        );
    }

    #[test]
    fn controlled_native_failed_reference_guard_records_zero_operations_without_building() {
        let fixture = Fixture::new();
        fs::write(&fixture.inventory, "revision = 'drift'\n").unwrap();
        let root = fixture.root("native-failed-guard");
        fixture.begin(&root, RUN);
        let slot = root.join(NATIVE_PRODUCER);
        let output = fixture.invoke_with_output(
            "produce",
            &root,
            RUN,
            &["--producer", NATIVE_PRODUCER],
            Some(&slot),
        );
        assert_eq!(output.status.code(), Some(2_i32), "{output:?}");
        assert_eq!(output.stderr, b"bidder_evidence_operation_failed\n");
        let receipt = read(&slot.join("receipt.json"));
        assert_eq!(receipt["schema_version"], 3_u64);
        assert_eq!(receipt["scope"], "native_only");
        assert_eq!(receipt["outcome"], "failed");
        assert_eq!(receipt["status"], "incomplete");
        assert_eq!(receipt["error"], "reference_integrity_failed");
        assert_eq!(receipt["operation_count"], 0_u64);
        assert_eq!(receipt["operations"], json!([]));
        assert_eq!(receipt["builds"], json!([]));
        assert_eq!(receipt["formal_qualified"], false);
        assert_eq!(receipt["wasm_ready"], false);
        let value = report(&fixture.invoke("check", &root, RUN, &[]), "incomplete");
        assert_eq!(value["native"]["status"], "incomplete");
        assert_eq!(value["native"]["error"], "bidder_evidence_operation_failed");
    }

    #[test]
    fn controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog() {
        let _cache = SCHEMA_CACHE_TESTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        fixture.produce(&root, RUN);
        let receipt = read(&root.join("reference_integrity/receipt.json"));
        assert_eq!(receipt["producer"], "reference_integrity");
        assert_eq!(receipt["operation"], "inspect_reference_inventory");
        assert_eq!(receipt["operation_count"], 1_u64);
        assert_eq!(receipt["outcome"], "completed");
        assert!(receipt.get("positive").is_none());
        assert!(receipt.get("negative").is_none());
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
        report(
            &fixture.invoke("check", &root, RUN, &["--require-complete"]),
            "complete",
        );
        let local = fixture.root("local");
        acceptance_report(&fixture.invoke("run", &local, RUN, &[]), "complete", true);
        assert_native_dispatch_receipt(&read(&local.join(NATIVE_PRODUCER).join("receipt.json")));
        assert_each_completed_producer_is_required(&fixture, &local);
        assert_native_receipt_guards(&fixture, &local);
        assert_native_consumer_requires_current_inputs(&fixture, &local);
        assert_skipped_native_cannot_reuse_previous_run(&fixture, &local);
        let native = acceptance_report(
            &fixture.invoke("check", &local, RUN, &["--require-complete"]),
            "complete",
            true,
        );
        assert_eq!(native["full_catalog_complete"], false);

        // The full-catalog inventory contract stays independent of native evidence.
        let full_output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(["bidders", "check", "--source"])
            .arg(var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap())
            .arg("--inventory")
            .arg(&fixture.inventory)
            .arg("--require-complete")
            .output()
            .unwrap();
        assert_eq!(full_output.status.code(), Some(1_i32), "{full_output:?}");
        let full: Value = serde_json::from_slice(&full_output.stdout).unwrap();
        assert_eq!(full["scope"], "reference_inventory_only");
        assert_eq!(full["status"], "migration_incomplete");
        assert_eq!(full["migrated"], 0_u64);
    }

    #[test]
    fn missing_failed_and_unknown_producers_are_incomplete() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        assert_incomplete(
            &fixture.invoke("check", &root, RUN, &[]),
            "bidder_evidence_membership",
        );
        fixture.produce(&root, RUN);
        let path = root.join("reference_integrity/receipt.json");
        let original = read(&path);
        for (key, value, reason) in [
            (
                "outcome",
                json!("failed"),
                "bidder_evidence_operation_failed",
            ),
            ("producer", json!("unknown"), "bidder_evidence_membership"),
            (
                "operation",
                json!("some_test"),
                "bidder_evidence_membership",
            ),
            (
                "operation_count",
                json!(0_u64),
                "bidder_evidence_membership",
            ),
            (
                "operation_count",
                json!(2_u64),
                "bidder_evidence_membership",
            ),
            (
                "manifest_sha256",
                json!("0".repeat(64)),
                "bidder_evidence_binding",
            ),
        ] {
            let mut receipt = original.clone();
            receipt[key] = value;
            write(&path, &receipt);
            assert_incomplete(&fixture.invoke("check", &root, RUN, &[]), reason);
        }
        write(&path, &original);
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
        fs::create_dir_all(root.join("unknown")).unwrap();
        assert_incomplete(
            &fixture.invoke("check", &root, RUN, &[]),
            "bidder_evidence_membership",
        );
    }

    #[test]
    fn manifest_cannot_shorten_or_replace_the_code_owned_requirements() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        fixture.produce(&root, RUN);
        let path = root.join("manifest.json");
        let original = read(&path);
        let receipt = read(&root.join("reference_integrity/receipt.json"));
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
        for (contract, reason) in [
            (json!({}), "bidder_evidence_input"),
            (
                json!({"producers": [], "unavailable": original["requirements"]["unavailable"]}),
                "bidder_evidence_membership",
            ),
            (
                json!({"producers": original["requirements"]["producers"].as_array().unwrap()
                    .iter().filter(|producer| producer["id"] != NATIVE_PRODUCER).cloned().collect::<Vec<_>>(),
                    "unavailable": original["requirements"]["unavailable"]}),
                "bidder_evidence_membership",
            ),
            (
                json!({"producers": original["requirements"]["producers"], "unavailable": []}),
                "bidder_evidence_membership",
            ),
        ] {
            let mut manifest = original.clone();
            manifest["requirements"] = contract;
            repin_manifest(&root, &manifest, &receipt);
            assert_incomplete(&fixture.invoke("check", &root, RUN, &[]), reason);
        }
        repin_manifest(&root, &original, &receipt);
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
    }

    #[test]
    fn each_current_input_binding_is_checked_even_with_a_matching_receipt_digest() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        fixture.produce(&root, RUN);
        let original = read(&root.join("manifest.json"));
        let receipt = read(&root.join("reference_integrity/receipt.json"));
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
        for (field, replacement) in [
            ("build_identity_sha256", json!("0".repeat(64))),
            ("executable_sha256", json!("0".repeat(64))),
            ("workspace_sha256", json!("0".repeat(64))),
            ("lock_sha256", json!("0".repeat(64))),
            ("features", json!([])),
            ("source_revision", json!("0".repeat(40))),
            ("inventory_sha256", json!("0".repeat(64))),
            ("evidence_target_directory", json!("/foreign/target")),
            ("evidence_tool_environment_sha256", json!("0".repeat(64))),
        ] {
            assert_ne!(
                original["inputs"][field], replacement,
                "row must vary {field}"
            );
            let mut manifest = original.clone();
            manifest["inputs"][field] = replacement;
            repin_manifest(&root, &manifest, &receipt);
            assert_incomplete(
                &fixture.invoke("check", &root, RUN, &[]),
                "bidder_evidence_binding",
            );
        }
        repin_manifest(&root, &original, &receipt);
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
    }

    #[test]
    fn same_input_previous_run_cannot_replace_skipped_producer() {
        let fixture = Fixture::new();
        let old = fixture.root("old");
        let current = fixture.root("current");
        fixture.begin(&old, RUN);
        fixture.produce(&old, RUN);
        let expected = "workflow-123.attempt-3.commit-deadbeef";
        fixture.begin(&current, expected);
        let old_manifest = read(&old.join("manifest.json"));
        let current_manifest = read(&current.join("manifest.json"));
        assert_eq!(old_manifest["inputs"], current_manifest["inputs"]);
        assert_ne!(old_manifest["run_id"], current_manifest["run_id"]);
        fs::create_dir_all(current.join("reference_integrity")).unwrap();
        for result in fs::read_dir(old.join("reference_integrity")).unwrap() {
            let entry = result.unwrap();
            assert!(entry.metadata().unwrap().is_file());
            fs::copy(
                entry.path(),
                current.join("reference_integrity").join(entry.file_name()),
            )
            .unwrap();
        }
        let value = report(
            &fixture.invoke("check", &current, expected, &[]),
            "incomplete",
        );
        assert_eq!(
            value["preparation"]["error"],
            "bidder_evidence_run_mismatch"
        );
    }

    #[test]
    fn supplied_expected_run_is_independent_including_attempt_and_commit() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        fixture.produce(&root, RUN);
        for expected in [
            "workflow-124.attempt-2.commit-deadbeef",
            "workflow-123.attempt-3.commit-deadbeef",
            "workflow-123.attempt-2.commit-feedface",
        ] {
            assert_incomplete(
                &fixture.invoke("check", &root, expected, &[]),
                "bidder_evidence_run_mismatch",
            );
            assert_eq!(
                fixture
                    .invoke("produce", &root, expected, &[])
                    .status
                    .code(),
                Some(2_i32)
            );
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"));
        let output = command
            .args(["bidders", "check", "--source"])
            .arg(var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap())
            .arg("--evidence-root")
            .arg(&root)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2_i32));
    }

    #[test]
    fn changed_inventory_is_stale_and_source_drift_is_not_success() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        fixture.produce(&root, RUN);
        fs::write(&fixture.inventory, "revision = 'drift'\n").unwrap();
        assert_incomplete(
            &fixture.invoke("check", &root, RUN, &[]),
            "bidder_evidence_binding",
        );
        let drift = fixture.root("drift");
        fixture.begin(&drift, RUN);
        let output = fixture.invoke("produce", &drift, RUN, &[]);
        assert_eq!(output.status.code(), Some(2_i32), "{output:?}");
        assert_eq!(output.stderr, b"bidder_evidence_operation_failed\n");
        let receipt_path = drift.join("reference_integrity/receipt.json");
        let mut receipt = read(&receipt_path);
        assert_eq!(receipt["outcome"], "failed");
        assert_eq!(receipt["error"], "reference_integrity_failed");
        assert_incomplete(
            &fixture.invoke("check", &drift, RUN, &[]),
            "bidder_evidence_operation_failed",
        );
        receipt["outcome"] = json!("completed");
        receipt["error"] = Value::Null;
        write(&receipt_path, &receipt);
        assert_incomplete(
            &fixture.invoke("check", &drift, RUN, &[]),
            "bidder_evidence_operation_failed",
        );
    }

    #[test]
    fn exclusive_run_and_producer_slots_cannot_be_reused_or_overwritten() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        let manifest = fs::read(root.join("manifest.json")).unwrap();
        assert_eq!(
            fixture.invoke("begin", &root, RUN, &[]).status.code(),
            Some(2_i32)
        );
        assert_eq!(fs::read(root.join("manifest.json")).unwrap(), manifest);
        fs::create_dir_all(root.join("reference_integrity")).unwrap();
        assert_eq!(
            fixture.invoke("produce", &root, RUN, &[]).status.code(),
            Some(2_i32)
        );
        assert!(!root.join("reference_integrity/receipt.json").exists());
        let fresh = fixture.root("fresh");
        fixture.begin(&fresh, RUN);
        fixture.produce(&fresh, RUN);
        let receipt = fs::read(fresh.join("reference_integrity/receipt.json")).unwrap();
        assert_eq!(
            fixture.invoke("produce", &fresh, RUN, &[]).status.code(),
            Some(2_i32)
        );
        assert_eq!(
            fs::read(fresh.join("reference_integrity/receipt.json")).unwrap(),
            receipt
        );
    }

    #[test]
    fn strict_json_and_exact_artifact_membership_are_required() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        fixture.produce(&root, RUN);
        for path in [
            root.join("manifest.json"),
            root.join("reference_integrity/receipt.json"),
        ] {
            let original = fs::read(&path).unwrap();
            let mut unknown: Value = serde_json::from_slice(&original).unwrap();
            unknown["command"] = json!("SECRET_PAYLOAD");
            write(&path, &unknown);
            assert_incomplete(
                &fixture.invoke("check", &root, RUN, &[]),
                "bidder_evidence_input",
            );
            // Deliberate duplicate wire keys: json! would erase the duplicate.
            let duplicate = format!(
                "{{\"run_id\":\"foreign\",{}",
                from_utf8(&original).unwrap().strip_prefix('{').unwrap()
            );
            for bytes in [
                duplicate.into_bytes(),
                vec![b'{'],
                vec![b' '; (1 << 20) + 1],
            ] {
                fs::write(&path, bytes).unwrap();
                let output = fixture.invoke("check", &root, RUN, &[]);
                assert_incomplete(&output, "bidder_evidence_input");
                assert!(!String::from_utf8_lossy(&output.stdout).contains("SECRET_PAYLOAD"));
            }
            fs::write(&path, &original).unwrap();
        }
        let extra = root.join("reference_integrity/extra.json");
        write(&extra, &json!({}));
        assert_incomplete(
            &fixture.invoke("check", &root, RUN, &[]),
            "bidder_evidence_membership",
        );
        fs::remove_file(extra).unwrap();
        fs::remove_file(root.join("reference_integrity/receipt.json")).unwrap();
        assert_incomplete(
            &fixture.invoke("check", &root, RUN, &[]),
            "bidder_evidence_membership",
        );
    }

    #[test]
    fn invalid_ids_unknown_producer_and_pending_waves_create_no_output() {
        let fixture = Fixture::new();
        for run in ["", "../escape", "has space", &"x".repeat(257)] {
            let root = fixture.root("invalid");
            assert_eq!(
                fixture.invoke("begin", &root, run, &[]).status.code(),
                Some(2_i32)
            );
            assert!(!root.exists());
        }
        for action in ["begin", "produce", "run"] {
            let root = fixture.root(action);
            assert_eq!(
                fixture
                    .invoke(action, &root, RUN, &["--wave", "pending.toml"])
                    .status
                    .code(),
                Some(2_i32)
            );
            assert!(!root.exists());
        }
        let root = fixture.root("unknown");
        let output = fixture.invoke("run", &root, RUN, &["--producer", "unknown"]);
        assert_eq!(output.status.code(), Some(2_i32));
        assert!(!root.exists());
        let begun = fixture.root("begun");
        fixture.begin(&begun, RUN);
        let producer_output = fixture.invoke("produce", &begun, RUN, &["--producer", "unknown"]);
        assert_eq!(producer_output.status.code(), Some(2_i32));
        assert_eq!(producer_output.stderr, b"bidder_evidence_producer\n");
        assert_eq!(fs::read_dir(&begun).unwrap().count(), 1);
    }

    #[test]
    fn producer_accepts_bare_manifest_and_sibling_output_from_the_run_directory() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .current_dir(&root)
            .args(["bidders", "evidence", "produce", "--source"])
            .arg(var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap())
            .arg("--inventory")
            .arg(&fixture.inventory)
            .args([
                "--run-manifest",
                "manifest.json",
                "--output",
                "reference_integrity",
                "--producer",
                "reference_integrity",
                "--run-id",
                RUN,
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0_i32), "{output:?}");
        let receipt = read(&root.join("reference_integrity/receipt.json"));
        assert_eq!(receipt["outcome"], "completed");
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
    }

    #[test]
    fn producer_output_must_be_the_manifest_sibling_slot_before_any_publication() {
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        for destination in [fixture.root("elsewhere"), root.join("unknown")] {
            let output = fixture.invoke_with_output("produce", &root, RUN, &[], Some(&destination));
            assert_eq!(output.status.code(), Some(2_i32), "{output:?}");
            assert_eq!(output.stderr, b"bidder_evidence_output\n");
            assert!(!destination.exists());
            assert!(!root.join("reference_integrity").exists());
            assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        }
        fixture.produce(&root, RUN);
        report(&fixture.invoke("check", &root, RUN, &[]), "complete");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_unsafe_parents_and_nonregular_inputs_are_refused() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let root = fixture.root("run");
        fixture.begin(&root, RUN);
        fixture.produce(&root, RUN);
        for path in [
            root.join("manifest.json"),
            root.join("reference_integrity/receipt.json"),
        ] {
            let saved = fixture.root("saved");
            fs::rename(&path, &saved).unwrap();
            symlink(&saved, &path).unwrap();
            assert_incomplete(
                &fixture.invoke("check", &root, RUN, &[]),
                "bidder_evidence_input",
            );
            fs::remove_file(&path).unwrap();
            fs::create_dir_all(&path).unwrap();
            assert_incomplete(
                &fixture.invoke("check", &root, RUN, &[]),
                "bidder_evidence_input",
            );
            fs::remove_dir(&path).unwrap();
            assert!(
                Command::new("/usr/bin/mkfifo")
                    .arg(&path)
                    .status()
                    .unwrap()
                    .success()
            );
            assert_incomplete(
                &fixture.invoke("check", &root, RUN, &[]),
                "bidder_evidence_input",
            );
            fs::remove_file(&path).unwrap();
            fs::rename(&saved, &path).unwrap();
        }
        let alias = fixture.root("alias");
        symlink(&root, &alias).unwrap();
        assert_incomplete(
            &fixture.invoke("check", &alias, RUN, &[]),
            "bidder_evidence_input",
        );
        let through_link = alias.join("new");
        assert_eq!(
            fixture
                .invoke("begin", &through_link, RUN, &[])
                .status
                .code(),
            Some(2_i32)
        );
        assert!(!root.join("new").exists());
        let saved = fixture.root("inventory-saved");
        fs::rename(&fixture.inventory, &saved).unwrap();
        symlink(&saved, &fixture.inventory).unwrap();
        assert_eq!(
            fixture
                .invoke("begin", &fixture.root("bad-input"), RUN, &[])
                .status
                .code(),
            Some(2_i32)
        );
        assert!(!fixture.root("bad-input").exists());
    }
}
