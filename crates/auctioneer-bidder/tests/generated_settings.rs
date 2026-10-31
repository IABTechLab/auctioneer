//! Compile the settings contract with synthetic declarations in owned source copies.

#![cfg(all(unix, not(target_arch = "wasm32")))]

#[expect(
    dead_code,
    reason = "shared build runner also supports discovery probes"
)]
mod build_workspace;

#[cfg(test)]
mod tests {
    use super::build_workspace::{
        BuildWorkspace, install_synthetic_fake, prepare_synthetic_catalog, require_success,
    };
    use std::{
        fmt::Write as _,
        fs,
        io::{Write as _, stderr},
        panic::catch_unwind,
        path::Path,
    };

    fn copy(source: &Path, destination: &Path) {
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(source, destination).unwrap();
    }

    #[test]
    fn generated_settings_contract_uses_actual_apis_with_only_fake_declarations() {
        let workspace = BuildWorkspace::with_crates(
            "fake_unused_probe",
            "fn main() {}",
            &[
                "auctioneer-core",
                "auctioneer-exchange",
                "auctioneer-privacy",
            ],
        );
        prepare_synthetic_catalog(workspace.bidder());
        install_synthetic_fake(workspace.bidder());
        let source = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = source.join("../..");
        let core = workspace.root().join("crates/auctioneer-core");
        copy(
            &root.join("edgezero.toml"),
            &workspace.root().join("edgezero.toml"),
        );
        // This is a normal integration test of the copied crates, not source
        // imported into an alternate crate with substitute implementations.
        copy(
            &source.join("tests/support/generated_settings_contract.rs"),
            &core.join("tests/generated_settings_contract.rs"),
        );
        copy(
            &source.join("tests/support/generated_exchange_settings.rs"),
            &core.join("tests/generated_exchange_settings.rs"),
        );
        for module in ["bounded_store", "config_funding_support", "fanout_limits"] {
            copy(
                &root
                    .join("crates/auctioneer-core/tests")
                    .join(module)
                    .join("mod.rs"),
                &core.join("tests").join(module).join("mod.rs"),
            );
        }
        for (key, fields) in [
            (
                "fake_integer_probe",
                "primary_number = 'optional_i64'\nsecondary_number = 'optional_i64'",
            ),
            ("fake_string_probe", "token = 'optional_string'"),
        ] {
            let directory = workspace.bidder().join("src/bidders").join(key);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("bidder.toml"), format!(
                "key = '{key}'\nfactory = false\n[settings.fields]\n{fields}\n[provenance]\norigin = 'local'\n"
            )).unwrap();
        }
        let string_manifest = workspace
            .bidder()
            .join("src/bidders/fake_string_probe/bidder.toml");
        let mut declaration: toml::Value =
            toml::from_str(&fs::read_to_string(&string_manifest).unwrap()).unwrap();
        declaration.as_table_mut().unwrap().insert(
            "endpoint".into(),
            toml::toml! {
                [fields]
                a_route = "ZuluRoute"
                z_token = "AlphaToken"
            }
            .into(),
        );
        fs::write(string_manifest, toml::to_string(&declaration).unwrap()).unwrap();
        let output = workspace.cargo_package(
            "auctioneer-core",
            "test",
            &[
                "--test",
                "generated_settings_contract",
                "--",
                "--test-threads=1",
            ],
            true,
        );
        require_success(&output, "fake generated-settings contract");
        let stdout = String::from_utf8(output.stdout).unwrap();
        reconcile_tests(&stdout);
        writeln!(stderr(), "{stdout}").unwrap();
        workspace.report();
    }

    const EXPECTED_TESTS: [&str; 14] = [
        "allocation_meter::canonical_allocator_tracks_allocations_without_refunding_deallocations",
        "config_limits::typed_settings_byte_ceiling_includes_the_envelope_and_json_escaping",
        "generated_exchange_settings::tests::both_request_and_host_aliases_borrow_settings_with_independent_field_precedence",
        "generated_exchange_settings::tests::wrong_base_or_missing_binding_never_falls_back_through_typed_projection",
        "generated_exchange_settings::tests::wrong_requested_binding_blocks_only_that_alias_before_construction",
        "generated_endpoints::generated_bindings_cannot_relax_template_membership_or_traversal_refusals",
        "generated_endpoints::generated_bindings_encode_values_through_the_existing_capped_builder",
        "generated_endpoints::query_set_validates_borrowed_macros_even_in_values_it_would_remove",
        "request_binding::settings_binding_borrows_both_entries_and_preserves_fieldwise_zero_precedence",
        "request_binding::settings_binding_enforces_limits_for_programmatic_callers_and_clears_previous_binding",
        "request_binding::settings_binding_refuses_each_wrong_or_missing_implementation_before_projection",
        "retained_owner::tests::retaining_settings_moves_the_original_buffers_and_drops_unrelated_entries",
        "retention::moving_protocol_settings_out_of_config_does_not_rebuild_the_map_or_clone_values",
        "runtime::malformed_generated_settings_fail_closed_without_payload_text_in_diagnostics",
    ];

    fn reconcile_tests(stdout: &str) {
        let mut expected = EXPECTED_TESTS.to_vec();
        let mut passed: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(" ... ok"))
            .collect();
        expected.sort_unstable();
        passed.sort_unstable();
        assert_eq!(
            passed, expected,
            "the exact child contract must execute: {stdout}"
        );
        assert!(
            stdout.contains(
                "test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
            ),
            "{stdout}"
        );
    }

    #[test]
    #[should_panic(expected = "the exact child contract must execute")]
    fn child_report_rejects_zero_test_success() {
        reconcile_tests(
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
        );
    }

    #[test]
    fn child_report_requires_the_complete_unique_pass_set() {
        fn report(names: &[&str]) -> String {
            let mut output = String::new();
            for name in names {
                writeln!(output, "test {name} ... ok").unwrap();
            }
            output.push_str(
                "test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
            );
            output
        }
        reconcile_tests(&report(&EXPECTED_TESTS));
        assert!(catch_unwind(|| reconcile_tests(&report(&EXPECTED_TESTS[1..]))).is_err());
        let mut duplicate = EXPECTED_TESTS;
        duplicate[0] = duplicate[1];
        assert!(catch_unwind(|| reconcile_tests(&report(&duplicate))).is_err());
        let mut renamed = EXPECTED_TESTS;
        renamed[0] = "fake_renamed_test";
        assert!(catch_unwind(|| reconcile_tests(&report(&renamed))).is_err());
    }
}
