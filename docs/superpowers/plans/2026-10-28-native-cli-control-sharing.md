# Native CLI Control Sharing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Execute the checkboxes in order and preserve the existing harness contracts.

**Goal:** Reuse the existing bounded native process and file controls for bidder catalog's
evidence commands without copying them or making maintenance tools depend on
embedded application data.

**Architecture:** Move the existing generic controls into the `auctioneer-cli`
library and keep qualification-specific orchestration and DTOs in the binary.
Both future evidence commands and existing qualification commands consume the
same explicit library API. This is a prerequisite of the already-approved
[Task 16](2026-10-16-bidder-catalog.md#task-16-evidence-derived-migration-checking),
not evidence-driver completion or permission to certify unfinished work.

**Tech Stack:** Existing Rust 1.98.1, Tokio, futures, Serde, SHA-256, rustix and
`auctioneer-runtime`. No new crate, dependency version, feature flag or executable.

## Scope

Preserve process-group cleanup, interruption/deadline refusal, bounded output,
environment clearing, strict JSON/duplicate-key handling, exclusive publication,
file permissions, fixed diagnostics and wire formats. Do not change historical
evidence, serving code, workloads, ceilings, SDK/toolchain pins or funding.

The complete reviewed foundation operation/count mapping and promoted wave
requirements are not yet available. Their absence remains explicit; this
refactor cannot manufacture passing producers or complete Task 16/17.

## Task 1: Baseline And Source-Closure Red Controls

**Files:**

- Read: `crates/AGENTS.md` and master spec section 3.3.
- Test: `crates/auctioneer-cli/src/qualification/harness/native/system.rs`.
- Test: `crates/auctioneer-cli/src/qualification/harness/providers/preparation.rs`.
- Read: `crates/auctioneer-cli/src/qualification/harness/{mod,io,process}.rs`.

- [x] Run existing process and I/O controls before moving them. Record actual
      counts, not source-reference counts. Run Cargo serially with `-j2` and
      `CARGO_INCREMENTAL=0`.

```sh
cargo test --offline --locked -j2 -p auctioneer-cli --bin auctioneer-cli qualification::harness::process -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-cli --bin auctioneer-cli qualification::harness::io -- --test-threads=2
```

Baseline on 2026-10-28 at `ec11aace`: 14 supervisor tests and 17 I/O tests
passed, zero failed/ignored. These filtered counts are not the full CLI suite.

- [x] Add independent source-closure controls using disposable trees. Mutate
      literal paths `crates/auctioneer-cli/src/native_control/mod.rs`, `io.rs` and
      `process.rs` individually. Both native and provider harness digests must
      change; provider fixture digests must also change. Neighboring unrelated
      Rust files must not enter these narrower harness digests. Preserve the
      existing native golden for its unchanged, old-only fixture.
- [x] Parameterize the provider's current private digest bodies by root and
      executable where necessary for these tests; production wrappers retain
      their existing root/executable selection and canonical payload shapes.
      This is testability of the real callers, not a separate test hasher.
- [x] Run the new controls and observe assertion failures from the omitted
      source namespace before changing inclusion rules. No compile failure or
      changed historical digest table counts as the intended red.

The three `native_control_source` controls compiled and failed their digest
inequality assertions with the namespace still excluded: 0 passed, 3 failed.
The pre-move migration-owner target separately passed all five controls.

## Task 2: Move Controls And Preserve Their Consumers

**Files:**

- Move: `crates/auctioneer-cli/src/qualification/harness/io.rs` to
  `crates/auctioneer-cli/src/native_control/io.rs`.
- Move: `crates/auctioneer-cli/src/qualification/harness/process.rs` to
  `crates/auctioneer-cli/src/native_control/process.rs`.
- Create: `crates/auctioneer-cli/src/native_control/mod.rs`.
- Modify: `crates/auctioneer-cli/src/lib.rs` and `Cargo.toml`.
- Modify: `crates/auctioneer-cli/src/qualification/harness/mod.rs`.
- Modify: both digest owners from Task 1,
  `crates/auctioneer-cli/tests/qualification_migration.rs` and
  `scripts/raw-json-fixtures.allow`.

- [x] Move `HarnessError` and its `Result<T>` alias unchanged into the new
      owner. Keep all existing unit tests inline beside their implementations.
      Do not duplicate or privately recompile either source file.

```rust
// lib.rs
pub mod native_control;

// qualification/harness/mod.rs, replacing local definitions/declarations
pub use auctioneer_cli::native_control::{HarnessError, Result};
use auctioneer_cli::native_control::{io, process};
```

- [x] Export only the existing consumed process/file API. Add concise public
      error/usage documentation required by strict lint; add no blanket lint
      allowances. Child output is bounded, not inherently trusted or redacted.
      Qualification-specific profiles/providers remain in their current owner.
- [x] Preserve the HTTP unit control that observes registered tasks. The
      library dependency does not inherit binary `cfg(test)`: gate the existing
      read-only observer with `cfg(any(test, feature = "test-support"))`, using
      the CLI's existing dev-dependency/feature, not a new feature or private
      source import. Do not remove the HTTP assertion.
- [x] Promote the existing `futures` and pure `auctioneer-runtime` dependencies
      from optional to unconditional and remove only their `dep:` entries from
      `application`. Neither depends on baked config. Keep every other feature
      edge unchanged; no-default maintenance builds must still work.
- [x] Define the workspace-relative `SOURCE_ROOT` constant in `native_control`,
      its new owning module. Include it in native harness selection,
      provider harness inventory and provider `FIXTURE_ROOTS`. Use path-component
      matching rather than a prefix that also admits neighboring directories.
      Preserve each caller's distinct canonical hash format/version.
      Do not add an unused constant to `resource_identity`, which is separately
      compiled by the application build script.
- [x] Update the migration-owner map/resolver for the two moved test families:
      explicit `lib::native_control::{io,process}::tests` owners resolve through
      `src/lib.rs`; other binary/integration owners retain their current roots.
      Preserve every family, named test and rejection control. Add positive and
      negative library-owner controls, without teaching the resolver to follow
      re-exports or private-source imports. Keep the historical diagnostic qualification plan intact.
- [x] Transfer the raw-fixture allowance to the new `io.rs` path with its
      unchanged count **12**, not the naive grep count. Do not reformat or
      re-pin frozen captures.
- [x] Run the red controls again and require green. Deleting new-root inclusion
      from either owner must fail that owner's test, not only its sibling.

## Task 3: Verification And Publish

**Files:** Existing CLI unit/integration targets, `ISSUES.md`, this plan and
the master Task 16 execution note.

- [x] Run the moved library tests under no-default features; then existing
      binary HTTP/native/provider controls under application features. Preserve
      exact error codes, serialized shapes, cleanup and positive controls.
- [x] Run real CLI maintenance and historical evidence consumers. Set the
      existing source-checkout and SDK-root variables where their tests require
      them. Permit only the tests' local loopback fixtures; no live deployments.

```sh
cargo test --offline --locked -j2 -p auctioneer-cli --no-default-features --lib -- --test-threads=2
cargo check --offline --locked -j2 -p auctioneer-cli --no-default-features
cargo test --offline --locked -j2 -p auctioneer-cli --all-targets -- --test-threads=2
cargo clippy --offline --locked -j2 --workspace --all-targets -- -D warnings
cargo clippy --offline --locked -j2 -p auctioneer-cli --no-default-features --all-targets -- -D warnings
cargo test --offline --locked -j2 -p auctioneer-bidder --test test_isolation
cargo fmt --all -- --check
bash scripts/check-raw-json-fixtures.sh
bash scripts/check-raw-json-fixtures-fixtures.sh
```

- [x] Obtain source/spec and independent quality review. Record exact scopes,
      failures and results; do not call a filtered run a complete suite.
- [x] Commit and push the verified refactor as one coherent change including
      all source-hash owners. Do not push an intermediate build that omits the
      moved helpers from an input digest.
- [ ] Continue master Task 16 with real run manifests, exclusive receipts,
      fixed producers and the consuming checker. Missing operation requirements
      or unavailable qualification stay incomplete, never successful defaults.

## Verification Record (2026-10-28)

The existing 14 process and 17 I/O tests remain inline in their new library
owner, with their assertions intact. The first no-application library build
exposed a test-only `futures::poll!` dependency on an indirectly enabled feature.
Replacing its three uses with standard `poll_fn` and the actual task context
preserves single-poll semantics without enabling another feature. No runtime
control body changed. Public documentation, `Default` delegating to `new`,
and narrower visibility for unconsumed helpers complete the library API.

The full library run also exposed an older two-reference expectation. Exercising
all three workflow declarations then revealed the missing supplemental ignore.
That exact-path repair and its guard were verified and pushed separately as
`758731ac`; no snapshot implementation, source pin or historical record changed.

| Execution | Result |
| --- | --- |
| No-application library suite | 35 passed |
| No-application maintenance build | Passed |
| Complete CLI all-target run, 15 targets | 848 passed, 0 failed, 2 existing ignored probes |
| Historical qualification evidence consumer | 5 passed within the complete run |
| Real CLI/boot parity target | 7 passed within the complete run |
| Test-ownership/migration target | 8 passed within the complete run |
| Final digest controls after restoration and lint-only local renaming | 7 passed |
| Strict all-target workspace / no-application CLI Clippy | Both passed |

The two outer-harness ignored probes are still explicitly executed by their
passing environment-overlay parents with isolated variables; no new ignored
test was added. Do not add the evidence/parity/migration rows to the 848 total:
they are named subsets of that one complete invocation. That invocation
preceded only two local variable renames for strict lint, not a behavior or
assertion change; it is not described as a second post-rename full run.

All three namespace-inclusion removals were tested independently after the
move. Each compiled, failed its own digest assertion and left the other two
checks passing (2 passed / 1 failed, exit 101). SHA-256 comparison verified
byte-identical restoration of both affected source files. The restored seven
source/digest controls passed, including the original native golden, provider
payload formats, root independence and neighboring-directory exclusions.

Source/spec and independent quality reviews approve this extraction only.
Workspace source-import/test isolation passed all 16 controls. Formatting,
whitespace and both raw-fixture gates pass, with the unchanged 188 fixtures
across 54 files and the moved I/O allowance still 12. Historical evidence and
profile files have no diff.
Existing GVL deletion notices and the dependency future-incompatibility warning
remain unrelated. No engine, SDK/toolchain or dependency pin, workload, ceiling,
funding coefficient, qualification wire format or historical artifact changed.
This is not Task 16 producer/checker completion, schema qualification, provider
readiness, or bidder catalog/resource qualification closure.
