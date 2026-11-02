#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 4 ]]; then
  printf 'Usage: %s LOG FILTERED [REPORTS [PASSED]]\n' "$0" >&2
  exit 2
fi
log="$1"
filtered="$2"
reports="${3:-1}"
passed='[1-9][0-9]*'
if ! [[ "$filtered" =~ ^[0-9]+$ && "$reports" =~ ^[1-9][0-9]*$ ]]; then
  exit 2
fi
if [[ $# -eq 4 ]]; then
  if ! [[ "$4" =~ ^[1-9][0-9]*$ ]]; then
    exit 2
  fi
  passed="$4"
fi

test "$(grep -Ec '^running [0-9]+ tests?$' "$log")" -eq "$reports"
test "$(grep -c '^test result:' "$log")" -eq "$reports"
test "$(grep -Ec "^test result: ok\\. $passed passed; 0 failed; 0 ignored; 0 measured; $filtered filtered out; finished in [0-9]+(\\.[0-9]+)?s$" "$log")" -eq "$reports"
