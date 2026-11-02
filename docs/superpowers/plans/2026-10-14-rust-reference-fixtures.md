# Rust Reference Fixture Tooling Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or
> superpowers:executing-plans for the bounded tasks below.

**Goal:** Remove the Go tooling without weakening independently captured evidence.

**Architecture:** A native Rust workspace utility verifies immutable captured
artifacts and optional upstream source fingerprints. Production crates remain
unchanged. Rust-authored hostile template controls are separate from captures.

**Tech Stack:** Rust, existing workspace Serde/SHA-256/CLI dependencies, Cargo.

## Task 1: Capture Integrity Utility

Files: root `Cargo.toml`/`Cargo.lock`, new `crates/auctioneer-reference-fixtures/`.

- [x] Inventory every output of the retiring generators and pin its current
      SHA-256 in a reviewed manifest. Do not change captured outputs.
- [x] Write failing tests for changed/missing evidence, duplicate IDs, invalid
      provenance, unsafe paths, and forbidden Go programs/harnesses.
- [x] Implement `validate --root` and read-only `check-capture --root
--capture-root`, with optional source-fingerprint validation.
- [x] Run `cargo test --offline -p auctioneer-reference-fixtures --all-targets` and
      `cargo clippy --offline -p auctioneer-reference-fixtures --all-targets -- -D warnings`.

## Task 2: Retire Repository Go Tooling

Files: GPP `data/reference/`, privacy `tests/data/privacy/reference/`, usersync
`tests/fixtures/usersync/reference/`, and their current fixture READMEs.

- [x] Remove `.go`, `.go.txt`, `go.mod` and `go.sum` tooling; retain source
      attribution, capture metadata, captured outputs and licenses.
- [x] Replace current reproduction commands with Rust validation/staged-capture
      checks and document external recapture. Do not rewrite historical test claims.
- [x] Run the Rust utility against the actual worktree and a changed scratch
      capture. Run the existing reference-consuming tests.

## Task 3: Close Confirmed Review Gaps

Files: `usersync/cookie.rs`, `usersync_handler_alloc.rs`,
`usersync_template.rs`, `usersync_callback_validation.rs`, fixture documentation.

- [x] Add an allocation-measured preflight rejection test and show it fails
      with the early branch disabled; preserve the final complete-cookie bound.
- [x] Replace the redundant signing-charge literal with an independently
      derived full-envelope assertion.
- [x] Add a Rust-authored adversarial five-macro input matrix without modifying
      captured reference data; mutation-check both splice points.
- [x] Replace callback rejection `is_err()` assertions with exact error values.
- [x] Execute the already-present `is_json_map` negative guard and document
      source presence rather than assuming commit ancestry proves absence.

## Task 4: Verify And Publish

- [x] Review the complete diff against the approved boundary; check that no
      production dependencies or feature unification were added by the utility.
- [x] Run affected tests, strict lint, formatting and fixture gates; update
      `ISSUES.md` and the M5 verification record with actual results and limitations.
- [x] Commit the completed repair, including documentation, and push M5.

Implementation and verification were published in `ef55516` on `m5-usersync`
on 2026-10-14. The verification record includes the repaired final-review
negative control and the explicit source-archive qualification limit.

# Slim Checker Follow-Up

AG approved option 1 on 2026-10-14. Apply the updated ownership boundary in
`docs/superpowers/specs/2026-10-14-rust-reference-fixtures-design.md` on the
existing M5 worktree. This supersedes the original generic-schema and two-command
implementation above, not its immutable-capture or Rust-only requirements.

- [x] Update `crates/auctioneer-reference-fixtures/tests/cli.rs`: staged captures use `validate --root`; prove the old command is rejected and staged pin files cannot override compiled-in pins. Run the explicit old-command test red before removing it.
- [x] Keep artifact counts in owning tests: core reference corpus counts; privacy group counts, unique scrub names and ordered valid fold pairs; compiler-typed GPP tuple count, unique headers and ordered IDs. Run these tests against the real frozen captures.
- [x] Replace utility `schema.rs` with `provenance.rs` containing only revision/toolchain/source-call checks and their direct negative controls. Remove operation schemas and GPP syntax parsing. In `integrity.rs`, use simple JSON provenance dispatch; keep bounded reads, hash/size pins, source checking and attribution.
- [x] Use pin manifest version 2 with only paths/digests/byte counts; preserve those tuples against the prior manifest. Remove `syn` from the utility's dependencies and mechanically regenerate the workspace lock offline. Keep the single `validate` command and remove `check-capture` with no alias.
- [x] Update current README commands, the design, `ISSUES.md`, and verification record. Preserve historical verification results as historical. Record source/test line reduction and dropped CLI schema promises explicitly.
- [x] Run utility/consumer suites, CLI negative controls, retained-guard mutations, strict lint, formatting and raw-fixture gates. Independently review the scoped diff.

Publication: commit and push all reviewed changes together on `m5-usersync`.

## Consumer Ownership Follow-Up

Feedback against `0418dae` identified gaps in the owning consumers, not a
reason to restore the deleted utility schema engine.

- [x] Pin exact input/expected keys for all 21 choice rows and four ordering rows, with operation counts and expected output types.
- [x] Pin execution counts in both real cookie drivers and reject unknown reference cookie operations.
- [x] Pin the source-selection-only label for all 36 selection rows in their real behavior driver.
- [x] Correct the duplicate-UID test's layer/name and exact config diagnostic; retain the private callback unit controls.
- [x] Recheck the current allocation preflight and flattened-map guard, using source/tests rather than commit ancestry.
- [x] Record the route flake as unreproduced, not fixed; add idle-gate diagnostics without changing process-global production state.

Verification and publication follow the same reviewed M5 repair boundary.
The outstanding route investigation is tracked in `ISSUES.md`, separately
from the controlled-clock determinism improvement.
