//! Wave-design checks do not execute producers, promote definitions or certify flows.

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeSet;
    use std::env::var_os;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::str::from_utf8;
    use tempfile::tempdir;

    fn manifest() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml")
    }

    fn pinned_manifest() -> toml::Value {
        let bytes = fs::read(manifest()).unwrap();
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            "aa9d38e3e6221c13d13b28ba18684e041f48e3c15608aa60ee11db86924a0c24",
            "complete frozen priority contract, including membership and per-owner operations"
        );
        toml::from_str(from_utf8(&bytes).unwrap()).unwrap()
    }

    fn run(path: &Path, complete: bool) -> Output {
        let bindings = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml",
        );
        run_with_bindings(path, complete, &bindings)
    }

    fn run_with_bindings(path: &Path, complete: bool, bindings: &Path) -> Output {
        let source = var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"));
        command
            .args(["bidders", "check", "--source"])
            .arg(source)
            .arg("--inventory")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                "../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml",
            ))
            .arg("--wave")
            .arg(path)
            .arg("--factory-bindings")
            .arg(bindings);
        if complete {
            command.arg("--require-complete");
        }
        command.output().unwrap()
    }

    #[test]
    fn priority_contract_names_exactly_the_source_backed_display_identities() {
        let data = pinned_manifest();
        let rows = data["bidders"].as_array().unwrap();
        let names: BTreeSet<_> = rows
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), rows.len());
        assert_eq!(rows.len(), 13);
        let mut total = 0;
        let mut owners = 0;
        for row in rows {
            let name = row["name"].as_str().unwrap();
            let operations = row["operations"].as_array().unwrap();
            for required in [
                "status_envelope",
                "privacy_limits",
                "sync_contract",
                "shared_feature_refusal",
            ] {
                assert_eq!(
                    operations
                        .iter()
                        .filter(|operation| operation["id"].as_str() == Some(required))
                        .count(),
                    1,
                    "{name}: {required}"
                );
            }
            total += operations.len();
            owners += row["source_files"].as_array().unwrap().len();
        }
        assert_eq!(total, 113);
        assert_eq!(owners, 38);
        assert_eq!(data["review_status"].as_str(), Some("pending"));
        let result = run(&manifest(), false);
        assert!(result.status.success(), "{result:?}");
    }

    #[test]
    fn checking_a_source_bound_wave_never_reports_execution_or_migration() {
        let result = run(&manifest(), false);
        assert!(result.status.success(), "{result:?}");
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["status"], "wave_design_valid");
        assert_eq!(report["scope"], "wave_design_only");
        assert_eq!(report["identities"], 13_u64);
        assert_eq!(report["migrated"], 0_u64);
        assert_eq!(report["wave"]["promoted"], 0_u64);
        assert_eq!(report["wave"]["review_status"], "pending");
        assert_eq!(report["wave"]["operations"], 113_u64);
        assert_eq!(report["wave"]["drivers"], 16_u64);
        let incomplete = run(&manifest(), true);
        assert_eq!(incomplete.status.code(), Some(1_i32));
        let incomplete_report: Value = serde_json::from_slice(&incomplete.stdout).unwrap();
        assert_eq!(incomplete_report["status"], "migration_incomplete");
        assert_eq!(incomplete_report["migrated"], 0_u64);
    }

    #[test]
    fn source_identity_schema_factory_and_owner_file_drift_are_independent_refusals() {
        let original: toml::Value =
            toml::from_str(&fs::read_to_string(manifest()).unwrap()).unwrap();
        let directory = tempdir().unwrap();
        let path = directory.path().join("wave.toml");
        let mut checked = 0_usize;
        for field in [
            "name",
            "base",
            "implementation",
            "metadata_blob",
            "schema_blob",
            "sync",
            "routing",
            "source_files",
            "missing_source",
        ] {
            let mut data = original.clone();
            let row = &mut data["bidders"].as_array_mut().unwrap()[0];
            if field == "missing_source" {
                row["source_files"].as_array_mut().unwrap().pop();
            } else if field == "source_files" {
                row[field].as_array_mut().unwrap()[0]["blob"] = toml::Value::String("a".repeat(40));
            } else {
                row[field] = toml::Value::String(match field {
                    "metadata_blob" | "schema_blob" => "a".repeat(40),
                    "sync" if row[field].as_str() == Some("absent") => "declared".to_owned(),
                    "sync" => "absent".to_owned(),
                    "routing" if row[field].as_str() == Some("https_review_pending") => {
                        "https_default_pending".to_owned()
                    }
                    "routing" => "https_review_pending".to_owned(),
                    _ => "fake_unknown_identity".to_owned(),
                });
            }
            fs::write(&path, toml::to_string(&data).unwrap()).unwrap();
            let result = run(&path, false);
            assert_eq!(result.status.code(), Some(1_i32), "{field}: {result:?}");
            let report: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(report["status"], "wave_source_drift", "{field}");
            assert_eq!(report["migrated"], 0_u64);
            checked += 1;
        }
        assert_eq!(checked, 9);
    }

    #[test]
    fn malformed_wave_input_is_fixed_diagnostic_without_echoing_its_contents() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("wave.toml");
        fs::write(&path, "SENSITIVE_CANARY invalid TOML").unwrap();
        let result = run(&path, false);
        assert_eq!(result.status.code(), Some(2_i32));
        assert!(result.stdout.is_empty());
        assert_eq!(
            String::from_utf8(result.stderr).unwrap().trim(),
            "bidder_wave_input"
        );
    }

    #[test]
    fn alias_cannot_claim_itself_as_base_or_inherit_undeclared_sync_support() {
        let original: toml::Value =
            toml::from_str(&fs::read_to_string(manifest()).unwrap()).unwrap();
        let directory = tempdir().unwrap();
        let path = directory.path().join("wave.toml");
        let mut checked = 0_usize;
        for field in ["base", "sync"] {
            let mut data = original.clone();
            let alias = data["bidders"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|row| row["name"] != row["base"] && row["sync"].as_str() == Some("absent"))
                .unwrap();
            alias[field] = if field == "base" {
                alias["name"].clone()
            } else {
                toml::Value::String("supports_only".to_owned())
            };
            fs::write(&path, toml::to_string(&data).unwrap()).unwrap();
            let result = run(&path, false);
            assert_eq!(result.status.code(), Some(1_i32), "{field}: {result:?}");
            let report: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(report["status"], "wave_source_drift");
            checked += 1;
        }
        assert_eq!(checked, 2);
    }

    #[test]
    fn source_ownership_is_checked_for_the_last_selected_identity_too() {
        let mut data: toml::Value =
            toml::from_str(&fs::read_to_string(manifest()).unwrap()).unwrap();
        let last = data["bidders"].as_array_mut().unwrap().last_mut().unwrap();
        last["source_files"].as_array_mut().unwrap()[0]["blob"] =
            toml::Value::String("a".repeat(40));
        let directory = tempdir().unwrap();
        let path = directory.path().join("wave.toml");
        fs::write(&path, toml::to_string(&data).unwrap()).unwrap();
        let result = run(&path, false);
        assert_eq!(result.status.code(), Some(1_i32), "{result:?}");
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["status"], "wave_source_drift");
        assert_eq!(report["migrated"], 0_u64);
    }

    #[test]
    fn factory_witness_cannot_be_replaced_by_a_consistently_remapped_proposal() {
        let directory = tempdir().unwrap();
        let bindings_path = directory.path().join("bindings.toml");
        let original = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml",
        ))
        .unwrap();
        let mut bindings: toml::Value = toml::from_str(&original).unwrap();
        let mut data: toml::Value =
            toml::from_str(&fs::read_to_string(manifest()).unwrap()).unwrap();
        let rows = data["bidders"].as_array().unwrap();
        let selected_name = rows[0]["name"].as_str().unwrap().to_owned();
        let donor = rows
            .iter()
            .find(|row| {
                row["implementation"] != rows[0]["implementation"]
                    && bindings["bindings"]
                        .get(row["name"].as_str().unwrap())
                        .is_some()
            })
            .unwrap()
            .clone();
        let donor_name = donor["name"].as_str().unwrap();
        assert_ne!(selected_name, donor_name);
        bindings["bindings"][selected_name.as_str()] = bindings["bindings"][donor_name].clone();
        fs::write(&bindings_path, toml::to_string(&bindings).unwrap()).unwrap();
        let selected = &mut data["bidders"].as_array_mut().unwrap()[0];
        selected["implementation"] = donor["implementation"].clone();
        selected["source_files"] = donor["source_files"].clone();
        let path = directory.path().join("wave.toml");
        fs::write(&path, toml::to_string(&data).unwrap()).unwrap();
        let result = run_with_bindings(&path, false, &bindings_path);
        assert_eq!(result.status.code(), Some(2_i32));
        assert!(result.stdout.is_empty());
        assert_eq!(
            String::from_utf8(result.stderr).unwrap().trim(),
            "bidder_wave_input"
        );
    }

    #[test]
    fn repository_root_defaults_find_the_inventory_and_factory_witness() {
        let result = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["bidders", "check", "--source"])
            .arg(var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap())
            .args([
                "--wave",
                "crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml",
            ])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["status"], "wave_design_valid");
        assert_eq!(report["migrated"], 0_u64);
    }

    #[test]
    fn a_factory_witness_without_a_wave_is_an_argument_error_not_an_ignored_input() {
        let result = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args([
                "bidders",
                "check",
                "--source",
                "unused",
                "--factory-bindings",
                "unused",
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2_i32));
        assert!(result.stdout.is_empty());
        let diagnostic = String::from_utf8(result.stderr).unwrap();
        assert!(diagnostic.contains("--wave"), "{diagnostic}");
        assert!(!diagnostic.contains("bidder_source"), "{diagnostic}");
    }
}
