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
