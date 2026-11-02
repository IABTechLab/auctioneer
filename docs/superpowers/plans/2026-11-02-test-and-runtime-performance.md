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

- No assertion is lost merely to reduce counts; moved controls have one named
  mandatory owner. Parent-feature tests still run under each relevant graph.
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
