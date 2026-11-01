# CI Prerequisite Repairs

Initial record: 2026-10-31. Status: prerequisite repairs and the subsequently
approved once-owned workflow partition are implemented. No complete passing CI
baseline or whole-CI performance improvement is claimed; artifact caching remains
deferred. Follow-up sections distinguish local checks from Linux acceptance.

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
