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
requires a full passing acceptance baseline and profiling before partitioning.
