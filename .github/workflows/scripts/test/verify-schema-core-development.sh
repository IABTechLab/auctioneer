#!/usr/bin/env bash
set -euo pipefail
env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test --locked -p auctioneer-core --features insecure-bidders --lib schema_artifact_ -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-core-development-normal.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/schema-core-development-normal.log")" -eq 1
grep -Fxq 'running 4 tests' "$RUNNER_TEMP/schema-core-development-normal.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/schema-core-development-normal.log")" -eq 1
grep -Eq '^test result: ok\. 4 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/schema-core-development-normal.log"
env AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT=unqualified cargo test --locked -p auctioneer-core --features insecure-bidders,auctioneer-bidder/schema-artifact-staging --lib schema_artifact_ -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-core-development-staged.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/schema-core-development-staged.log")" -eq 1
grep -Fxq 'running 4 tests' "$RUNNER_TEMP/schema-core-development-staged.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/schema-core-development-staged.log")" -eq 1
grep -Eq '^test result: ok\. 4 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/schema-core-development-staged.log"
