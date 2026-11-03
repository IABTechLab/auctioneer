# Rust Qualification Harnesses Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Replace both JavaScript diagnostic qualification harnesses and their non-browser fixtures/tests with behavior-preserving Rust commands in `auctioneer-cli`.

**Architecture:** Reuse the current qualification evaluator and schemas. Split common bounded process/IO/HTTP infrastructure, native resource orchestration and provider preparation/offline controls into CLI-local modules. Keep real fixture child processes and fail-closed cleanup; no new crate or live-provider execution.

**Tech Stack:** Rust, Tokio, existing serde/TOML/SHA-256/gzip/HTTP libraries, safe Unix process-signal APIs, existing Cargo/Git/OS meter/SDK tool boundaries.

**Spec:** [Rust qualification harnesses](../specs/2026-10-15-rust-qualification-harness-design.md).

---

## File Map

- Modify `Cargo.toml`, `Cargo.lock`, `crates/auctioneer-cli/Cargo.toml`: centrally declared native-only tooling dependencies; no deployed guest dependency changes.
- Modify `crates/auctioneer-cli/src/qualification/mod.rs`: route native/providers/private fixture commands through the existing sanitized qualification command boundary.
- Create `crates/auctioneer-cli/src/qualification/harness/mod.rs`, `io.rs`, `process.rs`, `http.rs`, `fixture/`, `meter.rs`: bounded shared infrastructure and colocated unit controls.
- Create `crates/auctioneer-cli/src/qualification/harness/native/`: candidate/catalog/geometry/profile inputs, admission/GVL/config observations and actual native runner.
- Create `crates/auctioneer-cli/src/qualification/harness/providers/`: typed offline evaluation, preparation/build/source receipts, Spin/Fastly handoffs, SDK verification and recovery controls.
- Create `crates/auctioneer-cli/tests/qualification_harness.rs` and narrowly scoped integration support as needed: real CLI, fixture/process/socket and migration completeness controls.
- Modify `.github/workflows/test.yml` and `crates/auctioneer-core/tests/ci_workflows.rs`: exact Cargo harness checks, no Node/yq dependency for non-browser controls.
- Update current `docs/testing/{resource-qualification,provider-handlers}.md`, diagnostic qualification spec/plan and `ISSUES.md`; retain dated capture records unchanged.
- Delete the five replaced `.mjs` files only after coverage owners pass. Keep browser scripts/tests.

## Task 1: Freeze Contracts and Porting Inventory

- [x] Read both scripts, provider fixture and all tests, including dynamically generated cases; record each test family's Rust owner and required refusal category.
- [x] Review this design/plan against the current source and user-approved migration boundary. Resolve review findings without changing diagnostic qualification scope or criteria.
- [x] Commit and push the migration design, plan and initial inventory before source implementation.

## Task 2: Bounded IO and Supervised Processes

- [x] Add red tests for byte/node/depth/file limits, structural identity, private writes and digest behavior.
- [x] Add real child red tests for ordinary/early exit, output flood, timeout, SIGINT/SIGTERM, signal resistance and a successful wrapper leaving a descendant.
- [x] Implement shared fixed-category errors, bounded IO and safe process groups with explicit cancellation/cleanup ownership. Use stdin/stdout JSON only for private fixture protocol.
- [x] Verify each failure is behavioral, not a compile error; run positive controls and strict scoped lint.
- [x] Commit and push this verified common-infrastructure task.

## Task 3: Typed Provider Controls and Recovery

- [x] Port the fourteen ordered offline case types, exact identity/features/environment checks and all negative mutations into colocated Rust tests.
- [x] Implement strict bounded serde inputs and fixed safe output; successful offline controls must still report hosted pending, qualification not evaluated and readiness false.
- [x] Port recovery state/fence tests: failed/unobserved injection, interruption, ignored abort, stalled completion, late write, failed restore and separate cleanup deadlines.
- [x] Implement cancellation plus explicit termination/completion fences; no provider-native writer or arbitrary command runner is added.
- [x] Verify CLI exit codes/redaction with malformed/oversized inputs and canaries, then commit/push. Native controls additionally pin prepared=0, completed nonqualification/preflight refusals/deadline=1, invalid input=2 and handled signals=130/143; use the actual `auctioneer-cli` executable name.

## Task 4: Provider Build and Artifact Handoffs

- [x] Port origin validation, production metadata isolation, source/fixture freeze, ignored/untracked inputs and precise Cargo-output exclusion tests.
- [x] Implement structured Rust TOML preparation and exact Cargo-emitted feature/target/artifact receipts; reject stale/mismatched/symlink artifacts.
- [x] Port Spin handoff, complete Fastly bundle/member/archive/digest tests and explicit pinned SDK verifier execution. An invalid explicit SDK root must fail.
- [x] Implement public typed handoff commands and the safe external SDK verification boundary; never deploy or access credentials.
- [x] Verify all source/fixture mutation and release refusal cases, then commit/push.

## Task 5: Bounded Fixture Processes and HTTP

- [x] Port actual loopback HTTP refusal, timeout, cap, failure, canary and controlled-bid tests before replacing fixture behavior.
- [x] Implement separate native/provider fixture child modes with capped records, private control channel, exact case IDs and no HTTP control routes.
- [x] Preserve optional provider TLS and bounded connection/header/body/time/request populations using existing Rust protocol libraries.
- [x] Preserve native configure/release/stats/coverage semantics, exact request geometry, gzip FEXTRA, final-wave stalls and per-ID active/seen bounds.
- [x] Verify malformed/unknown controls, independent fixture counters, cancellation and process teardown, then commit/push.

## Task 6: Native Inputs, Geometry and Measurement

- [x] Port exact request byte/node/impression shapes, separate FPD/debug geometry, gzip encoded/decoded shapes, debug references and candidate/catalog validators.
- [x] Reuse existing typed exports/profile/report/evaluator and checked debug caps; bind actual sanitized SDK environment and all development features.
- [x] Port Linux/macOS RSS/high-water units, meter ownership, process tree discovery, sampling caps and cleanup failures; unsupported platforms refuse.
- [x] Port immutable executable/source checks, typed snapshot installation and A/B/corrupt/restored observations.
- [x] Verify geometry and real schema acceptance/negative controls, then commit/push.

## Task 7: Native Admission, GVL and Load Runner

- [x] Port two-admission oracle negatives, real contemporaneous GVL held-work proof, ordinary ID/call/decode reconciliation and debug bounds.
- [x] Implement bounded concurrent load with all-offered histogram populations, status-first rejection semantics and unchanged scenario/repetition schedule.
- [x] Implement the whole native runner, separate absolute run/cleanup deadlines, cold/warm/scenario windows and safe artifacts/reports.
- [x] Run protocol controls that reject empty/fake/no-decode ordinary success and serialized GVL work; verify held work drains on every exit.
- [x] Run the opt-in frozen-native control against a known frozen executable, then commit/push.

## Task 8: CI, Documentation and JavaScript Removal

- [x] Replace exact CI Node harness command with guarded Cargo integration controls, preserving no-failure-ignore, feature and environment protections.
- [x] Remove `yq` where only these harnesses required it; retain Node/browser and external platform-tool requirements without broad unrelated CI changes.
- [x] Add a Rust migration ownership check asserting every old control family has an executed owner, not just a count or source-string claim.
- [x] Delete both old harnesses, their tests and provider fixture; update all current invocation/link paths and operator JSON examples.
- [x] Keep historical captures/commands unchanged; run formatter, current CLI/bidder/exchange/core guards and strict lint.
- [x] Review the complete migration independently, fix findings, update ISSUES/ledger, commit and push.

## Task 9: Fresh Rust Qualification and Final Verification

- [x] Build actual release executables with the same scoped insecure fixture policy; use no competing builds during measured traffic.
- [x] Prepare and compare the new digest-bound profile against the existing 45-row criteria. Commit/push the new predeclaration before traffic.
- [x] Run all 45 rows through the Rust harness. Preserve failures and diagnose without relaxing criteria. Retain only safe bounded evidence and hashes.
- [x] Independently review identities, actual ordinary dispatch/decode/GVL overlap, lifecycle evidence and local-vs-overall outcomes.
- [x] Run workspace/secure-default/feature/WASM/CI scopes proportionate to dependency changes and all new Rust harness controls, plus preserved browser checks.
- [x] Record exact commands/results/ignores and publisher pilot limits, then commit/push the final verified round and confirm clean/synchronized Git state.

## Task 10: Review Boundary Repairs

User-approved review scope: fix stale end-of-work RSS, duplicate provider JSON
fields, and archive expansion before SDK delegation, including the peer-review
coverage and documentation follow-ups. No deployed auction behavior, acceptance
threshold, SDK pin or historical capture is changed.

- [x] Write and run failing colocated tests for a fresh post-completion RSS sample,
  independent sampling limits, failure and cancellation; implement the bounded
  final sample and rerun the real service control.
- [x] Write and run failing raw-JSON tests for duplicate decoded keys at every
  nesting level, including ignored fields and escaped spelling; reject them
  before constructing a Value while retaining byte/node/depth bounds.
- [x] Add real CLI negative controls for duplicate authorization/identity fields,
  with valid and single-value denial controls.
- [x] Write and run failing archive tests proving expanded byte/member limits
  apply before SDK lookup or execution. Use the CLI-only Rust tar dependency,
  capped streaming gzip input and existing release/path/schema checks.
- [x] Add independent expanded-member, aggregate, count, malformed-archive and
  positive SDK controls; do not extract untrusted files in the preflight.
- [x] Exercise output sync failure, align evaluated JSON's trailing newline with
  harness output, and isolate the CLI override-feature test.
- [x] Document the evaluator's declared-schedule/missing-evidence invariant and
  retain defensive fixture-protocol checks with explicit ownership comments.
- [x] Run CLI/all-target, secure native, conformance, focused Axum and strict lint
  checks; review the integrated patch, update ISSUES and the execution record,
  commit and push. Live provider and deployment qualification remain publisher pilot.

## Execution Record

### Boundary Repair

The approved final-review repairs and ten-item peer reconciliation are implemented
and verified. Native all-target tests passed 2,923 with four ignored markers;
four conformance binaries passed 77 and Axum transport/profile tests passed 52.
Strict all-target CLI Clippy and formatting/fixture guards passed. The source
fixes, intentional canonical PAX subset, failures, negative controls and exact
scope are in the [boundary verification record](../../testing/evidence/diagnostics-qualification-verification-20261015.md#qualification-boundary-repair).
No threshold, SDK pin or retained capture changed. Live provider acceptance and
new deployment/resource qualification remain separate publisher pilot work.

### Post-Cutover Guard Review

The follow-up qualification review is closed with test-only changes: independently
exercised latency, unexpected-evidence, capability and call-ceiling refusals;
literal feature-state expectations; exact ordinary call widths; and a Rust
integration owner for all retained evidence indexes and profile bindings.
Seventeen distinct guard mutations fail behaviorally in an isolated source copy.
The final CLI matrix passes 378 tests, config passes 42 in each feature state,
CI guards pass 75, and strict CLI/config lint passes in both modes. See the
[follow-up verification record](../../testing/evidence/diagnostics-qualification-verification-20261015.md#qualification-guard-follow-up)
for commands, retained failures, exact scopes and the historical digest limitation.
No criteria, runtime behavior, SDK pin or retained capture changed; this is not
a new measured or live-provider qualification.

### Porting Inventory

The pre-migration suites have 75 native controls (one explicit frozen-artifact
opt-in) and 117 provider controls, including generated refusal rows. These are
behavior owners, not a requirement to preserve test-framework row counts.

| Existing control family | Rust owner | Required assertions |
| --- | --- | --- |
| JSON/file/output limits and key identity | harness/io.rs | exact caps, malformed/unknown/duplicate keys, no source/error canaries |
| Early exit/readiness, output flood, timeout and process descendants | harness/process.rs; CLI integration | observed termination, bounded pipes, parent-exit ownership, fixed diagnostics |
| SIGINT/SIGTERM, watchdog and cleanup ownership | harness/mod.rs; process.rs; CLI integration | 130/143, no respawn, separate cleanup deadline, no false success |
| HTTP body/status/inspection/refusal/timeout/disconnect | harness/http.rs | independent terminal categories and complete histograms |
| Exact request/gzip/FPD/debug geometry | harness/native/geometry.rs | exact bytes/nodes/calls/impressions, gzip decode and positive bounded capture |
| Candidate/catalog, features and SDK environment | harness/native/inputs.rs | exact schedule/settings, all development features, real schema refusal |
| Native metering and service/wrapper separation | harness/meter.rs | real service PID, Linux/BSD units, lifetime peak, bounded samples and cleanup |
| Frozen artifacts, source and typed config push | harness/native/snapshots.rs | target-directory independence, digest tamper refusal, A/B/corrupt/restore identities |
| Admission witnesses and ordinary geometry | harness/native/oracles.rs | exact 503/body/status, two held primaries, ordinary per-ID call/decode ownership |
| GVL consent/version/path and held-work overlap | harness/native/oracles.rs | fourteen actual archived versions, valid seed segments, no serialized credit |
| Primary load and debug populations | harness/native/load.rs; scenario.rs; runner.rs | offered/admitted/outcome arithmetic, all-offered histogram, bounded debug evidence |
| Fourteen provider cases and generated negative rows | harness/providers/evaluate.rs | exact identities/features/order/dispatch/outcomes; offline never grants hosted readiness |
| Approved HTTPS fixture origins | harness/providers/preparation.rs | credentials/path/query/private-address/malformed-host refusals |
| Isolated production metadata and source freeze | harness/providers/preparation.rs | no writes, ignored/untracked inputs, precise generated-target exclusions and mutations |
| Cargo emitted build receipt | harness/providers/artifacts.rs | exact override/adapter features, target/profile, fresh exact WASM and all digest changes |
| Spin component and Fastly complete release handoff | harness/providers/artifacts.rs | exact staged component, every member/path/digest, no extras/symlinks or stale artifact |
| Pinned SDK offline release verification | harness/providers/sdk.rs | actual verifier, explicit invalid SDK root fails, no credentialed deployment |
| Provider bidder failure/stall/canary/TLS and dispatch proof | harness/fixture/; CLI integration | real separate child, bounded sockets/requests/bodies, safe counters and forwarded-bid canary scope |
| Corruption templates and recovery controls | harness/providers/recovery.rs | bounded fixtures, authorization, abort plus termination/completion fence before fresh-deadline restore |

Independent review found two documentation gaps before implementation: use
`auctioneer-cli` rather than its Clap display name, and preserve native preparation,
preflight/deadline and handled-signal exit exceptions. Both are corrected above.
The corrected whole-pair re-review approved the design and plan with no further
actionable findings. This approves the migration contract, not implementation.

The user approved replacing both harnesses with Rust in diagnostic qualification. JavaScript remains
only for browser checks; external provider CLI runtimes remain provider-owned.
No implementation or new measurement is claimed by this initial document.

### Foundation Implementation

The CLI-local IO, process and HTTP foundation, exact JSON/gzip geometry, and
provider offline/recovery controls now have 53 passing colocated tests. These
modules remain test-registered until the public command integration is complete;
this is not a completed harness migration or a new qualification capture.

Fresh verification used `cargo test -p auctioneer-cli --bin auctioneer-cli --offline
--locked qualification::harness -- --nocapture` with loopback/process-group
permission. The safe private log is `/private/tmp/auctioneer-rust-foundation-final.log`.
Independent guard-removal runs failed behaviorally for in-flight cancellation,
node accounting, output overflow and private file mode, then the restored scope
passed. Exact JSON/gzip geometry, cleanup-error priority and cancellation inside
a completing recovery operation also had observed behavioral red/green runs.

IO uses descriptor-validated nonblocking opens, with no-follow/link-count
validation for artifacts. Syntactic node accounting precedes a depth-limited
JSON preflight and the original typed parse. Cleanup keeps processes and join
handles owned across cancellation; admission closes under the registry lock.
Darwin can refuse a signal racing a dead/reaped group: signal acknowledgement
is never cleanup evidence. A reaped root, verified group absence and completed
reader/monitor joins are all required; live or permission-unknown groups cannot
pass. No tests or provider evidence are skipped to mask a cleanup refusal.

The complete CLI scope passed 116 unit tests plus ten integration tests across
two binaries, with two existing ignored unit tests. Strict CLI all-target
Clippy passed with `-D warnings`. Common spec/quality re-review and the provider
recovery re-review approved the fixes. Logs are
`/private/tmp/auctioneer-rust-cli-foundation-all.log` and
`/private/tmp/auctioneer-rust-current-clippy.log`. This does not close the remaining
command, fixture, provider handoff, CI or new measurement tasks below.

### Provider Handoffs and Fixture Server

The provider preparation, Cargo receipt, Spin/Fastly staging and actual pinned
SDK archive-verification boundaries are implemented. Controls rehash changed
metadata, reject all missing or substituted bundle members, and refuse stale,
linked, changed and rehashed non-WASM artifacts. Git reads explicitly disable
replacement refs: an adversarial local clone demonstrates that an otherwise
pinned revision can expose substituted verifier bytes without this protection.
Production metadata and the actual SDK checkout are not modified by tests.

The loopback fixture server now exercises real HTTP and TLS, bounded populations,
native hold/release/coverage, final-wave stalls and exact gzip geometry. Its
HTTP parser has an 8 KiB library buffer floor; its separately checked typed head
limit is 4 KiB. These are fixture controls, not changes to measured application
ingress limits. Canonical native IDs are required so `q01` cannot credit `q1`.
Cancellation follows observed dispatch, and saturation tests use test-only
occupancy barriers rather than cumulative counts or short timing assumptions.

Fresh verification passed 95 harness tests and the complete CLI all-target scope
(158 unit tests, ten integration tests, two existing ignored unit tests).
Strict all-target Clippy passed with `-D warnings`, and formatting passed.
The canonical-ID guard has an observed behavioral removal failure. Safe logs:
`/private/tmp/auctioneer-rust-handoffs-review-green.log`,
`/private/tmp/auctioneer-rust-handoffs-cli-all.log`,
`/private/tmp/auctioneer-rust-handoffs-review-clippy.log`, and
`/private/tmp/auctioneer-rust-fixture-id-red.log`.
Independent handoff review approved the Git replacement and artifact controls.
Private child protocol and public CLI integration are still pending; this is
not a completed migration, deployed test, or new resource measurement.

### Rust Command And Native Runner Cutover

The public native/provider commands and private bounded fixture protocol now
execute Rust in the existing CLI. Native orchestration binds real source,
hardware, effective SDK settings, frozen executables and typed snapshots;
it reuses the production evaluator. The five replaced JavaScript files are
removed. Browser JavaScript and the pinned SDK's own archive-tool prerequisites
remain. Nineteen control families have concrete AST-resolved, unignored test
owners and an exact unfiltered CI command; registration is not a substitute for
assertion quality or measured evidence.

Independent reviews led to observed red/green repairs for cleanup deadlines,
retained writer state, externally owned HTTP drivers, output after cleanup,
persisted predeclaration identity, fatal-error precedence, scratch retention,
timeout-with-bids ordinary populations and the original settings/refusal budgets.
Source guards independently reject clean-filter, replacement-ref and executable
mode disguises. Three separately applied ordinary-accounting mutations each
compile and fail the intended tests, and were restored before final verification.

Fresh cutover verification:

- `cargo test -p auctioneer-cli --all-targets --offline --locked`: 342 unit and
  20 integration tests passed. Two existing ignored unit markers are explicitly
  driven by their environment-isolated parent tests; no new ignored test exists.
- `cargo clippy -p auctioneer-cli --all-targets --offline --locked -- -D warnings`:
  passed; only the existing GVL build diagnostics remain.
- `cargo test -p auctioneer-core --test ci_workflows --offline --locked`: 75 passed.
- Workspace formatting, roster, roster-fixture, batch-rebase, raw-JSON and
  raw-JSON-gate fixture checks passed. The preserved browser shutdown controls
  passed both cases.

Logs are `/private/tmp/auctioneer-rust-cutover-final-all-targets.log`,
`/private/tmp/auctioneer-rust-cutover-clippy-green.log`,
`/private/tmp/auctioneer-rust-cutover-ci-gates.log`, and
`/private/tmp/auctioneer-rust-cutover-browser-shutdown.log`.
The explicit frozen-artifact control has eight passing unit controls and an
approved independent source review, but its real execution and the new 45-row
release capture still belong to Task 9. Historical measurements are unchanged;
none is relabeled as Rust execution or hosted acceptance.

The verified cutover was committed and pushed as `aefcf47`. It is the source
identity for the first Rust release attempt. The next profile-only commit must
precede measured traffic; all Task 9 execution and the frozen-artifact check
remain pending at this checkpoint.

### First Rust Profile Predeclaration

Release build succeeded with `cargo build --release --locked --offline -p
auctioneer-adapter-axum -p auctioneer-cli --features
auctioneer-adapter-axum/insecure-bidders` from source
`aefcf47459eba3781e147f90fb4290d33518ebee`.
The real release CLI's `qualify native --prepare --no-build` returned prepared,
exit 0 and readiness false, with no stderr. Its authoritative profile is
`docs/testing/fixtures/resource-qualification/native-axum-rust-aefcf47.json`,
SHA-256 `de3e68dcefebe00cbda66317858f8e9823a06d29fdd51dc0d813cedeb6343aa2`.

A structural comparison against `native-axum-local-e0a540f.json`, excluding only
hardware/source identity and SHA-256 provenance fields, is exactly equal. All
other profile fields, including numeric criteria, subcase/fixture IDs, modes,
effects, SDK settings and debug bounds, remain unchanged. The three variants
schedule 45 rows across all 24 subcases. The real empty-evidence evaluator
accounts 2,533,631,672 bytes against 4 GiB with 256 MiB required headroom; local
acceptance is pending, overall qualification unsupported and readiness false.

The profile and this preparation record are committed before measured traffic.
The intended measured invocation is the same release CLI with `--no-build`,
the committed profile, source revision above and a fresh output directory. No
threshold or fixture population will be tuned after inspecting that execution.

### First Capture And Frozen-Control Repair

The `aefcf47` release run passed all 45 local rows, with 45 attestations, no
recorded failures and four actual A/B/corrupt/restored processes. Its report is
overall unsupported, with six SDK attestations pending and readiness false;
exit 1 is the required completed-nonqualification status. Maximum process-life
high-water was 49,807,360 bytes and sampled retained growth was 15,974,400 bytes.
The [unchanged safe capture](../../testing/evidence/native-axum-rust-aefcf47-local-pass/README.md)
also retains the separate frozen control's failure; measurement alone did not
complete the migration.

The failed control reused a valid generated proof for missing-consent input.
Production correctly denies that request without a malformed warning, but the
unrelated proof falsely earned parse credit. Credit now binds the exact supplied
consent before mutating its version set. Invalid controls separately require
missing-consent warning-free denial and malformed/core-only refusal; a source
mismatch cannot conceal loss of the latter check. A four-cell refusal matrix
and serialized-body source checks make both requirements independent.

The new source-binding and refusal assertions each had observed behavioral
red/green runs. All 156 native controls passed, strict CLI all-target Clippy
passed, and independent read-only review approved the three-file repair.
The repaired debug CLI then executed `qualify frozen-probe` against the
unchanged original frozen application: exit 0, fourteen distinct versions and
parses, three invalid controls with zero credit, sixteen ordinary and sixteen
recovery bids, 32 fixture calls, and unchanged evidence. Logs are
`/private/tmp/auctioneer-rust-consent-control-final-green.log`,
`/private/tmp/auctioneer-rust-consent-control-clippy.log`, and
`/private/tmp/auctioneer-rust-frozen-final-fixed.stdout`.

This verifies the repair, not a relabeling of the earlier executable. A newly
committed source, release build and predeclared profile must rerun all 45 rows
and the separate frozen control. Criteria and populations remain unchanged;
live-provider acceptance remains publisher pilot.

### Repaired Rust Profile Predeclaration

Release rebuild from `d68ba170f0b25b40fe4890d9d56bdd9d5048251c` passed with
the same locked/offline, application-scoped insecure fixture feature command.
The application digest is unchanged. The actual repaired release CLI returned
prepared, exit 0 and readiness false, with empty stderr. Its generated profile
is `docs/testing/fixtures/resource-qualification/native-axum-rust-d68ba17.json`,
SHA-256 `6bb2a0a158f097da31957b6122218b43aa868962a7c95a743369ddc11abd243c`.

Structural comparison against the first Rust profile, excluding only
hardware/source identity and SHA-256 provenance fields, returned exactly equal.
All other fields, including criteria, SDK settings, debug bounds and the 45-row
schedule, are unchanged. This profile and preparation record must be committed
and pushed before the new run. Both measurement and the separate frozen control
will use the repaired release, its source revision above and a fresh directory;
no current passing measurement is claimed by this predeclaration.

### Repaired Release Execution

The predeclaration was committed and pushed as `7a1b246` before traffic. The
actual `d68ba17` release then passed all 45 local rows with 45 attestations,
empty failures/reasons, three configuration variants and four snapshot process
observations. Fourteen GVL parse credits accompany held-work evidence; ordinary
dispatch/decode counters remain separate from witnesses. Maximum service
process-lifetime high-water and sampled retained growth were 37,765,120 and
15,826,944 bytes. The report remains overall unsupported, with six pending SDK
attestations and readiness false; exit 1 is required for that outcome.

The same repaired release's separate frozen control exited 0 with fourteen
distinct versions/parses, three invalid controls and zero invalid credit,
sixteen ordinary plus sixteen recovery bids, 32 fixture calls and unchanged
evidence. The [safe capture and scope](../../testing/evidence/native-axum-rust-d68ba17-local-pass/README.md)
retain exact JSON and hashes, not binaries or raw bodies/logs. An unrelated
project's Docker test was active near run start; no compiler was observed at
preflight or the later check. No Auctioneer build or source edits ran during traffic.
This is shared-host local evidence, not dedicated-host benchmarking or publisher pilot.

Independent read-only review approved the new capture, explicit frozen result
and safe retained copies, finding no actionable issue. It verified identities,
digests, schedule, ordinary-only work, overlap, lifecycle evidence, meter scope
and the local-only outcome. The final broader integration matrix remains in
progress at this checkpoint. Historical captures, including the first Rust
attempt's separate-control failure, remain unchanged.

### Native Integration And Test-Name Repair

Fresh serial, locked/offline native verification completed:

| Scope | Passed | Failed | Ignored |
| --- | --- | --- | --- |
| Full workspace, 214 binaries | 3,764 | 0 | 10 |
| Secure-default bidder/exchange/core, 131 binaries | 1,819 | 0 | 2 |
| Secure config, lib plus all ten non-harness targets | 390 | 0 | 0 |
| Explicit insecure-feature workspace, config excluded | 3,289 | 0 | 10 |
| Feature-sensitive config, five targets | 277 | 0 | 0 |
| Core default/conformance indicators, separate invocations | 5 each | 0 | 0 |
| Native dependency-only conformance/config-override exports | 35 each | 0 | 0 |
| Four-binary native conformance | 77 | 0 | 0 |
| Separate secure production usersync, explicit model coverage, Spin loader | 1 each | 0 | 0 |

The full workspace includes 345 CLI unit and twenty CLI integration passes.
All 84 isolated build-failure cases completed. Counts are per invocation, not
distinct-test totals. Parent-driven environment probes and the baked-config
probe execute outside their ignored marker rows; production usersync, model
coverage and Spin loader were explicitly driven. Local Fastly guest controls
and the three WASM check/lint/conformance-build rows are still pending here.

The first workspace attempt stopped on a zero-byte connection reset in the
slow-response disconnect transport test. Its exact case then passed standalone
and under the original workspace graph; six unchanged concurrent-sibling runs
passed 72 tests. The diagnostic workspace filter was interrupted after its
intended test passed rather than launching further zero-match binaries. The
complete unfiltered rerun above passed. Neither repetition nor independent
source investigation established the cause or a repair. The possible port
ownership/readiness race and missing failure diagnostics are actionable in
ISSUES.md; the original failed log is retained, and no retry/assertion changed.

Full workspace Clippy found the redundant `test_` prefix in the override-state
test. Only that function name under `cfg(test)` was changed. The same assertion
passes in both feature states. Default workspace, insecure-feature workspace
and conformance Clippy now pass with `-D warnings`; no lint suppression was
added. This post-capture test-only rename does not change production or harness
behavior, and does not relabel the retained `d68ba17` executable or measurements.

Logs are `/private/tmp/auctioneer-rust-final-*.log`, including the original
`workspace.log`, successful `workspace-rerun.log`, and separate failing
`clippy-workspace.log` / successful `clippy-workspace-green.log`. The broader
verification record will enumerate final cross-target and emulator results.
Browser shutdown passed both cases; formatting and repository/rejection gates
passed. Final cross-target verification, reconciliation and publication remain
open; no hosted acceptance is inferred.

### Final Cross-Target Reconciliation

All nine exact-CI WASM foundation, strict-lint and conformance-compilation
commands passed across Cloudflare `wasm32-unknown-unknown`, Fastly
`wasm32-wasip1`, and Spin `wasm32-wasip2`. The actual Fastly library-test guest
was built and selected from structured Cargo compiler-artifact output. Viceroy
0.21.0 then passed both logging and both health-store controls. Spin 4.1.0's
loader control also passed. These are local guest/emulator checks, not deployed
generated-App, raw-ingress, response-egress or production qualification.

The [final verification record](../../testing/evidence/diagnostics-qualification-verification-20261015.md#rust-harness-migration)
lists commands/scopes, counts, ignored-control drivers, the failed workspace
attempt, name-only lint repair, safe capture and pending SDK/live boundaries.
All substantive implementation, evidence and native repair rounds were already
committed/pushed through `ab9a872`; final publication includes this close-out
record. All original measurements and the first Rust control failure remain
unchanged. No threshold, fixture population, privacy policy or deployed binding
was relaxed or modified to close the migration.

Local diagnostic qualification implementation and migration verification are complete. Remote CI is
not green: the latest inspected test job has no assigned runner or steps and
reports account billing/spending refusal. The recorded socket reset remains
open/not reproduced, not a claimed fix. Six SDK attestations, selected-target
live acceptance and publisher rollout remain publisher pilot work.

Final independent read-only reconciliation found no actionable issue in counts,
links, revision boundaries, retained failures, historical browser evidence,
the unresolved reset, external CI refusal or deferred deployment scope.
