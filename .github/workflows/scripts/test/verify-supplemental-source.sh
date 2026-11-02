#!/usr/bin/env bash
set -euo pipefail
cargo test --locked -p auctioneer-bidder --lib supplemental_source_matches_immutable_reference \
  -- --ignored 2>&1 | tee "$RUNNER_TEMP/bidder-supplemental-source.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/bidder-supplemental-source.log")" -eq 1
grep -Fxq 'running 1 test' "$RUNNER_TEMP/bidder-supplemental-source.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/bidder-supplemental-source.log")" -eq 1
grep -Eq '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/bidder-supplemental-source.log"
