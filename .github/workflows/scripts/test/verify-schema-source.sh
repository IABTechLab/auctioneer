#!/usr/bin/env bash
set -euo pipefail

expected="$1"
filter="$2"
shift 2
[[ "$expected" =~ ^[1-9][0-9]*$ ]]
arguments=(--nocapture --test-threads=1)
if [[ "$filter" != schema_selection ]]; then
  arguments+=(--exact)
fi
log="$RUNNER_TEMP/schema-source.log"
cargo test "$@" --test schema_source_conformance "$filter" -- "${arguments[@]}" 2>&1 | tee "$log"
test "$(grep -Ec '^running [0-9]+ tests?$' "$log")" -eq 1
grep -Eq "^running $expected tests?$" "$log"
test "$(grep -c '^test result:' "$log")" -eq 1
grep -Eq "^test result: ok\\. $expected passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(\\.[0-9]+)?s$" "$log"
