# bounded fanout Review Fixes

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Repair the six findings in `2026-10-06-production-fanout-implementation-review.md` in
the existing `bounded-fanout` worktree. Preserve the SDK-owned transport paths,
the resource caps, and the current EdgeZero pin. New tests use behavior
names and live beside their implementation unless they exercise an
integration boundary.

## Decisions

- Returned bids must belong to the bidder's participating impressions and,
  when present, the originating call's `covered_imp_ids`. Empty call metadata
  falls back to bidder participation, not the entire auction or reject-all.
  Reject uncorrelated bids before winner selection using bounded diagnostics.
- Keep accepted aliases named `prebid` valid. Merge bidder and auction-wide
  diagnostic arrays at the shared wire key in deterministic order instead
  of introducing a new alias restriction.
- Targeting prefixes reserve `MIN_KEY_LENGTH` bytes within the existing
  20-byte hard cap. Bidder override map keys obey the existing 64-byte limit
  before value validation or diagnostic formatting.

## Checklist

- [x] R1: Reproduce and fix foreign/unknown/cross-call impression admission;
      cover aliases and empty correlation metadata.
- [x] R2: Reproduce and fix prefix fallback boundaries and UTF-8 byte limits.
- [x] R3: Reproduce and fix enabled-only discovery for shadowing aliases;
      preserve base-only behavior.
- [x] R4: Reproduce and fix alias/global error and warning preservation.
- [x] R5: Reproduce and fix runtime bidder override key validation before
      disabled-set construction and unbounded error formatting.
- [x] R6: Add peak allocation coverage through production config resolution,
      including extraction, integrity handling, and typed conversion.
- [x] Review the integrated patch and update governing documentation and
      finding dispositions with executed evidence.
- [x] Run native tests, strict Clippy, config/conformance tests and provider
      checks; commit and push the verified bounded fanout fix.
- [x] Carry the fix through the downstream capability stack and verify it.

## Verification

Behavioral fixes require failing regression tests before production edits.
The allocation coverage gap requires executing the real resolver and
measuring peak live overlap against its production-funded terms, not treating
cumulative allocation as peak. Record measured values and any unavailable
checks here before completion.

### Returned-bid Admission

`cargo test -q -p auctioneer-exchange --test bid_impression_correlation --locked
--offline` reproduced all six original admission regressions before the fix.
After the fix, seven integration tests pass, including separate alias/base
participation when both use empty call metadata. Diagnostics are fixed-size,
once per affected call, and reuse the retained split-group membership.

### Follow-up Review

The independent review found that the custom-validator derive captures the
whole rejected bidder map in error parameters even when its display message
is bounded. Manual root validation removes that copy, and the same root
account/alias copies, while preserving field keys, codes, nested validation,
Serde shape and `ValidateArgs`. New regressions failed before the repairs
and pass afterward. The focused configuration suite passes 39 tests.

The R2 follow-up reproduced escaped-control-byte expansion and now bounds
the quoted prefix after escaping as well as before. All 15 key unit tests
pass, including the total diagnostic byte bound.

### Production Config Allocation

The real SDK raw-store/runtime resolver guard passes all 14 tests. Five
valid adversarial fixtures and nine rejection probes fit the combined
production-funded config terms; neither the factor 6 nor production caps
were increased to pass the guard.

| Valid fixture                | Peak live bytes | Funded bytes |
| ---------------------------- | --------------: | -----------: |
| Minimal                      |             905 |        6,403 |
| Node-bounded empty accounts  |       3,512,365 |    6,967,648 |
| Account overlays and aliases |       1,931,886 |    5,520,167 |
| Dense envelope metadata      |       5,139,984 |    6,084,064 |
| Escaped publisher strings    |         594,628 |    7,561,310 |

Measurements cover native same-thread logical heap, backend request-time
raw-string cloning, extraction, integrity/typed conversion and retained
host/account clones. They exclude startup/provider-resident allocations,
allocator overhead and internal physical realloc overlap. These finite
fixtures validate combined funding, not every input or each factor alone.

### Final Verification Status

- Implementation commit `6dbbb00` is pushed on `bounded-fanout` and carried
  through privacy enforcement `54104f1`, user sync `33ce7d6` and diagnostic qualification `8508448`. Remote heads and
  sequential ancestry were verified; main was not changed. Later commits
  synchronize this completed verification record only.
- privacy enforcement integration: 421 focused tests and strict scoped Clippy passed.
  user sync integration: 475 focused merge tests, 60 user-sync integration tests,
  strict workspace Clippy and all three WASM checks passed. diagnostic qualification code is
  identical to verified user sync, with a fresh workspace all-target compile
  check and repository gates passing. Descendant records below describe
  their capability-specific merge decisions where present.
- Strict workspace Clippy (`--all-targets`, `-D warnings`): passed.
- Allocation guard: 14 passed; targeted core/catalog, exchange admission,
  diagnostic and targeting regressions passed.
- Formatting, diff whitespace, batch-rebase, roster and raw-JSON gates:
  passed, including the negative fixture suites.
- The full default native workspace/all-target suite passed, excluding
  `auctioneer-config`. Final focused config tests pass (72 tests), as does the
  final production allocation guard (14 tests); these cover the last
  root-map no-capture and rejection-peak additions after the broad run
  started.
- The duplicate full insecure workspace run was intentionally stopped
  during `health_gvl`, without a test assertion failure. It is not counted
  as a passing full suite. The replacement focused insecure-feature run
  passed all 374 tests across both changed libraries, config allocation,
  handler resolution, bid correlation, alias diagnostics and targeting.
- Provider fanout conformance: 111 tests passed across core and the three
  adapters. Cloudflare/Fastly/Spin WASM checks passed.
- The expensive `auctioneer-config` subprocess build-failure harness is not
  part of this repair's rerun. Its focused non-subprocess suite passed.
- Local tests and WASM builds do not upgrade hosted qualification or
  establish release performance. The debug all-target GVL cold-parse probe
  printed p95 38.2 ms against its 10 ms target; a release performance rerun
  is separate from these functional repairs. No thresholds were changed.

### privacy enforcement Integration

The merge preserves the activity/privacy projection pipeline, sealed
outbound transport and fixed non-USD diagnostic. Admission uses the already
retained bidder-scoped request rather than restoring bounded fanout's removed split
group API or allocating new membership storage. Catalog tests use privacy enforcement's
split API with a valid inventory fixture. The metadata test stays within
the scoped impression-count cap to reach response admission; a separate
case asserts that exceeding the cap is rejected by the outbound builder
before dispatch. The existing activity-validation allocation guard remains.

The scoped core/exchange unit suites, config allocation and handler
resolution guards, bid-correlation, alias diagnostic and prefix tests are
the merge verification set: 421 tests pass. Strict scoped all-target Clippy,
formatting and diff-whitespace checks pass.

### user sync Integration

The merge retains the renamed OpenRTB master spec and user sync's cookie/base64
dependencies. Manual root validation includes the added `host_cookie` and
`user_sync` nested validators; `validate_effective` still performs user sync's
merged cookie, user-sync and account namespace validation. No user-sync
route, cookie contract or registry behavior is replaced by this repair.

Strict workspace all-target Clippy and the Cloudflare/Fastly/Spin WASM
checks pass. The focused core/exchange merge tests and user-sync config,
lifecycle, precedence, catalog, routes and resources suites are the native
verification set for this descendant integration.
All 475 focused merge tests and 60 user-sync integration tests pass, as do
formatting, diff-whitespace, roster, batch-rebase and raw-JSON gates.

### diagnostic qualification Integration

The user sync merge is conflict-free. Code, manifests, workflows and scripts are
identical to the verified user sync tree; diagnostic qualification retains its own specification and
plan changes. A fresh workspace all-target compile check, formatting,
diff-whitespace, roster, batch-rebase and raw-JSON gates pass. Native and
WASM regression evidence for the identical source tree is recorded above
under user sync, not represented as a separate full diagnostic qualification test rerun.
