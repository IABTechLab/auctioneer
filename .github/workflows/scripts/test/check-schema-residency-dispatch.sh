#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  printf 'Usage: %s LOG SELECTED_NAMES EXPECTED_FRAMES REQUIRED_NAMES\n' "$0" >&2
  exit 2
fi
case "$3" in
  0|1) ;;
  *) printf 'EXPECTED_FRAMES must be 0 or 1\n' >&2; exit 2 ;;
esac

# Count actual libtest results, allowing one interrupted development result.
awk -v expected_frames="$3" -v required_file="$4" '
function refuse() { invalid = 1; exit 1 }
BEGIN {
  frames = 0
  opening = "AUCTIONEER_SCHEMA_GROWTH_BEGIN/development"
  closing = "AUCTIONEER_SCHEMA_GROWTH_END/development"
  while ((read_status = getline name < required_file) > 0) {
    if (name == "" || name ~ /[[:space:]]/ || (name in required)) refuse()
    required[name] = 1; required_count++
  }
  close(required_file)
  if (read_status < 0 || !required_count) refuse()
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
  if (invalid || !wanted || phase || frames != expected_frames) exit 1
  for (name in required) if (!(name in expected)) exit 1
  for (name in expected) if (results[name] != 1) exit 1
}
' "$2" "$1"
