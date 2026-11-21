#!/usr/bin/env bash
set -euo pipefail

# Reuse this lane's normal insecure binary, but prove the prefix runs every
# listed ownership case exactly once instead of accepting a zero-test filter.
cargo_args=(--locked -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance)
env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test "${cargo_args[@]}" -- --list --format terse > "$RUNNER_TEMP/schema-residency-all.list"
sed -n 's/: test$//p' "$RUNNER_TEMP/schema-residency-all.list" | LC_ALL=C sort > "$RUNNER_TEMP/schema-residency-all.names"
grep '^schema_residency::' "$RUNNER_TEMP/schema-residency-all.names" > "$RUNNER_TEMP/schema-residency-selected.names"
selected=$(wc -l < "$RUNNER_TEMP/schema-residency-selected.names" | tr -d ' ')
total=$(wc -l < "$RUNNER_TEMP/schema-residency-all.names" | tr -d ' ')
test "$selected" -gt 0
test "$total" -ge "$selected"
test "$(LC_ALL=C sort -u "$RUNNER_TEMP/schema-residency-selected.names" | wc -l | tr -d ' ')" -eq "$selected"
env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test "${cargo_args[@]}" schema_residency:: -- --test-threads=1 2>&1 | tee "$RUNNER_TEMP/schema-residency-development.log"
bash .github/workflows/scripts/test/check-libtest-result.sh "$RUNNER_TEMP/schema-residency-development.log" "$((total - selected))" 1 "$selected"
while IFS= read -r name; do
  test "$(grep -Fxc "test $name ... ok" "$RUNNER_TEMP/schema-residency-development.log")" -eq 1
done < "$RUNNER_TEMP/schema-residency-selected.names"

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
