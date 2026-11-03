# Final Review Corrections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Update the checkboxes as evidence is collected.

**Goal:** Repair all six findings in the final bounded fanout review and give every external qualification item an owner and executable exit criterion.

**Architecture:** Keep the generated EdgeZero App and shared handlers. Reserve four probe response slots independently of boot, auction, aux and static-response gates; bind each permit to the existing terminal-egress completion. Fund that constant population, not an assumed provider ingress ceiling. Matched non-body handlers drain under the SDK body cap/read deadline before executing handler work. Bidder response inspection exposes comparison, not extractable bytes. Cloudflare funds all three currently retained handoff buffers.

**Tech Stack:** Rust, the pinned EdgeZero SDK, existing atomic gates, bounded JSON, Cargo unit/integration/compile-fail tests.

## Code Repairs

- [x] Probe retention: add failing generated-App saturation/terminal/drop/recovery tests in `crates/auctioneer-core/tests/ingress_admission.rs`; reuse `EgressGate` in `host_primitives/gates.rs`, add a probe lease marker/permit in `ingress_lease.rs`, and admit through that pool in `ingress.rs`. Add a shared runtime permit constant and change `budget.rs` to fund it independently of platform population. Preserve pre-boot probes and isolation from other classes.
- [x] Cloudflare copies: add a failing envelope test for factor three, then correct `crates/auctioneer-adapter-cloudflare/src/envelope.rs`. Record the two Rust buffers plus JavaScript copy; do not claim hosted backpressure measurements.
- [x] Identity bounds: add co-located split tests for both explicit bidder maps, 64/65-byte ASCII and UTF-8 boundaries, and arbitrary legacy extension keys. Preflight every impression's borrowed identities before whole-request serialization or parameter copying. Allocation regressions in `sparse_request_copies.rs` verify that a later invalid impression prevents copying earlier valid parameter trees too.
- [x] Response seal: extend `crates/auctioneer-bidder/tests/response_body_is_sealed.rs` with borrowed getter/copy/rebudget probes; replace raw-byte borrowing with equality comparison in `types.rs`, updating conformance and Fastly correlation assertions without exposing another raw accessor.
- [x] Matched body caps: test exact/over-limit declared and streamed bodies, expired/pending reads, cancellation and configuration-work suppression. Update `handlers/aux_egress.rs` to use SDK bounded body reads and category-only error rendering. Probe and aux markers are valid; auction markers still report incorrect wiring.
- [x] FPD merge: add co-located missing/null/scalar-target nested-deletion regressions in `split.rs`; recursively normalize non-object targets to empty objects before applying object patches. Preserve array replacement and root scalar behavior.

## Verification

- [x] Observe each regression fail before its repair and pass afterwards.
- [x] Run focused exchange, bidder, runtime, core ingress/egress/config tests.
- [x] Run real-wire transport and all four fanout conformance suites.
- [x] Run workspace formatting, strict Clippy, repository guards and supported WASM checks.
- [x] Review the integrated changes for scoped ownership and lifecycle errors.
- [x] Record completed evidence and remaining external requirements without claiming hosted qualification.

## Remaining Acceptance Work

**2026-10-07 update:** The SDK pin now consumes the Q1/Q3 implementation
follow-up and Q4 qualification tooling from PR #275. See
[the current evidence boundaries](2026-10-07-edgezero-bounded-ingress-config-update.md)
for implemented native/config controls versus remaining Spin and hosted evidence.
The table below records the requirements at the earlier review revision, not a
claim that the newer implementation is absent.

These are open tasks, not waived requirements. None authorizes changing the SDK checkout or claiming an unexecuted production check.

| ID  | Project Owner                  | Gate                                 | Required Evidence                                                                                                                                                                                                                                             |
| --- | ------------------------------ | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1  | EdgeZero Axum adapter          | Before networked publisher pilot     | Configurable connection population, actual HTTP/1 parser bounds and stalled-header deadlines; excess incomplete connections cannot grow tasks/memory without bound, and disconnect/deadline returns capacity. Model HTTP/2 only when enabled.                 |
| Q2  | Auctioneer deployment              | Before publisher pilot               | Exact revision/SDK/target identity, warm serving baseline and maximum-shaped load including GVL cold parses, measured peak memory and latency, tuned limits, rollback procedure. An over-ceiling boot warning is not qualification.                           |
| Q3  | EdgeZero config-store adapters | Before each target's publisher pilot | Streamed allocation limits or an enforced/funded backend-value maximum, covering full backend materialization before extraction rejects an oversized envelope.                                                                                                |
| Q4  | EdgeZero provider adapters     | diagnostic qualification target qualification              | Hosted slow readers, cancellation, disconnect and deadline tests establish exactly-once completion/resource cleanup; report host handoff separately from actual client delivery. Raw ingress accounting remains unknown until supported by provider evidence. |

For Q1/Q3/Q4, keep EdgeZero issues separate from Auctioneer deployment/test tasks. Qualification artifacts must identify the tested commit, SDK pin, target and capability level. An unavailable hosted environment leaves its task open and blocks that target's qualification, not unrelated implementation.

## Completed Local Acceptance

- [x] Q5, Auctioneer transport tests: all 11 `wave_transport` tests and three cancellation tests pass over real loopback TCP. Five new cases establish overlapping siblings with distinct deadlines and original-slot correlation, exact compressed/decoded gzip caps, independent encoded/decoded overflow rejection and a clean HTTP EOF with a missing gzip trailer. Each new case checks full gate recovery with another healthy wave. Late batch-driver failure coverage remains in the existing `outbound/wave.rs` unit tests; malformed gzip never publishes a partial successful response.
- [x] Q6, Auctioneer config observability: `config_degradation_warning.rs` calls production store extraction with a classified backend failure, captures the permissive fallback warning, and asserts backend error detail is not exposed. `cargo test -p auctioneer-core --test config_degradation_warning --locked --offline` passes. A targeted `cargo mutants` run for `degrade_to_default` passes its unmodified baseline and catches the warning-removal mutation (one tested, one caught; no timeout or unviable result).

The process-global logger and allocator measurements remain in isolated integration binaries; ordinary behavioral unit tests remain beside their implementation. No new test is named after a capability.

The combined native run additionally exposed a process-global logging test conflict: capturing loggers raise the facade to TRACE, but two boot-sink tests assumed its default level. Those two tests now live in `boot_log_filter.rs`, isolated from capturing unit tests; their assertions and production logging behavior are unchanged. The pure configured-backend test remains beside `boot_log.rs`.

## Final Verification Run

On 2026-10-07, the focused command below exited successfully: 238 core library tests, 111 exchange library tests, 104 runtime library tests, 69 selected core integration tests (including both isolated boot-filter assertions), and eight exchange allocation tests. The bidder library's 55 tests and both response-seal compile-fail harness tests also pass. Strict workspace Clippy was rerun after the logging-test isolation change and exited successfully.

```sh
cargo test -q -p auctioneer-core -p auctioneer-exchange -p auctioneer-runtime --lib \
  --test boot_log_filter \
  --test ingress_admission --test probe_output_before_boot \
  --test aux_request_body_bounds --test config_degradation_warning \
  --test sparse_request_copies \
  --test aux_route_write_deadline --test aux_routes_wrapped --test egress_binding \
  --test probe_before_boot --test probe_pending_reads \
  --test node_budget_on_the_auction_path --test runtime_config_failclosed \
  --locked --offline -j 2
cargo clippy -q --workspace --all-targets --locked --offline -- -D warnings
```

Evidence applies to the working-tree repairs based on Auctioneer `ac9fcab282867d2fc513cc5eb53acad5e530bc7b` with EdgeZero pinned to `91f4af38821de3454dafff75803ea6fea65efb8b`. It is not a claim that the base commit alone contains these repairs. Q1 through Q4 above remain open qualification work; no hosted measurement, provider guarantee, push or merge was performed by this repair run.

## Regression Evidence

All six repairs have observed red/green coverage. In the additional identity-ordering check, rejecting a request with a later 65-byte bidder key allocated 275,028 bytes (modern map) or 275,666 bytes (typed map) before the repair. Both now pass a 4096-byte rejection-allocation bound while their valid input fixtures remain below the 256 KiB request cap. This test detects copying earlier valid impressions as well as copying the rejected parameters.

The generated-App probe tests cover four retained outputs, a bodyless fifth-request abort, release/recovery, pre-bootstrap admission and every terminal egress outcome. The non-body route tests cover status, both catalog paths and GVL health, including 4096/4097 boundaries across buffered, declared-length, chunked and host-managed bodies. Pending-read timeout coverage models the adapter's wakeup/error signal; it does not assert an SDK self-waking timer or hosted egress guarantee.

Native fanout conformance passes all four binaries (30 core, 27 each Cloudflare/Fastly/Spin). Supported WASM checks pass for Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1` and Spin `wasm32-wasip2`, including feature-enabled test compilation. These are local build and behavior checks, not hosted certification.

`cargo clippy --workspace --all-targets --locked --offline -- -D warnings` passes on the integrated repair. Workspace formatting, diff whitespace, raw-JSON fixture budgets, batch-API documentation and roster checks pass. The roster guard's negative fixtures also pass: missing, duplicate, unexpanded and arithmetic-mismatch cases are rejected, while a missing input fails as a script error rather than a successful gate.

Independent read-only review found the initial identity check ran after request-wide serialization; the allocation-ordering follow-up fixes that issue, and a second review found no remaining issue. A separate review of the body-wrapper and TCP/logging test changes found no additional issue. Wall-clock TCP timing on loaded hosts remains a test-environment risk.

Two broad workspace reruns were stopped because test-binary launches were excessively slow and redundant runs delayed focused verification. They are not recorded as passing. The final verification scope is the affected library units, selected ingress/egress/config/allocation/compile-fail integrations, real TCP tests, four feature-enabled fanout conformance binaries, strict workspace Clippy, repository guards and supported WASM checks.
