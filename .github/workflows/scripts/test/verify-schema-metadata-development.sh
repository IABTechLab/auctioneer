#!/usr/bin/env bash
set -euo pipefail

# Reuse this lane's normal insecure binary, but prove the prefix runs every
# listed ownership case exactly once instead of accepting a zero-test filter.
cargo_args=(--locked -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance)
env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test "${cargo_args[@]}" -- --list --format terse > "$RUNNER_TEMP/schema-residency-all.list"
sed -n 's/: test$//p' "$RUNNER_TEMP/schema-residency-all.list" | LC_ALL=C sort > "$RUNNER_TEMP/schema-residency-all.names"
total=$(wc -l < "$RUNNER_TEMP/schema-residency-all.names" | tr -d ' ')
test "$(LC_ALL=C sort -u "$RUNNER_TEMP/schema-residency-all.names" | wc -l | tr -d ' ')" -eq "$total"
validation_required=(
  schema_validation::handoff::tests::same_capture_is_required
  schema_validation::handoff::tests::every_child_requires_exactly_one_owner
  schema_validation::handoff::tests::caller_context_is_not_a_regex_child
  schema_validation::handoff::tests::zero_regex_declarations_still_require_work_and_stack
  schema_validation::handoff::tests::actual_bodies_require_the_descriptor_capture
  schema_validation::handoff::tests::body_receipts_reject_changed_owner_and_expansion
  schema_validation::handoff::tests::seed_and_options_drift_cannot_reuse_body_authority
  schema_validation::handoff::tests::unrelated_nonempty_syntax_cannot_replace_pristine_body
  schema_validation::handoff::source::tests::actual_sources_are_consumed_inside_the_bound_window
  schema_validation::handoff::source::tests::compiler_identity_must_match_the_sealed_seed
  schema_validation::handoff::source::tests::source_roster_and_consumed_bytes_refuse_drift
  schema_validation::handoff::source::tests::interpretation_errors_still_close_the_source_window
  schema_validation::operations::tests::unclassified_call_refuses_completion
  schema_validation::operations::tests::changed_branch_multiplicity_changes_the_recipe
  schema_validation::operations::tests::deep_boolean_source_preserves_every_occurrence
  schema_validation::operations::tests::non_regex_declaration_has_a_nonempty_closure
  schema_validation::operations::tests::changed_primitive_semantics_is_not_approved_by_rehashing
  schema_validation::operations::tests::expiry_binding_is_explicit
  schema_validation::operations::tests::unknown_callback_is_not_a_free_source_edge
  schema_validation::operations::tests::new_nested_literal_owner_requires_a_supported_proof
  schema_validation::operations::tests::cold_entropy_has_exact_identity_lifetime_and_nonfree_obligations
  schema_validation::tests::current_body_shapes_have_declared_operation_obligations
  schema_validation::report::tests::current_source_covers_every_discovered_owner
  schema_validation::report::tests::canonical_report_rejects_retired_and_hybrid_fields
  schema_validation::report::tests::retired_assembly_has_no_active_callers
  schema_validation::report::tests::unqualified_source_never_grants_serving_permission
  schema_validation::observations::tests::fake_cache_phase_accounts_for_prior_storage_and_transient_growth_independently
  schema_validation::observations::tests::fake_cache_population_covers_every_owner_input_verdict_and_release
  schema_validation::public_names::tests::every_public_name_has_exact_schema_disposition
  schema_validation::work::tests::dimensions_vary_independently
  schema_validation::work::tests::expiry_prefixes_bound_real_call_sites
  schema_validation::work::tests::attempts_before_admission_are_charged
  schema_validation::work::tests::branches_and_callbacks_have_source_derived_multiplicity
  schema_validation::work::tests::late_failure_includes_cleanup
  schema_validation::work::tests::overflow_refuses
  schema_validation::work::caller::tests::original_impressions_follow_both_core_admission_representations
  schema_validation::work::caller::tests::bidder_cap_records_clamp_then_nonzero_effective_cap
  schema_validation::work::caller::tests::scope_gates_do_not_discharge_retention_or_cleanup
  schema_validation::work::caller::tests::missing_and_rehashed_caller_sources_refuse_at_zero_attempts
  schema_validation::work::partition::tests::disjoint_children_across_helpers_have_linear_coefficients
  schema_validation::work::partition::tests::ancestor_rescans_require_source_bound_cap_lifting
  schema_validation::work::partition::tests::changed_iteration_and_call_bindings_are_refused
  schema_validation::work::partition::tests::parameterized_calls_preserve_instance_and_cache_bindings
  schema_validation::work::partition::tests::object_entry_values_partition_across_parameterized_helpers
  schema_validation::work::partition::tests::object_ancestor_rescans_require_key_funded_cap_lifting
  schema_validation::work::partition::tests::cache_and_object_binder_drift_are_independently_refused
  schema_validation::work::partition::coupled_tests::coupled_factories_keep_constant_work_separate_from_input_traversals
  schema_validation::work::partition::coupled_tests::factory_readiness_prefixes_and_source_duties_survive_zero_entries
  schema_validation::work::partition::coupled_tests::coupled_factory_bindings_and_overflow_refuse_independently
  schema_validation::work::prefix::tests::rejected_preflight_prefixes_remain_charged
  schema_validation::work::prefix::tests::keys_and_string_prechecks_bound_partial_prefixes
  schema_validation::work::prefix::tests::iterator_population_and_recursion_axes_remain_distinct
  schema_validation::work::prefix::tests::counter_protocol_source_drift_and_overflow_refuse
  schema_validation::work::prefix::tests::scalar_input_links_are_required_at_zero_and_one_preflights
  schema_validation::work::prefix::tests::scalar_source_refusals_survive_zero_preflights
  schema_validation::work::prefix::tests::scalar_prefix_failures_preserve_counts_transactionally
  schema_validation::work::prefix::scalar::tests::string_framing_bounds_rejected_prefixes
  schema_validation::work::prefix::scalar::tests::concrete_counter_protocol_requires_scalar_dispatch
  schema_validation::work::prefix::scalar::tests::missing_and_rehashed_scalar_sources_refuse_at_zero_attempts
  schema_validation::work::entropy::tests::constructor_default_and_std_identity_are_required
  schema_validation::work::entropy::tests::ordered_seed_prefixes_include_eager_inactive_siblings
  schema_validation::work::entropy::tests::hostname_seed_work_accumulates_and_overflow_refuses
  schema_validation::work::epsilon::tests::pristine_body_matches_the_independent_recipe
  schema_validation::work::epsilon::tests::capture_self_loop_has_literal_site_ceilings
  schema_validation::work::epsilon::tests::duplicate_union_alternatives_have_a_conservative_ceiling
  schema_validation::work::epsilon::tests::roots_vary_without_multiplying_epoch_dispatches
  schema_validation::work::epsilon::tests::epochs_vary_independently_of_root_calls
  schema_validation::work::epsilon::tests::nfa_states_vary_independently_of_branches
  schema_validation::work::epsilon::tests::branches_vary_independently_of_nfa_states
  schema_validation::work::epsilon::tests::empty_local_execution_does_not_discharge_primitive_lemmas
  schema_validation::work::epsilon::tests::reverse_tail_counts_the_final_none
  schema_validation::work::epsilon::tests::changed_known_source_is_not_approved_by_its_identity
  schema_validation::work::epsilon::tests::unknown_source_call_is_refused_not_assigned_zero_work
  schema_validation::work::epsilon::tests::malformed_epoch_hypotheses_are_refused
  schema_validation::work::epsilon::tests::overflow_is_refused_for_each_independent_formula_axis
  schema_validation::work::epsilon::primitives::tests::nfa_access_matches_pristine_recipes_and_literal_counts
  schema_validation::work::epsilon::primitives::tests::sparse_insert_matches_pristine_recipes_and_branch_counts
  schema_validation::work::epsilon::primitives::tests::zero_invocations_keep_required_helper_slots_and_premises
  schema_validation::work::epsilon::primitives::tests::helper_receipts_require_exact_concrete_receivers
  schema_validation::work::epsilon::primitives::tests::missing_duplicate_or_changed_source_receipts_are_refused
  schema_validation::work::epsilon::primitives::tests::compiler_source_identity_mismatch_is_refused
  schema_validation::work::epsilon::primitives::tests::known_helper_semantic_changes_are_refused
  schema_validation::work::epsilon::primitives::tests::unknown_helper_calls_are_not_zero_work
  schema_validation::work::epsilon::primitives::tests::unapproved_helper_attributes_are_refused
  schema_validation::work::epsilon::primitives::tests::sparse_invariants_require_matching_owner_and_epoch
  schema_validation::work::epsilon::primitives::tests::invalid_sparse_dimensions_and_ids_are_refused
  schema_validation::work::epsilon::primitives::tests::primitive_count_overflow_is_refused
  schema_validation::storage::source::tests::actual_cache_lifetimes_cover_discovered_owners
  schema_validation::storage::source::tests::initializer_order_preserves_complete_sibling_prefixes
  schema_validation::storage::source::tests::cache_escape_reset_clone_and_rebind_are_refused
  schema_validation::storage::source::tests::zero_cache_owner_still_requires_wrapper_and_callback_obligations
  schema_validation::storage::tests::fallback_is_covered_once
  schema_validation::storage::tests::heap_terms_and_inline_wrapper_vary_independently
  schema_validation::storage::tests::hostname_overlaps_the_live_wrapper
  schema_validation::storage::tests::fresh_cache_construction_and_partial_failure_are_included
  schema_validation::storage::tests::exact_release_rejects_missing_early_late_and_duplicate_events
  schema_validation::storage::tests::serial_calls_do_not_multiply_peak
  schema_validation::storage::tests::overlap_change_invalidates_serial_composition
  schema_validation::storage::tests::duplicate_ownership_is_not_a_disjoint_live_child
  schema_validation::storage::tests::checked_sums_reject_heap_and_combined_overflow
  schema_validation::storage::tests::lifetimes_require_ordered_in_trace_release_states
  schema_validation::storage::tests::missing_and_invalid_states_are_not_zero_storage_proofs
  schema_validation::storage::tests::structural_and_release_refusals_precede_arithmetic
  schema_validation::storage::tests::coverage_requires_exact_obligations_and_live_heap_parents
  schema_validation::storage::tests::owner_names_are_unique_and_allocation_aliases_must_not_overlap
  schema_validation::storage::tests::combined_peak_uses_same_state_not_independent_maxima
  schema_validation::stack::tests::nested_calls_sum_and_alternatives_take_max
  schema_validation::stack::tests::instance_and_schema_depth_are_independent
  schema_validation::stack::tests::heap_stack_records_are_not_frames
  schema_validation::stack::tests::missing_common_edge_differs_from_missing_target_fact
  schema_validation::stack::tests::each_frame_owner_and_target_slot_is_independently_required
  schema_validation::stack::tests::abi_and_panic_mode_must_match
  schema_validation::stack::tests::expiry_callback_facts_are_nonzero_and_identity_bound
  schema_validation::stack::tests::expiry_work_storage_and_frame_dimensions_vary_independently
  schema_validation::stack::tests::checked_overflow_refuses_instead_of_wrapping
  schema_validation::stack::source::tests::actual_stack_roots_and_call_sites_match_the_capture
  schema_validation::stack::source::tests::stack_paths_separate_calls_arguments_and_alternatives
  schema_validation::stack::source::tests::callbacks_and_opaque_stack_duties_are_not_inline_loops
  schema_validation::stack::source::tests::source_cycles_and_missing_callees_are_not_target_frame_gaps
  schema_validation::admission::tests::missing_common_proof_cannot_complete
  schema_validation::admission::tests::hypothetical_facts_cannot_qualify
  schema_validation::admission::tests::component_ratios_are_not_cpu_weights
  schema_validation::admission::tests::coefficient_boundaries_vary_independently
  schema_validation::admission::tests::uncharged_attempts_survive_zero_debit
  schema_validation::admission::tests::shape_complete_cannot_qualify_policy
  schema_validation::admission::tests::missing_target_facts_cannot_be_zero
  schema_validation::admission::tests::retired_candidate_cannot_supply_costs
  schema_validation::admission::tests::existing_policy_tuple_is_checked_independently
  schema_validation::admission::tests::declaration_maxima_preserve_components
  schema_validation::admission::tests::checked_arithmetic_refuses_overflow
)
printf '%s\n' "${validation_required[@]}" > "$RUNNER_TEMP/schema-validation-required.names"
for owner in schema_residency schema_validation; do
  names="$RUNNER_TEMP/$owner-selected.names"
  log="$RUNNER_TEMP/$owner-development.log"
  grep "^$owner::" "$RUNNER_TEMP/schema-residency-all.names" > "$names"
  selected=$(wc -l < "$names" | tr -d ' ')
  test "$selected" -gt 0
  test "$total" -ge "$selected"
  test "$(LC_ALL=C sort -u "$names" | wc -l | tr -d ' ')" -eq "$selected"
  if [[ "$owner" == schema_residency ]]; then
    frames=1
    required="$names"
  else
    frames=0
    required="$RUNNER_TEMP/schema-validation-required.names"
  fi
  env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test "${cargo_args[@]}" "$owner::" -- --test-threads=1 2>&1 | tee "$log"
  bash .github/workflows/scripts/test/check-libtest-result.sh "$log" "$((total - selected))" 1 "$selected"
  bash .github/workflows/scripts/test/check-schema-residency-dispatch.sh "$log" "$names" "$frames" "$required"
done
env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo clippy "${cargo_args[@]}" -- -D warnings

env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test --locked -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance tests::staged_ -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-metadata-development-normal.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/schema-metadata-development-normal.log")" -eq 1
grep -Fxq 'running 2 tests' "$RUNNER_TEMP/schema-metadata-development-normal.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/schema-metadata-development-normal.log")" -eq 1
grep -Eq '^test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/schema-metadata-development-normal.log"
env AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT=unqualified cargo test --locked -p auctioneer-bidder --features insecure-bidders,auctioneer-bidder/schema-artifact-staging --test schema_source_conformance tests::staged_ -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-metadata-development-staged.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/schema-metadata-development-staged.log")" -eq 1
grep -Fxq 'running 2 tests' "$RUNNER_TEMP/schema-metadata-development-staged.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/schema-metadata-development-staged.log")" -eq 1
grep -Eq '^test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/schema-metadata-development-staged.log"
