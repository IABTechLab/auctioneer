#!/usr/bin/env bash
#
# Measure the native (axum) target's run-A `baseline_bytes`.
#
# WHAT THIS MEASURES, stated rather than left to inference: the operating
# system's resident-set size (RSS) of a process that has completed
# auctioneer's production boot sequence and made the embedded GVL archive
# resident, and has then gone quiescent. RSS, in KiB, as `ps -o rss=`
# reports it, converted to bytes here.
#
# WHY RSS AND NOT AN ALLOCATOR TOTAL. The quantity the measurement
# contract names is whole-isolate resident memory, and two properties of
# this target make an allocator the wrong instrument for it:
#
#   - The embedded GVL archive is `include_bytes!` rodata in the
#     executable image. It never passes through the allocator, so an
#     allocator-based figure omits it entirely -- and the archive is the
#     single largest identifiable component of this baseline.
#   - A `#[global_allocator]` observes the whole process and accumulates,
#     so its reading is a property of everything the measuring process
#     did, not of auctioneer's boot. Making it trustworthy would require the
#     measuring process to do nothing else, which is exactly the
#     constraint RSS-from-outside removes.
#
# RSS has its own well-known failure mode: it tracks page-cache residency
# of lazily-faulted mapped pages, so an unwarmed run can read
# dramatically lower than a warm one purely because pages had not been
# faulted. The probe's touch pass is the fix, not a detail -- it
# dereferences one byte per 4 KiB of every archive blob and of the baked
# GCL, which forces those pages resident and makes the reading a property
# of the boot rather than of the cache. Run-to-run spread, printed below,
# is the evidence that it worked.
#
# RUN A ONLY. This boots and serves nothing. The per-auction factors are
# validated by a separate run-B exercise whose figure is never added to
# this one. Driving concurrency until OOM is a third, separate exercise
# that checks the formula's prediction and is likewise never the baseline
# source.
#
# THE MARGIN. `baseline_bytes = ceil(max_observed * 6/5)`, computed as
# the checked integer expression `(max * 6 + 4) / 5`. The 20% margin
# lives inside the committed constant and nowhere else:
# `validate_isolate_limits` consumes `baseline_bytes` as is and derives
# nothing further from it. One margin, one rule.
#
# Usage:
#   scripts/measure-baseline.sh [reps]
#
# `reps` defaults to the contract's 30. The script prints every
# observation, then the max, then the margin arithmetic.

set -euo pipefail

REPS="${1:-30}"
PROBE_BIN="auctioneer-baseline-probe"
PROBE_PKG="auctioneer-adapter-axum"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Release, because the committed constant describes a shipped binary. A
# debug build's RSS is a different program's RSS.
echo "building ${PROBE_BIN} (release)..." >&2
cargo build --release --package "$PROBE_PKG" --bin "$PROBE_BIN" >&2

probe_path="$(cargo metadata --format-version 1 --no-deps \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')/release/${PROBE_BIN}"

if [[ ! -x "$probe_path" ]]; then
  echo "probe binary not found at ${probe_path}" >&2
  exit 1
fi

echo "probe: ${probe_path}" >&2
echo "metric: process RSS (KiB from ps, reported in bytes) after boot + archive touch pass" >&2
echo "reps:   ${REPS}" >&2
echo >&2

# One observation: start the probe, wait for it to announce readiness,
# read its RSS, then terminate it. Reading only after the readiness line
# is what keeps this from racing boot -- at that point the probe has
# finished booting and touching, and is sleeping, so `ps` sees a settled
# process rather than one still allocating.
observe_once() {
  local work out err pid rss archive_bytes waited

  work="$(mktemp -d)"
  out="${work}/out"
  err="${work}/err"

  "$probe_path" >"$out" 2>"$err" &
  pid=$!

  # Bounded, so a broken probe fails the run instead of hanging it.
  waited=0
  while ! grep -q 'auctioneer-baseline-probe ready' "$out" 2>/dev/null; do
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "probe exited before signalling readiness:" >&2
      cat "$err" >&2
      rm -rf "$work"
      return 1
    fi
    if [ "$waited" -gt 600 ]; then
      echo "probe did not signal readiness within 60s" >&2
      kill "$pid" 2>/dev/null || true
      rm -rf "$work"
      return 1
    fi
    perl -e 'select(undef,undef,undef,0.1)'
    waited=$((waited + 1))
  done

  # ps reports KiB.
  rss="$(ps -o rss= -p "$pid" | tr -d ' ')"
  archive_bytes="$(sed -n 's/.*archive_bytes=\([0-9]*\).*/\1/p' "$out" | head -1)"

  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  rm -rf "$work"

  if [ -z "$rss" ]; then
    echo "could not read RSS for pid ${pid}" >&2
    return 1
  fi

  echo "$((rss * 1024)) ${archive_bytes}"
}

max=0
declare -a observations=()
archive_reported=""

for ((i = 1; i <= REPS; i++)); do
  # `observe_once` in its own command substitution loses `set -e`, so its
  # exit status is checked explicitly: a failed observation must fail the
  # whole run rather than silently shrink N.
  if ! line="$(observe_once)"; then
    echo "observation ${i} failed; aborting rather than reporting a short run" >&2
    exit 1
  fi
  read -r bytes archive <<<"$line"
  if [[ -z "$bytes" ]]; then
    echo "observation ${i} produced no RSS reading" >&2
    exit 1
  fi
  observations+=("$bytes")
  if [[ -z "$archive_reported" ]]; then
    archive_reported="$archive"
  elif [[ "$archive_reported" != "$archive" ]]; then
    echo "WARNING: archive size changed between runs (${archive_reported} -> ${archive})" >&2
  fi
  if (( bytes > max )); then
    max=$bytes
  fi
  printf 'observation %2d: %12d bytes (%6d KiB)\n' "$i" "$bytes" "$((bytes / 1024))"
done

echo
echo "archive bytes made resident by the touch pass: ${archive_reported}"
echo

# Spread, so an outlier is visible rather than hidden behind the max.
min=${observations[0]}
sum=0
for value in "${observations[@]}"; do
  (( value < min )) && min=$value
  sum=$((sum + value))
done
mean=$((sum / ${#observations[@]}))

printf 'n    = %d\n' "${#observations[@]}"
printf 'min  = %d bytes\n' "$min"
printf 'mean = %d bytes\n' "$mean"
printf 'max  = %d bytes   <- the contract takes the MAX, not the mean\n' "$max"
echo

# ceil(max * 6/5) as the checked integer expression (max * 6 + 4) / 5.
margined=$(( (max * 6 + 4) / 5 ))
echo "margin arithmetic (the 20% lives here and nowhere else):"
printf '  baseline_bytes = ceil(max * 6/5) = (%d * 6 + 4) / 5 = %d\n' "$max" "$margined"
echo
echo "commit this as BASELINE_BYTES in crates/auctioneer-adapter-axum/src/envelope.rs"
