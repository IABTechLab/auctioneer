# User Sync Final Review Repairs Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** Use `superpowers:subagent-driven-development` or `superpowers:executing-plans`; keep write sets disjoint and run mutations only in temporary source copies.

**Goal:** Close the remaining user-sync review defects and make the real build-failure harness complete with bounded, actionable failures.

**Architecture:** Repair the existing shared URL validators, cookie codec, privacy tests and EdgeZero-backed provider fixtures. Preserve approved privacy policy, immutable reference observations, generated handler routing, and the 4-KiB complete-cookie cutoff. Keep build-script rejection evidence end to end rather than substituting helper-only tests.

**Tech Stack:** Rust, Serde, URL parsing, RustCrypto HMAC, Cargo subprocesses and existing native/WASM verification tools.

Baseline: `6a70ebf`, including peer fixes `afe569a` and `6a70ebf`.
The prior [closure plan](2026-10-13-usersync-review-closure.md) and
[cookie-integrity design](../specs/2026-10-13-usersync-cookie-integrity-design.md)
remain authoritative. This repair does not add a new privacy policy or an
unsigned compatibility path.

## Task 1: Shared URL Safety

**Files:** `crates/auctioneer-config/build_support.rs`, `src/usersync.rs`,
`tests/url_safety.rs`, `crates/auctioneer-core/src/usersync/template.rs` and
`tests/usersync_template.rs`.

- [x] Add rejection tests for trailing-dot localhost and existing denied aliases, with public DNS and public/private IP controls; run them red before modifying classification.
- [x] Apply domain-root-dot classification consistently without widening the exact development loopback allowlist.
- [x] Add build/render equivalence tests for extra scheme slashes and authority backslashes, with valid base paths and ordinary endpoints as positive controls.
- [x] Share lexical validation rather than accepting URL-parser repairs at generation and rejecting them at rendering.
- [x] Verify `cargo test --locked -p auctioneer-config --test url_safety --test usersync_config --test usersync_callback_validation` and `cargo test --locked -p auctioneer-core --test usersync_template`; retain colocated algorithm tests.

## Task 2: Keyring Shape And Independent Crypto Guards

**Files:** `crates/auctioneer-core/src/usersync/signed.rs`,
`tests/usersync_integrity.rs`, `src/usersync/cookie.rs`, and
`crates/auctioneer-runtime/src/aux_work.rs`.

- [x] Reject positional arrays at both root and key-record levels; demonstrate red parser and real-App refusal tests before changing deserialization.
- [x] Enforce the documented object schema while retaining duplicate/unknown-field rejection, key limits, borrowed validation, fixed diagnostics and zeroizing decoded keys.
- [x] Authenticate an unsupported-version prefix with an independent HMAC construction, proving version rejection separately from MAC failure; preserve peer active-key and weak-key tests.
- [x] Replace the redundant budget fixture override with genuinely different complete-cookie caps and independent charge deltas; pin fixed signing scratch separately.
- [x] Pin the approved hard cutoff with lower/equal/higher requested caps and boundary-sized complete fields; retain the documented minimum-size preflight optimization.
- [x] Run signed codec/cookie unit tests, generated-App integrity tests and runtime auxiliary tests; prove version, shape and charge mutants are caught in isolated copies.

## Task 3: Privacy Acceptance And Divergence Evidence

**Files:** `crates/auctioneer-privacy/tests/sync_permissions.rs`,
`tests/sync_reference.rs`, `tests/data/sync/README.md`, a separate reviewed
extraction-divergence artifact, and colocated handler tests if needed.

- [x] Add differing host/account Basic/Full cases whose vendor declarations distinguish the algorithms; assert exact host and candidate decisions.
- [x] Add a valid below-byte-cap TCF with excessive trailing segments; require resource decisions and terminal HTTP resource errors independently of malformed-consent behavior.
- [x] Move the explicit 22-row extraction-difference allowlist into auditable fixture data with exact expected results, rationale and existing design references; do not alter captured reference observations or the seven independently captured divergence cases.
- [x] Validate unique reference-case membership, complete named reasons, design references, and actual inequality with each frozen reference result.
- [x] Retain immutable template reference rows; keep independently varied hostile inputs in behavioral tests, including both GDPR render stages from the peer commit.
- [x] Run all privacy targets and relevant core handler/template tests; replay account-algorithm and segment-classification mutants in temporary copies.

## Task 4: Provider Fixtures And CI Execution

**Files:** `crates/auctioneer-adapter-fastly/tests/manifest_health_stores.rs`,
other Fastly subprocess fixtures where applicable, and
`crates/auctioneer-core/tests/ci_workflows.rs`.

- [x] Strip signing-keyring environment input from unrelated child fixtures and prove isolation with a synthetic inherited sentinel; do not use operator credentials.
- [x] Reject execution-disabling shell overrides at step/job/workflow levels while preserving valid existing shells and exact command, activation and failure enforcement.
- [x] Run native manifest and CI contract tests, with mutations for both Fastly execution steps and credential isolation.

## Task 5: Build Harness Completion And Publication

**Files:** `crates/auctioneer-config/tests/build_failure_harness.rs`,
existing CI selection if necessary, `ISSUES.md`, and the verification record.

- [x] Reproduce the current harness slowdown with unique logs and bounded observation; inspect Cargo/compiler/wrapper activity before changing concurrency or storage.
- [x] Remove the measured bottleneck without suppressing tests, bypassing `build.rs`, loosening rejection checks, or accepting zero executed assertions.
- [x] Bound child execution, drain output without pipe deadlock, and reap children on timeout; add regression tests for the harness mechanism.
- [x] Prove independent fixture paths, forced build-script reevaluation and positive/negative transitions if dependency artifacts are reused; outer and cross-process build outputs must remain isolated.
- [x] Complete the full harness with actual result lines and a recorded fresh-target run; run secure/default config, core/privacy/runtime, applicable adapter tests, formatting, strict Clippy and documentation/fixture gates.
- [x] Review each patch against the spec and for code quality; update `ISSUES.md` and qualification evidence with actual outcomes.
- [x] Commit and push verified repair rounds without attribution trailers. Do not mark an unexecuted or still-stalled harness complete.

Repair commit `b49c823` was pushed to `origin/user-sync`, including the
already-local peer regression commits. The remote branch was checked after
the successful push; no history rewrite was used.

## Verification Outcome

The baseline harness completed 77 tests in 600.86 seconds, explaining a
ten-minute observation cutoff without demonstrating a deadlock. Exclusive
target reuse, bounded children, exact Cargo output selection and six new
regression cases are implemented. Final qualification disables both compiler
wrappers: all 84 cases passed in 472.10 seconds, with fresh child target trees.
The outer target was reused for that final run; an earlier 83-case run also
used a fresh outer target and passed. No failed infrastructure attempt was
counted as successful validator rejection.

Native verification passed 458 config tests and 2632 workspace tests, plus
1559 separately selected secure-default core/privacy/runtime executions and
176 feature-enabled conformance/changed-target executions. Strict workspace
Clippy passed in default and feature-enabled graphs, both deployment-target
checks passed, and the actual secure Axum probe executed and passed. Existing
ignored guest/coverage tests are not claimed as new execution evidence.

Independent review found no remaining behavior defects in Tasks 1-4. Harness
review found two cleanup issues, both corrected before final verification:
departed leaders must not leave workers behind, and daemon compiler wrappers
must not move workers outside the owned group. Target reuse intentionally
requires overlapping owners; serialized fixtures retain immediate cleanup.

See [the verification record](2026-09-30-usersync-verification.md#final-user-sync-review-repairs-october-13)
for exact commands, provenance checks and remaining qualification boundaries.
