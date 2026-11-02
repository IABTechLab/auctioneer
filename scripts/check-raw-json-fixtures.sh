#!/usr/bin/env bash
# Gate on raw-string JSON fixtures in Rust sources.
#
# A fixture that needs a VALID JSON object belongs in
# `serde_json::json!`: the macro rejects a missing brace or comma at
# compile time, carries no escaping, and diffs one field at a time. A
# raw string gets none of that, and the codebase had drifted into both
# conventions at once — `json!` in 31 files, raw strings in 30 more.
#
# Some fixtures legitimately cannot convert, so this is an allowlist
# gate rather than a ban. `scripts/raw-json-fixtures.allow` records the
# files that may hold raw fixtures AND HOW MANY each may hold. Allowing
# a path without a count would leave the door open that the exemptions
# opened: a new raw fixture dropped into an already-exempt file would
# pass. The count closes it.
#
# Two failure directions, both reported:
#
#   OVER  — a file holds more raw fixtures than its budget, or holds
#           any while absent from the allowlist. This is the drift the
#           gate exists to stop.
#   STALE — a file holds FEWER than its budget, or is listed and holds
#           none. Not a defect in the tree, but the allowlist is now
#           lying, and a budget with slack in it silently re-admits the
#           next raw fixture. Lower the number.
#
# Exit 0 = the tree matches the allowlist, 1 = it does not, >1 = the
# script itself failed (a masked failure must never read as a verdict).
set -euo pipefail

here=$(cd -- "$(dirname -- "$0")" && pwd)
repo=$(cd -- "$here/.." && pwd)
allow=${1:-$here/raw-json-fixtures.allow}
root=${2:-$repo/crates}

[ -r "$allow" ] || { echo "cannot read allowlist: $allow" >&2; exit 3; }
[ -d "$root" ] || { echo "not a directory: $root" >&2; exit 3; }

# Allowlist paths are written relative to the tree root's PARENT
# (`crates/foo/bar.rs`), not to this checkout, so the gate reads the
# same against the repository and against a throwaway copy of it. Both
# names are resolved here so the two halves of the report cannot drift
# onto different bases.
root=$(cd -- "$root" && pwd)
base=$(cd -- "$root/.." && pwd)
leaf=${root##*/}

# Skip complete quoted strings/chars before recognizing a raw opener; otherwise
# ordinary values such as "r" or "some r" followed by a block become fixtures.
# Preserve byte/C-string prefixes and zero-hash raw literals.
pattern='^([^"]|"([^"\\]|\\.)*"|'\''([^'\''\\]|\\.)*'\'')*(^|[^[:alnum:]_"])[bc]?r#*"[[:space:]]*\{'

work=$(mktemp -d) || { echo "mktemp failed" >&2; exit 3; }
trap 'rm -rf "$work"' EXIT

# Observed counts, as `<path> <count>`, paths relative to `$base`.
#
# grep exits 1 on no match, which here means a clean tree, so that one
# status is swallowed. A `cd` failure or a grep ERROR (exit >1) must
# not be: it would read as a clean tree and pass the gate.
cd -- "$base" || { echo "cannot enter $base" >&2; exit 3; }
grep -rnE "$pattern" "$leaf" --include='*.rs' >"$work/hits" && hits=0 || hits=$?
case "$hits" in
  0 | 1) ;;
  *) echo "grep failed over $root (exit $hits)" >&2; exit 3 ;;
esac
awk -F: '{ c[$1]++ } END { for (f in c) printf "%s %d\n", f, c[f] }' "$work/hits" \
  | sort >"$work/observed"

# Bidder-specific exceptions stay with their owner. Resolve against the tested
# tree, not this checkout, so disposable negative controls remain isolated.
lists=("$allow")
owner_allow="$root/auctioneer-bidder/tests/conformance/raw-json-fixtures.allow"
if [ -r "$owner_allow" ]; then
  lists+=("$owner_allow")
fi

# Allowed counts, comments and blank lines stripped.
sed -e 's/#.*//' -e '/^[[:space:]]*$/d' "${lists[@]}" \
  | awk '{ print $1, $2 }' | sort >"$work/allowed"

# Every allowlist row must be well formed; a typo'd row that parsed as
# a zero budget would quietly tighten the gate on an unrelated file.
if bad=$(awk 'NF != 2 || $2 !~ /^[0-9]+$/ { print NR": "$0 }' "$work/allowed") \
   && [ -n "$bad" ]; then
  echo "malformed allowlist rows (want '<path> <count>'):" >&2
  echo "$bad" >&2
  exit 3
fi

status=0

while read -r path count; do
  [ -n "$path" ] || continue
  budget=$(awk -v p="$path" '$1 == p { print $2 }' "$work/allowed")
  if [ -z "$budget" ]; then
    echo "OVER  $path: $count raw JSON fixture(s), not on the allowlist"
    echo "      Build them with serde_json::json!, or add the file to"
    echo "      $(basename "$allow") with a comment naming the exemption."
    status=1
  elif [ "$count" -gt "$budget" ]; then
    echo "OVER  $path: $count raw JSON fixture(s), budget $budget"
    echo "      The new one(s) must use serde_json::json!, or the budget"
    echo "      must rise with a comment naming the exemption."
    status=1
  elif [ "$count" -lt "$budget" ]; then
    echo "STALE $path: $count raw JSON fixture(s), budget $budget"
    echo "      Lower the budget to $count; slack re-admits raw fixtures."
    status=1
  fi
done <"$work/observed"

# Listed files that no longer hold any: the row is dead.
while read -r path budget; do
  [ -n "$path" ] || continue
  if ! awk -v p="$path" '$1 == p { found = 1 } END { exit !found }' "$work/observed"; then
    if [ -r "$base/$path" ]; then
      echo "STALE $path: 0 raw JSON fixture(s), budget $budget"
      echo "      Every fixture here converted; drop the allowlist row."
    else
      echo "STALE $path: file is gone; drop the allowlist row."
    fi
    status=1
  fi
done <"$work/allowed"

if [ "$status" -eq 0 ]; then
  total=$(awk '{ s += $2 } END { print s + 0 }' "$work/observed")
  files=$(wc -l <"$work/observed" | tr -d ' ')
  echo "ok: $total raw JSON fixture(s) across $files file(s), all budgeted"
fi

exit "$status"
