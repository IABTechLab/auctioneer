# Standard Test Modules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace test-only `#[path]` wiring throughout the workspace with normal Rust modules and real crate interfaces, without losing coverage or changing serving behavior.

**Architecture:** Unit tests stay beside the implementation. Integration and conformance targets use normal module trees in their owning crate's `tests/` directory, with reusable support grouped by responsibility. Cross-crate consumers use a deliberate shared interface rather than recompiling sibling sources; generated discovery of production bidder implementations remains outside this test-layout refactor.

**Tech Stack:** Rust 1.98.1, Cargo, existing workspace crates and test harnesses.

## Invariants

- Preserve test targets, feature-required lanes, assertions, ignored-test execution, source-versus-synthetic controls, and one allocator per measured executable.
- Counters and the installed allocator must share one state owner; wrappers used for allocation failure injection remain intact.
- No replacement cross-crate `include!`, symlinks, copied implementation code, new workspace crates, relaxed limits, or blanket lint suppression.
- No unnecessary serving API widening. Test-support features must not enable insecure bidders or configuration overrides; host build support must not enter serving dependency graphs.
- Generate Rust through `quote!` and `syn`, including scratch-only probes; formatting remains appropriate for ordinary diagnostics and paths.
- Build-time code shared with runtime belongs to an existing dependency-neutral owner; no dependency cycle or new serving parser/code-generator dependency.
- Generated standalone probes must have self-contained ordinary module trees, not absolute source-path imports.
- Keep historical captured fixtures and qualification evidence byte-identical. Update active source-path gates for moves without relaxing their conditions.

## Tasks

### 1. Baseline And Ownership

- [x] Inventory `#[path]` uses across every crate, including generated probe strings.
- [x] Confirm the clean M7 branch and approved organization.
- [x] Record test target/test-list baselines before moves; distinguish genuine tests from repeated support-module tests.
- [x] Resolve shared test-support and build-model ownership before removing cross-crate imports.

### 2. Ordinary Modules

- [x] Refactor `auctioneer-core` test modules and handler-private unit tests.
- [x] Refactor `auctioneer-exchange` test modules and private scrub controls.
- [x] Refactor `auctioneer-privacy`, including ordinary TCF helper modules and GPP fixtures.
- [x] Refactor adapter test modules, preserving live wire and allocator boundaries.
- [x] Refactor `auctioneer-cli` test/probe modules and resource identity ownership.
- [x] Refactor `auctioneer-bidder` support, conformance entrypoints, bidder-local conformance modules, and temporary schema probe construction.
- [x] Refactor configuration build/test code without importing sibling source files.

### 3. Integration Verification

- [x] Compare test inventories and inspect all target/feature changes for silent skips.
- [x] Run `cargo fmt --all -- --check` and the raw-JSON fixture gates.
- [x] Run affected unit/integration suites under secure defaults and explicit development/conformance features, serialized with `--offline --locked -j2`.
- [x] Run generated schema/native/WASM probe controls after changing their copy/module layout.
- [x] Run strict Clippy for affected targets and check the remaining `#[path]` inventory explicitly.
- [x] Independently review coverage/ownership and code quality; fix findings.

Commit and push after the final verification pass; Git history records the publication identity.

Production generated module discovery can legitimately select source paths; it must not be replaced with a hardcoded bidder registry. Any remaining non-test path selection is reported separately from the completed test cleanup.

## Verification Results

- The full workspace all-target sweep compiled and exercised the moved targets. It found eight stale fixture/build-graph targets, not eight serving defects. Each affected target was repaired and rerun individually; this record does not claim a subsequent fresh whole-workspace green run.
- Raw JSON gates passed with 188 fixtures in 54 files, including the planted negative controls. Roster arithmetic, roster negative controls and batch-rebase controls passed; all 105 CI-workflow guard tests passed.
- Serving dependency inspection retains `config-model` without `build-support`, test helpers, or insecure feature activation. All three actual WASM adapter libraries pass their target-specific `cargo check` commands.
- Native and three WASM schema dispatch probes linked the complete 261-validator inventory in both normal and explicit-cache modes. The 300 source parameter cases and source/catalog conformance targets passed in the workspace sweep.
- The schema build-support library passed 230 tests. Configuration codegen, smoke, baked-default snapshot and disclosure targets passed 2, 74, 16 and 2 tests respectively; the final 104-case configuration child-build harness also passed after the disclosure emitter conversion.
- Secure and explicit-development privacy wire suites each passed six tests, with all seven response/GDPR goldens also passing in each mode. Four owner-contract unit controls reject malformed or unsafe facts. The whole active published cohort participates; Core contains no provider-name branches or output-derived expectations.
- Exchange scrub/composition/allocation targets passed; the ignored exhaustive-model mutation harness also passed. All nine ignored immutable-source checks passed with both source bindings supplied locally.
- Explicit fanout conformance passed 35 Core and 14 tests per WASM adapter; handler conformance passed eight tests. The three adapter envelope proofs retain their independent equations, including the pre-existing correlation-storage term previously omitted by their fixture deltas.
- CLI import, resource and maintenance targets passed 7, 16 and 2 tests. The real CLI/boot parity target passed all seven tests, including 40 fresh boot installations and four traced accounting mutations. Its disposable workspace selects the complete same test-support graph for host and target builds; strict package/feature comparisons remain intact.
- Independent static review confirms all 239 Cargo test targets retain names and required-feature gates. The 16 source-isolation controls passed. Private-source `include!` and handwritten module-path imports are rejected; approved generated includes and production discovery remain.
- A syntax-based comparison found no changed multiline fixture values in the inlined unit-test blocks; moved captures and historical qualification bundles remain byte-identical. The two schema benchmark captures moved into bidder test data without changing bytes.
- Rust emission now uses `quote!`/`syn`; no workload limits, resource coefficients or SDK pins changed. Normal tests retain `allow-expect-in-tests`; AG approved scoped `expect_used` allowances in opt-in fixture helpers, not production code. The final strict workspace all-target Clippy run passed with `-D warnings` after the last fixture repairs.
- The pre-existing supplemental immutable-source CI selection gap is explicitly Open in `ISSUES.md`: local execution is not evidence of CI execution. Browser/live provider qualification and complete M7/R0 readiness remain outside this refactor's verification claims.

## Repair The Published Privacy Oracle

AG requested fixing the recorded wire-fixture mismatch in this round rather than deferring it.

- Replace extrapolation from each owner's unrelated first factory capture with explicit `fixtures/privacy.json` contracts beside the published implementations.
- Scope these authored Auctioneer expectations to the full privacy-context fixture. They are not new upstream captures or observations of the implementation under test.
- Describe publisher-ID substitution, preservation/replacement of the scoped impression extension, and exact root-extension output for absent versus present input extensions. Additive standard-banner response facts cover protocol-owned enrichment without overwriting common type, targeting or metadata assertions.
- Keep original-input presence guards and every identifier/geo denial assertion. Contracts cannot override user, device, or source privacy expectations.
- Use generic compiled test-support APIs to load and apply the facts. Core must not branch on provider names, skip cohort members, or compute expected values by invoking the real adapter.
- Validate malformed/unknown fixture fields and prove absent, empty, and nonempty input-extension cases independently before rerunning the complete secure/default and development-feature wire suites.
