#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
tests=(
  tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification
  tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
)

# Enumerate without accepting zero-dispatch as a successful producer run.
listing="$RUNNER_TEMP/evidence-producers-list.log"
cargo test --locked -p auctioneer-cli --test bidders_evidence -- --list \
  2>&1 | tee "$listing"
total="$(grep -Ec ': test$' "$listing")"
test "$total" -ge "${#tests[@]}"
for name in "${tests[@]}"; do
  test "$(grep -Fxc "$name: test" "$listing")" -eq 1
done
filtered="$((total - 1))"
for index in "${!tests[@]}"; do
  name="${tests[$index]}"
  log="$RUNNER_TEMP/evidence-producer-$index.log"
  cargo test --locked -p auctioneer-cli --test bidders_evidence "$name" -- \
    --exact --test-threads=1 2>&1 | tee "$log"
  bash "$script_dir/check-libtest-result.sh" "$log" "$filtered" 1 1
  test "$(grep -Fxc "test $name ... ok" "$log")" -eq 1
done
