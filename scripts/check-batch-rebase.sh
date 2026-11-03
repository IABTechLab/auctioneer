#!/usr/bin/env bash
# Fanout-0 gate: no document may normatively state the REMOVED upstream
# batch contract. EdgeZero hard-removed `OutboundHttpClient::send_all`;
# auctioneer's own `BidderFanout::send_all` is a different, surviving method,
# which is why this allowlists by type path rather than by word.
#
# Patterns are assembled from split literals so this file cannot match
# itself, and neither can prose quoting it.
#
# Exit 0 = clean, 1 = stale references, >1 = the scan itself failed.
set -euo pipefail

plan=${1:-docs/superpowers/plans/2026-07-10-multi-adapter-production-fanout.md}
spec=docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md
ledger=docs/superpowers/plans/2026-09-06-edgezero-outbound-spec-feedback.md
roots=("$plan" "$spec" "$ledger")
for f in "${roots[@]}"; do
  [ -r "$f" ] || { echo "cannot read $f" >&2; exit 3; }
done

# Match the BARE token with an identifier boundary: digits included, so
# `send_all2` is a different name and `send_all_until` (the CURRENT API)
# never matches at all.
occurrence="[A-Za-z_:.]*send""_all([^_A-Za-z0-9]|$)"
# auctioneer's OWN exchange-facing trait method and its impl types, plus the
# field-call form (`ctx.fanout.send_all(..)`) the auction core plan uses.
#
# `ProductionFanout` is the former name of `EdgeZeroFanout` and is kept
# here deliberately: the plan documents under `docs/superpowers/plans/`
# are a historical record and were not renamed, so dropping the old name
# makes this gate fail on them. Both stay until those documents do.
allow_own="(BidderFanout|EdgeZeroFanout|ProductionFanout|DevFanout|NoOutboundCapabilityFanout)::send""_all([^_A-Za-z0-9]|$)|fanout\.send""_all([^_A-Za-z0-9]|$)"
retired_names="send-all-""slot-isolation|(BudgetSource::)?\bBatch""Deadline\b"
obsolete_collect="Result<Outbound""BatchResults, Edge""Error>"
slot_invalidating="no per-slot outcome is trust""worthy|driver fail""ure.*invalidat"

# The scan runs ALONE and its status is checked before any filtering:
# piping it into the allowlist is what once let an exit-2 read as clean.
raw=$(mktemp)
set +e
grep -rEon -e "$occurrence" -e "$retired_names" \
           -e "$obsolete_collect" -e "$slot_invalidating" \
           "${roots[@]}" >"$raw" 2>/dev/null
scan_status=$?
set -e
if [ "$scan_status" -gt 1 ]; then
  echo "scanner failed (exit $scan_status)" >&2; exit "$scan_status"
fi

# The ledger's per-gap entries from '### G1 ' on are frozen history that
# Fanout-0 may not rewrite; its current-head summary ABOVE that line is in
# scope. A path exclusion cannot express this, because the boundary is a
# section rather than a file.
b=""
if ! b=$(grep -n '^### G1 ' "$ledger" | head -1 | cut -d: -f1) || [ -z "$b" ]; then
  echo "ledger boundary marker '### G1 ' not found in $ledger" >&2
  exit 3
fi
# The plan's "EdgeZero prerequisites" section records the PINNED state —
# what auctioneer compiles against TODAY, where send_all still exists — in a
# deliberate two-column pinned/head form. Rewriting it would falsify that
# record, so it is excluded by line range for the same reason the
# ledger's per-gap entries are: the boundary is a section, not a file.
pb=""
if ! pb=$(grep -n '^## EdgeZero prerequisites' "$plan" | head -1 | cut -d: -f1) \
   || [ -z "$pb" ]; then
  echo "plan section marker '## EdgeZero prerequisites' not found" >&2
  exit 3
fi
pe=$(awk -v s="$pb" 'NR>s && /^## /{print NR; exit}' "$plan")
[ -n "${pe:-}" ] || { echo "plan prerequisites section end not found" >&2; exit 3; }

kept=$(mktemp)
awk -F: -v L="$ledger" -v b="$b" -v P="$plan" -v pb="$pb" -v pe="$pe" \
    '!($1==L && $2>=b) && !($1==P && $2>=pb && $2<pe)' "$raw" >"$kept"

# Filter OCCURRENCES, not lines: a line carrying both an allowed and a
# forbidden call must still fail.
hits=$(grep -vE "$allow_own" "$kept" || true)

# A line that STATES the removal is the rebase's own output, not a
# violation — the task exists to produce those sentences. Matched on the
# source line rather than the occurrence, since the marker words sit
# outside the captured token. Same for the bare `async fn send_all(`
# declaration of auctioneer's OWN BidderFanout trait.
if [ -n "$hits" ]; then
  keep=""
  while IFS= read -r hit; do
    [ -z "$hit" ] && continue
    file=${hit%%:*}; rest=${hit#*:}; lineno=${rest%%:*}
    # Read a small WINDOW, not one line: these documents wrap, so the
    # sentence that marks a mention as historical ("is RETIRED",
    # "hard-removes", a table's RENAME cell) routinely sits on the line
    # above or below the matched token. A line-scoped check misses every
    # continuation line and reports the migration's own prose as a
    # violation.
    lo=$(( lineno > 2 ? lineno - 2 : 1 ))
    src=$(sed -n "${lo},$((lineno + 1))p" "$file" | tr '\n' ' ')
    case "$src" in
      *HARD-REMOVED*|*'is REMOVED'*|*'at the pinned EdgeZero'*|*'async fn send'*'_all('*) continue ;;
      # The migration describing ITSELF is the task's output, not a
      # violation: the P0 banner, Fanout-0's own body, this gate's
      # comments, the capability-rename rows, and the DAG cells that
      # say Fanout-11 is blocked BECAUSE the head removed the method.
      *'hard-remove'*|*RETIRED*|*retired*|*RENAME*|*replaces*|*'CHANGED AT HEAD'*) continue ;;
      *'may be implemented from'*|*'capability-name migration'*|*'migrated from'*) continue ;;
      *'states the '*' contract normatively'*|*'Anchored with'*|*'so `BidderFanout'*) continue ;;
      *'still targets'*|*'reconciles this'*|*'replace the `http.'*) continue ;;
      # An EXPLICIT, visible exemption. Some mentions are correct and
      # must survive the rebase: a PIN-SCOPED record of what
      # 120dd2aa published, this gate's own pattern list, and the
      # blocked-reason cells that say WHY Fanout-11 waits. Marking them
      # at the site beats widening heuristics until the gate stops
      # catching real regressions.
      *'batch-rebase-ok'*) continue ;;
    esac
    keep="${keep}${hit}\n"
  done <<EOF
$hits
EOF
  hits=$(printf '%b' "$keep" | sed '/^$/d')
fi
if [ -n "$hits" ]; then
  printf 'stale batch-API references:\n%s\n' "$hits" >&2
  exit 1
fi
echo "batch-API rebase clean"
