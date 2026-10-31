//! Compare real CLI subprocesses with the existing core boot probe's receipts.
//! Only the isolated adapter copy gets synthetic profiles; this parent never installs.

#[cfg(test)]
mod child_build;

#[cfg(test)]
mod tests {
    use super::child_build;
    use super::child_build::{Captured, ScratchCli, command, run};
    use auctioneer_core::host_primitives::boot_gates;
    use auctioneer_core::test_support::boot_parity_fixture as fixture;
    use auctioneer_runtime::IsolateTerm;
    use serde::Deserialize;
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fmt::Write as _;
    use std::str::from_utf8;
    use std::time::Duration;

    const BOOT_TEST_NAME: &str =
        "tests::boot_retains_resolved_inputs_and_matches_strict_accounting";
    const PREFIX: &str = "BOOT_PARITY_RECEIPT ";
    const OUTPUT_LIMIT: u64 = 1 << 20_u32;
    const EXPECTED_ADAPTER_BASELINES: [(&str, u64); 4] = [
        ("axum", 96_000_000),
        ("cloudflare", 96_010_007),
        ("fastly", 96_020_011),
        ("spin", 96_030_013),
    ];
    const RETAINED_FIELDS: [&str; 11] = [
        "raw_limits",
        "effective_limits",
        "target_constants",
        "construction_costs",
        "catalog_costs",
        "outbound_meta_bytes",
        "baked_config_bytes",
        "baked_config_sha256",
        "terms",
        "auction_receipts",
        "derived",
    ];

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Receipt {
        case: String,
        process_id: u32,
        installs: u32,
        retained: Value,
        warnings: Vec<String>,
    }

    fn receipt(stdout: &[u8], case: &str, pid: u32) -> Result<Receipt, &'static str> {
        let mut lines = from_utf8(stdout)
            .map_err(|_error| "non-UTF8 receipt")?
            .lines()
            .filter_map(|line| line.strip_prefix(PREFIX));
        let encoded = lines.next().ok_or("missing receipt")?;
        if lines.next().is_some() {
            return Err("duplicate receipt");
        }
        let result: Receipt = serde_json::from_str(encoded).map_err(|_error| "invalid receipt")?;
        if result.case != case || result.process_id != pid || result.installs != 1 {
            return Err("wrong receipt identity or install count");
        }
        Ok(result)
    }

    fn assert_receipt_controls() {
        let valid = json!({"case": "explicit", "process_id": 17_u32, "installs": 1_u32, "retained": {}, "warnings": []});
        let line = format!("{PREFIX}{valid}\n");
        receipt(line.as_bytes(), "explicit", 17).unwrap();
        for (invalid, error) in [
            ("running 0 tests\n".to_owned(), "missing receipt"),
            (format!("{line}{line}"), "duplicate receipt"),
            (format!("{PREFIX}{{\n"), "invalid receipt"),
        ] {
            assert_eq!(
                receipt(invalid.as_bytes(), "explicit", 17).map(|_| ()),
                Err(error)
            );
        }
        assert_eq!(
            receipt(line.as_bytes(), "all_default", 17).map(|_| ()),
            Err("wrong receipt identity or install count")
        );
        assert_eq!(
            receipt(line.as_bytes(), "explicit", 18).map(|_| ()),
            Err("wrong receipt identity or install count")
        );
        for (field, value, error) in [
            (
                "installs",
                json!(0_u32),
                "wrong receipt identity or install count",
            ),
            (
                "installs",
                json!(2_u32),
                "wrong receipt identity or install count",
            ),
            ("extra", json!(true), "invalid receipt"),
        ] {
            let mut invalid = valid.clone();
            invalid[field] = value;
            assert_eq!(
                receipt(format!("{PREFIX}{invalid}\n").as_bytes(), "explicit", 17).map(|_| ()),
                Err(error)
            );
        }
        for field in ["retained", "warnings"] {
            let mut invalid = valid.clone();
            invalid.as_object_mut().unwrap().remove(field);
            assert_eq!(
                receipt(format!("{PREFIX}{invalid}\n").as_bytes(), "explicit", 17).map(|_| ()),
                Err("invalid receipt")
            );
        }
    }

    fn cli(scratch: &ScratchCli, case: &str, strict: bool, mutation: Option<&str>) -> Captured {
        let mut child = command(scratch.executable());
        child
            .current_dir(scratch.source())
            .env(fixture::CASE_ENV, case)
            .args(["bidders", "resources", "--check", "--all-adapters"]);
        if strict {
            child.arg("--require-fit");
        }
        if let Some(kind) = mutation {
            child.env(child_build::MUTATION_ENV, kind);
        }
        run(&mut child, Duration::from_secs(30), OUTPUT_LIMIT)
    }

    fn report(output: &Captured, exit: i32) -> Value {
        assert_eq!(
            output.status.code(),
            Some(exit),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "CLI must not install or warn");
        assert!(output.stdout.ends_with(b"\n"));
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["schema_version"], 1_u16);
        assert_eq!(report["accounting_complete"], true);
        assert_eq!(report["selection"], json!({"scope": "all_adapters"}));
        let rows = report["adapters"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter()
                .map(|row| row["adapter"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["axum", "cloudflare", "fastly", "spin"]
        );
        report
    }

    fn assert_warning(installed: &Receipt, over: bool) {
        if !over {
            assert!(installed.warnings.is_empty(), "funded boot must not warn");
            return;
        }
        assert_eq!(
            installed.warnings.len(),
            4,
            "one headline and three dominant terms"
        );
        let warnings = installed.warnings.join("\n");
        assert_eq!(warnings.matches("Booting anyway").count(), 1);
        assert_eq!(warnings.matches("charges").count(), 3);
        let total = installed.retained["derived"]["total_bytes"]
            .as_u64()
            .unwrap();
        let cap = installed.retained["effective_limits"]["isolate_total_bytes"]
            .as_u64()
            .unwrap();
        for expected in [
            format!("requires {total} bytes"),
            format!("{cap}-byte platform ceiling"),
            format!("{} bytes over", total.checked_sub(cap).unwrap()),
        ] {
            assert!(
                warnings.contains(&expected),
                "missing {expected}: {warnings}"
            );
        }
    }

    fn assert_positive_terms(row: &Value) {
        let terms = row["terms"].as_object().unwrap();
        assert_eq!(terms.len(), IsolateTerm::ALL.len());
        for term in IsolateTerm::ALL {
            assert!(terms[term.name()].as_u64().unwrap() > 0);
        }
        let receipts = row["auction_receipts"].as_object().unwrap();
        assert_eq!(receipts.len(), 6);
        assert!(receipts.values().all(|value| value.as_u64().unwrap() > 0));
        for (term, bytes) in [
            ("catalog_resident_term", 1009_u64),
            ("catalog_initialization_term", 2017),
            ("schema_validation_term", 3011),
            ("catalog_context_term", 4001),
        ] {
            assert_eq!(row["derived"][term], bytes);
        }
        let auctions = row["effective_limits"]["max_concurrent_auctions"]
            .as_u64()
            .unwrap();
        assert!(auctions > 0);
        assert_eq!(
            row["terms"]["auctions"].as_u64().unwrap(),
            row["derived"]["per_auction_peak"]
                .as_u64()
                .unwrap()
                .checked_mul(auctions)
                .unwrap()
        );
    }

    fn assert_retained_inputs(row: &Value, installed: &Receipt, mask: u8) {
        let case = &installed.case;
        assert_eq!(
            installed
                .retained
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            RETAINED_FIELDS.into_iter().collect::<BTreeSet<_>>()
        );
        for field in RETAINED_FIELDS {
            assert_eq!(
                row[field], installed.retained[field],
                "{case}: retained {field}"
            );
        }
        for (field, bit, explicit, default) in [
            ("max_runtime_config_bytes", 1_u8, 7919_u64, 0x0100_0000_u64),
            ("max_runtime_config_blob_bytes", 2, 3571, 0x0080_0000),
            ("max_config_json_nodes", 4, 127, 8192),
        ] {
            assert_eq!(
                row["raw_limits"][field],
                if mask & bit == 0 { explicit } else { 0 }
            );
            assert_eq!(
                row["effective_limits"][field],
                if mask & bit == 0 { explicit } else { default }
            );
        }
        // The boot probe independently serializes its actual compiled tuple.
        // Parent --all-targets feature unification must not supply this oracle.
        assert!(row["baked_config_bytes"].as_u64().unwrap() > 0);
        let digest = row["baked_config_sha256"].as_str().unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            row["effective_limits"]["max_concurrent_auctions"],
            fixture::profile_for(case).limits.max_concurrent_auctions
        );
        assert_positive_terms(row);
    }

    fn assert_mutation_refusals(scratch: &ScratchCli) {
        let all_rows_then_validation = &["Axum", "Cloudflare", "Fastly", "Spin", "Axum"][..];
        let mut checked = 0_u32;
        for (mutation, adapters) in [
            ("raw_zero", all_rows_then_validation),
            ("zero_baked", all_rows_then_validation),
            ("raw_zero_cloudflare", &["Cloudflare", "Cloudflare"][..]),
            ("zero_baked_spin", &["Spin", "Spin"][..]),
        ] {
            let output = cli(scratch, "all_default_caps", true, Some(mutation));
            assert_eq!(
                output.status.code(),
                Some(2_i32),
                "{mutation}: must refuse corrupt real CLI accounting"
            );
            assert!(output.stdout.is_empty());
            let mut expected = String::new();
            for adapter in adapters {
                writeln!(expected, "RESOURCE_PARITY_MUTATION {mutation} {adapter}").unwrap();
            }
            expected.push_str("bidder_resource_input\n");
            assert_eq!(
                output.stderr,
                expected.as_bytes(),
                "{mutation}: intended adapter must execute corruption before refusal"
            );
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 4);
    }

    #[test]
    #[expect(
        clippy::print_stdout,
        reason = "bounded per-case verification evidence"
    )]
    fn boot_and_real_cli_retain_the_same_resource_budget() {
        assert!(boot_gates().is_none());
        assert_receipt_controls();
        let scratch = ScratchCli::build();
        let mut counted = 0_u32;
        for (case, mask, over) in fixture::CASES {
            let inspect = report(&cli(&scratch, case, false, None), 0);
            let strict = report(&cli(&scratch, case, true, None), i32::from(over));
            assert_eq!(strict, inspect, "strictness changes policy, not accounting");
            for (row, (adapter, expected_baseline)) in strict["adapters"]
                .as_array()
                .unwrap()
                .iter()
                .zip(EXPECTED_ADAPTER_BASELINES)
            {
                let boot = run(
                    command(scratch.boot_executable())
                        .current_dir(scratch.source())
                        .env(fixture::CASE_ENV, case)
                        .env(fixture::ADAPTER_ENV, adapter)
                        .args(["--exact", BOOT_TEST_NAME, "--nocapture"]),
                    Duration::from_secs(30),
                    OUTPUT_LIMIT,
                );
                assert!(
                    boot.status.success(),
                    "{case}: {}\n{}",
                    String::from_utf8_lossy(&boot.stdout),
                    String::from_utf8_lossy(&boot.stderr)
                );
                assert!(boot.stderr.is_empty());
                assert!(String::from_utf8_lossy(&boot.stdout).contains("1 passed; 0 failed"));
                let installed = receipt(&boot.stdout, case, boot.pid).unwrap();
                counted = counted.checked_add(installed.installs).unwrap();
                assert_warning(&installed, over);
                assert_eq!(row["adapter"], adapter);
                assert_eq!(row["raw_limits"]["baseline_bytes"], expected_baseline);
                assert_retained_inputs(row, &installed, mask);
                assert_eq!(row["required_fit"], !over);
                let validation = &row["validation"];
                assert_eq!(validation["required_fit"], !over);
                assert_eq!(validation["configured_cap_exceeded"], over);
                assert_eq!(validation["platform_cap_exceeded"], false);
                assert_eq!(validation["population_covered"], true);
                assert_eq!(validation["profile_matches_dispatch"], true);
                assert_eq!(validation["external_blockers"], json!([]));
                println!(
                    "adapter={adapter} case={case} mask={mask} boot_pid={} installs={} strict_exit={} total={} baked={}",
                    boot.pid,
                    installed.installs,
                    i32::from(over),
                    row["derived"]["total_bytes"],
                    row["baked_config_bytes"]
                );
            }
        }
        assert_eq!(
            counted, 40,
            "every existing boot child must actually install"
        );
        assert_mutation_refusals(&scratch);
        println!(
            "four traced scratch-only CLI mutations refused before exporting corrupt accounting"
        );
        assert!(boot_gates().is_none());
    }
}
