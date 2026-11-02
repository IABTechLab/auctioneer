#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT
checked=0

check() {
  local name="$1" expected="$2" body="$3" filtered="$4" reports="$5"
  shift 5
  printf '%s\n' "$body" > "$scratch/result.log"
  local actual=0
  bash "$script_dir/check-libtest-result.sh" "$scratch/result.log" "$filtered" "$reports" "$@" || actual=$?
  if [[ "$expected" == pass ]]; then
    test "$actual" -eq 0 || { printf 'valid summary refused: %s\n' "$name" >&2; exit 1; }
  else
    test "$actual" -ne 0 || { printf 'invalid summary accepted: %s\n' "$name" >&2; exit 1; }
  fi
  checked=$((checked + 1))
}

one=$'running 1 test\ntest tests::fake ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.01s'
many=$'running 19 tests\ntest result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2s'
check exact pass "$one" 18 1 1
check generic pass "$many" 0 1
check nested pass "$many"$'\n'"$many" 0 2
check wrong-passed refuse "$one" 18 1 2
check wrong-filter refuse "$one" 17 1 1
check zero-reports refuse "$one" 18 0 1
check bad-filter refuse "$one" invalid 1 1
check zero-passed-argument refuse "$one" 18 1 0
check bad-reports refuse "$one" 18 invalid 1
check missing-running refuse "${one#*$'\n'}" 18 1 1
check missing-summary refuse $'running 1 test\ntest tests::fake ... ok' 18 1 1
check extra-summary refuse "$one"$'\n'"$one" 18 1 1
check failed refuse "${one/0 failed/1 failed}" 18 1 1
check ignored refuse "${one/0 ignored/1 ignored}" 18 1 1
check measured refuse "${one/0 measured/1 measured}" 18 1 1
check zero-dispatch refuse "${one/1 passed/0 passed}" 18 1
check malformed-duration refuse "${one/0.01s/unknown}" 18 1 1
check mixed-nested refuse "$many"$'\n'"${many/0 ignored/1 ignored}" 0 2
printf 'libtest summary controls: %s passed\n' "$checked"
