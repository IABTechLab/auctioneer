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
            &quote::quote!(
                fn main() {}
            )
            .to_string(),
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
                "key = '{key}'\nfactory = true\n[settings.fields]\n{fields}\n[provenance]\norigin = 'local'\n"
            )).unwrap();
            let (owner_source, config_source) = synthetic_config_owner(key);
            fs::write(directory.join("mod.rs"), owner_source).unwrap();
            fs::write(directory.join("config.rs"), config_source).unwrap();
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

    const EXPECTED_TESTS: [&str; 18] = [
        "config_callbacks::supplied_values_are_independent_even_when_disabled_or_overridden",
        "config_callbacks::effective_requiredness_and_fieldwise_fallback_use_the_same_callback",
        "config_callbacks::missing_null_zero_empty_and_request_local_context_remain_distinct",
        "config_callbacks::swapped_callbacks_fail_the_behavioral_contract_without_factories",
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
                "test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
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
                "test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
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

    fn synthetic_config_owner(key: &str) -> (String, String) {
        let (validation, error) = match key {
            "fake_integer_probe" => (
                quote::quote! {
                    let requested = self.0.requested.and_then(BidderSettings::as_fake_integer_probe);
                    let base = self.0.base.and_then(BidderSettings::as_fake_integer_probe);
                    for supplied in [requested, base].into_iter().flatten() {
                        if supplied.primary_number.is_some_and(|value| value < 0) {
                            errors.add("primary_number", ValidationError::new("invalid"));
                        }
                        if supplied.secondary_number.is_some_and(|value| value < 0) {
                            errors.add("secondary_number", ValidationError::new("invalid"));
                        }
                    }
                    if self.0.enabled && self.0.definition.is_some() {
                        let resolved = auctioneer_config::bidder_settings::fake_integer_probe::resolve(requested, base);
                        if resolved.primary_number().is_none() {
                            errors.add("primary_number", ValidationError::new("required"));
                        } else if resolved.secondary_number().is_none() {
                            errors.add("secondary_number", ValidationError::new("required"));
                        }
                    }
                },
                quote::quote! {
                    let fields = errors.field_errors();
                    let field = if fields.contains_key("primary_number") { "primary_number" } else { "secondary_number" };
                    let reason = if fields[field].iter().any(|error| error.code == "invalid") { "invalid" } else { "required" };
                    BidderConfigError {field, reason}
                },
            ),
            "fake_string_probe" => (
                quote::quote! {
                    let requested = self.0.requested.and_then(BidderSettings::as_fake_string_probe);
                    let base = self.0.base.and_then(BidderSettings::as_fake_string_probe);
                    for supplied in [requested, base].into_iter().flatten() {
                        if supplied.token.as_deref().is_some_and(str::is_empty) {
                            errors.add("token", ValidationError::new("invalid"));
                        }
                    }
                    if self.0.enabled && self.0.definition.is_some()
                        && auctioneer_config::bidder_settings::fake_string_probe::resolve(requested, base).token().is_none() {
                        errors.add("token", ValidationError::new("required"));
                    }
                },
                quote::quote! {
                    let fields = errors.field_errors();
                    BidderConfigError { field:"token", reason: if fields["token"].iter().any(|error| error.code == "invalid") { "invalid" } else { "required" } }
                },
            ),
            _ => panic!("unknown synthetic settings owner"),
        };
        let root = quote::quote! {
            mod config;
            use crate::{BidderConfigError, BidderConfigInput};
            use validator::Validate as _;
            pub(crate) fn create_bidder() -> Box<dyn crate::Bidder> { crate::fake_synthetic::create_bidder() }
            pub(crate) fn validate_config(input: BidderConfigInput<'_>) -> Result<(), BidderConfigError> {
                config::Config(input).validate().map_err(config::error)
            }
        }.to_string();
        let config = quote::quote! {
            use auctioneer_config::BidderSettings;
            use crate::{BidderConfigError, BidderConfigInput};
            use validator::{Validate, ValidationError, ValidationErrors};
            pub(super) struct Config<'a>(pub(super) BidderConfigInput<'a>);
            impl Validate for Config<'_> {
                fn validate(&self) -> Result<(), ValidationErrors> {
                    let mut errors = ValidationErrors::new();
                    #validation
                    if errors.is_empty() { Ok(()) } else { Err(errors) }
                }
            }
            pub(super) fn error(errors: ValidationErrors) -> BidderConfigError {
                #error
            }
        }
        .to_string();
        (root, config)
    }
}
