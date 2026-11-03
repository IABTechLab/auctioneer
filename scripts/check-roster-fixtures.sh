#!/usr/bin/env bash
# Negative fixtures for `scripts/check-roster.sh`.
#
# A gate that has only ever been run against a tree it passes proves
# nothing: it could be returning 0 unconditionally. These cases plant a
# specific defect and require the gate to REJECT it with the matching
# verdict, then require the unmodified plan to pass.
#
# Fixtures are DERIVED from the real plan by a planting edit rather than
# checked in as copies. A checked-in copy of §9 would drift from the
# real one the first time the roster changed, and a gate tested against
# a stale fixture is the defect this file exists to rule out.
#
# Exit 0 = every case behaved, 1 = a case did not, >1 = the runner
# itself failed.
set -euo pipefail

here=$(cd -- "$(dirname -- "$0")" && pwd)
repo=$(cd -- "$here/.." && pwd)
gate="$here/check-roster.sh"
plan=${1:-$repo/docs/superpowers/plans/2026-07-10-multi-adapter-production-fanout.md}

[ -x "$gate" ] || { echo "gate not executable: $gate" >&2; exit 3; }
[ -r "$plan" ] || { echo "cannot read plan: $plan" >&2; exit 3; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

failures=0

# Run the gate over a fixture and require an exit code AND a verdict
# substring. Checking only the code would accept a rejection for the
# wrong reason — a fixture that happens to break the extractor also
# exits non-zero, and that is a different bug, not the one under test.
expect() {
  local label=$1 fixture=$2 want_code=$3 want_text=$4
  local out code
  out="$work/$label.out"
  set +e
  "$gate" "$fixture" >"$out" 2>&1
  code=$?
  set -e
  if [ "$code" -ne "$want_code" ]; then
    echo "FAIL [$label]: expected exit $want_code, got $code" >&2
    sed 's/^/    /' "$out" >&2
    failures=$(( failures + 1 ))
    return
  fi
  if ! grep -qF "$want_text" "$out"; then
    echo "FAIL [$label]: exit $code was right but the verdict did not mention '$want_text'" >&2
    sed 's/^/    /' "$out" >&2
    failures=$(( failures + 1 ))
    return
  fi
  echo "ok [$label]: exit $code — $(grep -m1 -F "$want_text" "$out")"
}

# --- case: the unmodified plan passes -------------------------------
#
# First, not last: every negative case below is only meaningful if the
# baseline is green, and a runner that reports three rejections over a
# plan the gate rejects anyway has proved nothing.
expect baseline "$plan" 0 "members enumerated, uniquely owned"

# --- case: a MISSING member -----------------------------------------
#
# `stream_gate_mixed_size_fifo_order_head_of_line()` is one of the
# seven the `7 weighted-gate` label claims. Deleting its name leaves
# the label's count intact, so the landing split still adds up and
# the OLD gate passed this fixture. The enumeration does not: six names
# cannot discharge a label that claims seven.
missing="$work/missing-member.md"
grep -v 'stream_gate_mixed_size_fifo_order_head_of_line' "$plan" >"$missing"
# Guard the fixture itself: a planting edit that changed nothing would
# make the case pass for the wrong reason.
if cmp -s "$plan" "$missing"; then
  echo "fixture 'missing-member' planted nothing — the target name is absent from the plan" >&2
  exit 3
fi
expect missing-member "$missing" 1 "ROSTER MISSING MEMBER"

# A member outside the counted-label table must be protected too. The
# declared total is unchanged, but one named declaration is now absent.
arbitrary_missing="$work/missing-proxy-member.md"
grep -v 'proxy_adapter_sets_cache_bypass' "$plan" >"$arbitrary_missing"
if cmp -s "$plan" "$arbitrary_missing"; then
  echo "fixture 'missing-proxy-member' planted nothing" >&2
  exit 3
fi
expect missing-proxy-member "$arbitrary_missing" 1 "ROSTER ENUMERATION MISMATCH"

# --- case: a DUPLICATED member --------------------------------------
#
# One member declared by two roster entries. The landing split cannot
# detect the extra declaring site, so unique ownership must reject it.
dupe="$work/duplicate-member.md"
awk '
  { print }
  /^   - `proxy_adapter_sets_cache_bypass\(\)` — round-162 #1: the request/ && !done {
    print "   - `proxy_adapter_sets_cache_bypass()` — a second entry claiming"
    print "     the same test, which no arithmetic check can notice."
    done = 1
  }
' "$plan" >"$dupe"
if cmp -s "$plan" "$dupe"; then
  echo "fixture 'duplicate-member' planted nothing — the anchor line moved" >&2
  exit 3
fi
expect duplicate-member "$dupe" 1 "ROSTER DUPLICATE MEMBER"

# A concrete declaration cannot also be owned by a per-target template.
# Deduplicating after expansion would hide this second owner entirely.
expanded_dupe="$work/duplicate-expanded-member.md"
awk '
  { print }
  /^   - `proxy_adapter_sets_cache_bypass\(\)`/ && !done {
    print "   - `wave_wait_does_not_expire_before_deadline_axum()`"
    done = 1
  }
' "$plan" >"$expanded_dupe"
if cmp -s "$plan" "$expanded_dupe"; then
  echo "fixture 'duplicate-expanded-member' planted nothing" >&2
  exit 3
fi
expect duplicate-expanded-member "$expanded_dupe" 1 "ROSTER DUPLICATE MEMBER"

# Removing a template's placeholder loses three concrete members, not
# just one declaration. Keep all four adapters under the count guard.
template="$work/unexpanded-template.md"
sed 's/wave_wait_does_not_expire_before_deadline_<target>/wave_wait_does_not_expire_before_deadline_axum/g' "$plan" >"$template"
if cmp -s "$plan" "$template"; then
  echo "fixture 'unexpanded-template' planted nothing" >&2
  exit 3
fi
expect unexpanded-template "$template" 1 "ROSTER MISSING MEMBER"

# --- case: the arithmetic check still fires -------------------------
#
# The property the gate already had, kept under test so the enumeration
# work cannot have silently dropped it.
arith="$work/bad-arithmetic.md"
awk '
  /one list, \+[0-9]+ total/ && !done {
    match($0, /\+[0-9]+ total/)
    total = substr($0, RSTART + 1, RLENGTH - 7) + 1
    $0 = substr($0, 1, RSTART - 1) "+" total " total" substr($0, RSTART + RLENGTH)
    done = 1
  }
  { print }
' "$plan" >"$arith"
if cmp -s "$plan" "$arith"; then
  echo "fixture 'bad-arithmetic' planted nothing — the total's wording moved" >&2
  exit 3
fi
expect bad-arithmetic "$arith" 1 "ROSTER ARITHMETIC MISMATCH"

# --- case: the gate's own failure is not a verdict ------------------
#
# Round-188 #5's lesson: a gate that cannot read its input must not
# exit 0 (clean) or 1 (violation). Both would be a lie about a scan
# that never ran.
expect self-failure "$work/does-not-exist.md" 3 "cannot read"

if [ "$failures" -ne 0 ]; then
  echo "$failures roster-gate fixture case(s) did not behave as required" >&2
  exit 1
fi
echo "all roster-gate fixture cases behaved"
