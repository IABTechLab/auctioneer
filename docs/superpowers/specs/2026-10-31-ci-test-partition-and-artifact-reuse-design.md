# CI Test Partition And Artifact Reuse

Status: Phase 1 start approved on 2026-10-31. Baseline repair and profiling
precede any workflow partitioning. In-process artifact caching remains deferred
and unapproved. This revision replaces the earlier cache-first proposal with a
measurement gate and two separately approved phases.

## Goal And Evidence

Reduce pull-request feedback time without removing acceptance checks, changing
feature unification, warming measurement processes, or weakening source and
artifact verification. Optimize measured critical-path work, not a presumed
compilation percentage.

The available GitHub Actions observations are failed, partial runs:

| Run / Commit                                                                                  | `test` Job Wall Time | Schema Step Wall Time | Outer Cargo Build | Synthetic Controls             | Production-Source Conformance    |
| --------------------------------------------------------------------------------------------- | -------------------- | --------------------- | ----------------- | ------------------------------ | -------------------------------- |
| [41182511076](https://github.com/IABTechLab/auctioneer/actions/runs/41182511076) / `f57ea6c7` | 1,075 s              | 826 s                 | 47.79 s           | 299 passed, 2 failed; 778.02 s | Not reached                      |
| [41184541480](https://github.com/IABTechLab/auctioneer/actions/runs/41184541480) / `f032ce5a` | 1,751 s              | 1,580 s               | 25.77 s           | 301 passed, 0 failed; 387.83 s | 652 passed, 3 failed; 1,166.11 s |

These are not passing-PR baselines. Later steps in `test` were skipped in
both runs; their costs are unknown. The two failures in the first run were
repaired in `f032ce5a`, and the second run confirms the synthetic controls
passed. It does not establish source-conformance or whole-workflow acceptance.
The raw job-log API recovers the source target's complete summary and failure
diagnostics, which were absent from the earlier formatted-log capture: the
normal dependency graph assertion, an unsupported native debug relocation and
a null inspection-function pointer. They are prerequisite repairs, not evidence
that test ownership or caching caused the failures.

The outer Cargo build time excludes compilation launched by tests themselves.
`schema_build::linked_catalog_for` performs native and three WASM Cargo builds
on each call. Five callers in `schema_build_cases` therefore request twenty
build invocations, not necessarily twenty complete recompilations; Cargo may
reuse dependency products. The production-source target has an additional
explicit-catalog caller. Generated-source refusal probes also run Cargo checks.
The reported libtest duration includes that work, lease waits and fresh native
and guest observations. Neither "5.7% is all compilation" nor "the remaining
778 seconds is all guest execution" follows from the first run.

### Measurement Gate

Before changing execution ownership:

1. Resolve current CI failures independently of optimization. Obtain a complete
   successful run of the current acceptance matrix at a recorded revision.
   A passing filtered target may diagnose a cost but cannot replace this gate.
2. Profile `schema_build_controls` and `schema_source_conformance` separately,
   with existing workloads, tool pins, features and libtest concurrency. Record
   per-test wall time, child Cargo build/check/metadata time, native/Wasmtime
   observations, source hashing, and lease wait/hold intervals. Separate outer
   harness compilation from child compilation. Use a disposable source copy
   for temporary parent-harness instrumentation, never another reviewer's
   working tree or an allocation-measured child. Keep its source identity and
   instrumentation diff so profiling is not represented as an unmodified run.
3. Record monotonic intervals and operation counts. Do not add overlapping
   child or lease durations to claim a percentage of wall time; show their
   interval union and overlap, and do not call child wall time compiler CPU.
   Timing test completions alone does not distinguish execution from waiting.
4. Record runner class, available concurrency, cache state, setup/checkout/fetch
   costs and complete job/step outcomes. Local timings remain local. Capture
   actual test-name/dispatch lists and effective Cargo feature graphs for each
   invocation below, including required-feature and explicitly ignored targets.

No full passing baseline or per-operation profile has been established by this
document. Do not invent either result, a speedup estimate or a maximum saving
from the failed-run totals.

## Alternatives And Scope

- Keep the current workflow and fix its failures: least complexity and the
  required baseline, but no deliberate reduction in repeated execution.
- Phase 1: separate proven independent workloads and remove only demonstrated
  duplicate executions. This is the first optimization to evaluate after the
  measurement gate; preserve all meaningful feature configurations.
- Phase 2: reuse verified generated build products inside a test process. This
  adds lifecycle, integrity and lease complexity and is deferred. Reconsider
  only if Phase 1 measurements show repeated child builds or regeneration
  still dominate. A new approval is required before implementing Phase 2.

Neither increasing libtest concurrency nor sharding a leased compiler target
is presumed to help. A shared global lease can serialize additional threads;
separate runners add setup, compilation and memory costs. Choose based on the
profile without changing measurement isolation.

## Phase 1: Execution Ownership

The existing YAML job ID is `test`, displayed as `cargo test`. Replace only
that job, if the measurement gate supports the split, with these proposed IDs:

- `test`: native/workspace acceptance, retaining its `cargo test` display name.
- `compiler-conformance`: compiler/schema/guest acceptance.
- `config-build-failures`: hermetic configuration rejection and the explicit
  ignored disclosure-model check.

Existing `conformance-suite` and matrix `cross-target-check` jobs remain
independent and unchanged. Preserve push-to-`main` and unrestricted
`pull_request` triggers. Every proposed job runs on every PR, without optional,
nightly or path-filtered acceptance. Do not serialize the new jobs with `needs`;
keep heavy commands sequential within a runner. Use separate target-cache keys
per ownership job and compatible toolchain/lock inputs; cache restoration is
not evidence that any test or identity check ran.

Preserve each moved command's environment, pinned source checkouts, required
tools and working directory. In particular, compiler cases still need
`rust-src`, all three WASM targets and the pinned Wasmtime; CLI source and SDK
checks retain their reference checkouts and verifier tools. Do not remove a
prerequisite from a job until its remaining commands are shown not to need it.

### Package And Feature Scopes

The following symbols name exact existing invocations, not interchangeable
labels for "secure" and "development":

| Scope | Existing Cargo Selection                                                                |
| ----- | --------------------------------------------------------------------------------------- |
| B     | `--locked -p auctioneer-bidder` with no added features                                  |
| S     | `--locked -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core --all-targets` |
| W0    | `--workspace --all-targets`                                                             |
| W1    | `--workspace --all-targets --features insecure-bidders --exclude auctioneer-config`     |
| CA    | `--locked -p auctioneer-cli --all-targets`                                              |
| CM    | `--locked -p auctioneer-cli --no-default-features --all-targets`                        |

Axum's development dependency enables `insecure-bidders` in W0. That does not
make W0 equivalent to W1 or CA. Retain these scopes separately; do not replace
S with a workspace-unified invocation or infer equivalence from one feature.

### Ownership Matrix

Rows are `(package, integration target)` unless explicitly identified as a
library filter. This is the initial exhaustive exception set, not permission
to skip all tests in the named packages.

| Owner                   | Target / Filter                                                                 | Required Execution                                                                                                                 |
| ----------------------- | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `compiler-conformance`  | `auctioneer-macros / conformance`                                               | Once with the existing standalone macro `--all-targets` command; keep library tests there and in ordinary scopes                   |
| `compiler-conformance`  | `auctioneer-bidder / schema_build_controls`                                     | Once in B; synthetic hermetic controls retain all native/three-WASM links and fresh observations                                   |
| `compiler-conformance`  | `auctioneer-bidder / factory_binding_build`                                     | Once in B; its child workspaces retain their own explicit feature controls                                                         |
| `compiler-conformance`  | `auctioneer-bidder / registry_build`                                            | Once in B with `--test-threads=1`; retain its feature-toggle/rebuild sequence                                                      |
| `compiler-conformance`  | `auctioneer-bidder / schema_source_conformance`                                 | Keep the full existing B, S, W0 and W1 executions; source conformance is not the synthetic hermetic row                            |
| `compiler-conformance`  | Core library `schema_artifact_` filter                                          | Keep all four existing secure/development by normal/staged invocations; four dispatched tests each                                 |
| `compiler-conformance`  | Source-conformance `tests::staged_` filter                                      | Keep all four existing secure/development by normal/staged invocations; two dispatched tests each, in addition to full-target runs |
| `compiler-conformance`  | `auctioneer-bidder / catalog_publication`, `auctioneer-config / bidder_storage` | Keep actual WASI secure and development invocations; 2 and 1 dispatched tests respectively per feature selection                   |
| `config-build-failures` | `auctioneer-config / build_failure_harness`                                     | Once with the existing standalone config selection; preserve all hermetic native/WASI child cases                                  |
| `config-build-failures` | `auctioneer-exchange / disclosure_model_coverage`                               | Keep the explicit `--ignored` invocation; do not replace it with an ordinary run that skips it                                     |
| `test`                  | All other emitted Cargo targets                                                 | Keep their existing package/feature scopes, filters and runner environments                                                        |

Move the existing four schema core/metadata scripts intact to their owner;
full normal source runs do not replace the staged combinations. Native
`catalog_publication` and `bidder_storage` executions remain ordinary tests;
WASI execution does not replace them.

Only full integration-target rows are exclusions from broad invocations.
Library-filter rows are additional observations, not permission to exclude a
library or suppress those cases within its ordinary complete test run.

In particular, `registry_join`, `catalog_alloc`, `registry_source_conformance`
and `feature_exports_source_conformance` are not part of the once-only set.
Some inspect the parent process's generated views or `cfg!(feature)` even when
other cases build hermetic children. Retain their current focused and broad
executions until graph equivalence is established case by case. Keep both CA
and CM complete, the named secure config non-harness list, the
`auctioneer-config-schema --features build-support` library invocation, explicit
source-provenance checks, socket/privacy probes, resource/source/documentation
checks and feature-expectation probes. Standalone CLI graphs are not substitutes
for workspace graphs.

Before deduplicating any once-only row, verify from its current implementation
that child features, sources and environment are independent of the removed
parent invocation. If that fails, retain the row's original scopes and revise
this matrix. Count savings only after actual execution; do not extend the
once-only classification to siblings by name.

### Target Selection And Its Cost

Cargo has package exclusions but no integration-target exclusion switch. Do not
exclude whole packages or add required-feature/ignore gates to hide slow tests
from normal local Cargo commands.

If Phase 1 needs target-level partitioning, use a workflow-local Bash/jq wrapper
under `.github/workflows/scripts/test/`. First run the original package/feature
selection with `--all-targets --no-run --message-format=json`. Derive runnable
integration targets from emitted executable artifacts, not guessed feature
rules. The explicit exception table assigns their owner. Execute each
complement through Cargo with the same package/feature selection, together with
libraries, binaries, examples and benches; do not execute artifact paths by
hand or change the dev-dependency graph.

This still compiles every selected target, including those executed elsewhere.
Across isolated jobs it can duplicate compilation. It saves selected repeated
test execution, not necessarily build time. Measure its enumeration cost and
the additional runner setup/cache/build cost before retaining the split. A
wrapper is not justified merely because its selector is correct.

Ownership is not a snapshot of workflow YAML. New ordinary targets stay in the
normal complement. Reject missing/duplicate ownership entries, invalid build
output, or target-name collisions crossing owners that Cargo's selection
syntax cannot express safely. Required-feature targets are included only when
Cargo emitted them; preserve their existing explicit feature jobs. Validate
ownership against an enabling scope, not a disabled scope that correctly emits
no artifact.

Before replacement, exercise tiny real Cargo workspaces covering dev-dependency
feature unification, duplicate ordinary target names, enabled/disabled required
features, exclusions, custom harnesses and library/bin/example/bench execution.
Selection tests validate behavior; do not restore `ci_workflows.rs` or snapshot
workflow YAML/shell text in Rust.

## Phase 2: Deferred Verified Build Reuse

This section records safety requirements, not an approved implementation task.
If profiling does not justify reuse, omit this phase rather than building a
general cache. Reuse only generated build products, never returned measurement
JSON, initialized validators, search caches or guest processes. Keep all five
existing compiler acceptance callers and the additional source-target caller.

Use bounded process-local successful-generation state keyed by schema mode and
synthetic/production inputs. A shared entry owns no file lease or leaked
`TempDir`. Each returned catalog reacquires and holds the existing global
compiler lease for its whole use; do not retain a cache mutex while waiting for
that lease. Preserve safe default/explicit access and release after failures.

Generated sources and synthetic inputs may use a bounded set of named
directories below Cargo's `target/` compiler-output area. A new test process
must establish its own verified generation; directory existence or a saved
on-disk receipt is never readiness. Refusal batches retain fresh private
workspaces because `check_generated_source` rewrites manifests and sources.

### Validation Before Mutation

The current `verify_probe_workspace` seeds the child `Cargo.lock` from the root
and runs Cargo metadata, which can materialize a lockfile. It is a build step,
not a read-only cache validator. Never call it before validating a candidate
cache hit: doing so would erase evidence of changed child inputs.

Under the compiler lease, compare existing generated sources, schema contents
and witnesses, root and resolved child locks, profile/feature declarations,
compiler/vendor identities and toolchain inputs against the process-local
receipt **without writing or running lock-mutating Cargo commands**. Verify all
four stored artifact sizes and digests; membership names alone are insufficient.
Artifact deletion, truncation, same-length corruption or unexpected cached-child
changes refuse reuse, not a repair followed by new trusted hashes.

A deliberate trusted-input change invalidates the generation. Build fresh
candidate inputs, then run lock seeding/metadata/build steps only on that new
candidate, recheck inputs after building and publish state only after every
target succeeds. Failed or partial builds publish nothing. Tests must
distinguish a legitimate input-change rebuild from refusing corrupted outputs.

Keep existing native and Wasmtime commands fresh per observation, including
allocation positive controls, cold initialization, repeated/population cases,
release balance and pre/post guest identity checks. Do not reduce workload,
coalesce measured processes or reuse observations to make timings smaller.

## Acceptance And Decision Gates

1. Phase 1 start was approved on 2026-10-31. This authorizes prerequisite repair,
   profiling and gated ownership work, not Phase 2 or remote publication.
2. Satisfy the measurement gate. Freeze the scope/target/feature/ignored-test
   execution census before any removal. A later newly discovered target is
   assigned explicitly or retained in its original scope, never silently lost.
3. Evaluate Phase 1 independently. Compare at least three successful matched
   before/after runs on the same runner class, toolchain, workload and recorded
   cache conditions. Report median end-to-end critical path, per-job wall time,
   aggregate runner time and dispatch/feature-graph equivalence, including the
   new setup and compile-all costs. Do not claim savings from invocation counts
   or compare a partial failure to a complete pass. If the split does not help,
   revise or abandon it; do not weaken acceptance to preserve the design.
4. Request separate approval for Phase 2 only with measured residual repeated
   build cost. Require hit/miss and independent-key controls, failed/partial
   builds, every input category, all four artifacts' deletion/truncation/
   same-length corruption, read-only lock validation before rewrite, lease
   release, private refusal workspaces and distinct observation children.
5. For implemented phases, preserve test assertions and independent oracles;
   run affected native/compiler/guest tests, strict Clippy, formatting, shell
   behavior checks and actionlint. A job that never ran remains not verified.

No runtime bidder behavior, dependency, resource equation, ceiling, workload,
warning policy, source watch, original evidence byte or readiness deferral
changes. No performance improvement is established by this draft.
