# CI Test Partition And Artifact Reuse

Status: On 2026-10-31 the user approved mandatory once-per-PR owners for hermetic
checks, preservation of feature-sensitive checks in normal scopes, and parallel
adapter artifact checks. The approved revision below supersedes the earlier
full-source-rerun and passing-baseline-before-partition policies. In-process
artifact caching remains deferred and unapproved.

## Approved Once-Owned Revision

The prerequisite repairs and native-only setup fix are committed. Both complete
schema targets pass locally, but Linux CI still spends excessive time in the
schema step; an earlier run also exhausted the existing lease-acquisition
deadline. Do not claim a passing Linux baseline or a measured full-CI speedup.
The user approved implementing the ownership change without waiting for those
redundant executions to finish. This is not permission to ignore a new failure.

Use Cargo's declarative `[[test]] test = false` for `schema_build_controls`,
`schema_source_conformance`, `factory_binding_build`, `registry_build` and
config's `build_failure_harness`. An explicit `cargo test --test NAME` remains
available locally and mandatory in CI. No required-feature gate, `#[ignore]`,
custom executable dispatcher or test-result/artifact cache is introduced.
The same setting excludes those targets from broad Clippy selection, so their
mandatory owners also invoke strict Clippy explicitly.

Hermetic checks execute once per PR with their existing explicit child feature
matrices. Normal B/S/W0/W1 feature graphs are not assumed interchangeable:
source tests matching `schema_selection` execute explicitly in S, W0 and W1,
and all four existing secure/development normal/staged metadata and core probes
remain. These observe the actual parent process's generated publication view.
Other ordinary targets, including feature-sensitive registry tests, retain their
current package and feature selections. CLI CA/CM coverage remains intact.

The compiler owner executes full synthetic controls and source conformance,
except the source experiments assigned their own unconditional jobs: compiler
private-layout facts, the selected source-bound native/guest probe, and actual
application artifact inspection. The latter becomes four owner-local tests, one
per adapter; each still checks both secure and development artifacts. Matrix
jobs run independently, with no `needs`, path filters or optional acceptance.
All owners preserve their required tools, source inputs and exact checks.
Cache prefixes are owner-specific; a restored cache is never a passing result.

Within each schema cohort, default and explicit full-target owners share their
already-built catalogs with the corresponding native assertion groups. This
reduces catalog build requests from 23 to 17 across synthetic and source suites.
It is not a speedup estimate or permission to reuse observation processes.
Every allocation/cold/reuse/release observation still starts its own child.
The source-only sibling/callback storage proof keeps its own native catalog.
Exact full-target membership remains checked. Native helpers require one native
report inside that set. Manifest-mutating refusal batches run last; the identical
default-mode reference-refusal batch runs once per cohort, while explicit-mode
API refusals remain separate. No independent oracle is removed.

Config rejection and the ignored disclosure-model mutation test get their own
mandatory owner. Remove the duplicate secure `disclosure_codegen` invocation,
not the secure-default test itself. Preserve workflow triggers, feature lanes,
resource/workload limits, source watches and guest acceptance.

Verify real Cargo default-versus-explicit dispatch using a tiny disposable
workspace and actual repository target lists. Compare case ownership before and
after, then execute all changed owners, meaningful source feature scopes,
strict Clippy, formatting and actionlint. Counts may decrease only where
assertion groups were moved into an existing owner, not where assertions were
lost. Keep the original baseline observations below as history. Performance
claims still require matched successful CI runs. Publication to draft PR #1 was
separately approved after local verification on 2026-10-31.

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

### Original Baseline Gate (Superseded For Ownership)

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

The approved once-owned revision supersedes this before-partition gate. These
observations remain prerequisites for a measured performance claim, not a reason
to keep redundant execution. Do not invent a passing baseline or speedup.

## Alternatives And Scope

### Approved Native-Only Probe Setup

On 2026-10-31 the user first approved native-only probe setup. That fix is
committed. The later once-owned revision above supersedes its separate-caller
arrangement without authorizing Phase 2 caching.

Use an explicit native-only/all-target selection in the existing catalog
builder. Keep the existing default/explicit all-target entrypoints unchanged
for both `all_schema_*_dispatch_links_on_native_and_three_wasm_targets` tests.
The native-only callers are:

- `compiled_explicit_cache_layouts_have_complete_typed_owner_membership`
- `native_generated_callbacks_have_isolated_cold_and_repeat_allocation_reports`
- `explicit_cache_measurements_separate_construction_growth_reuse_and_release`
- Source-only `every_compiled_schema_cache_layout_funds_sibling_and_callback_storage`

Preserve all their existing assertions and add an exact `native`-only report
membership check to guard against reintroducing unrelated builds. Preserve
the shared lease lifetime, fresh measurement subprocesses, private refusal
workspaces, workload sizes, source/artifact verification and all resource
limits. Keep the full real-application and compiler-layout experiments intact.
No generated products or observations are cached by this change.

The structural catalog-build census becomes 11 instead of 20 requests in the
synthetic binary, and 12 instead of 24 in the source binary: 23 instead of 44
for one execution of both targets. This is not a count of clean compilations
and not a speedup estimate. Compare the same native-only test with unchanged
assertions under temporary parent instrumentation before/after; label outer
compilation separately, preserve cache conditions, and make no full-CI timing
claim from this local microprofile. Full scope/feature/guest acceptance remains
required.

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

The YAML job IDs remain stable; their lane-qualified check names make ownership
visible. Preserve unconditional independent owners:

- `test`: native/workspace acceptance in four mandatory feature-scope lanes.
- `compiler-conformance`: compiler/schema/guest acceptance in four mandatory lanes.
- `config-build-failures`: hermetic configuration rejection and the explicit
  ignored disclosure-model check.
- Matrix owners for the source compiler-layout experiment, selected linked
  probe and each of four real application artifacts (two feature lanes each).

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

### Approved Parallel Lanes And Cache Retention

On 2026-11-01 AG approved the following scheduling revision after run
`41393299498`: native acceptance took 34:25 and compiler acceptance 27:33.
The native result includes later CLI work exposed by `--no-fail-fast`, so it
is not comparable with the earlier fail-fast duration as a speedup/regression.

| Job | Lane | Exclusive Work |
| --- | --- | --- |
| `test` | `secure` | Existing audits, source/provenance checks, secure package scopes and explicit feature controls |
| `test` | `workspace-default` | The unchanged complete W0 command and its source-selection check |
| `test` | `workspace-insecure` | The unchanged complete W1 command, source selection, named config scope and explicit socket checks |
| `test` | `cli` | Standalone CA, CM, maintenance lint and browser shutdown checks |
| `compiler-conformance` | `schema` | Both schema targets, WASI publication/storage and all four core/metadata scripts |
| `compiler-conformance` | `settings` | Complete generated-settings target and its 14-case child |
| `compiler-conformance` | `exports` | Complete synthetic export target with three isolated fixtures |
| `compiler-conformance` | `discovery` | Factory-binding target followed by the registry rebuild sequence |

Both matrices use `fail-fast: false`. Commands, flags, environments, prerequisites
and order within a lane remain unchanged. In particular, W0/W1 still select the
whole workspace: removing CLI and approximating its unified feature graph is
not approved. The compiler script defaults to the complete serial `all` mode;
an explicit lane runs only its owners and their strict Clippy checks. Preserve
fresh measurement processes and each incremental mutation/restoration sequence.

The two matrix jobs split ordinary Cargo restore/save actions with lane-specific
keys and their previous prefixes as fallbacks. Save after success or failure,
but not cancellation, only when a primary key exists and was not an exact hit.
The path set is unchanged. Saving compiled products never changes a failing
result or authorizes qualification. This is not Phase 2 result/observation reuse.
Do not claim matched end-to-end improvement until complete hosted runs measure it.

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
library filter. This is the current exhaustive exception set, not permission
to skip all tests in the named packages.

The 2026-11-01 approved follow-up adds `generated_settings` and
`insecure_bidders_snapshot` after a source audit: their child commands clear
inherited environment and select their own feature modes. Keep all 19 outer
cases per suite, the settings child's 14 named cases, each separate workspace,
and fresh child execution. Parent feature views remain ordinary tests.

| Owner                   | Target / Filter                                                                 | Required Execution                                                                                                                 |
| ----------------------- | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `test`                  | `auctioneer-macros / conformance`                                               | Keep the existing standalone macro `--all-targets` command and ordinary scopes                                                     |
| `compiler-conformance`  | `auctioneer-bidder / schema_build_controls`                                     | Once in B; synthetic hermetic controls retain all native/three-WASM links and fresh observations                                   |
| `compiler-conformance`  | `auctioneer-bidder / factory_binding_build`                                     | Once in B; its child workspaces retain their own explicit feature controls                                                         |
| `compiler-conformance`  | `auctioneer-bidder / registry_build`                                            | Once in B with `--test-threads=1`; retain its feature-toggle/rebuild sequence                                                      |
| `compiler-conformance`  | `auctioneer-bidder / generated_settings`                                        | Once in B; retain all outer cases, the complete 14-case child and allocation isolation; validate both success summaries          |
| `compiler-conformance`  | `auctioneer-bidder / insecure_bidders_snapshot`                                 | Once in B; retain ungated success, disabled-symbol refusal and enabled-symbol success in their three separate fixtures           |
| `compiler-conformance`  | `auctioneer-bidder / schema_source_conformance`                                 | Once in B, except experiments with independent mandatory owners; retain `schema_selection` in S, W0 and W1                         |
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

Only the seven explicit manifest targets are removed from broad invocations.
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

Use Cargo's `test = false` plus explicit `--test` owners. The real disposable
workspace confirms ordinary unit/integration tests still run in `--all-targets`,
the once-owned target does not, and naming it explicitly executes it. Verify
the repository's emitted target lists as well. There is no custom complement
selector to implement or qualify. New ordinary targets stay in broad execution;
do not exclude whole packages, add required features or add ignores.

Filters apply only to the explicitly named source target, never to a whole
workspace invocation that also contains custom harnesses. The full source owner
skips exactly the experiments executed by mandatory matrix jobs. Feature-view
invocations retain their original package/feature scope. Check their actual
dispatch, and keep the CLI's two exact source-test names and evidence contracts
unchanged. Do not restore `ci_workflows.rs` or snapshot workflow YAML/shell text.

Separate jobs can duplicate dependency compilation, checkout and setup work.
Report that cost and matched successful wall time before claiming a speedup.

## Phase 2: Deferred Verified Build Reuse

This section records safety requirements, not an approved implementation task.
If profiling does not justify reuse, omit this phase rather than building a
general cache. Reuse only generated build products, never returned measurement
JSON, initialized validators, search caches or guest processes. Keep all current
compiler owners and every independent native observation group.

Use bounded process-local successful-generation state keyed by schema mode,
synthetic/production inputs and requested build-target scope. A native-only
generation cannot satisfy an all-target request. A shared entry owns no file lease or leaked
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
receipt **without writing or running lock-mutating Cargo commands**. Verify the
exact requested artifact set and every stored size/digest: one native artifact
for native-only state or all four artifacts for all-target state. Membership
names alone are insufficient.
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

1. Phase 1 start and the once-owned revision were approved on 2026-10-31.
   They do not authorize Phase 2. Publication of locally verified ownership
   changes to draft PR #1 was separately approved.
2. Freeze the scope/target/feature/ignored-test
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
   builds, every input category, independent target-scope keys, refusal to widen
   native-only state into all-target state, all requested artifacts' deletion/
   truncation/same-length corruption, read-only lock validation before rewrite, lease
   release, private refusal workspaces and distinct observation children.
5. For implemented phases, preserve test assertions and independent oracles;
   run affected native/compiler/guest tests, strict Clippy, formatting, shell
   behavior checks and actionlint. A job that never ran remains not verified.

No runtime bidder behavior, dependency, resource equation, ceiling, workload,
warning policy, source watch, original evidence byte or readiness deferral
changes. No whole-CI performance improvement is claimed before matched runs.
