# CI Prerequisite Repairs

Initial record: 2026-10-31. Status: prerequisite repairs and the subsequently
approved once-owned workflow partition are implemented. No complete passing CI
baseline or whole-CI performance improvement is claimed; artifact caching remains
deferred. Follow-up sections distinguish local checks from Linux acceptance.

The approved 2026-11-01 follow-up also permits ordinary Cargo cache retention
after test failures. That is separate from deferred Phase 2 observation/artifact
reuse; tests and fresh measurements still execute.

## Failed CI Baseline

[Run 41184541480](https://github.com/IABTechLab/auctioneer/actions/runs/41184541480),
revision `f032ce5a`, completed with a failed native test job. The raw job-log API
contains the final results omitted from the earlier formatted-log capture:

- `schema_build_controls`: 301 passed, 0 failed, 387.83 seconds.
- `schema_source_conformance`: 652 passed, 3 failed, 1,166.11 seconds.
- All later steps in that native test job were skipped.

The source failures were an obsolete dependency assertion, an unsupported
native debug relocation, and a null inspection-function pointer. None was a
passing-workflow timing baseline. The 25.77-second outer Cargo build excludes
the Cargo builds launched by test cases.

## Repairs And Controls

The normal-graph test now requires the approved schema runtime in native
serving and excludes it from all three normal WASM graphs. The unchanged test
failed locally on Axum before the repair; the repaired four-graph test passed.

Portable typed ELF fixtures reproduce PIE function slots whose file bytes are
zero and whose real addresses are explicit relative relocations. The inspector
resolves supported ELF64 RELA records, retains defined-code checks, and refuses
missing, duplicate, unsupported or misplaced entries, invalid addends, and
unaccounted loader encodings. Five new controls failed against the old parser;
the complete initial inspection set subsequently passed 17 tests.

A subsequent review found that an unsupported 16-byte TLS-descriptor write
could overlap a record from beyond the inspector's original seven-byte
lookback. Eight added controls produced 18 passed / 7 failed before repair.
The final inspector tracks checked write extents and queries by their maximum
width. Known extents do not authorize resolving unsupported operations; unknown
or variable-sized writes refuse globally. Zero-width no-ops are discarded,
duplicate spans retain the widest end and remain errors, and address overflow
refuses. Both target binaries now pass all 25 inspection cases, including exact
touching/one-byte-overlap boundaries on x86-64 and AArch64. The two-pointer TLS
descriptor writes are documented by the
[x86-64 loader](https://github.com/bminor/glibc/blob/master/sysdeps/x86_64/dl-machine.h)
and [AArch64 loader](https://github.com/bminor/glibc/blob/master/sysdeps/aarch64/dl-machine.h).

Portable DWARF fixtures reproduce the unsupported ELF TLS expression shape.
Both consumers share a guarded reader: unused location expressions may remain
opaque, but any read overlapping their unresolved operands refuses. Supported
relocations still use the existing object library. Four new decoder controls
failed against the old wiring. After repair, all eight new TLS controls passed
in a ten-test filtered run; the complete layout-parser unit set passed 37 tests.
Eleven further reader controls cover operand bounds, invalid TLS targets,
duplicate relocations, partial and crossing reads, split readers, adjacent
ranges and arithmetic overflow. They passed in both complete local targets.

## Local Results And Limits

The complete local invocation, before the final relocation-extent follow-up,
finished successfully with no filters or ignored tests:

| Target                      | Passed | Failed |   Duration |
| --------------------------- | -----: | -----: | ---------: |
| `schema_build_controls`     |    323 |      0 |   440.68 s |
| `schema_source_conformance` |    680 |      0 | 1,142.15 s |

Outer Cargo compilation took 10.10 seconds. Target durations include child
builds, execution and lease waits; they are not compiler CPU measurements.
All three formerly failing cases passed in the source target. The final
relocation-extent follow-up was then verified separately: 25 inspection tests
passed in each target, with 306 and 663 cases respectively filtered out. Do not
represent these filtered reruns as full runs of the final source revision.

Final-source focused checks, all exit 0:

| Selection                          | Synthetic Target | Source Target |
| ---------------------------------- | ---------------: | ------------: |
| `inspection::tests::`              |        25 passed |     25 passed |
| `compiler_layout_`                 |        22 passed |     37 passed |
| `native_dwarf::tests::`            |        14 passed |     14 passed |
| Exact normal dependency-graph case |     Not selected |      1 passed |

The separate `test_isolation` target passed all 16 cases. Strict Clippy on both
repaired targets and `cargo fmt --all --check` passed. Existing GVL
deleted-vendor build warnings and the existing `proc-macro-error2`
future-incompatibility notice remain; warning policy and dependencies were not
changed.

## Native-Only Setup Follow-Up

The [9809d00 CI run](https://github.com/IABTechLab/auctioneer/actions/runs/41255500058)
subsequently completed its schema step with 331 synthetic
passes and 686 source passes / 2 failures. Both failures were 20-minute global
build-lock acquisition timeouts: the explicit cross-target catalog test and
the native engine-layout request. Other cases continued completing, so this
is evidence of contention, not a proven deadlock. The source target took
2,503.90 seconds. It is not a passing baseline.

The user approved building only native probes for the four audited native-only
callers. Existing default/explicit cross-target entrypoints remain all-target;
both complete native/three-WASM link tests, actual-application inspections,
compiler-layout requests, every original assertion and all fresh observations
remain. Exact native-only report checks and two selector controls guard the
new scopes. No cache, lock lifetime, timeout, measurement workload or workflow changes.

One instrumented before/after pair ran the exact source
`compiled_explicit_cache_layouts_have_complete_typed_owner_membership` case:

| Measurement               |       Before |       After |
| ------------------------- | -----------: | ----------: |
| Test wall time            |     138.91 s |     48.23 s |
| Native child build        |  45.865859 s | 46.028249 s |
| Three unused WASM builds  |  89.249309 s |        None |
| Lease hold                | 138.909585 s | 48.221504 s |
| Outer harness compilation |      12.74 s |     24.29 s |
| Whole local invocation    |     152.43 s |     73.23 s |

Both cases passed. Native inventory and cache-layout observations still used
fresh processes. The same child target cache was APFS-cloned from the unchanged
workspace cache before each sample, at the same disposable target path. Parent
compilation is reported separately. Neither sample executes Wasmtime because
this caller never consumed guest results. The selector regression first failed
against all-target selection, then both selection controls passed in both
integration contexts.

This is one local Apple Silicon pair, not an end-to-end CI improvement or a
complete per-operation suite profile. Its observed test reduction is 90.68
seconds; removed guest-build intervals explain 89.249309 seconds. Retaining
the unchanged shared lease leaves admission-timeout risk to check in Linux CI.

The current repair then passed both complete affected targets, with the pinned
Wasmtime available and no filters, ignores or failed cases:

| Target                      | Passed | Duration |
| --------------------------- | -----: | -------: |
| `schema_build_controls`     |    333 |  96.99 s |
| `schema_source_conformance` |    690 | 567.21 s |

Outer compilation took 25.41 seconds. These complete local results retain the
cross-target and real-application checks. They are not a matched full-suite
before/after comparison and do not close the failed Linux baseline.

Diagnostic artifacts remain under
`/private/tmp/auctioneer-ci-profile-9809d00/`: baseline source archive, parent
instrumentation, input/scope patches, before/after logs and `events.jsonl`.
The original revision is `9809d007a57a755c5d81fa42efff9439698036bc`. The temporary
instrumentation was not added to the repository or linked into measured child
programs. Its helper SHA-256 is
`8cc2988a1262c820707edabc2e49f07e46e232c0d9f8e733024a27bc246590c1`;
the event-log SHA-256 is
`86c6a485bfd99459191992b93a311c4f8fe594bd069d7f153d93da8f3f53f545`.

## Platform Distinction

Before the platform-specific repairs, the unchanged real-artifact probes passed
on this Apple Silicon Mac:

- ABI/layout probe: native plus three guest ABIs, 1 test, 431.39 seconds.
- Actual application probe: all four adapters in both feature lanes, 1 test,
  746.66 seconds.

Those local passes did not disprove the Linux CI failures. Synthetic ELF
controls exercise those paths without executing a foreign binary. A fresh Linux
CI run remains necessary. Local runtimes use the repository's pinned tool
versions; an existing external Cargo target directory was reused. These were
diagnostic runs, not matched performance samples.

## Reproduction

Select the pinned toolchain, three WASM targets and pinned Wasmtime described in
[development guidance](development.md). With an existing Cargo target cache:

```sh
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test schema_build_controls --test schema_source_conformance
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test schema_build_controls --test schema_source_conformance inspection::tests::
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test schema_build_controls --test schema_source_conformance compiler_layout_
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test schema_build_controls --test schema_source_conformance native_dwarf::tests::
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test schema_source_conformance \
  tests::normal_application_dependency_graphs_keep_schema_runtime_native_only \
  -- --exact --nocapture
cargo test --locked --offline -j2 -p auctioneer-bidder --test test_isolation
cargo clippy --locked --offline -j2 -p auctioneer-bidder \
  --test schema_build_controls --test schema_source_conformance -- -D warnings
cargo fmt --all --check
```

Publication of these verified repairs to draft PR #1 was authorized on
2026-10-31. This note records local verification only, not a successful full CI
acceptance run. The approved
[implementation plan](../superpowers/plans/2026-10-31-ci-test-partition-phase-one.md)
originally required a full passing acceptance baseline before partitioning.
The later user-approved once-owned revision supersedes that execution gate;
matched successful runs remain necessary before claiming a whole-CI speedup.

## Once-Owned Parallel Revision

The user approved mandatory once-per-PR hermetic owners and parallel independent
experiments on 2026-10-31, and separately approved publication after verification.
No artifact/measurement cache or automatic merge is authorized.

The preceding unsplit run at `34e4ff9`
([41268339684](https://github.com/IABTechLab/auctioneer/actions/runs/41268339684))
completed with 333 synthetic passes in 469.29 s and 688 source passes / two
failures in 2,193.86 s. Both failures were the unchanged 20-minute compiler-lease
acquisition limit. This is not a successful performance baseline.

The revised dispatch contract has one full compiler owner, six independently
owned source experiments (four adapters, private layouts and the selected
probe), and a config rejection owner. All run on every PR. Each application
test retains both feature lanes. Source selection still runs in the original
secure-package, default-workspace and insecure-workspace selections; the four
normal/staged metadata and core combinations remain separate.

Real Cargo behavior controls establish that `test = false` excludes an explicit
owner from `--all-targets` but naming it with `--test` executes it. A second tiny
workspace proves that retaining the original workspace/package selection also
retains dev-dependency feature unification when only the subject's integration
target is selected. Repository lists confirm 27 ordinary bidder integration
targets plus its library and 13 ordinary config targets plus its library stay
in broad execution. The config explicit owner still lists 105 cases, and the
source feature filter lists 34.

The scratch-source copier deliberately omits integration tests. Its new
manifest regression failed once before the fix, then passed after removing
only uncopied `[[test]]` declarations; package, feature, dependency and build
metadata remain equal. The first attempted command selected the wrong owning
binary and dispatched zero cases; it is not counted as red/green verification.
The corrected exact command dispatches one case in `registry_source_conformance`.
The full factory owner then exposed its outdated assumption that native discovery
builds need no production schema planning inputs. Its source-only workspace now
keeps an empty test-directory anchor and uses an explicitly unavailable schema
boundary only in the synthetic copy. The actual discovery, metadata validation
and registry generation functions remain unchanged as parsed Rust. The native
schema initializer refuses, owner/validation hooks return no result, and an
actual child build rejects the staging feature. This fixture cannot qualify
schemas; actual compiler, native resource and application artifact owners retain
the real unchanged schema path. No original factory assertion was removed.

The catalog request census becomes 17 rather than 23 per complete execution of
both schema cohorts. Three native assertion groups now share their respective
all-target owner's build, but every observation still starts a new process.
Refusal probes run last. The identical default-mode reference batch runs once
per cohort; explicit API refusals remain. This is a structural reduction, not
an elapsed-time claim. Actual passing verification is recorded below only after
the corresponding command completes.

The real source-selection runs passed 34 cases in each S/W0/W1 scope, reporting
`actual_cargo_insecure_bidders` as false/true/true respectively. Cargo tree
inspection records the same `regex-automata` feature vector in B/S/W0/W1, so the
parent cache layout comparison is not discarded across a different engine
feature build. This is not a claim that the complete graphs are identical.

A separate Cargo behavior fixture proves broad Clippy skips `test = false`:
an explicit-target-only `cfg(clippy)` compilation sentinel is not compiled by
`--all-targets` but fails the explicit `--test` command. The mandatory compiler
and config owners therefore also run strict Clippy explicitly. This control is
diagnostic, not a workflow snapshot or a new ignored project test.

### Local Results

All results are Apple Silicon local verification with Rust 1.98.1, the pinned
Wasmtime and the existing external Cargo cache, not a matched CI benchmark.

| Executed Group                    |  Passed | Failed | Notes                                                                                     |
| --------------------------------- | ------: | -----: | ----------------------------------------------------------------------------------------- |
| Synthetic compiler owner          |     330 |      0 | Complete target; all native/three-WASM and native observation assertions retained         |
| Source compiler owner             |     684 |      0 | Six independently owned experiments filtered as declared                                  |
| Application artifact tests        |       4 |      0 | All four adapters, each with secure and development artifacts; 460.53 s locally           |
| Source feature views              | 34 each |      0 | S, W0 and W1; actual feature bit false/true/true                                          |
| Factory binding owner             |      17 |      0 | Includes all original negative mutations, same-cache restoration and staging refusal      |
| Incremental registry owner        |      17 |      0 | Includes additions, edits, rename/removal and feature off/on/off through real executables |
| Scratch-workspace helper controls |      16 |      0 | Includes unchanged discovery AST and unavailable synthetic schemas                        |
| Source-import isolation           |      16 |      0 | Whole workspace; no handwritten path/include exemption                                    |

The real source wrapper also rejects a nonexistent exact test (zero dispatched).
Strict explicit-owner Clippy (all four bidder owners plus the registry source
target, and the config harness), workspace Rust formatting, actionlint,
shellcheck, raw-fixture budgeting (200 fixtures / 57 files) and the 139-member
roster check pass. The known GVL deletion warnings and the Cargo
`proc-macro-error2` future-incompatibility notice remain unchanged. The two
unchanged compiler-layout/selected-probe experiments and full config rejection
harness were not re-executed locally in this scheduling change; the config
harness's full 105-case list and explicit strict lint remain checked. Their
mandatory Linux jobs, the staged combinations and complete workspace acceptance
remain required. Local success does not close those CI or hosted/readiness gates.

## Standalone Dependency Follow-Up

The first partitioned run, revision `e0d72aa`
([41276006971](https://github.com/IABTechLab/auctioneer/actions/runs/41276006971)),
completed with 12 of 13 test jobs passing. All separated compiler, application
artifact and config-rejection owners passed. The native job stopped in
`Verify factory and metadata feature ownership`: the feature-export target
reported one pass and three failures. Unlike the preceding run, these were not
compiler-lease timeouts. Its standalone Cargo roots fetched the stock schema
compiler, which rejects the patched `cache = explicit` option. Cargo does not
inherit a path dependency's root workspace patches or lockfile.

The feature-export and sealed-response targets now share explicitly test-only
dependency setup. Each standalone root receives the workspace patch table with
absolute path rebasing and the original reviewed lock. Offline resolution adds
the fixture root; its resulting name/version/source/checksum identities must be
in the reviewed closure, apart from exactly one expected fixture package. Child
builds then run locked and offline. Inherited target and Rust flags are removed
so the fixture manifest still owns its feature selection. No dependency,
production code, schema option, workload, assertion, timeout or workflow changes.

Two feature-input controls failed before repair, then passed after repair. The
module-qualified sealed-response input control also failed before repair and
passed after it; an earlier incorrectly qualified command dispatched zero tests
and is not counted as evidence. Shared lock controls independently reject name,
version, source and checksum changes, an added dependency, and missing, duplicate
or mistyped fixture roots. Existing positive compiler controls, exact compiler
refusals and executable API membership checks remain unchanged.

An adjacent registry check found a Triplelift provenance prose line exceeding
the existing 100-column bound. Rewrapping that line also requires refreshing
its bidder-owned `manifest_sha256`; that derived checksum is the only factory
fixture value changed. Protocol cases and external captured expectations remain
unchanged.

Final local secure/default invocation, all complete targets, no filters or ignores:

```sh
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test feature_exports_source_conformance --test response_body_is_sealed \
  --test registry_join --test catalog_alloc --test registry_source_conformance \
  --test test_isolation -- --test-threads=1
```

| Target                               | Passed | Failed |
| ------------------------------------ | -----: | -----: |
| `feature_exports_source_conformance` |      9 |      0 |
| `response_body_is_sealed`            |      7 |      0 |
| `registry_join`                     |     21 |      0 |
| `catalog_alloc`                     |      5 |      0 |
| `registry_source_conformance`       |     30 |      0 |
| `test_isolation`                    |     16 |      0 |

This is 88 local test executions, not a complete workspace or Linux CI run.
The same complete compiler targets also passed with `--features insecure-bidders`:
nine feature-export and seven sealed-response cases, no filters or ignores.
Strict Clippy for both targets passed in both parent feature configurations;
workspace Rust formatting and `git diff --check` passed. Existing GVL deletion
warnings and the `proc-macro-error2` future-incompatibility notice remain.
Local timings use warm caches and are not a matched whole-CI speedup measurement.
The Linux native job's later steps were skipped; a new complete CI run remains
required.

## Importer Evidence Follow-Up

At `1b13dc3`, [run 41347146065](https://github.com/IABTechLab/auctioneer/actions/runs/41347146065)
passed the previously failing factory/API step. The native job then stopped at
`Verify immutable bidder source inventory`: `bidders_import` reported seven
passes and one failure. The original exact test reproduced locally against the
same pinned source revision (one failure, seven filtered). Importing twice was
deterministic; checking the committed catalog failed because the read-only
projection did not recognize its existing `tests/schema/semantics.toml` files.

Recognition now uses strict typed declarations for the document, rows, expected
counts and optional source-case metadata. The exact filename, directory,
64-KiB file limit and declared schema-owner match remain required. This is
ownership/shape validation, not schema qualification or permission to route.
Source witnesses, retained-byte checks, publication refusal and the evidence
consumers' behavioral assertions remain unchanged. No capture or pin changes.

Two new unit controls failed on positive recognition before repair. All 12
import-projection controls then passed, including the existing drift refusals
and new wrong-owner, misplaced-file, unknown-field, missing-field and type
controls. The full failed CI step then passed locally with the required pinned
source bindings: `bidders_import` 8/0, `bidders_source` 24/0 and `bidders_wave`
9/0, all complete targets with no filters or ignores.

The broader CLI unit run exposed two further real issues and one setup error
(742 passed, three failed, two existing ignores). That initial command omitted
the required source-checkout environment variable, causing the source-pin case
to fail at setup. The other failures were independently traced:

- The patched-lock comparison baseline was sent through current-snapshot
  validation even though both allowed baseline digests identify historical
  records. It now uses the existing frozen validation lane, just like the
  original baseline. Exact raw pins, continuity verification, arithmetic,
  receipts and current runtime-fact checks remain; the test additionally asserts
  that current-audit validation still rejects this historical record.
- The harness identity golden encoded the source path before the Auctioneer
  rename. An independent Python standard-library calculation over the literal
  three-row source inventory (directory markers included), fixed source bytes,
  executable `abc` and protocol version 1 yields
  `db58af025b3a069ac54e58c37a704e511ed383a9ac1557acf102b81cb3025e46`.
  The prior-path control reproduces the previous golden exactly. Only the test
  golden changes; the production checksum algorithm does not.

Strict Clippy on the CLI binary and all three affected integration targets
passed under `-D warnings`. No current source identity, archived capture,
threshold, workload, feature selection, workflow or serving behavior changes.
The final complete CLI unit run, with both required source bindings supplied,
passed 745 cases, failed none and retained two existing ignored child-probe
entrypoints. The latter belong to the process-isolated config-overlay tests;
no ignore or filter was added. The complete command was:

```sh
cargo test --locked --offline -j2 -p auctioneer-cli --bin auctioneer-cli
```

Workspace Rust formatting, `git diff --check` and the raw-JSON fixture gate
(200 fixtures across 57 files) pass. The existing GVL deletion warnings and
`proc-macro-error2` future-incompatibility notice remain unchanged. These are
local native results, not complete Linux workspace acceptance or a whole-CI
performance measurement. A fresh full CI run remains required.

## Bidder Conformance And Grouping Follow-Up

At `14a5fff`, [run 41349649200](https://github.com/IABTechLab/auctioneer/actions/runs/41349649200)
passed the repaired importer step, then failed 13 Triplelift protocol cases.
The earlier metadata wrap updated the factory fixture's manifest checksum but
missed the companion Rust literal. The original conformance subset reproduced
locally with one pass and 13 failures. Only that stale literal changes; the
manifest, protocol cases and external captures remain unchanged.

A complete local secure-default failure-collection run then reached every
target in the bidder, exchange and core package group: 3,763 passed, five
failed and 11 existing ignored cases. Cargo reported three failed targets.
That invocation is not a green result. It exposed these additional repairs:

- Two candidate-decoder depth controls expected admission at 105 containers,
  beyond the existing source limit of 64. Admission now exercises depth 64
  with literal source-node counts (141 and 139); depths 65 and 105 refuse with
  the original wide node budget. The independent tight-node refusals remain.
  Generic and AppNexus retain their original depth-100 chain admission controls.
- Sharethrough's typed-bid test expected an obsolete whole-response failure.
  It now requires a valid sibling to survive the malformed bid, checks the
  exact surviving bid and error, and independently requires all-bad and invalid
  envelope refusal. Production decoding does not change.
- A compiler-manifest test still forbade the runtime dependency already
  approved for controlled-native serving. It now pins native serving and
  optional, nondefault guest staging while retaining the exact compiler
  version, feature list and disabled retrieval. No dependency changes.
- The deployed-registry maximum-cardinality test measured 2,189,096 bytes of
  grouping bookkeeping against its unchanged 2,097,152-byte allowance. Empty
  impression-index vectors grew to four slots on their first insertion.
  Starting them at one slot reduces the measured peak to 1,854,248 bytes with
  the same 262,144-byte input and 13,952 participants. The original 8x factor,
  cardinality, selection caps and clone allowances remain. Existing tests
  retain 1/8/100-impression sweeps and ordered multi-impression materialization.

Complete follow-up targets, without test-name filters:

| Selection                     | Passed | Failed | Existing Ignored |
| ----------------------------- | -----: | -----: | ---------------: |
| `bidder_response_conformance` |     32 |      0 |                0 |
| `schema_compiler_controls`    |    193 |      0 |                0 |
| Exchange `--all-targets`      |    648 |      0 |                1 |
| Three-package secure group   |  3,768 |      0 |               11 |

The final three-package invocation finished with exit 0 after all repairs,
including the Clippy naming corrections. Its total includes the 14 fresh
generated-settings child-test executions, as did the original failing sweep;
it is not a count of unique test functions or of the full CI matrix.

The earlier complete `auctioneer-bidder --lib` run passed 1,177 cases with nine
existing ignores after the checksum repair; the Triplelift conformance subset
contains 14 cases. No ignore, target selector, timeout, workflow, input limit,
resource charge or historical artifact was changed.

```sh
cargo test --locked --offline -j2 -p auctioneer-bidder \
  -p auctioneer-exchange -p auctioneer-core --all-targets --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-bidder \
  --test bidder_response_conformance --test schema_compiler_controls --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-exchange --all-targets --no-fail-fast
```

These are local native results, not a complete passing Linux CI matrix or a
whole-CI performance measurement. Strict Clippy on both affected libraries and
the three repaired integration targets passes under `-D warnings`; workspace
and bidder-owned Rust formatting, `git diff --check` and the raw-JSON fixture
gate (200 fixtures across 57 files) pass. Existing GVL deletion warnings and the
`proc-macro-error2` future-incompatibility notice remain unchanged.

## Compiler Warning-Policy Parity Follow-Up

At `7a90b87`, [run 41354575403](https://github.com/IABTechLab/auctioneer/actions/runs/41354575403)
passed 12 of 13 test jobs. The native job stopped at
`response_body_is_sealed`: six passed and one failed. Its positive core API
control defined but never referenced `inspect`; the Rust setup action supplies
`CARGO_BUILD_WARNINGS=deny`, so that warning rejects the child build. The earlier
complete local package sweep did not set that variable and therefore did not
reproduce CI's warning policy.

The exact failing case reproduced locally before repair: one failure, six
filtered. A separate new runner-policy regression also failed before repair,
showing the child command lacked an explicit warning setting. The positive
control now uses `quote!` and references `inspect` from `main`. The shared
standalone runner always sets the existing CI policy to `deny`; the regression
inspects explicit child overrides rather than relying on an inherited value.
All public-accessor positives and private-field/read-only refusal checks remain.
Production code, dependency pins, compiler diagnostics, thresholds and workflow
selection are unchanged; no lint allowance is added.

Both complete affected targets pass with `CARGO_BUILD_WARNINGS=deny`:

| Target                               | Secure Defaults | Development Feature |
| ------------------------------------ | --------------: | ------------------: |
| `feature_exports_source_conformance` |       10 passed |           10 passed |
| `response_body_is_sealed`            |        8 passed |            8 passed |

There are no filters, failures or ignores in those four target executions.
The development feature is `insecure-bidders`; it does not relax the standalone
child manifests' own feature selection.

```sh
CARGO_BUILD_WARNINGS=deny cargo test --locked --offline -j2 \
  -p auctioneer-bidder --test response_body_is_sealed \
  --test feature_exports_source_conformance --no-fail-fast
CARGO_BUILD_WARNINGS=deny cargo test --locked --offline -j2 \
  -p auctioneer-bidder --features insecure-bidders \
  --test response_body_is_sealed --test feature_exports_source_conformance --no-fail-fast
```

Strict Clippy on both targets passes in both feature lanes under `-D warnings`.
Workspace Rust formatting, `git diff --check` and the raw-JSON fixture gate
(200 fixtures across 57 files) pass. Existing GVL deletion warnings and the
`proc-macro-error2` future-incompatibility notice remain unchanged.

A broader `CARGO_BUILD_WARNINGS=deny cargo test --locked --offline -j2
--workspace --all-targets --no-fail-fast` attempt used the required pinned source
corpus and SDK bindings. It was interrupted during compilation (exit 130) when
free disk space fell to 3.4 GiB; 2.5 GiB remained after shutdown. No tests ran in
that invocation. It is not a passing workspace result. Full Linux acceptance
requires the subsequent CI run.

## Cloudflare Cached-Tool Installation Follow-Up

[Run 41381814843](https://github.com/IABTechLab/auctioneer/actions/runs/41381814843)
failed before packaging: `cargo install worker-build --version 0.8.5 --locked`
refused existing `worker-build` and `worker-codegen` executables. The workflow
caches `~/.cargo/bin/`, not Cargo's installation records, so a restored binary
can exist without the metadata that identifies its package.

The installer now adds `--force`, retaining the version and locked dependency
selection. No cache paths, build flags, packager commands, size ceilings or
acceptance checks change. The one-line command stays inline; no workflow
snapshot test or additional script is introduced.

A dependency-free temporary Cargo package with two binaries was installed,
then only its install records were removed to reproduce the cache-only state.
The original install command failed with exit 101 naming both existing
binaries. Adding `--force` succeeded with exit 0 and restored installation
records for both binaries at the same fixture version. The fixture is not the
vendor tool and does not establish actual worker packaging or size acceptance;
the next Cloudflare CI run remains its owner.

## WASM Compiler-Fixture Dependency Follow-Up

The warning-policy repair in `1c6af43` introduced `quote!` into a fixture that
all three guest jobs compile, but left `quote` in native-only dev-dependencies.
The superseded test workflow was cancelled overall while its three WASM lint
jobs had genuine failures; the current Spin job reproduced the same `E0433`.
The exact Spin all-target lint command also failed locally before this repair.

The existing workspace-managed `quote` dependency now appears in common
dev-dependencies instead of the native-only test table. Its build-dependency
declaration, version and features do not change. No target is excluded, no
test is ignored, and no workflow command or warning policy is relaxed.

The original bidder `cargo clippy --all-targets` commands pass for
`wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`, each with
`--locked --offline -j2` and `-- -D warnings`. These lint compilations do not
execute guest tests. Native fixture targets pass 10 and eight cases under both
secure defaults and `insecure-bidders`; the latter run also passes all 16
source-isolation cases. No tests are filtered or ignored in those native runs.

Before/after `cargo tree --edges normal` outputs compare byte-identically for
the native host and all three guest targets. `Cargo.lock` is unchanged. Workspace
Rust formatting and `git diff --check` pass. Existing GVL deletion warnings and
the `proc-macro-error2` future-incompatibility notice remain. These results do
not substitute for the subsequent complete Linux CI matrix.

## Native Accounting And Remaining Compiler Owners

At `b7b882d`, [run 41386614042](https://github.com/IABTechLab/auctioneer/actions/runs/41386614042)
passed every separated compiler/artifact/config owner. All three guest lint
jobs, vendor packaging and formatting also passed. The native workspace run
stopped at three Axum accounting assertions: 20 passed, three failed. The
preceding secure bidder/exchange/core scope passed.

The rename lengthened the baked cookie name and family by four bytes each.
The serialized host/account tuple is now 1,592 bytes, not 1,584. Existing
accounting correctly charges the extra eight bytes at 128 bytes per input byte
per admitted auction: 1,024 per auction and 2,048 across the two-auction row.
Only stale test references change. A new inline control varies those strings
against shorter synthetic names and pins both serialized sizes and
all three accounting deltas. The original 4-GiB ceiling, 256-MiB headroom,
128-MiB auxiliary allowance, workload and production equations remain unchanged.

The same Linux log explains part of the repeated compilation. In the secure
scope, `generated_settings` took 135.45 s and `insecure_bidders_snapshot` took
154.63 s; their parent compilation took 1 min 44 s. The settings suite creates
one fresh compiler tree and the export suite creates three. Its three roughly
150-second fixture builds overlapped, so their durations must not be added as
450 seconds of wall time. The workspace stopped before reaching either suite
again; the repetitions discussed here are those in a complete planned matrix,
not twelve fresh builds observed in this failed run.

These two suites explicitly choose child features and clear inherited
environment. Their input contract is independent of the three parent scopes.
AG approved giving each one mandatory execution in the existing compiler job.
Both now have `[[test]] test = false` and explicit test/strict-Clippy commands;
all 19 outer cases per suite, the 14-case settings child and four fresh build
trees remain. Feature-sensitive targets stay in their existing scopes. These
ownership changes alter no cache/reuse mechanism, child fixture, assertion or
limit.

The owner's summary checker now requires every expected report to be a
nonzero-test success with no failures, ignores or unexpected filters. Settings
must provide both parent and child summaries. The existing exact child-name
reconciliation and six independently owned source exclusions remain intact.
Both broad workspace commands gain `--no-fail-fast`: later targets execute,
but any failed target still fails the job. This does not make later workflow
steps run after a failed command.

Initial local verification reproduced the original three failures. Adding the
byte-growth control first produced 21 passes and the same three failures;
correcting the references then produced 24 passes with no failures or filters.
The sibling isolated boot target independently reproduced the stale 1,584-byte
assertion (two passed, one failed). Its three related references receive the
same correction; the final complete Axum run passes all 24 unit and three boot
integration cases. The adjacent auxiliary-floor comment now matches the actual
58,647,552-byte floor and 75,570,176-byte reservation slack. Frozen historical
snapshots remain unchanged.

Both new explicit owner suites pass completely: settings 19 outer plus 14
child cases, exports 19 outer cases; zero failures, ignores or filtered tests.
All child compiler trees were fresh. The updated report checker accepts these
actual logs and passes 13 synthetic success/refusal controls, including missing,
extra, failed-child, zero-pass, ignored and incorrectly filtered reports.

A dependency-free real Cargo workspace confirms that `test = false` omits its
target from `--all-targets` while explicit `--test` executes it. Current-repo
Cargo metadata independently reports exactly seven explicitly owned integration
targets. A separate two-target failure probe exits 101 in both modes: without
`--no-fail-fast` it stops at the first target; with the flag the second target
executes successfully and the overall invocation still fails. These are Cargo
selection/exit controls, not a complete local Auctioneer workspace run.

Strict Clippy passes for Axum's library/boot targets and both newly explicit
compiler owners under `-D warnings`. Workspace Rust formatting, Bash syntax,
ShellCheck, actionlint and the raw-JSON fixture gate (200 fixtures in 57 files)
pass. Source-import isolation passes all 16 cases with no filters or ignores.
`Cargo.lock`, tool pins, the two relocated suites and their child-build
helper are unchanged. Existing GVL deletion warnings and the
`proc-macro-error2` future-incompatibility notice remain.

Local checks use the existing native target cache with `--locked --offline
-j2` and `CARGO_BUILD_WARNINGS=deny`; child workspaces keep their own fresh
targets. The complete workspace and all existing compiler/artifact owners were
not rerun locally because disk space is limited. The next Linux CI run remains
the full acceptance owner. No matched complete CI before/after measurement is
available, so no end-to-end speedup is claimed.

## Parallel Lanes And Complete Failure Collection

At `6b8674f`, [run 41393299498](https://github.com/IABTechLab/auctioneer/actions/runs/41393299498)
passed all other PR checks. Native acceptance failed after 34:25 and compiler
acceptance passed after 27:33. Both moved compiler suites executed exactly once;
the secure group took 6:39 instead of the preceding observed 10:35. These are
different source/cache conditions, not a matched speedup measurement. The
default workspace continued for 10:12 after its first failure, exposing all six
failed targets, including CLI work that earlier runs never reached.

| Failing Target | Root Cause And Repair |
| --- | --- |
| `native_schema_resources` | The insecure feature adds a legacy schema. Independently counted expected totals are 3 bindings, 33 authored, 105 stress, 489 evaluations; secure remains 2/25/70/360. Limits and allocation observations are unchanged. |
| CLI reference units, `reference_check` | A physical directory walk treated ignored CI reference clones as owned Go tooling. Git roots now use tracked plus nonignored untracked paths; non-Git staged roots retain the complete walk. A real CLI negative control proves inherited Git repository/index variables cannot redirect the audit. |
| `bidders_evidence` | Strict artifact hashing rejected the running executable's normal Cargo hardlink. Only OS-selected self-identification uses the existing bounded link-permitting hasher; supplied evidence retains strict hashing. |
| `bidders_resources` | Native catalog reservations were incorrectly expected to be zero. Tests pin the shipped 16-MiB resident, 8-MiB initialization and 512-KiB scratch charges; guests remain zero here. |
| `resource_boot_parity` | The scratch AST selector still expected an explicit `map_err` removed from the real resolver. Its needle now matches the current typed statement, with a fast current-source regression before the application build. |

A dependency-free probe in the already-installed Rust 1.98.1 Linux image,
with networking disabled, reproduced Cargo's two-link executable. The real
CLI hardlink regression first failed with `bidder_evidence_binding`, then passed.
The owned-source regression first rejected ignored external Go; the repaired
audit passes that case and still rejects every owned Go form and a tracked file
inside an ignored directory. A separate redirection regression first produced
a false pass with `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE` set to an empty
repository; the production Git command now clears inherited environment using
the existing metadata-command convention. Tooling and privacy caps did not move.

The compiler script previously spent 22:14 serially in six full targets.
Mandatory schema/settings/exports/discovery lanes retain their exact owners,
prerequisites, filters, nested-report checks, strict lint and fresh children.
Native secure/W0/W1/standalone-CLI lanes keep the original selections intact;
no CLI exclusion or inferred feature equivalence is introduced. Matrices do not
fail fast. The existing ordinary Cargo path set uses split restore/save actions,
lane-specific keys and legacy fallback prefixes. Failed jobs can retain compiled
products; cancellation, missing primary keys and exact hits do not save. Test
failure remains failure, and no observation/result caching is introduced.

Disposable structured-YAML comparison preserves all 36 native acceptance
commands, environments, flags and lane-relative order, plus triggers and five
unrelated jobs. A Cargo-command stub preserves the compiler default sequence,
reconciles exactly six test/lint owners across lanes, checks eight failing-command
propagations, and rejects an unknown lane before Cargo. These are orchestration
checks, not Rust acceptance. The 13 existing summary success/refusal controls
still pass. Hosted cache retention and complete end-to-end timing require CI.

The initial broad local evidence baseline was deliberately interrupted with exit
130 while it was still running expensive application-copy producers. It does
not count as a complete result. Focused failing-branch regressions replace that
diagnostic run; the full CI target selections and unique assertions remain.
Final scoped results are recorded below after verification.

The full parity run first reached the repaired injection point, then refused
the fixture's `/var` temporary target path on macOS as a symlink ancestor. Its
fresh workspace now uses the canonical temporary parent, matching the other
owned-source harnesses. The staging guard is unchanged. Local Rust tooling also
emitted a `rust-objcopy` LLVM-loading warning; it was not the build failure's
cause. Neither run is recorded as passing parity acceptance.

After canonicalization, the complete parity target passes all eight cases with
no failures, filters or ignores, including the real CLI/boot executables and
all corruption controls (236.86 s locally). The cold schema target passes once
under secure defaults and once with `insecure-bidders`; each process preserves
its cold allocator state and every original reservation. Reference units pass
19 cases, the complete real reference CLI passes three, real evidence-begin
controls pass two (including the hard-linked executable), and the exact failing
resource-report case passes one. Filtered runs are not full CLI/workspace runs.

After final lint cleanup, reference units pass all 19 selected cases again and
source isolation passes all 16 cases without filters or ignores. All seven
selected fast parity-harness controls pass after cleanup; the
earlier complete eight-case parity run remains the application-build evidence.
Strict Clippy passes for the CLI library/binary and all five changed integration owners,
and for bidder's cold-schema and source-isolation owners, under `-D warnings`.
The existing artifact-I/O group passes 18 selected cases, including supplied-file
hardlink, symlink and size refusals. No new lint allowance was added.

Final orchestration probes, actionlint, ShellCheck and Bash syntax pass.
Rust formatting, raw-JSON budgets, documentation rebase and roster gates pass;
the roster enumerates all 139 members. Existing GVL deletion warnings and the
`proc-macro-error2` future-incompatibility notice remain. Tool pins, `Cargo.lock`,
captured evidence and resource ceilings are unchanged. This verifies the scoped
repairs locally, not every CLI target or a complete Linux acceptance run.
