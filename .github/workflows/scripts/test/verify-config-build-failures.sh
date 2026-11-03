#!/usr/bin/env bash
set -euo pipefail

cargo test --locked -p auctioneer-config-data --test build_failure_harness \
  2>&1 | tee "$RUNNER_TEMP/config-build-failures.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/config-build-failures.log")" -eq 1
test "$(grep -c '^test result:' "$RUNNER_TEMP/config-build-failures.log")" -eq 1
grep -Eq '^test result: ok\. [1-9][0-9]* passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/config-build-failures.log"

cargo clippy --locked -p auctioneer-config-data --test build_failure_harness -- -D warnings

cargo test --locked -p auctioneer-exchange --test disclosure_model_coverage -- --ignored \
  2>&1 | tee "$RUNNER_TEMP/disclosure-model-coverage.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/disclosure-model-coverage.log")" -eq 1
grep -Fxq 'running 1 test' "$RUNNER_TEMP/disclosure-model-coverage.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/disclosure-model-coverage.log")" -eq 1
grep -Eq '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/disclosure-model-coverage.log"
