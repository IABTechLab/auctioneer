#!/usr/bin/env bash
set -euo pipefail

# Full targets execute every case except the six independently owned source cases.
check_complete() {
  local log="$1"
  local filtered="$2"
  test "$(grep -Ec '^running [0-9]+ tests?$' "$log")" -eq 1
  test "$(grep -c '^test result:' "$log")" -eq 1
  grep -Eq "^test result: ok\\. [1-9][0-9]* passed; 0 failed; 0 ignored; 0 measured; $filtered filtered out; finished in [0-9]+(\\.[0-9]+)?s$" "$log"
}

cargo test --locked -p auctioneer-bidder --test schema_build_controls -- \
  --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-controls.log"
check_complete "$RUNNER_TEMP/schema-controls.log" 0

cargo test --locked -p auctioneer-bidder --test schema_source_conformance -- \
  --test-threads=1 \
  --skip tests::actual_application_artifacts_axum_retain_every_selected_validator_in_both_feature_lanes \
  --skip tests::actual_application_artifacts_cloudflare_retain_every_selected_validator_in_both_feature_lanes \
  --skip tests::actual_application_artifacts_fastly_retain_every_selected_validator_in_both_feature_lanes \
  --skip tests::actual_application_artifacts_spin_retain_every_selected_validator_in_both_feature_lanes \
  --skip tests::compiler_private_layout_facts_cover_native_and_every_guest_abi \
  --skip tests::selected_schema_probe_links_and_executes_the_source_bound_cohort \
  2>&1 | tee "$RUNNER_TEMP/schema-source-compiler.log"
check_complete "$RUNNER_TEMP/schema-source-compiler.log" 6

cargo test --locked -p auctioneer-bidder --test factory_binding_build -- \
  --test-threads=1 2>&1 | tee "$RUNNER_TEMP/factory-binding-build.log"
check_complete "$RUNNER_TEMP/factory-binding-build.log" 0

cargo test --locked -p auctioneer-bidder --test registry_build -- \
  --test-threads=1 2>&1 | tee "$RUNNER_TEMP/registry-build.log"
check_complete "$RUNNER_TEMP/registry-build.log" 0

# `test = false` also removes these targets from broad Clippy selection.
cargo clippy --locked -p auctioneer-bidder --test schema_build_controls \
  --test schema_source_conformance --test factory_binding_build --test registry_build \
  -- -D warnings
