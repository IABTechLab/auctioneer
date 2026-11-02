# Schema Matcher Comparison Plan

> **For agentic workers:** Use superpowers:subagent-driven-development and test-driven-development. Serialize Cargo in the existing M7 worktree.

**Goal:** Obtain reproducible native comparison evidence for the current meta matcher and the locked library's direct PikeVM, without selecting a production engine.

**Architecture:** Extend the existing source-conformance target. Discover actual emitted helpers from all pinned schemas using the existing compiler/AST reader, then compare the two public library APIs. Reuse the existing test-only allocation meter; keep ordinary correctness controls separate from opt-in timing output.

**Tech Stack:** Rust 1.98.1, locked `regex-automata 0.4.18`, existing schema compiler, Serde JSON, source inventory and parameter captures. No crate, dependency, feature, production compiler patch or runtime change.

## Approved Scope

AG approved this qualification-only comparison on 2026-10-27. It does not amend
the approved explicit-cache serving contract or qualify either engine.
Schemas, translations, UTF-8 semantics, LeftmostFirst matching, the 65,536-byte
NFA limit, current meta strategies and its 65,536-byte hybrid-cache setting stay
unchanged. Direct PikeVM uses the same forward NFA configuration and boolean
`is_match`; retain captures in its compiled program rather than silently adding
a capture-elimination optimization. This candidate has no meta prefilter.

## Task 1: Reuse The Existing Meter

**Files:** `tests/support/schema_allocation.rs`, new sibling `schema_meter.rs`,
and `tests/conformance/schema.rs`, all under `crates/auctioneer-bidder`.

- [x] Extract the existing TLS/System allocator and Metrics/measure API without
  changing their arithmetic, realloc accounting, panic cleanup or field meanings.
  The original allocation helper imports that module locally so generated probe
  source and native-only module ownership remain unchanged.
- [x] Add inline controls for literal allocation/reallocation/release, metric
  composition, panic cleanup and other-thread exclusion. Observe a semantic red
  before accepting the shared meter. Keep one allocator per executable.

## Task 2: Compare Actual Patterns

**Files:** new `tests/support/schema_matchers.rs`; existing source target above.

- [x] Discover all 261 schemas and exactly 23 emitted helpers, preserving each
  schema/helper identity; add the separate hostname pattern. Reconcile the helper
  set with existing adjacent declarations, not a new handwritten bidder roster.
- [x] Gather strings and object keys from all 300 captured parameter inputs.
  These are deduplicated candidate subjects, not observed helper invocations.
  Record parse failures, deduplication and timing-subset selection; run agreement
  on the entire candidate set and record any missing positive witness.
  Add a deterministic bounded panel: empty, short ASCII, Unicode, multiline,
  repeated/alternating prefixes and late-invalid suffixes, including 65,536-byte
  schema subjects. Hostname calls retain the existing `<256` guard.
  Report guard skips separately; they are not engine negatives.
- [x] Require exact boolean agreement before accepting measurements. Report
  positive and negative populations separately; a missing positive witness is
  explicit, not evidence of complete semantic coverage. Synthetic literal
  positive/negative controls independently reject constant-result matchers.
- [x] Measure construction timings and incremental first/changed/repeated
  searches separately. Observe release in uninterrupted construction-through-drop
  allocation windows, not as an isolated phase. Reuse one cache through
  a deterministic input history for schema helpers. For hostname additionally
  measure the actual fresh-cache-per-call path, including construction/drop.
  Include forward-NFA compilation in direct PikeVM construction. Measure complete
  program-plus-cache and cache-only construction-through-drop lifetimes in
  additional uninterrupted meter windows; require final retained deltas of zero.
  Individual search-window metrics describe incremental allocation activity,
  not total retained cache storage.
- [x] Keep input generation, schema compilation/AST extraction, serialization and
  file I/O outside measured search windows. Track requested heap, not RSS or a
  conservative bound; retain before/after ownership and release checks.
- [x] Timing uses seven paired samples, alternating engine order, black-boxed
  inputs/results, and fixed per-case iterations `clamp(4096/max(bytes,64),1,64)`.
  Record raw batch nanoseconds and iteration counts, not an unsupported p99.
  Each paired sample starts with two fresh caches and identical input history:
  one first search, then repeated matching of that same input. Correctness and
  allocation phases use different caches and cannot prewarm the first search.
  Meta matching stays explicit `search_half_with(...earliest(true))`.
  Allocate no timing vectors inside a timed batch. Keep allocation accounting
  disabled during timing; disclose that the allocator wrapper remains installed.
  A 180-second comparison-phase deadline refuses output rather than publishing
  partial rows. Collection, initial agreement, metadata and serialization are
  outside this deadline; compilation is not part of the comparison phase.

## Task 3: Run, Review And Publish

- [x] Run correctness/meter controls in the ordinary test lane. Performance is
  opt-in through `AUCTIONEER__SCHEMA__MATCHER_REPORT`, and must use the existing release
  profile. Refuse an existing destination; no report is a qualification verdict.
  Test absent-output and refusal paths in the ordinary lane. Refuse debug
  reporting before measurement; use authoritative no-clobber creation only
  after full reconciliation and the run-deadline check.
- [x] Include source/capture digests, exact engine/compiler/toolchain/profile
  identities, complete membership, workload counts, phase results and sample
  ranges. Preserve complete JSON output in `crates/auctioneer-bidder/tests/data/schema-matchers/`
  and summarize it in `docs/bidders/schema-qualification.md`.
- [x] Verify default/insecure affected tests, meter/probe reuse, strict Clippy,
  formatting and fixture gates. Obtain separate spec and quality reviews.
  Update ISSUES with the comparison's limits and remaining qualification work.
- [x] Commit/push the measured checkpoint.

Commands (locked/offline, two jobs, `CARGO_INCREMENTAL=0`):
Set `AUCTIONEER__BIDDER__SOURCE_DIR` and `AUCTIONEER_APS_SOURCE` to the local reference
checkout used by the existing source-conformance checks. A report path must not
already exist.

```sh
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_source_conformance schema_meter::tests
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_source_conformance schema_matchers::tests
AUCTIONEER__SCHEMA__MATCHER_REPORT=/tmp/auctioneer-schema-matchers.json cargo test --offline --locked -j2 --release -p auctioneer-bidder --test schema_source_conformance schema_matchers::tests::catalog_matchers_agree_before_optional_measurement -- --exact --nocapture
```

Timing is exploratory local evidence, especially with concurrent host jobs. Do
not infer complete schema-validation latency, allocator/stack bounds, guest fit,
production readiness or approval of an engine switch from these measurements.

## Timing Precision Follow-Up

The first optimized pilot completed all 24 patterns and 596 timing cases. Fifteen
admitted meta warm-search series had a zero median because one operation on a
fast-refusal path could be below clock resolution. The unchanged pilot is
retained as `crates/auctioneer-bidder/tests/data/schema-matchers/native-pilot-20261027.json`.
It is exploratory output from the intermediate prototype, not qualification or
a claim that its exact source tree was a published implementation checkpoint.

Before the second measurement, freeze revision 2 batching as
`clamp(65536/max(bytes,64),64,1024)`, still seven paired samples and the same
subjects, engines and limits. This raises only observations per timed batch;
it changes no workload admission or qualification threshold. Retain the original
formula above as pilot history and emit the revised formula in the new record.
First-search samples remain single operations. Mark any zero/resolution-limited
series explicitly in the analysis; never divide by zero or infer a precise
speedup from those readings. Increased batching does not guarantee precision.

## Verification And Disposition

Executed in the M7 worktree on 2026-10-27 with Rust 1.98.1 and the locked
dependencies. Cargo jobs were serialized; host scheduling was not isolated.

- The final `schema_source_conformance schema_m` selection passed 35 tests under
  secure defaults and 35 with `insecure-bidders`: 14 comparator controls, six
  meter controls and 15 existing schema helper/inventory controls. No failures
  or ignored tests. These are selected tests, not the complete source target or
  workspace suite.
- The historical-record integrity test passed separately. It pins both exact
  JSON digests and checks exploratory scope, 24-pattern/596-case membership,
  seven-sample completeness and zero final retained deltas. It does not assert
  that historical binaries or timings describe every future source revision.
- The existing `schema_build_controls` test
  `native_generated_callbacks_have_isolated_cold_and_repeat_allocation_reports`
  passed after meter extraction. It exercises the standalone synthetic native
  probes; it is not the complete production-schema resource qualification.
- The optimized pilot and refined runs each passed their complete reporting
  test, taking 6.01 and 24.56 seconds of test execution respectively. Both cover
  the same 24 patterns and 596 selected cases. Neither had a boolean mismatch.
  The refined run has no zero-median warm series; 81 first-search engine/case
  series remain resolution-limited.
- Strict Clippy passed for `schema_source_conformance` and
  `schema_build_controls`, under defaults and with `insecure-bidders`, using
  `-- -D warnings`. Existing GVL-deletion build notices and dependency
  future-incompatibility notices remain; they are not suppressed by this work.
- Workspace formatting, explicit matcher/meter formatting, diff whitespace,
  the raw-JSON budget (189 fixtures across 55 files) and its rejection controls
  passed. Separate spec and quality reviews approved the scoped implementation;
  the final quality pass independently reproduced the table values and record
  digests from the frozen observations.

**Disposition:** preserve the current engine and all serving/resource contracts.
Warm search and fresh-cache setup show different tradeoffs; PikeVM memory is
not uniformly smaller. The [results](../../bidders/schema-qualification.md#native-matcher-comparison-2026-10-27)
retain both optimized records and state what remains unproven. This checkpoint
finishes the approved comparison, not schema resource qualification or M7.
