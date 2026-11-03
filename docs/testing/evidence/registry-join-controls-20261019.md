# Compiled Registry Join Validation

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-19
Initial join verification completed: 2026-10-20
Incremental discovery preparation verified: 2026-10-21
Branch: `bidder-catalog`, based on `21eea5d1`
Scope: noninstalled join validation and discovery controls from bidder catalog Tasks 7-8.

## Boundary

`ValidatedRegistryPlan` accepts complete metadata/factory declarations and the
actually active metadata, binding and linked-factory tables. It refuses duplicate
public identities, factories and bindings, missing or dangling bindings,
inconsistent feature predicates or membership, and invalid source-alias ancestry.
Disabled definitions and inactive declarations still participate. Every factory
declaring the same optional feature must agree on actual linked membership,
including factories with no public definition yet.

Validation never calls a constructor, even on success. Its ordered public/base/
source-alias/factory views borrow the supplied static metadata. Independent bases
sharing an implementation retain distinct metadata and ancestry. Construction is
an explicit subsequent operation; callers must fund metadata materialization,
validation/index storage and bidder instances before using this API live.

This adds no process singleton or installed catalog. The serving registry and its
current layout remain unchanged; `registry.rs` moves to `registry/mod.rs` to
follow the repository's module-layout lint. Generated storage bounds, precharge,
one cached typed result, duplicate explicit-insertion protection, consumer error
propagation and removal of the old name match/fallback/clone-and-leak remain open.
No bidder, endpoint, workload limit, coefficient or SDK revision changes here.

## Regression Controls

The initial empty-plan stub compiled and failed all seven initial unit tests.
The generated-view integration target then failed four of five tests against
that stub. An earlier integration attempt had a setup type error; only the
corrected, compiling run counts as RED. All five pass with the implementation.

The sixteen colocated validation tests require exact typed refusals and zero
constructor calls. Positive controls check borrowed pointer identity, independent
public bases, sorted views, distinct constructed instances, reordered declarations,
fully inactive feature views and legal unused factories. Alias controls cover
chains, cycles, missing parents/bases, duplicate/dangling declarations and factory
switching. The real generated-view target checks secure/feature membership and
executes each actual bidder through a capped sink, asserting request URLs,
headers, bodies and distinguishable response behavior without network traffic.

Spec review found that two implementations declaring one feature could have
split membership. The added four-state control reproduced acceptance of an
unused-only partial projection: zero passed, one failed. A complete feature-group
check repairs that case before construction; both mixed states now refuse and
both consistent states pass. The reviewer rechecked the correction statically.

The workflow gate initially failed on the missing exact command (four passed,
two failed under the `workflow` filter). CI now runs `registry_join` beside
`factory_binding_build`. Existing command-mutation controls separately remove
either target and reject skipped, compile-only or failure-ignoring invocations.
The normal feature-enabled workspace command also includes the new target.

## Verification

Rust 1.98.1, locked/offline dependency resolution, two build jobs. Counts are per
invocation rather than disjoint totals; all completed scopes below have zero
failures and zero ignored tests.

| Scope                                                                                   | Result                                                                                    |
| --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Final bidder library and `registry_join`, secure defaults                               | 150 passed: 145 + 5                                                                       |
| Same final targets with `insecure-bidders`                                              | 161 passed: 156 + 5                                                                       |
| Broader secure bidder run before the final feature-group repair                         | 176 passed: 144 library + 24 discovery + 3 feature-snapshot + 5 join                      |
| Core library and complete workflow target                                               | 606 passed: 503 + 103                                                                     |
| Final isolated factory-build target plus generated join                                 | 11 passed: 6 + 5; five intended child refusals, five recoveries, four executable controls |
| Final bidder guest-library checks, three ABIs in both feature states                    | Six passed                                                                                |
| Final workspace all-targets Clippy, `insecure-bidders,conformance-suite`, `-D warnings` | Passed                                                                                    |
| Workspace formatting, diff and raw-JSON gates                                           | Passed; 168 counted fixtures across 50 files                                              |
| Existing roster and batch-rebase checks                                                 | Passed                                                                                    |

The broader discovery run initially failed because sandboxed Unix-socket creation
was prohibited. The same command with local socket permissions passed; no test
was ignored or weakened. The final child-build sequence completed in 64.48 s.
Guest checks prove library compilation only, not guest execution or physical fit.

Native logs are `/private/tmp/bidder-catalog-registry-bidder-final-{secure,insecure}.log`,
`bidder-catalog-registry-bidder-secure.log`, `bidder-catalog-registry-core.log` and
`bidder-catalog-registry-child-build.log` under the same directory. Guest logs use
`bidder-catalog-registry-<ABI>-{secure,insecure}.log` for `wasm32-unknown-unknown`,
`wasm32-wasip1` and `wasm32-wasip2`. RED logs retain the `plan-red`,
`generated-red-corrected`, `group-red` and `workflow-red` suffixes.

Final lint command: `cargo clippy --workspace --all-targets --offline --locked
-j2 --features insecure-bidders,conformance-suite -- -D warnings`, recorded in
`/private/tmp/bidder-catalog-registry-clippy-final.log`. Initial lint errors were repaired
without exemptions. Existing GVL deletion notices and the tracked
`proc-macro-error2` future-compiler warning remain. Spec and quality review
approved only this preparatory slice after the feature-group and evidence-link
corrections; both reviews were static, with the parent running verification.

No complete workspace test run, hosted CI result, deployed-provider execution,
schema qualification, funded live-catalog cutover, resource qualification readiness or bidder catalog completion
is claimed by this record.

## Incremental Discovery Preparation, October 21

Based on `fd8fd21c`, Rust 1.98.1. This increment extends the noninstalled
validation boundary above; it does not complete Task 7 or full Task 8.
Production implementations, metadata, manifests, lockfile, workload limits,
catalog descriptors and SDK pin are unchanged.

`registry_build` copies seven local owner crates and committed config data into
one disposable workspace. The copied source population is 22,370,048 logical
bytes, below its 64-MiB copy ceiling. Git, hidden credentials, tests and build
outputs are excluded. Cargo runs from the scratch workspace root with an
explicit bidder manifest in a different directory. Children are offline,
locked after seeded lock materialization, use two jobs and no incremental or
debug-info storage, and clear inherited override/feature environment. Every
resolved package must retain a reviewed name/version/source/checksum tuple.
The existing factory-binding target shares this infrastructure; its five
feature/forwarding/factory refusals and recoveries remain intact.

The new child binary joins actual generated metadata/declarations/bindings and
factories using `ValidatedRegistryPlan`. It constructs synthetic bidders and
executes their real bounded request sinks, without network I/O. Thirteen
successful stages require independent complete membership, lookup, endpoint,
module-path, body and positive constructor/execution expectations:

- Initial unknown implementation; source-only second factory, then metadata.
- Exact missing-declaration and missing-source refusals, each followed by
  successful restoration without cleaning.
- Nested-helper protocol edit without a metadata change; complete source,
  declaration and metadata rename with the old identity absent.
- Metadata removal while its unused factory remains, then source-only removal.
- A metadata-only public identity sharing an existing factory but retaining its
  own endpoint and public context; the second invocation has literal count two.
- Feature off/on/off, retaining inactive declarations but not active metadata,
  factories or execution. An inactive-module compile error independently guards
  module exclusion; the enabled row has three synthetic executions.

The source-only stages intentionally prevent config dependency changes from
masking missing directory invalidation. No generated-text snapshot substitutes
for executing the rebuilt artifact. Final removal is checked in both lookup
APIs, full artifact sets and the execution list.

Each child has a five-minute watchdog and 256-KiB cap per output stream; each
workspace has a fifteen-minute watchdog. Scratch logical lengths are sampled
once per second and after child cleanup, refusing a sample above 4 GiB. This is
not a continuous physical-disk peak bound: transient unsampled storage, filesystem
allocation and snapshots are not measured. On every exit the runner attempts
group/direct termination and waits for the Cargo child; temporary workspaces
are cleaned up on success or failure. This is not a hostile-descendant isolation
or termination proof.

Parent verification, locked/offline with two jobs:

| Scope                                                                                              | Result                                                                                       |
| -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Existing `factory_binding_build`                                                                   | 8 passed; 15 children, 62.844 s child runtime, 752,750,542-byte sampled logical high-water   |
| New `registry_build`                                                                               | 8 passed; 16 children, 130.580 s child runtime, 696,544,233-byte sampled logical high-water  |
| Restored `registry_build`, after the root-tracking mutation                                        | 8 passed; 16 children, 91.979 s child runtime, 696,544,233-byte sampled logical high-water   |
| Bidder library, discovery, generated join, allocator and feature-snapshot targets, secure defaults | 192 passed: 159 + 24 + 5 + 1 + 3                                                             |
| Same broader bidder targets, `insecure-bidders`                                                    | 203 passed: 170 + 24 + 5 + 1 + 3                                                             |
| Core `ci_workflows`                                                                                | 104 passed; includes exact target, serial execution and narrowing/skip/compile-only refusals |
| Workspace all-targets Clippy, `insecure-bidders,conformance-suite`, `-D warnings`                  | Passed                                                                                       |
| Raw JSON gate and negative controls                                                                | Passed; unchanged 168 fixtures across 50 files                                               |
| Batch-rebase and roster checks                                                                     | Passed; 139 roster members                                                                   |
| Formatting                                                                                         | Workspace and standalone probe-source checks passed                                          |

The shared helper's seven tests execute in both build targets; these counts are
per invocation, not sixteen distinct cases. There are no ignored tests. The
initial empty-execution child failed its literal count-one expectation. A later
fixture-only absent-TOML-key panic was fixed with table insertion before the
complete parent run. The missing workflow step produced one intended failure;
adding the exact step restored all 104 workflow tests. CI also runs these native
targets through existing workspace/all-target invocations.

A parent mutation removed only the discovery root from the production rerun
inventory. The child still compiled and executed the first bidder, then failed
exactly at `source/declaration addition without metadata`: the newly added
factory was absent from both generated and validated factory views. The run
reported zero passed, one failed and seven filtered tests, not a compiler or
infrastructure failure. The source was then restored to its original SHA-256
`535f6e401e493df145fb0dd4633277b21dec312a3fe5292b20ac63e02b29a534`.

Logs: `/private/tmp/bidder-catalog-incremental-build-green.log`,
`bidder-catalog-incremental-ci-missing-step-red.log` and `bidder-catalog-incremental-ci-green.log` under
the same directory; the mutation log is `bidder-catalog-incremental-root-tracking-red.log`.
The clean rerun is `bidder-catalog-incremental-build-restored.log`; broader bidder results
are `bidder-catalog-incremental-bidder-{secure,features}.log`.
Strict lint is recorded in `bidder-catalog-incremental-workspace-clippy-final.log`.
Its initial run correctly rejected helpers missing explicit test-only module
gates and ordinary style issues. The repair uses the repository's existing
test policy, smaller functions, private path storage, checked counters/time/
disk arithmetic and explicit summary reporting. No lint exemption was added;
`TempDir` retains cleanup ownership without a logging-only `Drop` implementation.
The final post-refactor command reran both complete build targets: eight passed
in each, with no failures or ignored tests. Its factory workspace ran fifteen
children in 56.343 s; discovery ran sixteen in 93.624 s. Their sampled logical
high-waters stayed 752,750,542 and 696,544,233 bytes, respectively. The record is
`bidder-catalog-incremental-build-final.log`. The follow-up static review confirmed all
thirteen successful stages, both missing-input refusals and five existing
factory refusal/recovery cases remained intact.
Static spec review required retaining the full serving
lifecycle acceptance as open; subsequent quality review found no concrete
defects. No complete workspace test execution, hosted CI, guest execution,
resource qualification, bidder promotion or resource qualification readiness is inferred.
