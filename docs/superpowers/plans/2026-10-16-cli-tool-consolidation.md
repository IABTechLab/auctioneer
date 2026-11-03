# CLI Tool Consolidation Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans. Track each step below and verify before committing.

**Goal:** Consolidate operator commands in `auctioneer-cli`, removing two tool-only packages without making archive repair depend on the archive being valid.

**Architecture:** Keep one CLI binary with default `application` features for the existing application/config/qualification commands. Its `--no-default-features` build retains bidder reference inspection, GVL refresh, reference checking and OpenRTB catalogue commands, but has no application/config dependency. Keep `auctioneer-openrtb-catalogue` as a library for config build-time consumers; hosting entry points stay separate.

**Tech Stack:** Existing Rust/Cargo, Clap, Serde/TOML and maintained HTTP clients. Central workspace dependency declarations; no new workspace package or runtime behavior.

Approved command surface:

```text
auctioneer gvl refresh [existing refresh options]
auctioneer reference check --root PATH [--source PATH]
auctioneer openrtb catalogue generate --manifest PATH --source PATH --output PATH
auctioneer openrtb catalogue check --manifest PATH --source PATH --catalogue PATH
```

Keep all imported captures, pin values, byte limits, lock/swap/fsync behavior,
source-review requirements and read-only checks. Preserve the moved tools'
success/refusal exits and Clap parse errors. No legacy binary or subcommand
aliases. Do not run a live archive refresh or invent refreshed evidence.

## Task 1: Prove The New Surface And Dependency Boundary

**Files:** `crates/auctioneer-cli/tests/{help,maintenance_build}.rs` and application-only qualification integration targets.

- [x] Add command-help checks before wiring the new dispatch; observe failure for absent namespaces. Test valid and invalid nested subcommands, and maintenance-only exclusion of application commands.
- [x] Add a child-build regression in an isolated workspace copy with a deliberately corrupt GCL. First prove the copied archive breaks `auctioneer-config`, then build/run the CLI without default features successfully. A GVL retention-argument refusal must occur before network access. Check the actual built dependency graph/output, not a hardcoded feature list. Never mutate the live archive or use the CLI test binary's own target directory for child builds.
- [x] Gate application-only integration targets explicitly with the default `application` feature; preserve their normal default execution. Maintenance tests execute in both CLI modes, and CI runs both modes explicitly.

## Task 2: Move Commands Without Changing Their Domain Behavior

**Files:** New CLI `src/maintenance/{mod.rs,gvl/mod.rs,reference/{mod,integrity,provenance}.rs,openrtb/{mod.rs,catalogue/mod.rs}}`; new CLI `tests/{openrtb_catalogue,reference_check}.rs`; CLI reference capture manifest and README.

- [x] Move refresh implementation/unit tests into `maintenance/gvl`, retaining its validation-before-network ordering and data-root resolution. Expose a Clap subcommand and integer-exit dispatcher; retain colocated tests. Keep this module independent of `auctioneer-config`/core/privacy.
- [x] Move the catalogue executable and its three subprocess tests into CLI. Keep its library and library tests in `auctioneer-openrtb-catalogue`; remove that library package's binary and Clap dependency. Preserve all source/pin/output-alias controls and unit assertions.
- [x] Move reference checking, compiled-in capture pins and its two subprocess tests into CLI. Fix sibling-module paths and input include paths. Preserve the exact capture manifest and captured artifacts; expected results must not be regenerated or derived from Auctioneer.

## Task 3: Wire Features And Remove Redundant Packages

**Files:** Root/CLI/catalogue manifests and lockfile; CLI `src/main.rs`.

- [x] Declare `default = ["application"]` in CLI. Make application/config/qualification-only dependencies optional and enable them only through `application`. Inspect every direct/transitive path to config; conditional dispatch and imports must compile cleanly with and without the feature. Bidder reference commands stay available without it.
- [x] Update the source-ownership audit in `tests/qualification_migration.rs` only as necessary for the explicit enabled `application` gate, with negative controls retaining refusal of unknown/disabled conditions. Do not replace the audit with a blanket feature-condition exemption; it must still prove the required test drivers execute in the application lane.
- [x] Centralize moved chrono/HTTP dependencies without upgrading versions or changing fetch/decoding policy. Wire all three namespaces to their moved implementations. Do not pull hosting runtimes into portable or build-only libraries.
- [x] Delete `auctioneer-refresh-gvl-archive` and `auctioneer-reference-fixtures` package directories/manifests after their contents are moved. Remove workspace members and lock entries. Keep the catalogue build-only library and its consumers, avoiding a config-to-CLI dependency cycle.

## Task 4: Migrate Automation And Current Guidance

**Files:** `.github/workflows/{test,refresh-gvl,check-openrtb-drift}.yml`; core `tests/ci_workflows.rs`; GVL/config build diagnostics; raw JSON fixture allowlist; architecture and relevant operator/spec/fixture documentation.

- [x] Replace executable commands with `cargo run --locked -p auctioneer-cli --no-default-features -- ...`. Add maintenance-only CLI tests/clippy to CI and mutation-sensitive exact-command guards. Default application CLI coverage must remain intact.
- [x] Update repair diagnostics and current docs to the runnable new commands. Historical execution evidence stays historical; add explicit supersession notes where needed instead of rewriting past results. Relocate documentation with moved commands and keep repository links valid.
- [x] Adjust only relocated raw-JSON fixture paths/count owners; do not weaken aggregate budgets or drop fixtures.

## Task 5: Verify, Review, Commit And Push

- [x] Run `cargo test --locked -p auctioneer-cli --no-default-features --all-targets -j 2` with the existing pinned bidder source prerequisite, and clippy in the same mode with `-D warnings`.
- [x] Run default `cargo test --locked -p auctioneer-cli --all-targets -j 2` with pinned source/SDK prerequisites. Freeze CLI binaries and HEAD while qualification tests execute. Confirm all former tool unit/integration tests now execute from CLI.
- [x] Run catalogue library tests, core workflow guards, workspace feature-ful clippy, formatting, raw-JSON gate and `git diff --check`. Check Cargo metadata shows 15 workspace members and no removed binary targets.
- [x] Obtain independent code review of dependency isolation and moved safety invariants. Record actual verification, including any environment limitations; do not claim live network/provider verification.
- [x] Commit all consolidation changes without authorship trailers and push to the existing bidder catalog branch/PR; confirm clean synchronized status.

## Verification Record

Implementation committed and pushed as `faa8298e` on `bidder-catalog`
(PR #21); the worktree and upstream were synchronized afterward.

Verified locally on 2026-10-17 with Rust 1.98.1, the pinned bidder checkout
supplied through `AUCTIONEER__BIDDER__SOURCE_DIR`, and the pinned SDK checkout through
`EDGEZERO_SDK_ROOT` for the default application suite.

| Check                                                                                                                                 | Result                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Default CLI, `cargo test --locked -p auctioneer-cli --all-targets -j 2 -- --quiet`                                                        | 576 passed, zero failed, two existing ignored                         |
| Default CLI, `cargo test --locked -p auctioneer-cli --test maintenance_build -j 2 -- --quiet` after adding the manifest-default assertion | Two passed                                                            |
| Maintenance-only CLI, `cargo test --locked -p auctioneer-cli --no-default-features --all-targets -j 2 -- --quiet`, run separately         | 146 passed, zero failed, zero ignored                                 |
| Catalogue library tests                                                                                                               | 18 passed                                                             |
| Core `ci_workflows` target                                                                                                            | 85 passed, including disabled/weakened maintenance-command controls   |
| Workspace all-targets clippy with `insecure-bidders,conformance-suite` and `-D warnings`                                              | Passed                                                                |
| Maintenance-only all-targets clippy with `-D warnings`                                                                                | Passed                                                                |
| Formatting, raw-JSON budget and rejection fixtures, whitespace checks                                                                 | Passed; raw-JSON count unchanged at 169 across 50 files               |
| Cargo metadata                                                                                                                        | 15 members; catalogue library only; both removed tool packages absent |

All 53 moved unit tests and five moved subprocess tests execute from the CLI.
The capture manifest is byte-identical to its predecessor; no captured
artifact or live archive was refreshed. The corrupt-archive child-build
control passed with both a fresh cache and a reused cache. It uses a locked,
stable copied workspace because build scripts embed `CARGO_MANIFEST_DIR`.

Run the two executable feature-mode suites sequentially: Cargo otherwise
replaces their common `target/debug/auctioneer-cli` path during a concurrent run.
An overlapped verification attempt demonstrated this executable collision;
it is not evidence of a command-dispatch failure.

Independent read-only implementation review found no remaining code defects.
Automation/documentation review found five stale operator READMEs; all five
were corrected and rechecked. Existing GVL deleted-vendor build diagnostics
and the dependency's `proc-macro-error2` future-compatibility warning remain.
This is native tooling verification, not live-provider or actual WASM-target
qualification.
