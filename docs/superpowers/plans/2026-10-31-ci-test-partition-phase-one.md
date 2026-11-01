# CI Test Partition Phase One Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` task by task.

**Goal:** Establish a passing, profiled baseline and then evaluate the approved
test-ownership split without reducing coverage or changing serving behavior.

**Architecture:** Repair existing harness failures first. Profile nested Cargo,
lease and observation costs in a disposable copy. Gate workflow partitioning on
the complete passing baseline required by the design. Artifact reuse remains a
separate, unapproved phase.

**Tech Stack:** Existing Rust/Cargo tests, Bash/jq workflow scripts, GitHub
Actions, the pinned Wasmtime, Prettier and actionlint. No new dependency.

**Design:** [CI test partition and artifact reuse](../specs/2026-10-31-ci-test-partition-and-artifact-reuse-design.md).

## Working Boundaries

- Work in the existing `import/auctioneer` branch in the Auctioneer checkout.
  Keep diagnostic mutations and profiling instrumentation in a disposable copy.
- Serialize heavy Cargo commands and reuse the existing build cache. Preserve
  workload sizes, limits, source/identity checks and measurement isolation.
- Publication of the verified prerequisite repairs to draft PR #1 was approved
  on 2026-10-31. Do not publish further scope or manually rerun remote workflows
  without approval.
- Update this plan incrementally. Do not mark a filtered or local pass as the
  complete CI baseline. Do not implement the deferred in-process cache.

## Task 1: Diagnose And Repair Current CI Failures

**Files to inspect, modify only if the diagnosis requires it:**

- `crates/auctioneer-bidder/tests/schema_source_conformance.rs`
- `crates/auctioneer-bidder/tests/schema_build/application.rs`
- `crates/auctioneer-bidder/tests/schema_build/compiler_layout.rs`
- `crates/auctioneer-bidder/tests/schema_build/inspection.rs`
- `crates/auctioneer-bidder/tests/schema_build/mod.rs`
- `crates/auctioneer-bidder/tests/schema_compiler_layout/mod.rs`
- Add the test-only shared `crates/auctioneer-bidder/tests/schema_build/native_dwarf.rs`
  reader so both producer and layout decoding guard opaque TLS operands.

- [x] Read run `41184541480`, its failure annotations and existing assertions.
- [x] Reproduce `tests::normal_application_dependency_graphs_exclude_unqualified_schema_runtime` unchanged. It fails on Axum's native `jsonschema` dependency; the current approved native dependency declaration intentionally includes it.
- [x] Repair and verify the graph control as `normal_application_dependency_graphs_keep_schema_runtime_native_only`: 1 passed, 0 failed, all four adapter graphs checked.
- [x] Retrieve the complete raw job log: production-source conformance ran 655 tests, with 652 passed and 3 failed in 1,166.11 seconds. The original formatted CLI capture omitted its final summary.
- [x] Run `tests::compiler_private_layout_facts_cover_native_and_every_guest_abi` unchanged: Mac native plus three guest ABIs pass in 431.39 seconds. Recover the Linux `Unsupported relocation type` diagnostic and reproduce it with portable ELF TLS controls.
- [x] Run `tests::actual_application_artifacts_retain_every_selected_validator_in_both_feature_lanes` unchanged: both lanes/all four adapters pass on Mac in 746.66 seconds. Reproduce Linux's `null inspection function` with portable ELF PIE records.
- [x] Compare each failure to the approved controlled-native/staging contracts;
      distinguish obsolete assertions, real receipt mismatches and resource/lock
      failures before choosing a fix.
- [x] Add narrow red controls for any changed helper, then implement only the
      root-cause repair. Preserve exact target/feature and artifact identity checks.
- [x] Complete both repaired targets before the final overlap follow-up:
      323 synthetic controls and 680 source cases passed with zero failures.
      The latter includes the three formerly failing cases. These are local
      Apple Silicon results, not the full Linux CI baseline.
- [x] Reproduce the final ELF overlap follow-up: 18 passed / 7 failed before
      checked write extents; afterward all 25 inspection cases pass in each
      target. Preserve 8/16-byte touching/overlap boundaries, unknown-operation
      refusal, duplicate spans, zero-width no-ops and checked address arithmetic.
- [x] Run the three formerly failing cases, their helper tests and strict
      Clippy. Final focused checks pass: inspection 25/25, layout 22/37 and
      guarded reader 14/14 across synthetic/source targets; the four-graph case
      passes again, source-import isolation passes 16 cases, formatting passes.
      See the verification note for full-run versus final-filtered boundaries.

Diagnostic command pattern, with the existing external target cache and pinned
Wasmtime selected in the caller environment:

```sh
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test schema_source_conformance \
  tests::normal_application_dependency_graphs_keep_schema_runtime_native_only \
  -- --exact --nocapture
```

Use the other two full test names in separate invocations so a failure has an
immediate diagnostic rather than waiting for the complete source target. The
655-case count belongs to the original CI revision; added controls increase it.
These filtered runs diagnose; they do not satisfy the full baseline gate.

## Task 2: Profile The Unchanged Workload

**Disposable files only:**

- Copy the recorded source revision and local repairs into a temporary source
  tree; retain the source identity and instrumentation diff.
- Instrument only the parent helpers in `tests/schema_build/mod.rs`, including
  `run`, `acquire_lease` and the catalog build/identity phases.
- Keep generated validators, allocator-instrumented children and all test
  assertions unchanged.

- [ ] Establish a complete passing local run of each schema target, with the
      pinned prerequisites and no filtered-out cases.
- [ ] Profile each target separately using its existing libtest concurrency.
      Preserve fresh native/guest processes and existing workloads.
- [ ] Capture monotonic begin/end intervals, test names, child operation class,
      outcome and lease wait/hold time. Never log raw inputs or credentials.
- [ ] Report outer compilation separately from child Cargo build/check/metadata,
      native/guest observations and source/artifact hashing.
- [ ] Compute interval unions/overlap rather than adding concurrent durations
      into a false wall-time percentage. Label local architecture/cache conditions.
- [ ] Save a concise diagnosis under `docs/testing/` with reproducible commands
      and actual outcomes; do not claim a full PR speedup.

## Task 3: Establish The Full Acceptance Baseline

**Files:** `docs/testing/` evidence and this plan; no workflow edits yet.

- [x] Obtain publication approval before committing/pushing the verified
      repairs: explicitly approved for draft PR #1 on 2026-10-31. Workflow
      partitioning and artifact caching are not part of this publication.
- [ ] Confirm a successful run of every existing acceptance job, including all
      later steps that were skipped in the failed runs.
- [ ] Record revision, runner/tool identities, cache state, job/step times and
      actual test dispatch lists. Freeze the effective B/S/W0/W1/CA/CM feature
      graphs and explicitly ignored/required-feature executions from the design.
- [ ] Select the smallest justified Phase 1 change from the measured result.
      If the split is not justified, revise it instead of implementing by inertia.

## Task 4: Implement And Evaluate Approved Ownership Partitioning

**Only after Tasks 1-3 pass. Potential files:**

- `.github/workflows/test.yml`
- `.github/workflows/scripts/test/` target ownership/selection implementation
  and colocated behavior tests
- `docs/testing/development.md`

- [ ] Implement the exact scope/target matrix from the approved design, keeping
      `conformance-suite`, `cross-target-check`, triggers and feature-sensitive
      standalone invocations intact.
- [ ] If a selector is needed, first write real tiny Cargo-workspace behavior
      controls for feature unification, exclusions, required features, duplicate
      target names, custom harnesses, lib/bin/example/bench execution, malformed
      artifact output and stale/duplicate ownership.
- [ ] Observe those controls fail before implementing the Bash/jq selector.
      Derive executable targets from actual Cargo JSON and execute through Cargo.
- [ ] Preserve all prerequisites and exact per-command environments. Do not
      introduce optional jobs, skip gates or source/YAML snapshots.
- [ ] Compare before/after test dispatch and feature graphs. Run behavior tests,
      shell syntax checks, actionlint and affected Rust acceptance.
- [ ] After authorized publication, compare at least three successful matched
      before/after runs. Report critical path, aggregate runner time and added
      compile/setup costs. Do not claim improvement before these runs exist.

## Completion

- [x] Implemented prerequisite changes verified, with full-current-revision CI,
      profiling and workflow partitioning still pending. See
      [local verification](../../testing/ci-prerequisite-repairs-20261031.md).
- [x] Phase 2 remains deferred unless separately approved from measured need.
- [ ] Report changed files and actual verification; do not merge automatically.
