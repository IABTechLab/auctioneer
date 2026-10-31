//! Concrete owners for the approved migration inventory, not qualification evidence.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::env::consts::OS;
    use std::fs::{create_dir_all, read_to_string, write};
    use std::path::Path;

    use syn::{Attribute, Expr, File, Item, Lit, Meta, Token, punctuated::Punctuated};

    type ModuleOwners = (&'static str, &'static [&'static str]);
    type Family = (&'static str, &'static [ModuleOwners]);

    // Owners are relative to the binary harness, except explicit cli:: and lib:: roots.
    // The exact CI all-target invocation runs these tests; this guard never invokes Cargo.
    const FAMILIES: &[Family] = &[
        (
            "JSON/file/output limits and key identity",
            &[
                (
                    "lib::native_control::io::tests",
                    &[
                        "bounded_json_accepts_the_exact_cap_and_refuses_overflow_without_source",
                        "private_json_is_bounded_complete_and_does_not_overwrite",
                        "canonical_digests_ignore_object_order_but_not_values_or_array_order",
                        "structural_limits_count_discarded_values_before_typed_parsing",
                        "artifact_hashing_refuses_links_and_over_limit_files",
                    ],
                ),
                (
                    "native::inputs::tests",
                    &["raw_candidate_rejects_unknown_duplicate_and_mistyped_fields_without_source"],
                ),
            ],
        ),
        (
            "Early exit/readiness, output flood, timeout and process descendants",
            &[
                (
                    "lib::native_control::process::tests",
                    &[
                        "output_overflow_and_timeout_reap_the_group",
                        "successful_wrapper_exit_does_not_abandon_descendants_or_their_pipes",
                        "private_json_lines_are_bounded_and_round_trip_without_a_control_route",
                    ],
                ),
                (
                    "fixture::protocol::tests",
                    &["malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener"],
                ),
                (
                    "cli::tests",
                    &["fixture_cli_uses_private_records_and_observes_eof_before_exiting"],
                ),
            ],
        ),
        (
            "SIGINT/SIGTERM, watchdog and cleanup ownership",
            &[
                (
                    "commands::tests",
                    &["command_watchdog_latches_the_deadline_without_an_active_io_operation"],
                ),
                (
                    "lib::native_control::process::tests",
                    &[
                        "deadline_and_signal_latches_prevent_every_later_spawn",
                        "failed_cleanup_overrides_an_earlier_command_failure_without_hanging",
                        "cancelling_a_pending_cleanup_keeps_its_join_handle_owned",
                    ],
                ),
                (
                    "cli::tests",
                    &[
                        "fixture_cli_handles_each_signal_and_closes_the_listener_before_exit",
                        "a_full_public_output_pipe_cannot_keep_the_runtime_alive_after_its_write_deadline",
                    ],
                ),
            ],
        ),
        (
            "HTTP body/status/inspection/refusal/timeout/disconnect",
            &[(
                "http::tests",
                &[
                    "actual_http_classifies_status_before_inspection_and_observes_flush_and_headers",
                    "actual_http_caps_bodies_and_preserves_timeout_disconnect_and_slow_read_categories",
                    "unrequested_bodies_are_counted_not_retained_and_nonloopback_urls_refuse",
                ],
            )],
        ),
        (
            "Exact request/gzip/FPD/debug geometry",
            &[
                (
                    "native::geometry::tests",
                    &[
                        "request_shapes_reach_exact_byte_node_impression_and_alias_targets",
                        "fpd_and_dense_debug_geometry_remain_separate_bounded_shapes",
                        "legal_gzip_members_hit_encoded_and_decoded_caps_independently",
                    ],
                ),
                (
                    "fixture::server::tests",
                    &["native_exact_coverage_probe_fpd_and_gzip_are_observed_not_inferred"],
                ),
                (
                    "native::scenario::tests",
                    &[
                        "denied_request_changes_only_the_publisher_and_preserves_byte_and_node_geometry",
                    ],
                ),
            ],
        ),
        (
            "Candidate/catalog, features and SDK environment",
            &[
                (
                    "native::inputs::tests",
                    &[
                        "catalog_is_closed_ordered_and_has_exact_families_variants_modes_and_outcome_caps",
                        "binding_preserves_all_criteria_and_exactly_forty_five_rows_and_three_variants",
                        "binding_checks_every_expected_limit_sdk_cache_feature_and_variant_matrix",
                        "service_environment_has_exactly_ten_sdk_values_and_four_fixed_values",
                    ],
                ),
                (
                    "native::snapshots::tests",
                    &[
                        "structured_toml_uses_the_real_semantic_validator_without_enabling_fixture_metadata",
                    ],
                ),
            ],
        ),
        (
            "Native metering and service/wrapper separation",
            &[
                (
                    "meter::tests",
                    &[
                        "bsd_high_water_is_already_bytes",
                        "gnu_high_water_converts_kibibytes_to_literal_bytes",
                        "high_water_refuses_missing_zero_duplicate_malformed_and_overflow",
                    ],
                ),
                (
                    "meter::unix_tests",
                    &[
                        "real_sleep_has_distinct_service_pid_private_report_and_wait4_high_water",
                        "idle_window_observes_duration_and_only_bounded_service_rss",
                        "memory_requires_stop_and_reuses_lifetime_peak_not_sampled_peak",
                        "failed_cleanup_stays_latched_and_invalidates_memory",
                    ],
                ),
            ],
        ),
        (
            "Frozen artifacts, source and typed config push",
            &[
                (
                    "native::snapshots::tests",
                    &[
                        "frozen_artifacts_are_private_independent_and_detect_source_and_copy_tampering",
                        "push_spec_has_exact_frozen_cli_arguments_empty_environment_and_bounded_output",
                    ],
                ),
                (
                    "native::probes::tests",
                    &[
                        "lifecycle_uses_four_fresh_processes_and_observes_live_a_after_installing_b",
                        "all_unproven_stops_block_restoration_and_new_spawns",
                    ],
                ),
                (
                    "native::system::tests",
                    &[
                        "source_commit_refuses_each_tracked_build_input_change_or_deletion",
                        "source_inventory_refuses_untracked_ignored_and_src_target_inputs_independently",
                        "clean_filters_cannot_mask_changed_compiler_inputs",
                    ],
                ),
            ],
        ),
        (
            "Admission witnesses and ordinary geometry",
            &[
                (
                    "native::oracles::tests",
                    &[
                        "two_primary_requests_are_constructed_without_headers_and_are_drained_after_both_challenges",
                        "serialized_construction_or_a_wrong_overload_body_refuses_and_drains_held_work",
                    ],
                ),
                (
                    "native::scenario::tests",
                    &[
                        "ordinary_successes_require_bids_and_exact_coverage_of_actual_debug_ids",
                        "timeout_ordinary_bids_allow_zero_success_ids_with_exact_empty_coverage",
                        "empty_ordinary_population_cannot_borrow_oracle_dispatch_or_decode",
                        "all_rejected_ordinary_population_cannot_borrow_oracle_credit",
                        "zero_success_ordinary_work_requires_independent_dispatch_and_decode",
                        "successful_ordinary_work_requires_exact_dispatch_and_decode_evidence",
                    ],
                ),
            ],
        ),
        (
            "GVL consent/version/path and held-work overlap",
            &[
                (
                    "native::consents::tests",
                    &[
                        "consent_variants_preserve_disclosure_segments_and_match_fourteen_compiled_versions",
                        "gvl_credit_requires_a_fresh_compiled_valid_consent_and_an_actual_nonstale_privacy_path",
                    ],
                ),
                (
                    "native::oracles::tests",
                    &["gvl_credit_requires_an_ordinary_request_to_remain_held_across_each_lookup"],
                ),
                (
                    "native::frozen_probe::tests",
                    &[
                        "original_four_capture_files_and_both_executables_must_each_remain_unchanged",
                        "overlap_requires_fourteen_distinct_credits_and_sixteen_ordinary_bids_and_calls",
                        "invalid_consent_requires_observed_privacy_denial_no_bids_and_zero_credit",
                    ],
                ),
            ],
        ),
        (
            "Primary load and debug populations",
            &[
                (
                    "native::load::tests",
                    &["real_load_counts_each_offered_request_without_padding_missing_work"],
                ),
                (
                    "native::scenario::tests",
                    &[
                        "outcome_counters_accumulate_all_primary_work_without_inventing_cancellations",
                        "debug_ranges_count_only_actual_outputs_and_bind_two_finalized_oracle_populations",
                        "capture_release_keeps_oracle_ranges_when_ordinary_outputs_are_absent",
                    ],
                ),
                (
                    "native::runner::tests",
                    &[
                        "empty_preflight_contains_no_measurements_or_sdk_qualification_credit",
                        "only_qualified_results_have_a_zero_measurement_exit_code",
                    ],
                ),
            ],
        ),
        (
            "Fourteen provider cases and generated negative rows",
            &[
                (
                    "providers::tests",
                    &[
                        "offline_report_verifies_fourteen_ordered_cases_without_live_acceptance",
                        "identities_bind_every_digest_commit_sdk_target_and_runtime_component",
                        "features_require_exact_provider_and_override_without_development_features",
                        "every_case_requires_exact_order_dispatch_status_and_outcome",
                        "malformed_top_level_environment_identity_and_case_containers_refuse_safely",
                    ],
                ),
                (
                    "cli::tests",
                    &[
                        "offline_cli_success_never_grants_deployed_acceptance_or_publisher_readiness",
                    ],
                ),
            ],
        ),
        (
            "Approved HTTPS fixture origins",
            &[(
                "providers::preparation::tests",
                &["approved_origins_refuse_every_unsafe_family"],
            )],
        ),
        (
            "Isolated production metadata and source freeze",
            &[
                (
                    "providers::preparation::tests",
                    &[
                        "filesystem_inventory_includes_ignored_untracked_build_inputs_and_precise_target_sources",
                        "cargo_owned_targets_ignore_create_change_remove_even_broken_links",
                        "preparation_is_private_isolated_secure_and_production_metadata_never_changes",
                    ],
                ),
                (
                    "providers::artifacts::tests",
                    &[
                        "capture_and_component_handoff_reject_modified_added_deleted_and_untracked_build_inputs",
                        "capture_and_component_handoff_reject_each_isolated_metadata_member_change",
                    ],
                ),
            ],
        ),
        (
            "Cargo emitted build receipt",
            &[(
                "providers::artifacts::tests",
                &[
                    "capture_accepts_only_exact_compiler_features_target_profile_and_emitted_file",
                    "component_identity_refuses_missing_stale_linked_and_rehashed_non_wasm_files",
                ],
            )],
        ),
        (
            "Spin component and Fastly complete release handoff",
            &[(
                "providers::artifacts::tests",
                &[
                    "spin_handoff_binds_receipt_identity_and_the_exact_staged_component",
                    "complete_fastly_bundle_handoff_requires_every_member_digest_identity_and_exact_set",
                    "release_metadata_and_member_type_changes_cannot_be_reauthorized_by_rehashing",
                    "package_member_names_reject_traversal_duplicates_and_ambiguous_components",
                ],
            )],
        ),
        (
            "Pinned SDK offline release verification",
            &[(
                "providers::sdk::tests",
                &[
                    "pinned_sdk_verifies_an_actual_complete_archive_without_provider_credentials",
                    "explicit_invalid_sdk_roots_always_fail_without_executing_a_caller_command",
                    "replacement_refs_cannot_substitute_scripts_under_the_pinned_revision",
                ],
            )],
        ),
        (
            "Provider bidder failure/stall/canary/TLS and dispatch proof",
            &[
                (
                    "fixture::server::tests",
                    &[
                        "provider_failure_stall_and_canary_are_real_http_outcomes",
                        "provider_rejects_bad_methods_paths_ids_shapes_and_caps_without_dispatch",
                        "provider_tls_serves_a_verified_bid_and_rejects_a_malformed_certificate",
                        "every_provider_case_has_a_real_bid_and_counters_are_fixture_local",
                    ],
                ),
                (
                    "fixture::protocol::tests",
                    &[
                        "private_records_preserve_framing_at_the_cap_and_refuse_partial_or_overflow_data",
                    ],
                ),
                (
                    "cli::tests",
                    &[
                        "fixture_cli_uses_private_records_and_observes_eof_before_exiting",
                        "provider_command_refusals_do_not_echo_input_paths_or_contents",
                    ],
                ),
            ],
        ),
        (
            "Corruption templates and recovery controls",
            &[(
                "providers::recovery::tests",
                &[
                    "corruption_templates_have_exact_bounded_bytes_and_a_wrong_digest",
                    "mutation_gate_refuses_all_prerequisite_violations_before_callbacks",
                    "interrupted_injection_restores_on_a_fresh_non_aborted_deadline",
                    "ignored_abort_and_late_write_are_fenced_before_restoration",
                    "termination_ack_without_completion_prevents_restoration_even_when_a_write_arrives_later",
                    "missing_or_invalid_completion_fence_cannot_trigger_restoration",
                    "stalled_termination_and_stalled_restore_remain_bounded",
                ],
            )],
        ),
    ];

    // Only the cfg vocabulary used by these owners is reviewed. Unknown conditions
    // and cfg_attr fail closed instead of guessing whether a test executes in CI.
    fn cfg_state(meta: &Meta) -> Result<(bool, bool), String> {
        match meta {
            Meta::Path(path) if path.is_ident("test") => Ok((true, true)),
            Meta::Path(path) if path.is_ident("unix") => Ok((cfg!(unix), false)),
            Meta::NameValue(predicate_value) if predicate_value.path.is_ident("feature") => {
                if let Expr::Lit(expression) = &predicate_value.value
                    && let Lit::Str(feature) = &expression.lit
                    && feature.value() == "application"
                {
                    return Ok((cfg!(feature = "application"), false));
                }
                Err("unreviewed cfg condition on a migration owner".to_owned())
            }
            Meta::NameValue(predicate_value) if predicate_value.path.is_ident("target_os") => {
                if let Expr::Lit(expression) = &predicate_value.value
                    && let Lit::Str(os_name) = &expression.lit
                {
                    return Ok((os_name.value() == OS, false));
                }
                Err("target_os must be a literal string".to_owned())
            }
            Meta::List(list) if list.path.is_ident("all") || list.path.is_ident("any") => {
                let predicates = list
                    .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                    .map_err(|error| error.to_string())?;
                let all = list.path.is_ident("all");
                let mut state = (all, !all);
                for predicate in predicates {
                    let (enabled, test_only) = cfg_state(&predicate)?;
                    state = if all {
                        (state.0 && enabled, state.1 || test_only)
                    } else {
                        (state.0 || enabled, state.1 && test_only)
                    };
                }
                Ok(state)
            }
            Meta::Path(_) | Meta::List(_) | Meta::NameValue(_) => {
                Err("unreviewed cfg condition on a migration owner".to_owned())
            }
        }
    }

    fn scope(attributes: &[Attribute], mut test_only: bool) -> Result<bool, String> {
        for attribute in attributes {
            let path = attribute.path();
            if path.is_ident("ignore") || path.is_ident("cfg_attr") || path.is_ident("path") {
                return Err("ignored, conditional-attribute or redirected owner".to_owned());
            }
            if path.is_ident("cfg") {
                let predicate: Meta = attribute.parse_args().map_err(|error| error.to_string())?;
                let (enabled, requires_test) = cfg_state(&predicate)?;
                if !enabled {
                    return Err("cfg-disabled migration owner".to_owned());
                }
                test_only |= requires_test;
            }
        }
        Ok(test_only)
    }

    fn test_attribute(attribute: &Attribute) -> bool {
        let path = attribute.path();
        path.is_ident("test")
            || (path.segments.len() == 2
                && path.segments[0].ident == "tokio"
                && path.segments[1].ident == "test")
    }

    fn require_file(
        file: &Path,
        directory: &Path,
        modules: &[&str],
        names: &[&str],
        test_only: bool,
    ) -> Result<(), String> {
        let source =
            read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?;
        let syntax = syn::parse_file(&source).map_err(|error| error.to_string())?;
        let file_test_only = scope(&syntax.attrs, test_only)?;
        require_items(&syntax.items, directory, modules, names, file_test_only)
    }

    fn require_items(
        items: &[Item],
        directory: &Path,
        modules: &[&str],
        names: &[&str],
        test_only: bool,
    ) -> Result<(), String> {
        if let Some((name, rest)) = modules.split_first() {
            let mut candidates = items
                .iter()
                .filter_map(|item| {
                    let Item::Mod(module) = item else {
                        return None;
                    };
                    Some(module)
                })
                .filter(|module| module.ident == *name);
            let module = candidates
                .next()
                .ok_or_else(|| format!("missing module {name}"))?;
            if candidates.next().is_some() {
                return Err(format!("ambiguous owner module {name}"));
            }
            let module_test_only = scope(&module.attrs, test_only)?;
            let module_directory = directory.join(name);
            return if let Some((_brace, content)) = &module.content {
                require_items(content, &module_directory, rest, names, module_test_only)
            } else {
                let direct = module_directory.with_extension("rs");
                let nested = module_directory.join("mod.rs");
                let file = match (direct.is_file(), nested.is_file()) {
                    (true, false) => direct,
                    (false, true) => nested,
                    _ => return Err(format!("missing or ambiguous source module {name}")),
                };
                require_file(&file, &module_directory, rest, names, module_test_only)
            };
        }
        for name in names {
            let mut candidates = items
                .iter()
                .filter_map(|item| {
                    let Item::Fn(function) = item else {
                        return None;
                    };
                    Some(function)
                })
                .filter(|function| function.sig.ident == *name);
            let function = candidates
                .next()
                .ok_or_else(|| format!("missing test {name}"))?;
            if candidates.next().is_some()
                || !scope(&function.attrs, test_only)?
                || !function.attrs.iter().any(test_attribute)
                || !function.sig.inputs.is_empty()
                || !function.sig.generics.params.is_empty()
                || function.block.stmts.is_empty()
            {
                return Err(format!(
                    "{name} must be a concrete unignored test under cfg(test)"
                ));
            }
        }
        Ok(())
    }

    fn require_owner(crate_root: &Path, owner_module: &str, names: &[&str]) -> Result<(), String> {
        let (root, qualified_module) = if let Some(module) = owner_module.strip_prefix("lib::") {
            ("src/lib.rs", module.to_owned())
        } else if let Some(module) = owner_module.strip_prefix("cli::") {
            ("tests/qualification_harness.rs", module.to_owned())
        } else {
            (
                "src/main.rs",
                format!("qualification::harness::{owner_module}"),
            )
        };
        let file = crate_root.join(root);
        let modules: Vec<_> = qualified_module.split("::").collect();
        require_file(&file, file.parent().unwrap(), &modules, names, false)
    }

    #[test]
    fn every_inventory_family_has_concrete_active_unignored_rust_test_owners() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let plan = read_to_string(
            crate_root
                .join("../../docs/superpowers/plans/2026-10-15-rust-qualification-harnesses.md"),
        )
        .unwrap();
        let (_, inventory) = plan.split_once("### Porting Inventory\n").unwrap();
        let (_, rows) = inventory
            .split_once("| Existing control family | Rust owner | Required assertions |\n")
            .unwrap();
        let mut expected = BTreeSet::new();
        for row in rows.lines().skip(1).take_while(|row| row.starts_with('|')) {
            let name = row.split('|').nth(1).unwrap().trim();
            assert!(!name.is_empty());
            assert!(expected.insert(name), "duplicate inventory family: {name}");
        }
        assert!(!expected.is_empty());
        let mapped: BTreeSet<_> = FAMILIES.iter().map(|(name, _owners)| *name).collect();
        assert_eq!(mapped.len(), FAMILIES.len(), "duplicate mapped family");
        assert_eq!(
            mapped, expected,
            "every approved control family needs ownership"
        );
        for (family, owners) in FAMILIES {
            assert!(!owners.is_empty(), "{family} has no Rust owners");
            for (owner_module, names) in *owners {
                assert!(!names.is_empty(), "{family}: {owner_module} has no tests");
                require_owner(crate_root, owner_module, names)
                    .unwrap_or_else(|error| panic!("{family}: {owner_module}: {error}"));
            }
        }
    }

    fn fixture() -> File {
        syn::parse_quote! {
            #[cfg(test)]
            mod tests {
                #[test]
                fn owner() { assert!(checked()); }
            }
        }
    }

    fn require_fixture(file: &File) -> Result<(), String> {
        let test_only = scope(&file.attrs, false)?;
        require_items(
            &file.items,
            Path::new("unused"),
            &["tests"],
            &["owner"],
            test_only,
        )
    }

    fn fixture_module(file: &mut File) -> &mut syn::ItemMod {
        let Item::Mod(module) = &mut file.items[0] else {
            panic!("fixture module")
        };
        module
    }

    fn fixture_function(file: &mut File) -> &mut syn::ItemFn {
        let items = &mut fixture_module(file).content.as_mut().unwrap().1;
        let Item::Fn(function) = &mut items[0] else {
            panic!("fixture test")
        };
        function
    }

    fn write_owner_fixture(root: &Path, relative: &str, syntax: &File) {
        let file = root.join(relative);
        create_dir_all(file.parent().unwrap()).unwrap();
        write(file, quote::quote!(#syntax).to_string()).unwrap();
    }

    fn library_owner_fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        write_owner_fixture(
            root.path(),
            "src/lib.rs",
            &syn::parse_quote! { pub mod native_control; },
        );
        write_owner_fixture(
            root.path(),
            "src/native_control/mod.rs",
            &syn::parse_quote! { pub mod io; pub mod process; },
        );
        for relative in ["src/native_control/io.rs", "src/native_control/process.rs"] {
            write_owner_fixture(root.path(), relative, &fixture());
        }
        root
    }

    #[test]
    fn library_owners_resolve_from_lib_rs_through_ordinary_modules() {
        let root = library_owner_fixture();
        for owner in [
            "lib::native_control::io::tests",
            "lib::native_control::process::tests",
        ] {
            require_owner(root.path(), owner, &["owner"]).unwrap();
        }
        assert!(require_owner(root.path(), "io::tests", &["owner"]).is_err());
        assert!(require_owner(root.path(), "process::tests", &["owner"]).is_err());
        assert!(require_owner(root.path(), "cli::tests", &["owner"]).is_err());
    }

    #[test]
    fn library_owners_refuse_missing_redirected_reexported_or_disabled_roots() {
        let root = library_owner_fixture();
        let invalid_roots: [(&str, File); 7] = [
            ("missing", syn::parse_quote! {}),
            ("renamed", syn::parse_quote! { pub mod renamed; }),
            (
                "redirected",
                syn::parse_quote! { #[path = "native_control/mod.rs"] pub mod native_control; },
            ),
            (
                "reexported",
                syn::parse_quote! { pub use hidden::native_control; },
            ),
            (
                "private include",
                syn::parse_quote! { include!("native_control/mod.rs"); },
            ),
            (
                "disabled",
                syn::parse_quote! { #[cfg(any())] pub mod native_control; },
            ),
            (
                "ambiguous",
                syn::parse_quote! { pub mod native_control; pub mod native_control; },
            ),
        ];
        for (name, syntax) in invalid_roots {
            write_owner_fixture(root.path(), "src/lib.rs", &syntax);
            assert!(
                require_owner(root.path(), "lib::native_control::io::tests", &["owner"]).is_err(),
                "accepted {name} library root"
            );
        }
    }

    #[test]
    fn library_owners_preserve_active_unignored_concrete_test_guards() {
        let root = library_owner_fixture();
        for mutation in [
            "deleted",
            "renamed",
            "ignored",
            "disabled",
            "unregistered",
            "empty",
        ] {
            let mut syntax = fixture();
            match mutation {
                "deleted" => fixture_module(&mut syntax)
                    .content
                    .as_mut()
                    .unwrap()
                    .1
                    .clear(),
                "renamed" => fixture_function(&mut syntax).sig.ident = syn::parse_quote!(renamed),
                "ignored" => fixture_function(&mut syntax)
                    .attrs
                    .push(syn::parse_quote!(#[ignore])),
                "disabled" => fixture_module(&mut syntax)
                    .attrs
                    .push(syn::parse_quote!(#[cfg(any())])),
                "unregistered" => fixture_function(&mut syntax).attrs.clear(),
                "empty" => fixture_function(&mut syntax).block.stmts.clear(),
                _ => panic!("unknown library owner mutation: {mutation}"),
            }
            write_owner_fixture(root.path(), "src/native_control/io.rs", &syntax);
            assert!(
                require_owner(root.path(), "lib::native_control::io::tests", &["owner"]).is_err(),
                "accepted {mutation} library owner"
            );
        }
    }

    #[test]
    fn deleted_renamed_ignored_and_unregistered_ast_owners_are_rejected() {
        let original = fixture();
        require_fixture(&original).unwrap();
        for mutation in ["deleted", "renamed", "ignored", "unregistered", "empty"] {
            let mut file = original.clone();
            match mutation {
                "deleted" => fixture_module(&mut file)
                    .content
                    .as_mut()
                    .unwrap()
                    .1
                    .clear(),
                "renamed" => fixture_function(&mut file).sig.ident = syn::parse_quote!(renamed),
                "ignored" => fixture_function(&mut file)
                    .attrs
                    .push(syn::parse_quote!(#[ignore])),
                "unregistered" => fixture_function(&mut file).attrs.clear(),
                "empty" => fixture_function(&mut file).block.stmts.clear(),
                _ => panic!("unknown owner mutation: {mutation}"),
            }
            assert!(require_fixture(&file).is_err(), "accepted {mutation} owner");
        }
    }

    #[test]
    fn application_gate_is_recognized_without_permitting_disabled_owners() {
        let enabled = [syn::parse_quote!(#[cfg(feature = "application")])];
        assert_eq!(scope(&enabled, true), Ok(true));
        let disabled = [syn::parse_quote!(#[cfg(all(feature = "application", any()))])];
        assert_eq!(
            scope(&disabled, true),
            Err("cfg-disabled migration owner".to_owned())
        );
        let unknown = [syn::parse_quote!(#[cfg(feature = "application-extra")])];
        assert_eq!(
            scope(&unknown, true),
            Err("unreviewed cfg condition on a migration owner".to_owned())
        );
    }

    #[test]
    fn disabled_or_conditionally_ignored_ast_owners_and_ancestors_are_rejected() {
        let disabled_attributes: [Attribute; 4] = [
            syn::parse_quote!(#[cfg(any())]),
            syn::parse_quote!(#[cfg(not(test))]),
            syn::parse_quote!(#[cfg(feature = "unreviewed-owner")]),
            syn::parse_quote!(#[cfg_attr(test, ignore)]),
        ];
        for attribute in disabled_attributes {
            for location in ["file", "module", "function"] {
                let mut file = fixture();
                let target_attributes = match location {
                    "file" => &mut file.attrs,
                    "module" => &mut fixture_module(&mut file).attrs,
                    "function" => &mut fixture_function(&mut file).attrs,
                    _ => panic!("unknown owner location: {location}"),
                };
                target_attributes.push(attribute.clone());
                assert!(
                    require_fixture(&file).is_err(),
                    "accepted disabled {location}"
                );
            }
        }
        let mut no_test_scope = fixture();
        fixture_module(&mut no_test_scope).attrs.clear();
        assert!(require_fixture(&no_test_scope).is_err());
        let mut disjunctive_scope = fixture();
        fixture_module(&mut disjunctive_scope).attrs =
            vec![syn::parse_quote!(#[cfg(any(test, unix))])];
        assert!(require_fixture(&disjunctive_scope).is_err());
    }

    #[test]
    fn comments_strings_and_unregistered_helpers_cannot_impersonate_test_owners() {
        let file: File = syn::parse_quote! {
            #[cfg(test)]
            mod tests {
                // #[test] fn owner() { assert!(checked()); }
                const DECOY: &str = "#[tokio::test] async fn owner() { checked().await; }";
                fn helper() { assert!(checked()); }
            }
        };
        assert!(require_fixture(&file).is_err());
        let mut registered = fixture();
        fixture_function(&mut registered).attrs = vec![syn::parse_quote!(#[tokio::test])];
        fixture_function(&mut registered).sig.asyncness = Some(syn::parse_quote!(async));
        require_fixture(&registered).unwrap();
    }
}
