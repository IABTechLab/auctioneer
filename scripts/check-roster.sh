#!/usr/bin/env bash
# Roster gate for §9 of the bounded fanout plan.
#
# §9 is the ONE authoritative test roster. Two different properties can
# be wrong about it, and a gate that checks only the first passes a
# roster that is materially broken:
#
#   1. ARITHMETIC — §9 declares a total and a landing split. Check the
#      split, then require the named, expanded roster to equal the total.
#      Historical round-by-round addends are not a second roster.
#   2. ENUMERATION AND OWNERSHIP — the acceptance contract §9 actually
#      carries: every member is NAMED, each name is claimed by exactly
#      ONE declaring site, and every group label that stands in for a
#      count of members is DISCHARGED by that many named tests or by an
#      entry in the LABEL -> OWNER index.
#
# Checking (1) alone is what let the roster read as consistent while a
# member was missing or declared twice: a missing member and a
# compensating block edit still add up. (2) is what fails on those.
#
# Exit 0 = consistent and enumerable, 1 = a violation, >1 = the script
# itself failed (a masked failure must never read as either verdict).
set -euo pipefail

plan=${1:-docs/superpowers/plans/2026-07-10-multi-adapter-production-fanout.md}
[ -r "$plan" ] || { echo "cannot read $plan" >&2; exit 3; }

start=$(grep -n '^9\. \*\*Tests (land with Fanout-2' "$plan" | head -1 | cut -d: -f1) || true
[ -n "${start:-}" ] || { echo "section 9 heading not found" >&2; exit 3; }
end=$(awk -v s="$start" 'NR>s && /^1[0-9]\. \*\*|^## /{print NR; exit}' "$plan")
[ -n "${end:-}" ] || { echo "section 9 end not found" >&2; exit 3; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
body="$work/body"
awk -v s="$start" -v e="$end" 'NR>=s && NR<e' "$plan" >"$body"

# ---------------------------------------------------------------------
# Part 1: the declared arithmetic.
# ---------------------------------------------------------------------

declared=$(grep -oE '\+[0-9]+ total' "$body" | head -1 | grep -oE '[0-9]+') || true
[ -n "${declared:-}" ] || { echo "declared total not found" >&2; exit 3; }

fanout2=$(grep -oE '[0-9]+ land with Fanout-2' "$body" | head -1 | awk '{print $1}') || true
fanout10=$(grep -oE '[0-9]+ land with Fanout-10' "$body" | head -1 | awk '{print $1}') || true
if [ -z "${fanout2:-}" ] || [ -z "${fanout10:-}" ]; then
  echo "numeric landing split not found" >&2
  exit 3
fi
sum=$(( fanout2 + fanout10 ))
echo "landing split: Fanout-2 $fanout2 + Fanout-10 $fanout10 = $sum"
echo "declared total: $declared"

if [ "$sum" -ne "$declared" ]; then
  echo "ROSTER ARITHMETIC MISMATCH: the landing split sums to $sum, but §9 declares $declared." >&2
  echo "Reconcile the split and total with the named roster — do not invent members." >&2
  exit 1
fi

# ---------------------------------------------------------------------
# Part 2: the enumeration.
#
# A member is a backticked `name()` inside §9. `<target>` is a template
# standing for FOUR concrete tests (axum / cloudflare / spin / fastly),
# per round-146 #3, so it expands to four members rather than one.
#
# Awk, not a grep pipeline, because the answer needs the LINE each name
# sits on: a duplicate is only reportable with the two sites that claim
# it, and a count without sites is the same unactionable output the
# arithmetic check already produced.
# ---------------------------------------------------------------------

TARGETS="axum cloudflare spin fastly"

names="$work/names"
# Fields: name, line number. A name on a continuation line counts: §9
# wraps at 70 columns, so a member's backticks routinely sit on a line
# that does not begin a bullet.
awk '
  {
    line = $0
    while (match(line, /`[a-z][a-z_0-9]*(_<target>)?\(\)`/)) {
      token = substr(line, RSTART + 1, RLENGTH - 2)
      sub(/\(\)$/, "", token)
      print token "\t" FNR
      line = substr(line, RSTART + RLENGTH)
    }
  }
' "$body" >"$names"

# `cancel()` and `drop()` are upstream API calls quoted in a member's
# prose, not members. The list is EXPLICIT rather than a heuristic:
# a heuristic that drops short names would also drop a real member
# named tersely, and this gate's whole purpose is to notice a member
# going missing.
not_a_member="^(cancel|drop|configure|clone)$"

total_names=$(cut -f1 "$names" | grep -vcE "$not_a_member" || true)
[ "${total_names:-0}" -gt 0 ] || { echo "no members enumerated — the extractor matched nothing" >&2; exit 3; }

# One member per name, with the lines that mention it.
sites="$work/sites"
awk -F'\t' -v exclude="$not_a_member" '$1 !~ exclude' "$names" \
  | sort -u -k1,1 -k2,2n > "$sites"

# --- unique ownership ------------------------------------------------
#
# A name may legitimately appear twice: §9 cross-references a fixture
# from the member that shares it ("the same fixture the `x()` uses",
# "stays covered by `x()` in the outbound quartet"). Those are
# REFERENCES. A second DECLARATION is the defect — the same test
# claimed by two roster entries, which inflates the total by one and
# leaves one entry unimplementable.
#
# A declaration is a site whose line, or the line above it, is NOT
# marked as a reference. The marker list is explicit for the same
# reason the non-member list is.
reference_marker='the same fixture|stays covered by|covered by|see the|as corrected|REPLACES|replaces|retired|RETIRED|which specified|enumerates|cites|counterpart|not duplicates|whose Step'

# §9 has two regions that can declare a member, and the distinction is
# what makes a duplicate reportable:
#
#   - the BULLET LIST, from "The base arithmetic" on, where a member is
#     declared with its assertion;
#   - the LABEL -> OWNER INDEX above it, which declares the members
#     whose names live furthest from §9 by naming their owning unit.
#
# A name in BOTH is not a duplicate: the bullet declares it and the
# index points at it. A name twice in the SAME region is the defect.
# Splitting the regions beats a marker-word filter over the index,
# whose whole purpose is to mention members it may not declare.
index_start=$(grep -n 'LABEL . OWNER' "$body" | head -1 | cut -d: -f1) || true
[ -n "${index_start:-}" ] || { echo "LABEL -> OWNER index not found in §9" >&2; exit 3; }
bullets_start=$(awk '/^   The base arithmetic/{print NR; exit}' "$body")
[ -n "${bullets_start:-}" ] || { echo "roster bullet-list boundary 'The base arithmetic' not found" >&2; exit 3; }
[ "$index_start" -lt "$bullets_start" ] || { echo "§9 regions out of order: index at $index_start, bullets at $bullets_start" >&2; exit 3; }

# A site's region: "index" between the two boundaries, "bullets" at or
# after the bullet list, "head" above the index (the group summary,
# which states counts and declares nothing).
region_of() {
  if [ "$1" -ge "$bullets_start" ]; then echo bullets
  elif [ "$1" -ge "$index_start" ]; then echo index
  else echo head
  fi
}

declarations="$work/declarations"
: >"$declarations"
while IFS=$'\t' read -r name lineno; do
  [ -n "$name" ] || continue
  region=$(region_of "$lineno")
  [ "$region" = head ] && continue
  lo=$(( lineno > 1 ? lineno - 1 : 1 ))
  window=$(awk -v lo="$lo" -v hi="$lineno" 'NR>=lo && NR<=hi' "$body" | tr '\n' ' ')
  if printf '%s' "$window" | grep -qE "$reference_marker"; then
    continue
  fi
  printf '%s\t%s\t%s\n' "$name" "$lineno" "$region" >>"$declarations"
done <"$sites"

dupes=$(cut -f1,3 "$declarations" | sort | uniq -d | cut -f1 | sort -u || true)
if [ -n "$dupes" ]; then
  echo "ROSTER DUPLICATE MEMBER: a test is declared twice in the same region of §9." >&2
  echo "A duplicate inflates the total by one and leaves one entry with no test to write." >&2
  while IFS= read -r name; do
    [ -n "$name" ] || continue
    at=$(awk -F'\t' -v n="$name" '$1==n{printf " %s:line %s", $3, $2}' "$declarations")
    echo "  $name declared at:$at" >&2
  done <<EOF
$dupes
EOF
  exit 1
fi

# --- expansion -------------------------------------------------------
#
# Count AFTER the duplicate check, so a duplicated template does not
# quietly add four to the enumeration. A `<target>` template expands to
# FOUR concrete members (round-146 #3).
expanded="$work/expanded"
cut -f1 "$declarations" | sort -u >"$work/distinct-declarations"
while IFS= read -r name; do
  [ -n "$name" ] || continue
  case "$name" in
    *_'<target>')
      stem=${name%_<target>}
      for target in $TARGETS; do
        printf '%s\t%s\n' "${stem}_${target}" "$name"
      done
      ;;
    *) printf '%s\t%s\n' "$name" "$name" ;;
  esac
done <"$work/distinct-declarations" >"$expanded"

expanded_dupes=$(cut -f1 "$expanded" | sort | uniq -d)
if [ -n "$expanded_dupes" ]; then
  echo "ROSTER DUPLICATE MEMBER: distinct declarations own the same expanded test." >&2
  while IFS= read -r name; do
    awk -F'\t' -v n="$name" '$1==n{printf "  %s owned by %s\n", $1, $2}' "$expanded" >&2
  done <<EOF
$expanded_dupes
EOF
  exit 1
fi
members="$work/members"
cut -f1 "$expanded" | sort >"$members"

enumerated=$(wc -l <"$members" | tr -d ' ')
declared_names=$(cut -f1 "$declarations" | sort -u | wc -l | tr -d ' ')
templates=$(cut -f1 "$declarations" | sort -u | grep -c '<target>' || true)
echo "enumerated members: $enumerated ($declared_names distinct declarations, $templates per-target templates expanded x4)"

# --- missing members -------------------------------------------------
#
# §9's own rule (round-209): "a group label may appear here ONLY if the
# same line names the unit that owns it", and every label is
# "DISCHARGED BY NAMED TESTS IN ITS OWNING UNIT'S BODY".
#
# So each label carrying a COUNT owes that many members. A label is
# discharged either by the LABEL -> OWNER index naming its owning unit,
# or by at least that many named members whose names carry the label's
# own stem. A label with a count and neither is the "claimed by none"
# case the acceptance contract fails on, and it is exactly how a
# missing member hides: the count stays in the sum while no test exists.
#
# The label stems are listed rather than parsed out of prose. §9 wraps
# mid-phrase, so a prose scan reads "3 incomplete" and "503 precedence"
# as labels; the authoritative list is short and changing it is a
# deliberate edit.
#
# Format: label-as-written<TAB>expected-count<TAB>name-stem-regex
label_table="$work/labels"
cat >"$label_table" <<'LABELS'
weighted-gate	7	^stream_gate_(zero_wave|wave_exceeding|weighted_grant|mixed_size|cancel_before|cancel_after|invariant_free)
elapsed-timing	3	elapsed
time-domain	3	(clock|instant|deadline_composition|fake_clock)
per-target wave-wait	4	^wave_wait_does_not_expire_before_deadline_
per-adapter redirect-destination	4	^redirect_destination_never_contacted_
per-target profile-vs-capability-table	4	^outbound_profile_matches_capability_table_
inbound parser	6	^inbound_h[12]_
node-counting	2	^bounded_json_counts_
stream-gate wake	2	^stream_gate_(notifies_only|wakes_outside)
batch-contract	3	^(batch_item_index|completed_is_observed|no_explicit_cancel)
LABELS

owner_index_start=$(grep -n 'LABEL . OWNER' "$body" | head -1 | cut -d: -f1) || true

missing=""
while IFS=$'\t' read -r label expected stem; do
  [ -n "$label" ] || continue
  # Does §9 state this label with a count at all?
  if ! grep -qE "[0-9]+ ${label}" "$body"; then
    continue
  fi
  owned=0
  if [ -n "${owner_index_start:-}" ] \
     && awk -v s="$owner_index_start" 'NR>=s' "$body" | grep -qE "\`[0-9]* ?${label}\`"; then
    owned=1
  fi
  found=$(grep -cE "$stem" "$members" || true)
  # Ownership excuses a label whose members §9 never enumerates
  # individually — but only then. Once the stem matches ANY member, §9
  # is enumerating this group, so a short count is a member that went
  # missing and ownership cannot stand in for it. Gating the shortfall
  # on `owned` alone is what let a deleted member pass: its group label
  # was still owned, so the count dropping from 7 to 6 was forgiven.
  if [ "${found:-0}" -gt 0 ] && [ "${found:-0}" -lt "$expected" ]; then
    missing="${missing}  '${label}' claims ${expected} members; §9 names only ${found}\n"
  elif [ "$owned" -eq 0 ] && [ "${found:-0}" -lt "$expected" ]; then
    missing="${missing}  '${label}' claims ${expected} members; §9 names ${found} and the LABEL -> OWNER index does not own it\n"
  fi
done <"$label_table"

if [ -n "$missing" ]; then
  echo "ROSTER MISSING MEMBER: a group label stands in for members that are neither named nor owned." >&2
  printf '%b' "$missing" >&2
  echo "Name the missing members, or add the label to the LABEL -> OWNER index with its owning unit." >&2
  echo "Reconcile the named roster explicitly; do not silently change a group count." >&2
  exit 1
fi

# The historical label-index guard: a counted label with no index at
# all is the shape that made the roster read as "~95 unnamed".
if grep -qE '^   [0-9]+ (weighted-gate|elapsed-timing|time-domain)' "$body" \
   && [ -z "${owner_index_start:-}" ]; then
  echo "group labels present with no label-to-owner index" >&2
  exit 1
fi

if [ "$enumerated" -ne "$declared" ]; then
  echo "ROSTER ENUMERATION MISMATCH: §9 names $enumerated expanded members, but declares $declared." >&2
  echo "Reconcile the actual named roster; an owner label cannot replace a member." >&2
  exit 1
fi

echo "roster arithmetic consistent; $enumerated members enumerated, uniquely owned, every counted label discharged"
