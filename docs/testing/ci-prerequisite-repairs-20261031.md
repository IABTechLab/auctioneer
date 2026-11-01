# CI Prerequisite Repairs

Date: 2026-10-31. Status: prerequisite repairs verified locally; no complete
passing CI baseline or performance improvement is claimed. Workflow partitioning
and artifact caching are unchanged.

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
