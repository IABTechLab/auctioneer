#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
producer_script="${1:-$script_dir/verify-evidence-producers.sh}"
scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT
mkdir -p "$scratch/bin"
schema='tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification'
native='tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog'
cat > "$scratch/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
set -euo pipefail
schema='tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification'
native='tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog'
scenario="$AUCTIONEER__TEST__EVIDENCE_SCENARIO"
if [[ " $* " == *' -- --list '* ]]; then
  printf 'list\n' >> "$AUCTIONEER__TEST__EVIDENCE_COMMANDS"
  printf '%s: test\n' "$schema"
  if [[ "$scenario" != missing-native ]]; then printf '%s: test\n' "$native"; fi
  if [[ "$scenario" == duplicate-schema ]]; then printf '%s: test\n' "$schema"; fi
  printf 'tests::lightweight_control: test\n'
  exit 0
fi
selected=''
for argument in "$@"; do
  if [[ "$argument" == "$schema" || "$argument" == "$native" ]]; then selected="$argument"; fi
done
test -n "$selected"
[[ " $* " == *' -- --exact --test-threads=1 '* ]]
printf '%s\n' "$selected" >> "$AUCTIONEER__TEST__EVIDENCE_COMMANDS"
case "$scenario" in
  zero) printf 'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n'; exit 0 ;;
  failed) printf 'running 1 test\ntest %s ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s\n' "$selected"; exit 101 ;;
  wrong-test) selected='tests::unrelated_control' ;;
esac
printf 'running 1 test\ntest %s ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s\n' "$selected"
CARGO
chmod +x "$scratch/bin/cargo"
checked=0
check() {
  local name="$1" outcome="$2" scenario="$3" schema_count="$4" native_count="$5"
  shift 5
  local case_dir="$scratch/$name" actual=0
  mkdir -p "$case_dir"
  : > "$case_dir/commands"
  PATH="$scratch/bin:$PATH" RUNNER_TEMP="$case_dir" \
    AUCTIONEER__TEST__EVIDENCE_SCENARIO="$scenario" \
    AUCTIONEER__TEST__EVIDENCE_COMMANDS="$case_dir/commands" \
    bash "$producer_script" "$@" > "$case_dir/output" 2>&1 || actual=$?
  if [[ "$outcome" == pass ]]; then
    test "$actual" -eq 0 || { cat "$case_dir/output"; printf 'valid selection refused: %s\n' "$name" >&2; exit 1; }
  else
    test "$actual" -ne 0 || { printf 'invalid selection accepted: %s\n' "$name" >&2; exit 1; }
  fi
  test "$(grep -Fxc "$schema" "$case_dir/commands" || true)" -eq "$schema_count" || { printf 'wrong schema dispatch count: %s\n' "$name" >&2; exit 1; }
  test "$(grep -Fxc "$native" "$case_dir/commands" || true)" -eq "$native_count" || { printf 'wrong native dispatch count: %s\n' "$name" >&2; exit 1; }
  checked=$((checked + 1))
}
check schema-only pass good 1 0 schema-source
check native-only pass good 0 1 controlled-native
check default-all pass good 1 1
check explicit-all pass good 1 1 all
check unknown refuse good 0 0 unknown
check empty-selection refuse good 0 0 ""
check extra-argument refuse good 0 0 schema-source unexpected
check missing-roster-member refuse missing-native 0 0 schema-source
check duplicate-roster-member refuse duplicate-schema 0 0 controlled-native
check zero-dispatch refuse zero 1 0 schema-source
check failed-dispatch refuse failed 0 1 controlled-native
check wrong-dispatch refuse wrong-test 1 0 schema-source
printf 'evidence producer selection controls: %s passed\n' "$checked"
