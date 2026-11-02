# Test And Runtime Performance Repair Plan

> **For agentic workers:** Use the systematic-debugging, test-driven-development
> and verification-before-completion skills. Keep build-heavy verification serial.

**Goal:** Remove verified repeated test work, bound CI execution and reduce
unnecessary request construction without changing accepted traffic or privacy.

**Architecture:** Exhaustive unit controls use small independent fixtures and
one assertion owner; real compiler, process and wire contracts retain explicit
mandatory owners. Share Cargo download caches across jobs and retain compiled
outputs only for the slow native lanes. Do not cache test results or mutable
source snapshots. Production changes must preserve wire output, tie ordering,
resource ceilings and signed-config freshness.

**Approval:** AG requested these repairs and incorporation of the supplied
duration analysis on 2026-11-02. Shared downloads and selective compiled caches
are approved. Paid runners, remote cache deletion and cross-request config
caching are excluded. Publication requires approval after local verification.

## Tasks

- [x] Verify and correct the duration analysis against job logs, cache metadata
      and official runner/cache documentation. Keep observations distinct from
      causal claims; no matched passing speedup is available.
- [x] Give hermetic sealed-body and feature-export child compiler cases one
      mandatory owner; retain the parent-feature predicate in normal lanes.
- [x] Replace whole-checkout unit mutation fixtures with minimal source trees
      plus independently constructed build identities. Retain live-tree binding
      coverage and every edit/add/delete/move/evidence classification assertion.
      Reuse immutable preparation seeds in qualification unit matrices without
      sharing mutations or caching identity results across edits.
- [x] Use the slow-suite timeline to give the two expensive evidence producers
      one exact-dispatch owner. Keep every original assertion and strengthen
      feature bindings. Only libtest skip arguments change broad invocations,
      leaving their Cargo graphs and 17 lightweight cases intact. Exhaustive
      verifier field cases remain; no assertions are deleted to reduce time.
- [x] Add explicit job and long-step timeouts. Share download cache keys, limit
      compiled caches to native acceptance lanes, retain save-after-failure,
      and preserve all feature graphs, explicit owners and workflow triggers.
- [x] Optimize cloning of discarded competitor data only with independent wire
      equivalence and allocation controls. Record additional config/targeting
      optimization requirements without introducing unmeasured cache/index
      ownership changes in this repair. Existing config update semantics and
      resource accounting stay unchanged.
- [x] Verify focused regressions, exact test ownership/dispatch, strict Clippy,
      formatting and shell/workflow checks. Record local versus hosted evidence
      and remaining limitations; update ISSUES.md and development ownership.

### Explicit Follow-Ups

The 42 shared synthetic helper unit cases remain unchanged. Selecting one
owner through special cfgs/features, private imports or new public APIs would
violate the module strategy for little critical-path benefit. Broader clean
consolidation is deferred rather than hidden behind an ignore or a gate.
Config caching and targeting indexes remain profiling/design follow-ups; they
are not prerequisites for the reviewed request-copy correction.

AG also approved commit/push of the verified repair round to draft PR #1 on
2026-11-02. Do not merge or claim hosted success before CI completes.

## Acceptance

- No unique assertion is lost merely to reduce counts; moved controls have one
  named mandatory owner. Duplicate cases may be removed only with an explicit
  retained guarantee. Parent-feature tests still run under each relevant graph.
- Source mutations are observed afresh and independent fixtures cannot pollute
  each other. Measurement subprocesses remain fresh.
- Production output, original input immutability and admission/privacy order
  are unchanged. Allocation reductions are measured with cross-size controls.
- Download caches contain no target artifacts. Compiled caches contain no test
  observations and cannot turn a failing Cargo command into success.
- No hard claim of a cache quota, vCPU count, billing effect, hung workflow or
  elapsed-time improvement is made without evidence for this repository/run.

## Local Verification

Native Cargo checks use the existing target directory, `--locked --offline`
and two build jobs. These are focused local results, not full hosted acceptance.

| Owner | Result |
| --- | --- |
| CLI resource units | 12 passed; 7.74 seconds of test execution |
| Provider artifact units | 15 passed, including all 8/6 mutation rows |
| Sealed response compiler cases | 8 passed |
| Feature export compiler cases | 9 passed |
| Registry source conformance | 31 passed secure, 31 passed development |
| Schema diagnostic controls | 2 passed; other compiler cases not selected |
| Exchange unit suite | 286 passed |
| Sparse request allocation target | 12 passed; discarded and retained payload controls |
| Workspace test isolation | 16 passed |
| Browser shutdown | 2 passed against the real Axum child |
| Evidence feature binding | 1 passed, 18 filtered; 591.05 seconds locally |

The original focused Clippy pass covered the modified libraries and named
integration targets, not their unit-test targets. A subsequent CI follow-up
reproduced the split unit case's forbidden `unreachable!()` and the CLI unit
helper's absolute path; both are corrected without allowances or assertion
changes. Follow-up lint commands use CI's `--all-targets` selections.
Both complete native workspace feature lanes and all three WASM
privacy/adapter lint selections pass locally with `-D warnings`; no guest
execution is implied by these compilation checks.
The affected split module rerun passes 27 cases; the CLI resource module
rerun passes 12, including live source binding. Formatting and diff checks
remain clean. Hosted confirmation stays pending until the new CI run finishes.
Formatting, raw-JSON fixture gate (200
fixtures in 57 files), all five workflow files under actionlint, four shell
helpers under ShellCheck and Bash syntax, and 18 libtest-summary controls pass.
A disposable Cargo stub also confirms the producer owner rejects a missing
case, zero dispatch, ignored results and a failed Cargo command. A structured
workflow check confirms unchanged broad Cargo graphs, eight explicit bidder
compiler owners, one producer owner and disabled implicit setup caches.

The isolated CLI job now explicitly builds both browser prerequisites; the
empty-target startup failure was reproduced before running both passing
shutdown controls. No full Chromium/publisher or hosted-provider test is
claimed by the pending-launch stub. Full compiler/producer and hosted feature
matrices remain mandatory CI owners, not locally re-run acceptance claims.
The exact CLI binding matrix remains expensive: every mutation still invokes
the real process and current-input checks. Its successful execution is not a
claim of reduced latency; further profiling and ownership review is separate.

## Approved Test Simplification

AG approved this follow-up scope on 2026-11-02: reduce redundant test work and
simplify ownership, without additional CI jobs. Use writing-plans and
subagent-driven-development; verify heavy Cargo work serially. Keep the current
passing baseline at `85aba38` separate from results of the cleanup.

### Baseline

All 25 checks passed. Run Tests `41481375478` completed in 31m49s. Its default
workspace test step took 1651 seconds, CLI test step 1292 seconds and the two
producer cases 351.23 and 944.69 seconds. The default workspace's lighter
evidence cases took 416.78 seconds and its boot-parity target 294.13 seconds.
This is a baseline, not a projected speedup. Catalogue-independent probes and
closed child graphs can have one owner; distinct parent feature contracts stay.

### Task A: Evidence And Provider Verifiers

Files: `crates/auctioneer-cli/src/bidders/evidence/{mod.rs,native/mod.rs}`,
`crates/auctioneer-cli/tests/bidders_evidence.rs` and
`crates/auctioneer-cli/src/qualification/harness/providers/artifacts.rs`.

- [x] Characterize exhaustive binding and receipt refusals in inline units,
      with independently valid fixtures, one changed field and exact errors.
- [x] Separate pure manifest/compiler-message comparisons from existing I/O
      only where needed; keep production freshness checks and error precedence.
- [x] Replace exhaustive CLI mutation loops with their unit owners, retaining
      positive real dispatch, actual source/executable/feature binding,
      representative rejection routing, restoration, leases and replay guards.
- [x] Keep provider source/fixture mutation and archive/staging boundaries real;
      avoid repeating full preparation for pure compiler-message field checks.

Focused checks: `cargo test --locked -p auctioneer-cli --bin auctioneer-cli
bidders::evidence::`; provider `artifacts::tests::`; the full `bidders_evidence`
target with only the two existing heavy producers excluded initially. Both
heavy producer cases remain mandatory and are verified by their existing owner.

### Task B: Compiler Build Reuse Within A Suite

Files: `crates/auctioneer-bidder/tests/schema_build_cases/mod.rs`,
`crates/auctioneer-bidder/tests/schema_source_conformance.rs`,
`crates/auctioneer-config/tests/build_failure_harness.rs` and
`crates/auctioneer-config/tests/bidder_settings_compile/mod.rs`.

- [x] Give catalogue-independent API/reference compiler refusals only the
      synthetic compiler owner; retain every separate negative and positive.
- [x] Run source cache-layout assertions using the existing full explicit
      catalogue before refusal controls mutate its scratch workspace.
- [x] Reuse the clean no-settings config fixture for its generated consumer,
      preserving all trait, discriminator, wire-shape and success assertions.

Focused checks: the exact linked catalogue cases in both explicit schema test
targets; `child_build_succeeds_with_clean_tempdir`; all native cache-layout
assertions remain reachable. Reconcile exact source-owner dispatch counts.

### Task C: Remove Or Relocate Proven Duplicates

Files: the three guest adapters' `tests/fanout_conformance.rs`,
`auctioneer-privacy/{tests/stub_invariants.rs,src/auction_context.rs}`,
`auctioneer-runtime/{tests,src}/{json_size,auction_budget}.rs` and
`auctioneer-core/{tests/usersync_live_catalog.rs,src/test_support/live_catalog.rs}`.

- [x] Remove adapter wrappers that call the same core-only rows; retain their
      core owner and every actual adapter primitive/socket contract.
- [x] Remove duplicate permissive privacy controls, keeping their inline owner
      and distinct sync-normalization/public activity cases.
- [x] Move pure runtime and helper self-tests inline, preserving assertions and
      every justified cold-state/global-allocator/process isolation boundary.

Focused checks: privacy stub/inline units, runtime units, core live-catalog
tests and native adapter fanout conformance with its required feature. Run
`auctioneer-bidder --test test_isolation` and the existing roster/raw-JSON gates.

### Task D: Report Integration And Closed-Graph Ownership

Files: `crates/auctioneer-cli/tests/bidders_resources.rs`,
`crates/auctioneer-cli/Cargo.toml`, `.github/workflows/test.yml` and
`docs/testing/development.md`.

- [x] Consolidate four identical ordinary report invocations and one strict
      invocation into one of each, preserving every report/feature assertion.
- [x] Give `resource_boot_parity` one explicit owner in the existing CLI lane;
      retain all eight registered tests, ten profiles, forty fresh adapter
      installs and actual feature receipts. Select its lint target explicitly
      as well; ownership must not silently narrow all-targets lint coverage.
- [x] Record each removed case and retained owner, update current documentation
      and reconcile actual Cargo dispatch before claiming coverage preservation.

### Verification And Publication

- [x] Inspect the complete diff for unique assertions, feature/environment
      dependencies, hidden global state and accidental source-input changes.
- [x] Run affected unit/public/integration cases, full native all-targets lint
      lanes, impacted guest lint, formatting and existing source/roster gates.
- [x] Review independently and record executed versus not-run checks.
- [ ] Complete the hosted acceptance matrix and compare actual durations
      against the passing baseline; do not infer a whole-CI speedup locally.

Commit and push the verified round to PR #1 under AG's standing publication
request. Report hosted results separately; no promised time target or merge.

Do not add a test framework, crate, feature flag, source import workaround or
global cached verdict. Share setup and unchanged build products only where
ownership permits; never share expected outputs or allocation observations.
Independent privacy oracles and source-provenance checks remain distinct.

### Coverage Ownership Ledger

| Removed or reduced work | Retained owner and guarantee |
| --- | --- |
| Five `transport_contract` / `entry_deadline` cases repeated in three guest targets | Same five case names in core `tests/fanout_conformance.rs`; every adapter-specific row stays |
| `auction_stub_never_enforces_ccpa` and `auction_stub_take_warnings_is_idempotent_empty` | Inline `auction_context::tests::{explicit_permissive_context_does_not_enforce_ccpa,explicit_permissive_context_has_no_warnings}`; distinct public activity and sync cases stay |
| Twelve runtime and two helper integration cases | Same assertions inline in their owning modules; no allocator or global-state binary merged |
| 16 binding CLI checks reduced to three | Inline independent binding/feature matrix plus real linked-feature refusal and restoration |
| Native receipt mutation subprocesses reduced from 13 to six | Exact-error native verifier matrix; real producer, malformed input, executable I/O, error routing and exclusivity stay |
| Six reference receipt mutations reduced to one | Nine-field inline receipt matrix plus unknown-producer CLI refusal and restoration |
| Four requirement mutations reduced to one | Inline membership/decoding controls plus real missing-native-producer refusal |
| 15 provider capture calls reduced to two | Independent compiler-message matrix; actual capture success/refusal and source/fixture mutations stay |
| 15 repeated source-root compiler checks | Synthetic owner retains all API/option/reference refusals and positive controls |
| One standalone native source catalogue | Source-only cache-layout assertions run on the full four-target catalogue before mutation |
| One no-settings config build | Same generated consumer executes inside the clean child-build test's active lease |
| Five report subprocesses reduced to two | Combined ordinary/strict case retains report, accounting, linked-feature and exit assertions |
| Three boot-parity parent dispatches reduced to one | Explicit CLI owner still runs eight cases, ten profiles and forty fresh adapter installs; explicit lint retained |

Independent review found no missing unique assertion or changed production
error ordering in these changes. Runtime measurements and hosted duration
claims still require their own completed runs.

### Local Verification Of The Simplification

All Cargo commands used the locked dependency graph, offline mode and two
build workers, with heavy builds serialized and the existing local target cache
retained. Results below are executed cases, not filtered registrations.

| Check | Result |
| --- | --- |
| Core, privacy and runtime library suites | 609 / 288 / 266 passed |
| Evidence verifier units | 39 passed |
| Provider compiler-message selector and real capture | One passed each; actual capture retains success and rejection |
| Privacy stub and compiled usersync catalogue | Four / one passed |
| Guest adapter and core fanout targets, `conformance-suite` enabled | Nine per guest plus 35 core cases passed |
| Workspace source-import / test isolation | 16 passed |
| Real resource-report CLI target | 13 passed |
| Lighter evidence integration target | 17 passed, exactly two heavy producers filtered; 196.86 seconds locally |
| Clean config child build with generated-settings consumer | One passed, 103 filtered |
| Both `all_schema_` cases in both compiler owners | Two per owner passed; synthetic owner 328 filtered, source owner 689 filtered |
| Explicit resource boot parity | Eight passed; 236.57 seconds locally |
| Native workspace all-targets Clippy, default and insecure modes | Both passed with `-D warnings` |
| Explicit compiler/config/parity target Clippy | Passed with `-D warnings` |
| Guest Clippy including adapter, exchange, privacy, bidder and runtime tests | All three deployment triples passed with `-D warnings`; compilation/lint, not guest execution |
| Formatting, diff, workflow, fixture and roster checks | Passed; 200 raw fixtures in 57 files, 139 uniquely owned roster members |

Total executed tests: 1,330 passed. The first config-child attempt was blocked
by the sandbox before dispatching its child; the authorized rerun passed.
Two test-only Clippy naming/complexity findings were repaired without lint
allowances, followed by the passing complete lint selections above.

An actual `auctioneer-cli --all-targets -- --list` run confirms boot parity is
absent from broad execution. Its explicit execution and lint remain in the
existing CLI job. Cargo metadata now enumerates 239 integration targets rather
than 241, reflecting only the two pure runtime targets relocated inline.

The complete workspace feature matrix, remaining compiler cases, and both real
evidence-producer cases were not rerun locally in this round. They retain their
mandatory CI owners, including fresh producer builds and exact dispatch checks.
Neither local times nor comparisons across different machines establish a
hosted critical-path improvement. No workload limit, resource ceiling, frozen
capture or independent privacy oracle changed.
