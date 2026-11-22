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

# Exercise the same transport-aware dispatch owner used by development CI.
dispatch_checked=0
check_dispatch() {
  local name="$1" expected="$2" input="$3" selection="${4-$'fake_growth\nfake_regular\n'}"
  printf '%s\n' "$input" > "$scratch/dispatch.log"
  printf '%s' "$selection" > "$scratch/dispatch.names"
  local actual=0
  bash "$script_dir/check-schema-residency-dispatch.sh" "$scratch/dispatch.log" "$scratch/dispatch.names" || actual=$?
  if [[ "$expected" == pass ]]; then
    test "$actual" -eq 0 || { printf 'valid dispatch refused: %s\n' "$name" >&2; exit 1; }
  else
    test "$actual" -ne 0 || { printf 'invalid dispatch accepted: %s\n' "$name" >&2; exit 1; }
  fi
  dispatch_checked=$((dispatch_checked + 1))
}

opening='AUCTIONEER_SCHEMA_GROWTH_BEGIN/development'
closing='AUCTIONEER_SCHEMA_GROWTH_END/development'
content='{"marker":"AUCTIONEER_SCHEMA_GROWTH_END/default","text":"test fake_unknown ... FAILED"}'
regular='test fake_regular ... ok'
frame="test fake_growth ... $opening"$'\n'"$content"$'\n'"$closing"$'\n''ok'
complete="$frame"$'\n'"$regular"
nested="$opening"$'\n'"$content"
discarded="$content"$'\n'"$regular"
check_dispatch framed-first pass "$complete"
check_dispatch framed-last pass "$regular"$'\n'"$frame"
check_dispatch missing-frame refuse $'test fake_growth ... ok\ntest fake_regular ... ok'
check_dispatch missing-begin refuse "${complete/"$opening"/}"
check_dispatch missing-end refuse "${complete/"$closing"/}"
check_dispatch missing-result refuse "$regular"$'\n'"${frame%$'\nok'}"
check_dispatch failed-result refuse "${frame%ok}FAILED"$'\n'"$regular"
check_dispatch inexact-result refuse "${frame%ok} ok"$'\n'"$regular"
check_dispatch empty-body refuse "${complete/"$content"/}"
check_dispatch whitespace-body refuse "${complete/"$content"/$' \t'}"
check_dispatch duplicate-framed-name refuse "$complete"$'\n'"$frame"
check_dispatch second-framed-name refuse "$frame"$'\n'"${frame/fake_growth/fake_regular}"
check_dispatch duplicate-ordinary-success refuse "$complete"$'\n'"$regular"
check_dispatch unknown-name refuse "$complete"$'\n''test fake_unknown ... ok'
check_dispatch unknown-framed-name refuse "${complete/fake_growth/fake_unknown}"
check_dispatch mismatched-lane refuse "${complete/"$opening"/AUCTIONEER_SCHEMA_GROWTH_BEGIN/default}"
check_dispatch mismatched-end refuse "${complete/"$closing"/AUCTIONEER_SCHEMA_GROWTH_END/default}"
check_dispatch nested-frame refuse "${complete/"$content"/"$nested"}"
check_dispatch discarded-result refuse "${complete/"$content"/"$discarded"}"
check_dispatch missing-ordinary-case refuse "$frame"
check_dispatch failed-ordinary-case refuse "$frame"$'\ntest fake_regular ... FAILED'
check_dispatch inexact-ordinary-case refuse "$frame"$'\n'"$regular "
check_dispatch intervening-result refuse "${frame%ok}$regular"$'\nok\n'"$regular"
check_dispatch unmatched-end refuse "$complete"$'\n'"$closing"
check_dispatch zero-names refuse "$complete" ''
check_dispatch duplicate-names refuse "$complete" $'fake_growth\nfake_regular\nfake_growth\n'
check_dispatch blank-name refuse "$complete" $'fake_growth\n\nfake_regular\n'
printf 'schema residency dispatch controls: %s passed\n' "$dispatch_checked"
