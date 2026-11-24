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
  schema_validation::operations::tests::unclassified_call_refuses_completion
  schema_validation::operations::tests::changed_branch_multiplicity_changes_the_recipe
  schema_validation::operations::tests::non_regex_declaration_has_a_nonempty_closure
  schema_validation::operations::tests::changed_primitive_semantics_is_not_approved_by_rehashing
  schema_validation::operations::tests::expiry_binding_is_explicit
  schema_validation::operations::tests::unknown_callback_is_not_a_free_source_edge
  schema_validation::operations::tests::new_nested_literal_owner_requires_a_supported_proof
  schema_validation::operations::tests::cold_entropy_has_exact_identity_lifetime_and_nonfree_obligations
  schema_validation::tests::current_body_shapes_have_declared_operation_obligations
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
