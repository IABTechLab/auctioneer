//! Exercise the real offline resource command, not a replacement calculator.

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::fs;
    use std::process::{Command, Output};
    use std::str::from_utf8;

    fn run(arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .args(["bidders", "resources"])
            .args(arguments)
            .env(
                "AUCTIONEER__TEST__RESOURCE_SECRET_CANARY",
                "DO_NOT_EXPORT_RESOURCE_SECRET",
            )
            .output()
            .unwrap()
    }

    fn report(output: &Output) -> Value {
        assert!(output.stdout.len() <= 1 << 20_usize);
        assert!(output.stdout.ends_with(b"\n"));
        for stream in [&output.stdout, &output.stderr] {
            assert!(!String::from_utf8_lossy(stream).contains("DO_NOT_EXPORT_RESOURCE_SECRET"));
        }
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn run_all(arguments: &[&str]) -> Output {
        let mut selected_arguments = arguments.to_vec();
        selected_arguments.push("--all-adapters");
        run(&selected_arguments)
    }

    #[test]
    fn adapter_selector_uses_the_same_names_as_build_and_deploy() {
        for (adapter, abi) in [
            ("axum", "native"),
            ("cloudflare", "wasm32-unknown-unknown"),
            ("fastly", "wasm32-wasip1"),
            ("spin", "wasm32-wasip2"),
        ] {
            let output = run(&["--check", "--adapter", adapter]);
            assert_eq!(output.status.code(), Some(0_i32), "{output:?}");
            let value = report(&output);
            assert_eq!(value["selection"]["scope"], "adapter");
            assert_eq!(value["selection"]["adapter"], adapter);
            let rows = value["adapters"].as_array().unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["adapter"], adapter);
            assert_eq!(rows[0]["identity"]["target_abi"], abi);
            assert!(value.get("targets").is_none());
            let strict = run(&["--check", "--adapter", adapter, "--require-fit"]);
            assert_eq!(strict.status.code(), Some(1_i32), "{strict:?}");
            assert_eq!(value, report(&strict));
        }
    }

    #[test]
    fn adapter_selection_is_explicit_and_does_not_accept_compiler_targets() {
        let review = "1111111111111111111111111111111111111111";
        for arguments in [
            vec!["--check"],
            vec!["--check", "--adapter", "wasm32-wasip1"],
            vec!["--check", "--adapter", "unknown"],
            vec!["--check", "--adapter", "fastly", "--all-adapters"],
            vec!["--check", "--adapter", "fastly", "--capture-audit"],
            vec!["--check", "--adapter", "fastly", "--audit", "audit.toml"],
            vec!["--check", "--target", "wasm32-wasip1"],
            vec!["--check", "--capture-audit"],
            vec!["--check", "--audit", "audit.toml"],
            vec!["--check", "--audit", "audit.toml", "--plan", "plan.toml"],
            vec!["--check", "--review-commit", review, "--staged"],
            vec!["--check", "--review-commit", review, "--head", "HEAD"],
            vec![
                "--check",
                "--adapter",
                "fastly",
                "--review-commit",
                review,
                "--staged",
            ],
            vec![
                "--check",
                "--adapter",
                "fastly",
                "--review-commit",
                review,
                "--head",
                "HEAD",
            ],
        ] {
            let output = run(&arguments);
            assert_eq!(
                output.status.code(),
                Some(2_i32),
                "{arguments:?}: {output:?}"
            );
            assert!(output.stdout.is_empty());
            assert!(output.stderr.starts_with(b"error:"), "{output:?}");
        }
    }

    #[test]
    fn all_adapter_report_preserves_accounting_features_and_strict_refusal() {
        let output = run_all(&["--check"]);
        assert_eq!(output.status.code(), Some(0_i32), "{output:?}");
        let value = report(&output);
        let strict = run_all(&["--check", "--require-fit"]);
        assert_eq!(strict.status.code(), Some(1_i32), "{strict:?}");
        assert_eq!(value, report(&strict));
        assert_eq!(value["selection"]["scope"], "all_adapters");
        let rows = value["adapters"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter()
                .map(|row| row["adapter"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["axum", "cloudflare", "fastly", "spin"]
        );
        for (row, name) in rows.iter().zip(["axum", "cloudflare", "fastly", "spin"]) {
            assert_eq!(row["adapter"], name);
            assert_eq!(row["required_fit"], false);
            assert!(row["baked_config_bytes"].as_u64().unwrap() > 0);
            assert_layout_caps_and_features(row, name);
            let terms = row["terms"].as_object().unwrap();
            let expected: Vec<_> = auctioneer_runtime::IsolateTerm::ALL
                .iter()
                .map(|term| term.name())
                .collect();
            assert_eq!(terms.len(), expected.len());
            for term in expected {
                assert!(terms.contains_key(term));
            }
            let (resident, initialization, scratch) = if name == "axum" {
                (0x0100_0000_u64, 0x0080_0000_u64, 0x0008_0000_u64)
            } else {
                (0, 0, 0)
            };
            assert_eq!(row["terms"]["catalog_resident"], resident);
            assert_eq!(row["terms"]["catalog_initialization"], initialization);
            assert_eq!(
                row["catalog_costs"]["schema_validation_per_auction_bytes"],
                scratch
            );
            assert_eq!(row["auction_receipts"].as_object().unwrap().len(), 6);
            assert!(
                !row["validation"]["external_blockers"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        assert_platform_facts(rows);
    }

    fn assert_platform_facts(rows: &[Value]) {
        let axum = rows.first().unwrap();
        assert!(axum["validation"]["effective_ceiling_bytes"].is_null());
        assert!(axum["validation"]["minimum_headroom_bytes"].is_null());
        assert!(axum["validation"]["gap_bytes"].is_null());
        let spin = rows.last().unwrap();
        assert_eq!(spin["platform_profile"], "spin_hosted");
        assert_eq!(spin["dispatch_profile"], "spin_hosted");
        assert_eq!(spin["validation"]["profile_matches_dispatch"], true);
        assert_eq!(spin["dispatch_facts"], spin["platform_facts"]);
        let blockers = spin["validation"]["external_blockers"].as_array().unwrap();
        assert!(
            blockers
                .iter()
                .all(|reason| reason != "platform_dispatch_mismatch")
        );
        assert!(
            blockers
                .iter()
                .any(|reason| reason == "unknown_host_ingress_memory_accounting")
        );
    }

    fn assert_layout_caps_and_features(row: &Value, name: &str) {
        let (layout, backend, blob) = if name == "axum" {
            ("native_control", 0x0002_0000_u64, 0x0001_0000_u64)
        } else {
            ("host_native_substitute", 0x0010_0000_u64, 0x0010_0000_u64)
        };
        assert_eq!(row["identity"]["layout_evidence"], layout);
        assert_eq!(row["effective_limits"]["max_runtime_config_bytes"], backend);
        assert_eq!(
            row["effective_limits"]["max_runtime_config_blob_bytes"],
            blob
        );
        assert_eq!(row["effective_limits"]["max_config_json_nodes"], 8_192_u32);
        assert_eq!(
            row["construction_costs"]["fixed_per_auction_bytes"],
            // Audit/scope, wire shape, and measured native bridge owners.
            800_u64 + 32 + 19_160
        );
        assert_eq!(
            row["construction_costs"]["reusable_scratch_bytes"],
            // Scrub frames plus the independently funded credential scratch bank.
            2_488_u64 + 128 * 1_024
        );
        assert_eq!(
            row["construction_costs"]["retained_per_scope_bytes"],
            78_u64
        );
        let features = row["identity"]["features"].as_array().unwrap();
        assert!(features.iter().any(|feature| feature == "application"));
        assert_eq!(
            features
                .iter()
                .any(|feature| feature == "conformance_suite"),
            auctioneer_core::CONFORMANCE_SUITE_ENABLED
        );
        assert_eq!(
            features
                .iter()
                .any(|feature| feature == "test_harness_override"),
            auctioneer_config_data::TEST_HARNESS_OVERRIDE_ENABLED
        );
    }

    #[test]
    fn require_fit_requires_check_and_does_not_accept_platform_overrides() {
        assert_eq!(run_all(&["--require-fit"]).status.code(), Some(2_i32));
        assert_eq!(
            run_all(&["--check", "--platform-memory", "999999999999"])
                .status
                .code(),
            Some(2_i32)
        );
    }

    #[test]
    fn frozen_review_requires_exactly_one_diff_selector_and_cannot_claim_fit() {
        let review = "1111111111111111111111111111111111111111";
        for arguments in [
            vec!["--check", "--review-commit", review],
            vec!["--check", "--head", "HEAD"],
            vec!["--check", "--staged"],
            vec![
                "--check",
                "--review-commit",
                review,
                "--head",
                "HEAD",
                "--staged",
            ],
            vec![
                "--check",
                "--review-commit",
                review,
                "--head",
                "HEAD",
                "--require-fit",
            ],
        ] {
            let output = run_all(&arguments);
            assert_eq!(
                output.status.code(),
                Some(2_i32),
                "{arguments:?}: {output:?}"
            );
            assert!(output.stdout.is_empty());
            assert!(
                output.stderr.starts_with(b"error:"),
                "parser must refuse {arguments:?}: {output:?}"
            );
        }
    }

    #[test]
    fn frozen_review_reaches_validation_and_refuses_a_missing_commit() {
        let output = run_all(&[
            "--check",
            "--review-commit",
            "0000000000000000000000000000000000000000",
            "--head",
            "HEAD",
        ]);
        assert_eq!(output.status.code(), Some(2_i32));
        assert_eq!(output.stderr, b"bidder_resource_input\n");
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn audit_modes_require_check_and_cannot_mix_capture_review_or_fit() {
        for arguments in [
            vec!["--capture-audit"],
            vec!["--check", "--plan", "plan.toml"],
            vec!["--check", "--capture-audit", "--audit", "audit.toml"],
            vec!["--check", "--capture-audit", "--require-fit"],
            vec![
                "--check",
                "--capture-audit",
                "--review-commit",
                "1111111111111111111111111111111111111111",
                "--head",
                "HEAD",
            ],
            vec![
                "--check",
                "--audit",
                "audit.toml",
                "--plan",
                "plan.toml",
                "--require-fit",
            ],
            vec![
                "--check",
                "--audit",
                "audit.toml",
                "--plan",
                "plan.toml",
                "--review-commit",
                "1111111111111111111111111111111111111111",
                "--head",
                "HEAD",
            ],
        ] {
            let output = run_all(&arguments);
            assert_eq!(
                output.status.code(),
                Some(2_i32),
                "{arguments:?}: {output:?}"
            );
            assert!(output.stdout.is_empty());
            assert!(
                output.stderr.starts_with(b"error:"),
                "parser must refuse {arguments:?}: {output:?}"
            );
        }
    }

    #[test]
    fn audit_capture_and_render_use_the_real_source_identity_without_overwriting_inputs() {
        let report = run_all(&["--check"]);
        let identity = serde_json::from_slice::<Value>(&report.stdout).unwrap();
        let clean = identity["adapters"][0]["identity"]["dirty"] == false
            && identity["adapters"][0]["identity"]["source_tree_matches_commit"] == true;
        let captured = run_all(&["--check", "--capture-audit"]);
        if !clean {
            assert_eq!(captured.status.code(), Some(2_i32));
            assert!(captured.stdout.is_empty());
            assert_eq!(captured.stderr, b"bidder_resource_input\n");
            return;
        }
        assert_eq!(captured.status.code(), Some(0_i32), "{captured:?}");
        let audit: toml::Value = toml::from_str(from_utf8(&captured.stdout).unwrap()).unwrap();
        assert_eq!(audit["adapters"].as_array().unwrap().len(), 4);
        assert_eq!(
            audit["code_base_commit"].as_str().unwrap(),
            identity["adapters"][0]["identity"]["code_base_commit"]
                .as_str()
                .unwrap()
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("audit.toml");
        fs::write(&path, &captured.stdout).unwrap();
        let output = run_all(&["--check", "--audit", path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(0_i32), "{output:?}");
        assert!(output.stdout.starts_with(b"# Adapter Resource Envelopes\n"));
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("No remediation package is approved")
        );
        assert_eq!(fs::read(&path).unwrap(), captured.stdout);
    }

    #[test]
    fn current_audit_refuses_malformed_records_without_echoing_record_contents() {
        let directory = tempfile::tempdir().unwrap();
        let audit = directory.path().join("audit.toml");
        let plan = directory.path().join("plan.toml");
        fs::write(&audit, "not_a_field = 'SECRET_RECORD_CANARY'\n").unwrap();
        fs::write(&plan, "schema_version = 1\n").unwrap();
        let output = run_all(&[
            "--check",
            "--audit",
            audit.to_str().unwrap(),
            "--plan",
            plan.to_str().unwrap(),
        ]);
        assert_eq!(output.status.code(), Some(2_i32));
        assert_eq!(output.stderr, b"bidder_resource_input\n");
        assert!(output.stdout.is_empty());
    }

    const CATALOG_BASELINE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml"
    );

    const PATCHED_CATALOG_BASELINE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline-patched-lock.toml"
    );

    #[test]
    fn patched_catalog_capture_is_accepted_only_for_its_recorded_build() {
        let source = report(&run_all(&["--check"]));
        let bytes = fs::read(PATCHED_CATALOG_BASELINE).unwrap();
        let baseline: toml::Value = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
        let frozen = serde_json::to_value(&baseline["adapters"][0]["report"]).unwrap();
        let matching_build = [
            "sdk_revision",
            "lock_sha256",
            "features",
            "compiler_verbose",
            "declared_toolchain",
            "compiled_target",
            "reporter_abi",
            "target_abi",
            "layout_evidence",
        ]
        .iter()
        .all(|field| source["adapters"][0]["identity"][*field] == frozen["identity"][*field])
            && source["adapters"][0]["outbound_meta_bytes"] == frozen["outbound_meta_bytes"];
        let output = run_all(&["--check", "--catalog-baseline", PATCHED_CATALOG_BASELINE]);
        let strict = run_all(&[
            "--check",
            "--catalog-baseline",
            PATCHED_CATALOG_BASELINE,
            "--require-fit",
        ]);
        if matching_build {
            assert_eq!(output.status.code(), Some(0_i32));
            let value = report(&output);
            assert_eq!(value["scope"], "matched_catalog_arithmetic_only");
            assert_eq!(value["catalog_qualification"], "not_evaluated");
            assert_eq!(value["candidate"], source);
            assert_eq!(
                value["baseline"]["code_base_commit"],
                "7286bedfd1f1999edae9a5e3e5665b1f95117541"
            );
            assert_eq!(
                value["baseline"]["sha256"],
                "1d20dfa95f09a32a45f106f5503094efd71a718d794814fac353f83f357219d0"
            );
            let continuity = &value["baseline"]["continuity"];
            assert_eq!(
                continuity["original_code_base_commit"],
                "e71e40aba611215bb9305f82e2449e13fb0f41ed"
            );
            assert_eq!(
                continuity["original_sha256"],
                "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39"
            );
            let comparisons = value["comparisons"].as_array().unwrap();
            assert_eq!(comparisons.len(), 4);
            let origins = continuity["adapters"].as_array().unwrap();
            assert_eq!(origins.len(), 4);
            for ((comparison, origin), (adapter, scaled)) in comparisons.iter().zip(origins).zip([
                ("axum", 0x0002_0000_u64),
                ("cloudflare", 0x0008_0000),
                ("fastly", 0x0002_0000),
                ("spin", 0x0002_0000),
            ]) {
                assert_eq!(comparison["adapter"], adapter);
                assert_eq!(comparison["total_delta_bytes"], 0_u64);
                assert_eq!(comparison["absolute_required_fit"], false);
                assert_eq!(origin["adapter"], adapter);
                assert_eq!(
                    origin["auxiliary_chooser_per_permit_delta_bytes"],
                    0x0001_0000_u64
                );
                assert_eq!(origin["auxiliary_chooser_component_delta_bytes"], scaled);
            }
            assert_eq!(strict.status.code(), Some(1_i32));
            assert_eq!(report(&strict), value);
        } else {
            for refusal in [output, strict] {
                assert_eq!(refusal.status.code(), Some(2_i32));
                assert_eq!(refusal.stderr, b"bidder_resource_input\n");
                assert!(refusal.stdout.is_empty());
            }
        }
    }

    #[test]
    fn catalog_comparison_requires_an_explicit_complete_adapter_selection() {
        for arguments in [
            vec!["--check", "--catalog-baseline", CATALOG_BASELINE],
            vec![
                "--check",
                "--adapter",
                "axum",
                "--catalog-baseline",
                CATALOG_BASELINE,
            ],
            vec![
                "--check",
                "--all-adapters",
                "--capture-audit",
                "--catalog-baseline",
                CATALOG_BASELINE,
            ],
            vec![
                "--check",
                "--all-adapters",
                "--audit",
                CATALOG_BASELINE,
                "--catalog-baseline",
                CATALOG_BASELINE,
            ],
        ] {
            let output = run(&arguments);
            assert_eq!(output.status.code(), Some(2_i32));
            assert!(output.stdout.is_empty());
            assert!(output.stderr.starts_with(b"error:"));
        }
    }

    #[test]
    fn catalog_comparison_refuses_a_baseline_with_a_different_lockfile() {
        let source = report(&run_all(&["--check"]));
        let bytes = fs::read(CATALOG_BASELINE).unwrap();
        let baseline: toml::Value = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
        let frozen = serde_json::to_value(&baseline["adapters"][0]["report"]).unwrap();
        assert_ne!(
            source["adapters"][0]["identity"]["lock_sha256"],
            frozen["identity"]["lock_sha256"]
        );
        let output = run_all(&["--check", "--catalog-baseline", CATALOG_BASELINE]);
        assert_eq!(output.status.code(), Some(2_i32));
        assert_eq!(output.stderr, b"bidder_resource_input\n");
        assert!(output.stdout.is_empty());
        let strict = run_all(&[
            "--check",
            "--catalog-baseline",
            CATALOG_BASELINE,
            "--require-fit",
        ]);
        assert_eq!(strict.status.code(), Some(2_i32));
        assert_eq!(strict.stderr, b"bidder_resource_input\n");
        assert!(strict.stdout.is_empty());
    }

    #[test]
    fn catalog_comparison_rejects_semantically_identical_unpinned_baselines() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("baseline.toml");
        let mut bytes = fs::read(CATALOG_BASELINE).unwrap();
        bytes.extend_from_slice(b"\n# SECRET_BASELINE_CANARY\n");
        fs::write(&path, &bytes).unwrap();
        let output = run_all(&["--check", "--catalog-baseline", path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(2_i32));
        assert_eq!(output.stderr, b"bidder_resource_input\n");
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
