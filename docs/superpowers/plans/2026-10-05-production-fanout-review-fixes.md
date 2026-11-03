# Auction Resource and Bootstrap Review Fixes

> **For agentic workers:** Use test-driven-development and verification-before-completion. Independent tasks may be dispatched in parallel with disjoint file ownership.

**Goal:** Correct the seven confirmed implementation findings from the review of `b1561c0`, and use the latest EdgeZero PR #275 revision requested by the user. Internal and public Auctioneer APIs may change without compatibility shims; retain the intentional warn-mode budget policy.

**Architecture:** Enforce identifier bounds before request data can multiply into bid output. Account for node-heavy request copies and probe output populations explicitly, keep cold probes within their config-work limit, and preserve application-owned logging configuration. Parser refusal classification must depend on an observed refusal, not input text.

**Tech Stack:** Rust, workspace Cargo tests, pinned EdgeZero adapters, allocator regression harnesses.

## Constraints

- Work only in `bounded-fanout`; do not merge or modify later capability branches.
- Keep unit tests alongside implementation; use integration tests for process logging, allocation measurement, and real asynchronous request lifetimes.
- Name tests for behavior, not capability numbers.
- Preserve unsheddable liveness probes and fail-closed privacy/config behavior elsewhere.
- Update only the EdgeZero git revision and the integration it requires; no plugin API or unrelated refactoring.

## Contract Decisions

- Bidder names and alias identifiers, including base names, have a shared 64-byte UTF-8 limit. Oversized request aliases are rejected without echoing the identifier. Pushed host overlays fail validation; invalid baked entries are skipped defensively by the exchange resolver. Baked bidder names are validated during generation.
- Targeting key prefixes remain capped at 20 bytes, and truncated keys do not retain the untruncated allocation.
- Complete status response bodies, including degradation diagnostics, are capped at 8192 UTF-8 bytes. Baked and runtime banners are byte-validated; rendering reserves space for diagnostics before truncating a banner.
- Every actual probe config resolution owns a permit. Cold callers wait for a published verdict or a released slot; cancellation unregisters waiters and releases slots. No uncounted fallback read is retained.
- Aux accounting funds the two distinct populations as `aux + max(aux, PROBE_CONFIG_MIN_PERMITS)`, so disabling aux routes does not remove funding for the probe's minimum config slot.
- Probe output retention is charged independently of config-work capacity, using the platform request population and egress handoff factor, plus one resident verdict cache.
- Application-owned loggers apply independent filters: `BOOT_LOG_TARGET` admits WARN/ERROR and ordinary targets use `resolve_logging_level`. The facade admits the union. Do not temporarily widen ordinary logging or reset the facade below the boot filter.

## Tasks

### 1. Identifier Amplification

Files: `crates/auctioneer-exchange/src/alias.rs`, `crates/auctioneer-exchange/src/targeting/keys.rs`, shared bidder identifier constants as needed.

- [x] Add failing tests for an oversized request alias, bounded diagnostic text, valid boundary-sized aliases, and bounded targeting allocation.
- [x] Reject oversized request aliases before registry resolution or warning construction; apply consistent host-alias bounds and avoid oversized targeting scratch buffers.
- [x] Run focused alias and targeting tests, then verify rejection through the real auction route before construction.

### 2. Node-Heavy Request Copies

Files: `crates/auctioneer-runtime/src/budget.rs`, `crates/auctioneer-bidder/src/bidders/appnexus/mod.rs`, focused allocator integration tests.

- [x] Add a failing budget/allocation regression using sparse nested extension data, not only large strings.
- [x] Charge retained node copies separately from wire-byte copies; remove avoidable adapter copies without changing the outbound wire shape.
- [x] Verify both minimum and narrowed envelopes, checked arithmetic, and adapter transform tests.

### 3. Probe Work and Response Lifetimes

Files: `crates/auctioneer-core/src/handlers/{status,probe_config}.rs`, runtime/baked status validation, focused probe integration tests. Coordinate any budget changes with task 2.

- [x] Add failing tests that hold actual config reads at capacity before the first verdict, and retain responses after config work finishes.
- [x] Share or wait for in-flight cold resolution rather than resolving without a permit.
- [x] Bound baked/runtime probe bodies and explicitly fund the possible output population while preserving probe availability.
- [x] Verify cancellation, degraded/cached verdicts, UTF-8 byte limits, and all target population differences.

### 4. Application-Owned Logging

Files: native adapter bootstrap, shared boot logging helpers, Fastly bootstrap, logging unit/process integration tests.

- [x] Add failing configured-level and critical boot-warning tests.
- [x] Apply Axum's configured logging level while preserving startup warnings.
- [x] Ensure successful Fastly endpoint initialization cannot filter required startup warnings, including ERROR/OFF configuration.
- [x] Verify ordinary logging filtering, bootstrap order, and WASM compilation.

### 5. Parser Error Provenance

Files: `crates/auctioneer-runtime/src/bounded_json.rs`, bidder response classification tests.

- [x] Add a failing wrong-type string test containing the internal RawValue marker.
- [x] Record actual RawValue refusal state in the parse counter; do not infer refusal from input-bearing error text alone.
- [x] Preserve genuine RawValue rejection, node-budget refusal, and syntax/type error codes.

## Final Verification

The earlier decoder, conformance, and compilation results used `5527b58`.
All final checks below must use `dfd439c` after the requested pin update.
The workspace run also exposed two failed-open status scenarios sharing a
cached verdict; their integration test now isolates process-global state.

- [x] Review all patches for scope and interactions, including independent logging-integration review and its three follow-up fixes.
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace --locked --offline`, including all 55 config build-failure harness cases.
- [x] `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
- [x] Strict Fastly WASM Clippy for library and test targets; target-only imports, rustdoc markup, and test-module gating corrected without suppressing lints.
- [x] Fanout conformance suites (111 cases) and the three provider WASM checks; Fastly was checked again after adopting the single-environment dispatch path.
- [x] Real Fastly endpoint logging tests in Viceroy: two host tests driving seven fresh guest runs, including ERROR/OFF/DEBUG and the single missing-runtime-env diagnostic, passed on the final rebuilt WASM artifact.
- [x] Repository fixture, roster, and rebase guard scripts; `git diff --check`.
- [x] Report the changed behavior and verification. Local tests do not establish hosted-provider qualification; changes remain uncommitted and unpushed.

## EdgeZero Pin Update (2026-10-06)

- [x] Read PR #275's current head and compare it with the previous pin.
- [x] Pin all six workspace dependencies and eight resolved packages to `dfd439c613af4f6ed295e71c032881b15bf7247c`; no other lockfile packages changed.
- [x] Adopt the boot log target and shared level resolver. Native uses the existing SimpleLogger dependency with a per-target boot filter; Fastly uses the upstream endpoint filter at the configured ordinary level.
- [x] Handle `FastlyRuntimeConfig.env` and emit its deferred boot diagnostics after installing a backend; apply equivalent filters to the stderr fallback.
- [x] Resolve Fastly runtime overrides once for both health setup and dispatch. Use the documented `App::build` plus `send_with_registries_and_hooks` custom-entrypoint path; keep conversion, registries, admission, routing and egress in EdgeZero.
- [x] Require successful dispatch in process tests and verify that the selected WASM test actually ran. Reproduce the duplicate missing-runtime-env warning before correcting its ownership; add a process regression requiring exactly one diagnostic.
- [x] Update the structural pin assertion and current documentation. Outbound, ingress, config-read, and codec contracts are unchanged by this logging-only upstream commit.
- [x] Re-run all final verification against the new revision; PR #275's head was rechecked as `dfd439c613af4f6ed295e71c032881b15bf7247c` while still open.
