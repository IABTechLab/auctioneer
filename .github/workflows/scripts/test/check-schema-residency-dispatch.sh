#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  printf 'Usage: %s LOG SELECTED_NAMES\n' "$0" >&2
  exit 2
fi

# Count actual libtest results, allowing one interrupted development result.
awk '
function refuse() { invalid = 1; exit 1 }
BEGIN {
  opening = "AUCTIONEER_SCHEMA_GROWTH_BEGIN/development"
  closing = "AUCTIONEER_SCHEMA_GROWTH_END/development"
}
FILENAME == ARGV[1] {
  if ($0 == "" || $0 ~ /[[:space:]]/ || ($0 in expected)) refuse()
  expected[$0] = 1; wanted++; next
}
{
  marker = ($0 ~ /^AUCTIONEER_SCHEMA_GROWTH_(BEGIN|END)\/[^[:space:]]+$/)
  if (phase == 2) {
    if ($0 != "ok") refuse()
    results[active]++; phase = 0; next
  }
  if (phase == 1) {
    if ($0 == closing) {
      if (!body) refuse()
      phase = 2; next
    }
    if (marker || $0 ~ /^test /) refuse()
    if ($0 !~ /^[[:space:]]*$/) body++
    next
  }
  if (marker || $0 == "ok") refuse()
  if ($0 ~ /^test / && $0 !~ /^test result:/) {
    name = $0; sub(/^test /, "", name); sub(/ \.\.\..*$/, "", name)
    if (!(name in expected) || results[name]) refuse()
    if ($0 == "test " name " ... ok") { results[name]++; next }
    if ($0 != "test " name " ... " opening || frames) refuse()
    active = name; frames++; body = 0; phase = 1
  }
}
END {
  if (invalid || !wanted || phase || frames != 1) exit 1
  for (name in expected) if (results[name] != 1) exit 1
}
' "$2" "$1"
