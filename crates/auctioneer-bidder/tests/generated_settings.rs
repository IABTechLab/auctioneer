//! Compile the settings contract with synthetic declarations in owned source copies.

#![cfg(all(unix, not(target_arch = "wasm32")))]

mod bidder_settings_compile;
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
        let workspace = BuildWorkspace::with_prepared_crates(
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
            prepare_settings_contract,
        );
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
        run_catalog_resource_variants(&workspace);
    }

    const RESOURCE_TEST: &str = "sdk_validation_catalog_scratch_fits_config_charge";

    fn run_catalog_resource_variants(workspace: &BuildWorkspace) {
        let path = workspace
            .root()
            .join("crates/auctioneer-core/tests/generated_settings_contract.rs");
        let mut contract = syn::parse_file(&fs::read_to_string(&path).unwrap()).unwrap();
        let test: syn::Item = syn::parse_quote! {
            #[test]
            fn sdk_validation_catalog_scratch_fits_config_charge() {
                sdk_validation_allocation::check();
            }
        };
        contract.items.push(test);
        fs::write(path, quote::quote!(#contract).to_string()).unwrap();

        let string_path = workspace
            .bidder()
            .join("src/bidders/fake_string_probe/bidder.toml");
        let mut string: toml::Value =
            toml::from_str(&fs::read_to_string(&string_path).unwrap()).unwrap();
        assert!(
            string
                .as_table_mut()
                .unwrap()
                .remove("definitions")
                .is_some()
        );
        fs::write(string_path, toml::to_string(&string).unwrap()).unwrap();
        let integer_path = workspace
            .bidder()
            .join("src/bidders/fake_integer_probe/bidder.toml");
        let mut integer: toml::Value =
            toml::from_str(&fs::read_to_string(&integer_path).unwrap()).unwrap();
        let mut failures = Vec::new();
        let mut previous_definitions = 3;
        for count in [1_usize, 256, 512] {
            assert_eq!(
                integer["definitions"].as_table().unwrap().len(),
                previous_definitions
            );
            let definitions = (0..count)
                .map(|index| {
                    (
                        format!("fake_resource_{index:04}_{}", "n".repeat(45)),
                        toml::Value::Table(toml::toml! {
                            endpoint = "https://settings.example.test/openrtb"
                            defaultEnabled = false
                            [capabilities.site]
                            mediaTypes = ["banner"]
                        }),
                    )
                })
                .collect::<toml::Table>();
            integer["definitions"] = toml::Value::Table(definitions);
            fs::write(&integer_path, toml::to_string(&integer).unwrap()).unwrap();
            let output = workspace.cargo_package(
                "auctioneer-core",
                "test",
                &[
                    "--test",
                    "generated_settings_contract",
                    RESOURCE_TEST,
                    "--",
                    "--exact",
                    "--nocapture",
                    "--test-threads=1",
                ],
                true,
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            let diagnostic = String::from_utf8(output.stderr).unwrap();
            writeln!(
                stderr(),
                "catalog resource variant definitions={count}:\n{stdout}\n{diagnostic}"
            )
            .unwrap();
            reconcile_resource_test(&stdout, &diagnostic, count, output.status.success());
            if !output.status.success() {
                failures.push(format!("definitions={count}: {}", output.status));
            }
            previous_definitions = count;
        }
        workspace.report();
        assert!(
            failures.is_empty(),
            "catalog resource variants failed without changing config funding: {failures:?}"
        );
    }

    fn reconcile_resource_test(stdout: &str, diagnostic: &str, definitions: usize, success: bool) {
        let suffix = if success { " ... ok" } else { " ... FAILED" };
        let passed: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(suffix))
            .collect();
        assert_eq!(
            passed,
            [RESOURCE_TEST],
            "the actual SDK allocation case must execute once: {stdout}"
        );
        let summary = if success {
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 31 filtered out"
        } else {
            "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 31 filtered out"
        };
        assert!(stdout.contains(summary), "{stdout}");
        let measurements: Vec<_> = diagnostic
            .lines()
            .filter(|line| line.starts_with("sdk-validation-allocation:"))
            .collect();
        assert_eq!(
            measurements.len(),
            2,
            "both resource cases must report: {diagnostic}"
        );
        for (measurement, case, payload, cap, nodes, component, charge) in [
            (measurements[0], "minimal", 20, 155, 6, 43_573, 228_789),
            (measurements[1], "invalid-tmax", 69, 204, 9, 58_035, 243_251),
        ] {
            assert!(measurement.starts_with(&format!("sdk-validation-allocation: definitions={definitions} name_bytes=64 case={case} payload={payload} backend_cap={cap} blob_cap={cap} nodes={nodes} product_bytes=1447 component_charge={component} ")), "{measurement}");
            assert!(
                measurement.ends_with(&format!("charge={charge} callbacks=0")),
                "{measurement}"
            );
        }
    }

    fn prepare_settings_contract(workspace: &BuildWorkspace) {
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
        install_settings_observations(workspace.bidder());
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
            let path = directory.join("bidder.toml");
            let mut manifest: toml::Value =
                toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            let names: &[(&str, Option<&str>, bool)] = if key == "fake_integer_probe" {
                &[
                    ("fake_wire_base", None, false),
                    ("fake_wire_parent", Some("fake_wire_base"), true),
                    ("fake_wire_leaf", Some("fake_wire_parent"), true),
                ]
            } else {
                &[("fake_wire_text", None, false)]
            };
            let definitions = names
                .iter()
                .map(|(name, parent, enabled)| {
                    let mut definition = toml::toml! {
                        endpoint = "https://settings.example.test/openrtb"
                        [capabilities.site]
                        mediaTypes = ["banner"]
                    };
                    definition.insert("defaultEnabled".into(), (*enabled).into());
                    if let Some(parent) = parent {
                        definition.insert("aliasOf".into(), (*parent).into());
                    }
                    ((*name).to_owned(), toml::Value::Table(definition))
                })
                .collect::<toml::Table>();
            manifest
                .as_table_mut()
                .unwrap()
                .insert("definitions".into(), toml::Value::Table(definitions));
            fs::write(path, toml::to_string(&manifest).unwrap()).unwrap();
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
    }

    const EXPECTED_TESTS: [&str; 31] = [
        "compiled_sdk::sdk_and_typed_paths_dispatch_discovered_callbacks_once_with_real_compiled_aliases",
        "compiled_sdk::sdk_and_typed_paths_reject_bindings_before_any_discovered_callback",
        "compiled_sdk::sdk_and_typed_paths_check_disabled_and_shadowed_supplied_settings",
        "compiled_sdk::sdk_and_typed_paths_preserve_compiled_ancestry_and_per_identity_requiredness",
        "config_callbacks::complete_policy_validation_preserves_generated_settings_and_owner_refusals",
        "config_limits::aggregate_validation_delegates_structural_limits_with_fixed_redacted_errors",
        "config_callbacks::shared_admission_rejects_settings_for_unlinked_factories",
        "config_callbacks::shared_admission_visits_each_identity_once_with_fieldwise_alias_fallback",
        "config_callbacks::shared_admission_checks_disabled_and_shadowed_supplied_values",
        "config_callbacks::shared_admission_enforces_requiredness_only_for_known_enabled_identities",
        "config_callbacks::shared_admission_rejects_bindings_before_any_callback",
        "config_callbacks::shared_admission_rejects_settings_limits_before_any_callback",
        "config_callbacks::shared_admission_redacts_rejected_values_and_credential_references",
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
                "test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
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
                "test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
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

        let resource_stdout = format!(
            "test {RESOURCE_TEST} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 31 filtered out\n"
        );
        let resource_diagnostic = "sdk-validation-allocation: definitions=256 name_bytes=64 case=minimal payload=20 backend_cap=155 blob_cap=155 nodes=6 product_bytes=1447 component_charge=43573 peak=1 retained=0 cumulative=1 charge=228789 callbacks=0\nsdk-validation-allocation: definitions=256 name_bytes=64 case=invalid-tmax payload=69 backend_cap=204 blob_cap=204 nodes=9 product_bytes=1447 component_charge=58035 peak=1 retained=0 cumulative=1 charge=243251 callbacks=0\n";
        reconcile_resource_test(&resource_stdout, resource_diagnostic, 256, true);
        for diagnostic in [
            resource_diagnostic.replace("definitions=256", "definitions=1"),
            resource_diagnostic.replace("name_bytes=64", "name_bytes=18"),
            resource_diagnostic.replace("backend_cap=155", "backend_cap=156"),
            resource_diagnostic.replace("product_bytes=1447", "product_bytes=1448"),
            resource_diagnostic.replace("charge=228789", "charge=43573"),
            resource_diagnostic.replace("charge=243251", "charge=58035"),
            resource_diagnostic.replace("callbacks=0", "callbacks=1"),
            resource_diagnostic.lines().next().unwrap().to_owned(),
        ] {
            assert!(
                catch_unwind(|| reconcile_resource_test(&resource_stdout, &diagnostic, 256, true))
                    .is_err()
            );
        }
        assert!(
            catch_unwind(|| reconcile_resource_test(
                "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 32 filtered out",
                resource_diagnostic,
                256,
                true
            ))
            .is_err()
        );
        let resource_failure = format!(
            "test {RESOURCE_TEST} ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 31 filtered out\n"
        );
        reconcile_resource_test(&resource_failure, resource_diagnostic, 256, false);
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
                        let resolved = auctioneer_bidder::bidder_settings::fake_integer_probe::resolve(requested, base);
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
                        && auctioneer_bidder::bidder_settings::fake_string_probe::resolve(requested, base).token().is_none() {
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
                crate::settings_observations::record(#key, input);
                config::Config(input).validate().map_err(config::error)
            }
        }.to_string();
        let config = quote::quote! {
            use auctioneer_bidder::BidderSettings;
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

    fn install_settings_observations(bidder: &Path) {
        let path = bidder.join("src/lib.rs");
        let mut library = syn::parse_file(&fs::read_to_string(&path).unwrap()).unwrap();
        let observations: syn::File = syn::parse2(quote::quote! {
            pub mod settings_observations {
                use crate::{BidderConfigInput, BidderSettings};
                use std::cell::RefCell;

                #[derive(Debug, PartialEq, Eq)]
                pub struct Observation {
                    pub owner: &'static str,
                    pub definition: Option<String>,
                    pub enabled: bool,
                    pub primary: Option<i64>,
                    pub secondary: Option<i64>,
                }

                thread_local! {
                    static SEEN: RefCell<Vec<Observation>> = const { RefCell::new(Vec::new()) };
                }

                pub fn record(owner: &'static str, input: BidderConfigInput<'_>) {
                    let resolved = crate::bidder_settings::fake_integer_probe::resolve(
                        input.requested.and_then(BidderSettings::as_fake_integer_probe),
                        input.base.and_then(BidderSettings::as_fake_integer_probe),
                    );
                    SEEN.with_borrow_mut(|seen| seen.push(Observation {
                        owner,
                        definition: input.definition.map(|info| info.name.clone()),
                        enabled: input.enabled,
                        primary: resolved.primary_number(),
                        secondary: resolved.secondary_number(),
                    }));
                }

                pub fn take() -> Vec<Observation> {
                    SEEN.with_borrow_mut(std::mem::take)
                }
            }
        })
        .unwrap();
        library.items.extend(observations.items);
        fs::write(path, quote::quote!(#library).to_string()).unwrap();
    }
}
