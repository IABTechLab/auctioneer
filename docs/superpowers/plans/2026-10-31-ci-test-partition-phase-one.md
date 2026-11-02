# CI Test Partition Phase One Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` task by task.

**Goal:** Remove repeated hermetic execution with explicit parallel CI owners,
without reducing unique coverage or changing serving behavior.

**Architecture:** Keep ordinary feature-sensitive scopes intact. Use Cargo's
explicit target selection for mandatory hermetic owners, consolidate redundant
catalog setup and parallelize independent artifact checks. Artifact reuse remains
a separate, unapproved phase; baseline timing is not a claim of performance.

**Tech Stack:** Existing Rust/Cargo tests, Bash/jq workflow scripts, GitHub
Actions, the pinned Wasmtime, Prettier and actionlint. No new dependency.

**Design:** [CI test partition and artifact reuse](../specs/2026-10-31-ci-test-partition-and-artifact-reuse-design.md).

## Approved Ownership Revision

On 2026-10-31 the user approved once-owned parallel checks. This section
supersedes Task 3's passing-baseline prerequisite and Task 4's original selector
and full-source-rerun policy. Their historical incomplete boxes are not claims
of verification. Artifact reuse and publication remain separately gated.

1. [x] Audit actual repeated work and prove Cargo default/explicit ownership
       in a tiny real workspace: `test = false` excludes broad `--all-targets`
       execution but an explicit `--test` still dispatches the test.
2. [x] Add manifest ownership for bidder's schema controls, source conformance,
       factory binding and registry rebuild targets, plus config's build-failure
       harness. No ignore, required-feature switch or custom dispatcher.
3. [x] Consolidate the three native catalog assertion groups into their
       existing default/explicit cross-target owners. Keep every observation
       subprocess fresh; assert exact membership and run mutating refusals last.
       Keep the source-only extra storage proof separate (23 to 17 builds).
4. [x] Give compiler/source checks, config refusals, compiler layouts, selected
       probe and each adapter artifact inspection unconditional CI owners.
       Preserve both feature lanes per artifact and all prerequisites. Keep
       heavy commands sequential per runner and use owner-specific cache keys.
5. [x] Retain actual source feature-view checks explicitly in S/W0/W1; retain
       all four metadata/core normal/staged commands, ordinary registry checks,
       CLI CA/CM, guests, privacy and sockets. Remove only the confirmed duplicate
       `disclosure_codegen` execution.
6. [x] Compare before/after names and assertion owners, run changed owner groups,
       actual Cargo selection controls, strict Clippy, formatting and actionlint.
       Report local versus CI results and any unexecuted owner separately.
7. [x] Update development ownership documentation and the verification note.
       Do not claim a whole-CI speedup without successful matched runs. Obtain
       publication approval before a new commit/push to PR #1: explicitly granted
       after local verification on 2026-10-31.

## Approved Native Accounting And Remaining Compiler Owners

On 2026-11-01 AG approved correcting the native baked-config accounting
references, moving two individually audited hermetic suites to the existing
compiler owner, and collecting every target failure within each workspace run.
No cache/reuse change, assertion removal, workload reduction or platform change.

- [x] Reproduce the three Axum failures; verify the renamed cookie identities
      add eight serialized bytes, 1,024 bytes per auction and 2,048 total bytes.
      Correct independent references and retain the 4-GiB/256-MiB constraints.
- [x] Audit `generated_settings` and `insecure_bidders_snapshot`: child sources,
      features and environment are independent of the parent feature lane.
      Preserve 19 cases per suite, the 14-case settings child and all fixtures.
- [x] Add both target declarations in `auctioneer-bidder/Cargo.toml` with
      `test = false`; add full executions and explicit strict Clippy to
      `verify-schema-compiler-conformance.sh`. Validate both settings reports.
- [x] Add `--no-fail-fast` to both workspace commands in `test.yml`; a failed
      target must still produce a failed job, while later targets execute.
- [x] Verify real broad/explicit Cargo selection, full target execution,
      nested-result refusal controls, native accounting, strict lint and syntax.
- [x] Update development guidance, the exhaustive exception matrix and measured
      evidence. Full Linux acceptance and end-to-end timing remain pending CI.

Publication of these verified repairs to PR #1 is approved. Push after the final
diff check; do not claim the subsequent CI result before it completes.

## Working Boundaries

- Work in the existing `import/auctioneer` branch in the Auctioneer checkout.
  Keep diagnostic mutations and profiling instrumentation in a disposable copy.
- Serialize heavy Cargo commands and reuse the existing build cache. Preserve
  workload sizes, limits, source/identity checks and measurement isolation.
- Publication of the verified repairs, native-only setup and once-owned
  follow-up to draft PR #1 was approved on 2026-10-31 after local verification.
  Do not publish additional scope or manually rerun workflows without approval.
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

### Approved Native-Only Setup Repair

The user explicitly approved this narrow repair on 2026-10-31. It is separate
from workflow partitioning and does not authorize artifact caching.

**Files:** `tests/schema_build/mod.rs`, `tests/schema_build_cases/mod.rs`,
and `tests/schema_source_conformance.rs` within `auctioneer-bidder`.

- [x] Audit every catalog caller and its transitive observations. Four callers
      use only native outputs; retain both full native/three-WASM link tests.
- [x] Profile a representative unchanged native-only case in a disposable copy;
      record child build targets/outcomes and separate outer compilation.
- [x] Add a failing target-selection control against the old all-target setup:
      one control failed and the all-target control passed before repair.
- [x] Add explicit native-only entrypoints, route only the four audited callers,
      and assert their report membership is exactly `native`.
- [x] Keep every original assertion, workload, measurement process, lease,
      target/feature gate, refusal workspace and source/artifact check unchanged.
- [x] Verify both full targets: 333 synthetic and 690 source cases passed with
      no failures, ignores or filters. Strict Clippy, formatting and all 16
      source-isolation controls passed.
- [x] Repeat the microprofile with the same instrumentation and input/cache
      conditions. Report measured local results and the 44-to-23 request census;
      the exact case passed in 138.91 s before / 48.23 s after. This is not an
      end-to-end CI speedup or full-suite profile.
- [x] Obtain separate publication approval for this new scope before pushing:
      explicitly approved for draft PR #1 on 2026-10-31. Workflow partitioning
      and artifact caching remain unchanged.

The microprofile above is diagnostic only. The complete per-operation profiling
below and a passing CI baseline remain prerequisites for workflow partitioning.

**Disposable files only:**

- Copy the recorded source revision and local repairs into a temporary source
  tree; retain the source identity and instrumentation diff.
- Instrument only the parent helpers in `tests/schema_build/mod.rs`, including
  `run`, `acquire_lease` and the catalog build/identity phases.
- Keep generated validators, allocator-instrumented children and all test
  assertions unchanged.

- [x] Establish a complete passing local run of each schema target, with the
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

## Original Task 3: Full Acceptance Baseline (Superseded Gate)

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

## Original Task 4: Initial Partition Proposal (Superseded)

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

- [x] Prerequisites and once-owned parallel execution implemented and reviewed.
      Scoped local verification is recorded below; new Linux acceptance and
      matched whole-CI performance measurements remain pending. See
      [local verification](../../testing/ci-prerequisite-repairs-20261031.md).
- [x] Phase 2 remains deferred unless separately approved from measured need.
- [x] Record changed ownership and actual local verification; do not merge automatically.
