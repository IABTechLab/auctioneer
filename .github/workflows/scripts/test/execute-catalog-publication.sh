#!/usr/bin/env bash
set -euo pipefail
export CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir $PWD"
cargo test --locked -p auctioneer-bidder --test catalog_publication --target wasm32-wasip1 \
  -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/catalog-guest-secure.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/catalog-guest-secure.log")" -eq 1
grep -Fxq 'running 2 tests' "$RUNNER_TEMP/catalog-guest-secure.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/catalog-guest-secure.log")" -eq 1
grep -Eq '^test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/catalog-guest-secure.log"
cargo test --locked -p auctioneer-bidder --test catalog_publication --target wasm32-wasip1 \
  --features insecure-bidders -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/catalog-guest-development.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/catalog-guest-development.log")" -eq 1
grep -Fxq 'running 2 tests' "$RUNNER_TEMP/catalog-guest-development.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/catalog-guest-development.log")" -eq 1
grep -Eq '^test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/catalog-guest-development.log"
cargo test --locked -p auctioneer-config --test bidder_storage --target wasm32-wasip1 \
  -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/catalog-storage-guest-secure.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/catalog-storage-guest-secure.log")" -eq 1
grep -Fxq 'running 1 test' "$RUNNER_TEMP/catalog-storage-guest-secure.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/catalog-storage-guest-secure.log")" -eq 1
grep -Eq '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/catalog-storage-guest-secure.log"
cargo test --locked -p auctioneer-config --test bidder_storage --target wasm32-wasip1 \
  --features insecure-bidders -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/catalog-storage-guest-development.log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$RUNNER_TEMP/catalog-storage-guest-development.log")" -eq 1
grep -Fxq 'running 1 test' "$RUNNER_TEMP/catalog-storage-guest-development.log"
test "$(grep -c '^test result:' "$RUNNER_TEMP/catalog-storage-guest-development.log")" -eq 1
grep -Eq '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+(\.[0-9]+)?s$' "$RUNNER_TEMP/catalog-storage-guest-development.log"
