#!/usr/bin/env bash
set -euo pipefail
env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test --locked -p auctioneer-bidder --test schema_source_conformance tests::staged_ -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-metadata-secure-normal.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/schema-metadata-secure-normal.log")" -eq 1
grep -Fxq 'running 2 tests' "$RUNNER_TEMP/schema-metadata-secure-normal.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/schema-metadata-secure-normal.log")" -eq 1
grep -Eq '^test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/schema-metadata-secure-normal.log"
env AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT=unqualified cargo test --locked -p auctioneer-bidder --features auctioneer-bidder/schema-artifact-staging --test schema_source_conformance tests::staged_ -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-metadata-secure-staged.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/schema-metadata-secure-staged.log")" -eq 1
grep -Fxq 'running 2 tests' "$RUNNER_TEMP/schema-metadata-secure-staged.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/schema-metadata-secure-staged.log")" -eq 1
grep -Eq '^test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/schema-metadata-secure-staged.log"
