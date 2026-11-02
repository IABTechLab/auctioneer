#!/usr/bin/env bash
set -euo pipefail

lane="${1:-all}"
if [[ $# -gt 1 ]]; then
  printf 'Usage: %s [all|schema|settings|exports|discovery]\n' "$0" >&2
  exit 2
fi
case "$lane" in
  all)
    targets=(schema_build_controls schema_source_conformance factory_binding_build registry_build generated_settings insecure_bidders_snapshot response_body_is_sealed feature_exports_source_conformance)
    ;;
  schema) targets=(schema_build_controls schema_source_conformance) ;;
  settings) targets=(generated_settings) ;;
  exports) targets=(insecure_bidders_snapshot response_body_is_sealed feature_exports_source_conformance) ;;
  discovery) targets=(factory_binding_build registry_build) ;;
  *)
    printf 'Usage: %s [all|schema|settings|exports|discovery]\n' "$0" >&2
    exit 2
    ;;
esac

# Count both reports when a full target forwards its child contract's output.
check_complete() {
  bash "$(dirname -- "${BASH_SOURCE[0]}")/check-libtest-result.sh" "$@"
}

if [[ "$lane" == all || "$lane" == settings ]]; then
  cargo test --locked -p auctioneer-bidder --test generated_settings -- \
    --test-threads=1 2>&1 | tee "$RUNNER_TEMP/generated-settings.log"
  check_complete "$RUNNER_TEMP/generated-settings.log" 0 2
fi

if [[ "$lane" == all || "$lane" == exports ]]; then
  cargo test --locked -p auctioneer-bidder --test insecure_bidders_snapshot -- \
    --test-threads=1 2>&1 | tee "$RUNNER_TEMP/insecure-bidders-snapshot.log"
  check_complete "$RUNNER_TEMP/insecure-bidders-snapshot.log" 0

  cargo test --locked -p auctioneer-bidder --test response_body_is_sealed -- \
    --test-threads=1 2>&1 | tee "$RUNNER_TEMP/response-body-is-sealed.log"
  check_complete "$RUNNER_TEMP/response-body-is-sealed.log" 0

  cargo test --locked -p auctioneer-bidder --test feature_exports_source_conformance -- \
    --test-threads=1 2>&1 | tee "$RUNNER_TEMP/feature-exports-source-conformance.log"
  check_complete "$RUNNER_TEMP/feature-exports-source-conformance.log" 0
fi

if [[ "$lane" == all || "$lane" == schema ]]; then
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
fi

if [[ "$lane" == all || "$lane" == discovery ]]; then
  cargo test --locked -p auctioneer-bidder --test factory_binding_build -- \
    --test-threads=1 2>&1 | tee "$RUNNER_TEMP/factory-binding-build.log"
  check_complete "$RUNNER_TEMP/factory-binding-build.log" 0

  cargo test --locked -p auctioneer-bidder --test registry_build -- \
    --test-threads=1 2>&1 | tee "$RUNNER_TEMP/registry-build.log"
  check_complete "$RUNNER_TEMP/registry-build.log" 0
fi

# `test = false` also removes these targets from broad Clippy selection.
clippy_targets=()
for target in "${targets[@]}"; do
  clippy_targets+=(--test "$target")
done
cargo clippy --locked -p auctioneer-bidder "${clippy_targets[@]}" -- -D warnings
