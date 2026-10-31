#!/usr/bin/env bash
# Negative fixtures for `scripts/check-raw-json-fixtures.sh`.
#
# A gate that has only ever been run against a tree it passes proves
# nothing: it could be returning 0 unconditionally. Each case below
# plants one specific defect in a throwaway copy of the tree and
# requires the gate to REJECT it with the matching verdict, then
# requires the unplanted copy to pass.
#
# The copies are DERIVED from the real sources by a planting edit, not
# checked in. A checked-in copy would drift from the real allowlist the
# first time a fixture converted, and a gate tested against a stale
# fixture is exactly the defect this file rules out.
#
# Exit 0 = every case behaved, 1 = a case did not, >1 = the runner
# itself failed.
set -euo pipefail

here=$(cd -- "$(dirname -- "$0")" && pwd)
repo=$(cd -- "$here/.." && pwd)
gate="$here/check-raw-json-fixtures.sh"
allow="$here/raw-json-fixtures.allow"

[ -x "$gate" ] || { echo "gate not executable: $gate" >&2; exit 3; }
[ -r "$allow" ] || { echo "cannot read allowlist: $allow" >&2; exit 3; }

work=$(mktemp -d) || { echo "mktemp failed" >&2; exit 3; }
trap 'rm -rf "$work"' EXIT

failures=0

# Runs the gate over a planted tree and checks the verdict.
#   expect_reject <label> <grep pattern> <tree root> <allowlist>
expect_reject() {
  local label=$1 want=$2 root=$3 list=$4 out rc
  out=$("$gate" "$list" "$root" 2>&1) && rc=0 || rc=$?
  if [ "$rc" -ne 1 ]; then
    echo "FAIL $label: expected exit 1, got $rc"
    printf '%s\n' "$out" | sed 's/^/      /'
    failures=$((failures + 1))
    return
  fi
  if ! printf '%s\n' "$out" | grep -q -- "$want"; then
    echo "FAIL $label: exit 1 but no verdict matching '$want'"
    printf '%s\n' "$out" | sed 's/^/      /'
    failures=$((failures + 1))
    return
  fi
  echo "ok   $label"
}

expect_pass() {
  local label=$1 root=$2 list=$3 out rc
  out=$("$gate" "$list" "$root" 2>&1) && rc=0 || rc=$?
  if [ "$rc" -ne 0 ]; then
    echo "FAIL $label: expected exit 0, got $rc"
    printf '%s\n' "$out" | sed 's/^/      /'
    failures=$((failures + 1))
    return
  fi
  echo "ok   $label"
}

# A minimal tree standing in for `crates/`: one file on the allowlist
# with a budget of 1, one file on it with a budget of 0 rows (absent).
build_tree() {
  local dest=$1
  mkdir -p "$dest/crates/demo/tests"
  cat >"$dest/crates/demo/tests/budgeted.rs" <<'RS'
// Raw byte-string fixture: this is a wire body, a `&[u8]`.
const BODY: &[u8] = br#"{"id":"r"}"#;
RS
  cat >"$dest/crates/demo/tests/clean.rs" <<'RS'
let v = serde_json::json!({"id": "r"});
RS
  printf 'crates/demo/tests/budgeted.rs 1\n' >"$dest/allow"
}

# -- Case 0: the unplanted tree passes. -------------------------------
# Without this the rejections below prove only that the gate rejects
# everything, which is as useless as a gate that accepts everything.
base="$work/base"; build_tree "$base"
expect_pass "unplanted tree passes" "$base/crates" "$base/allow"

# -- Case 1: a new raw fixture in a file that is not listed. ----------
c1="$work/new-file"; build_tree "$c1"
cat >>"$c1/crates/demo/tests/clean.rs" <<'RS'
let req = r#"{"id":"r","imp":[]}"#;
RS
expect_reject "raw fixture in an unlisted file" \
  "OVER  crates/demo/tests/clean.rs" "$c1/crates" "$c1/allow"

# -- Case 2: a new raw fixture in a file that IS listed. --------------
# The case a path-only allowlist would wave through, and the reason the
# allowlist carries counts.
c2="$work/over-budget"; build_tree "$c2"
cat >>"$c2/crates/demo/tests/budgeted.rs" <<'RS'
const EXTRA: &[u8] = br#"{"id":"sneaked-in"}"#;
RS
expect_reject "extra raw fixture inside an allowlisted file" \
  "OVER  crates/demo/tests/budgeted.rs: 2 raw JSON fixture(s), budget 1" \
  "$c2/crates" "$c2/allow"

# -- Case 3: a budget with slack. -------------------------------------
# Slack is how an allowlist stops gating: it re-admits the next raw
# fixture silently.
c3="$work/slack"; build_tree "$c3"
printf 'crates/demo/tests/budgeted.rs 4\n' >"$c3/allow"
expect_reject "budget larger than the real count" \
  "STALE crates/demo/tests/budgeted.rs" "$c3/crates" "$c3/allow"

# -- Case 4: a listed file that no longer holds any. ------------------
c4="$work/dead-row"; build_tree "$c4"
printf 'crates/demo/tests/budgeted.rs 1\ncrates/demo/tests/clean.rs 1\n' >"$c4/allow"
expect_reject "allowlist row for a file with no raw fixtures" \
  "STALE crates/demo/tests/clean.rs" "$c4/crates" "$c4/allow"

# -- Case 5: a malformed allowlist row must not read as a verdict. ----
# A row that parsed as a zero budget would quietly tighten the gate on
# an unrelated file, so this is exit 3, not exit 1.
c5="$work/malformed"; build_tree "$c5"
printf 'crates/demo/tests/budgeted.rs notanumber\n' >"$c5/allow"
out=$("$gate" "$c5/allow" "$c5/crates" 2>&1) && rc=0 || rc=$?
if [ "${rc:-0}" -ne 3 ]; then
  echo "FAIL malformed allowlist row: expected exit 3, got ${rc:-0}"
  printf '%s\n' "$out" | sed 's/^/      /'
  failures=$((failures + 1))
else
  echo "ok   malformed allowlist row is a script failure, not a verdict"
fi

# -- Case 6: the gate matches objects, not every raw string. ----------
# A raw string holding a URL or a JSON fragment is not a fixture and
# must not be charged against a budget.
c6="$work/not-an-object"; build_tree "$c6"
cat >>"$c6/crates/demo/tests/clean.rs" <<'RS'
let url = r#"https://bidder.example/rtb?x={y}"#;
let fragment = r#""gcl": { "path": "gcl.json" }"#;
RS
expect_pass "raw strings that are not JSON objects are not charged" \
  "$c6/crates" "$c6/allow"

# -- Case 7: an ordinary string ending in r is not a raw opener. -----
c7="$work/ordinary-string"; build_tree "$c7"
cat >>"$c7/crates/demo/tests/clean.rs" <<'RS'
if object == "User" {
    consume(object);
}
if object == "r" {
    consume(object);
}
if object == "some r" {
    consume(object);
}
if object == "some \"r\" {" {
    consume(object);
}
RS
expect_pass "ordinary string suffixes are not raw JSON fixtures" \
  "$c7/crates" "$c7/allow"

# -- Case 8: raw strings without hashes still count. -----------------
c8="$work/unhashed-raw"; build_tree "$c8"
cat >>"$c8/crates/demo/tests/clean.rs" <<'RS'
let req = r"{ }";
RS
expect_reject "unhashed raw object in an unlisted file" \
  "OVER  crates/demo/tests/clean.rs" "$c8/crates" "$c8/allow"

# -- Case 9: raw C-string objects keep their existing coverage. -------
c9="$work/raw-c-string"; build_tree "$c9"
cat >>"$c9/crates/demo/tests/clean.rs" <<'RS'
let req = cr#"{ }"#;
RS
expect_reject "raw C-string object in an unlisted file" \
  "OVER  crates/demo/tests/clean.rs" "$c9/crates" "$c9/allow"

# -- Case 10: quoted tokens before a raw literal cannot hide it. ------
c10="$work/quoted-prefix"; build_tree "$c10"
cat >>"$c10/crates/demo/tests/clean.rs" <<'RS'
let name = "r"; let req = r#"{ }"#;
let quote = '"'; let req = r#"{ }"#;
RS
expect_reject "raw objects after ordinary strings or quote characters" \
  "OVER  crates/demo/tests/clean.rs: 2 raw JSON fixture(s)" \
  "$c10/crates" "$c10/allow"

# -- Case 11: owner-local budgets are read from the tested tree. -----
c11="$work/owner-local"; build_tree "$c11"
mkdir -p "$c11/crates/auctioneer-bidder/tests/conformance"
mv "$c11/allow" "$c11/crates/auctioneer-bidder/tests/conformance/raw-json-fixtures.allow"
touch "$c11/allow"
expect_pass "owner-local budget admits its exact fixture count" "$c11/crates" "$c11/allow"
printf '%s\n' 'const EXTRA: &[u8] = br#"{"id":"extra"}"#;' >>"$c11/crates/demo/tests/budgeted.rs"
expect_reject "owner-local budget rejects an extra raw fixture" \
  "OVER  crates/demo/tests/budgeted.rs: 2 raw JSON fixture(s), budget 1" \
  "$c11/crates" "$c11/allow"
printf 'crates/demo/tests/budgeted.rs 3\n' >"$c11/crates/auctioneer-bidder/tests/conformance/raw-json-fixtures.allow"
expect_reject "owner-local budget rejects slack" \
  "STALE crates/demo/tests/budgeted.rs" "$c11/crates" "$c11/allow"

# -- Case 12: the real tree passes its real allowlist. ----------------
expect_pass "the repository matches its own allowlist" "$repo/crates" "$allow"

if [ "$failures" -ne 0 ]; then
  echo "$failures case(s) did not behave"
  exit 1
fi
echo "all cases behaved"
