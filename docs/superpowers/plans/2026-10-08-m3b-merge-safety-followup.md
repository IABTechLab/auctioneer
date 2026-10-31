# M3b Merge Safety Follow-up

**Goal:** Verify the new review claims and repair confirmed merge blockers.

**Architecture:** Keep malformed health inputs fail-closed, but retain only the
static key in the shared error. Keep native release diagnostics while building
the Spin deployment artifact without debug information. Resolve pricing and
opaque device scrubbing against the current contract before changing behavior.

## Execution

- [x] Add colocated health error Display/Debug regression tests and a
      token-in-flag scenario in the actual Worker/Spin process tests. Confirm red.
- [x] Remove the rejected raw flag from `HealthBootError::NonBooleanFlag`.
      Run registry and adapter health tests; keep static key diagnostics.
- [x] Verify the current Spin release profile and manifest/workflow paths.
      Add a no-debuginfo deployment profile and a manifest/profile regression;
      build and measure the real component without increasing its size ceiling.
- [x] Verify pricing behavior against Appendix E.2 and its callers. Add
      regressions before repairing confirmed coverage or validation defects.
- [x] Verify device catch-all ID denial. Cover unknown top-level and extension
      identifiers, allowed-ID controls, and real bidder wire serialization.
- [x] Rerun the deadline cancellation regression; do not replace an already
      corrected assertion without a demonstrated failure.
- [ ] Leave historical-plan restoration pending an exact file and revision.
- [x] Run affected native tests, strict Clippy, deployment-target checks,
      formatting, whitespace, and fixture/roster guards. Record evidence and limits.
- [x] Commit and push verified M3b repairs.
- [x] Propagate confirmed fixes through the existing downstream stack, checking
      each integration and preserving unrelated work.

## Verification Commands

Use Linux Rust 1.95.0 with the locked dependency graph and isolated or explicitly
invalidated workspace build artifacts. Do not reuse another worktree's cached
build-script executable as evidence for this branch.

```text
cargo test --locked -p auctioneer-core --lib host_primitives::registry::tests
cargo test --locked -p auctioneer-adapter-cloudflare -p auctioneer-adapter-spin --lib health::tests
cargo test --locked -p auctioneer-exchange --lib
cargo test --locked -p auctioneer-exchange --test privacy_enforcement --test targeting_prefix
cargo test --locked -p auctioneer-adapter-axum --test cancellation --test wave_transport
cargo test --locked -p auctioneer-core --test ci_workflows
cargo clippy --locked --workspace --all-targets --features conformance-suite -- -D warnings
cargo fmt --all --check
```

Provider packaging measurements and WASM compilation are separate from hosted
runtime qualification. GitHub's account-level job-start failure remains an
external limitation, not a successful CI result.

## Health Error Evidence

`merge-safety-health-red.log` fails on the rejected-value confidentiality
assertion. `merge-safety-adapter-health-red.log` fails both actual adapter
process tests with the token in the flag slot. With the raw error field
removed, the four registry tests and both adapter process scenarios pass in
the corresponding `-green.log` files. Display and Debug retain the static
key but no rejected input. Logs remain in the retained verification directory
documented in the consolidated repair record.

## Pricing And Privacy Evidence

The pricing red runs fail with `1.20` instead of `1.23`, both in the parser
tests and through global/media auction targeting. After dropping malformed
explicit minima without advancing the next implicit minimum, 21 pricing unit
tests and all 10 targeting integration tests pass. A positive first minimum
and an empty bucket below it remain intentional Appendix E behavior. Invalid
retained topology or unsafe arithmetic still falls back to medium; this repair
does not introduce warning plumbing or change those semantics.

Device regressions fail before the repair, including `device.uid2` in the
actual bidder body. Clearing both device catch-all maps passes all 15 scrub
unit tests and 11 privacy integration tests. Tests cover nested extension
identifiers, arbitrary opaque context, ATT status, typed context, permitted geo,
independent geo denial, and idempotence. The conservative loss of unclassified
device context is explicit in Appendix C.2.

The pre-header cancellation predicate already rejects a seen request with no
cleanup outcomes. Its comment now accurately distinguishes the one-call batch
cutoff from the separate per-slot deadline regression; the assertion is not
replaced. Health installation's API documentation now records that a scope
conflict does not roll back an already installed policy.

## Spin Packaging Evidence

The real Spin 4.1.0 CLI builds the `wasm-release` component at
`target/wasm32-wasip2/wasm-release/auctioneer_adapter_spin.wasm`: 25,234,776 bytes
against the unchanged 50,000,000-byte ceiling. Native release retains its
workspace line information. Manifest, CI measurement/upload, and command guards
all refer to that same profile and artifact.

The first vendor build exposed an additional manifest error: Spin rejects the
empty words in `edgezero__logging__level`. `spin up` failed parsing that name,
and the new structured manifest test failed before the repair. The component
now reads `edgezero_logging_level` and explicitly maps it to
`EDGEZERO__LOGGING__LEVEL` for `EnvConfig`; EdgeZero's key is unchanged. All five
static manifest/profile tests pass, and the final vendor build completes
without the manifest warning. This proves packaging, not hosted runtime
resource qualification.

## Combined Verification

Linux Rust 1.95.0, locked dependencies at EdgeZero `ea66eff0`, two build jobs:

- Workspace/all-target native run with `conformance-suite`, excluding the
  unchanged config compile-failure harness: 1,561 passed, zero failed,
  five ignored. Nested feature/sealing probe caches were invalidated first.
- Final Spin all-target recheck after the variable rename: 38 passed,
  zero failed or ignored, including all five manifest/profile tests.
- Strict workspace/all-target Clippy with `conformance-suite`: passed.
  The initial run caught two test-only lints; both were repaired and the
  full command rerun successfully.
- Strict all-target/conformance Clippy on Cloudflare's
  `wasm32-unknown-unknown`, Fastly's `wasm32-wasip1`, and Spin's
  `wasm32-wasip2`: passed.
- Real TCP transport suite: 13 passed, including pre-header cleanup,
  established-read cancellation, distinct sibling deadlines, and recovery.
- Format, whitespace, raw-JSON budgets/rejection fixtures, roster ownership/
  rejection fixtures, and batch-API documentation gates: passed.

Final logs use the `merge-safety-` prefix in the retained verification directory.
The five ignored tests, external fixture corpus, and the unchanged config
compile-failure matrix were not rerun here. Prior evidence is not presented as
a fresh run. The only deliberately stopped command in this round was the
redundant queued logging-variable Cargo red check; its failure was established
by the direct structured regression and real Spin loader instead.

## Downstream Integration

M4 retains its composed GDPR/activity/CCPA/LMT/COPPA scrubs. Its independent
wire expectations in the initial repair removed opaque device maps under ID
denial, including `devicetype`, which was then unclassified. Typed context, ATT status,
independently permitted geo, and allowed-bidder controls remain covered.

Fresh M4 integration checks: 232 exchange tests, four registry tests, four
adapter health tests, and 21 deployment/logging/workflow guards passed. Strict
all-target Clippy for exchange/core/Worker/Spin with `conformance-suite`, format,
whitespace, roster, raw-JSON, and batch-documentation gates passed. This is
affected-surface validation, not a new full workspace or vendor packaging run.

An interruption removed the temporary M3b logs described above; those results
record completed earlier runs, not retained files. M4 checks were rerun after
restoring the verifier. Their logs use the `merge-safety-m4-` prefix in the
verification directory.

The broader downstream run also caught an obsolete handler-level expectation
that opaque `device.ext.context` survived denied identifiers. The M4-owned
acceptance fixture now requires its removal, root `uid2` removal, and nested
metadata removal for all three shipped adapters, with preservation required
in the allowed control. After that repair, the M4 core/runtime all-target run
with `conformance-suite` passed 690 tests across 71 result groups, zero failed,
one ignored config-harness child probe. That probe's parent harness was not
rerun here.

M5 integration preserves user-sync behavior and the renamed OpenRTB master
spec. Its final core/runtime all-target run with `conformance-suite` passed
947 tests across 86 result groups, zero failed, one ignored config-harness
child probe. The 232 affected exchange tests, four Worker/Spin health tests,
and ten Spin deployment/logging/manifest guards also passed. Strict native
all-target Clippy passed for core/runtime/exchange/Worker/Spin. Strict
all-target/conformance Clippy passed on each actual deployment target:
Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1`, and Spin
`wasm32-wasip2`. Format, whitespace, roster, raw-JSON (including rejection
fixtures), and batch-documentation gates passed. Logs use `merge-safety-m5-`;
the initial core/runtime log records the obsolete M4 acceptance failure and
the `-final.log` records the complete successful rerun. No new hosted-provider
execution or packaging-size qualification is claimed for M5.

GitHub check 113650165226 again reports that the job was not started because
account payments failed or the spending limit needs increasing. Local
verification does not turn those checks green.

M6 integrated the verified M5 parent without conflicts. Its crates, dependency
manifests/lockfile, workflow configuration, scripts, runtime manifest, and data
are identical to the tested M5 tree; no duplicate cold build is claimed.
Format, whitespace, roster, raw-JSON, and batch-documentation gates passed on
M6. The four preexisting M6 document edits were backed up separately and
restored: spec/plan SHA-256 hashes and the tracked documents' stable patch
identity match before and after integration. They were kept out of the repair
commits and are committed separately with the M6 planning updates at the
user's request. The backup stash is retained.

## Context And Targeting Review Follow-up

- [x] Reproduce standard device context loss in the scrub and real bidder body.
- [x] Type the standard Device, UserAgent, and BrandVersion context. Preserve it
      under ID denial while removing arbitrary identifiers and nested extensions.
      Exercise all four ID/geo combinations and scrub idempotence at the unit boundary.
- [x] Reproduce empty winner/per-bidder price keys through global and media
      auction targeting. Omit only empty price keys at their shared emitter.
- [x] Cover every malformed middle-row drop path. All already preserve the
      implicit minimum; an explicitly retained gap/overlap still defaults to medium.
      Do not stretch a declared range across a gap or relax arithmetic validation.
- [x] Prove that the CI validator accepts disabled measurement/upload and a
      raised Spin ceiling, then add active, failure-enforcing structured checks.
- [x] Complete fresh native, allocation-bound, WASM, and strict lint checks;
      record results before committing and pushing this round.
- [x] Propagate verified changes through M4, M5, and M6, retaining and committing
      each milestone's implementation and documents.

The slot-deadline cancellation finding was withdrawn by its reviewer. No socket
assertion, workload, workflow trigger, or size ceiling changes in this round.
Medium-fallback warning plumbing and workflow-trigger mutation protection remain
follow-up requirements, not claims of completed fixes.

Fresh Linux Rust 1.95.0 evidence at the unchanged EdgeZero `ea66eff0` pin:

- Final native workspace/all-target/conformance sweep, excluding the unchanged
  config compile-failure harness: 1,573 passed, zero failed, five ignored.
  Includes the 13 real TCP tests, allocation guards, actual adapter health tests,
  and all 16 structured CI workflow tests.
- OpenRTB/exchange all-target sweep: 344 passed, zero failed or ignored.
- Strict workspace/all-target/conformance Clippy and provider all-target Clippy
  on `wasm32-unknown-unknown`, `wasm32-wasip1`, and `wasm32-wasip2`: passed.
- Formatting, whitespace, raw-JSON budgets and planted rejection cases,
  roster ownership and planted rejection cases, and batch documentation: passed.

Retained logs: `/private/tmp/auctioneer-m3b-review-20261007/context-*.log`.
`context-device-unit-red.log` and `context-device-wire-red.log` demonstrate
context loss; `context-targeting-red.log` and `context-targeting-auction-red.log`
demonstrate empty price-key emission; `context-ci-red.log` demonstrates all five
CI weakening cases before the repair. No allocation bound or size ceiling was
raised. This round does not rerun hosted qualification, the real vendor package
size measurement, external fixtures, or the config compile-failure matrix.

### M4 Context Integration

The new typed device context survives M4's composed GDPR, activity, CCPA, LMT,
and COPPA ID scrubs. Its oracle no longer expects `devicetype` removal; opaque
root/extension identifier channels remain closed. The actual handler fixture
checks make/model/type preservation for all three shipped bidder adapters.

Fresh M4 native all-target/conformance checks across OpenRTB, exchange, core,
runtime and all three WASM adapters: 1,273 passed, zero failed, four ignored.
The final handler/CI guard recheck passed 18 tests. Strict all-target Clippy
for those packages, formatting, whitespace, roster and raw-JSON guards passed.
Logs use the `context-m4-` prefix in the retained verification directory.
This is affected-surface validation, not hosted qualification or a vendor
packaging measurement. The device-oracle red test failed before its correction.

### M5 Context Integration

The tested M4 context/targeting/CI repairs merge without code conflicts; the
renamed OpenRTB master spec and M5 usersync behavior remain intact. Fresh native
all-target/conformance checks for OpenRTB, exchange, core, runtime and the three
WASM adapters: 1,533 passed, zero failed, four ignored. Strict all-target native
and all three provider-target Clippy checks passed. Format, whitespace, roster,
raw-JSON budgets/rejection fixtures and batch-documentation gates passed.
Logs use the `context-m5-` prefix. These checks do not rerun hosted-provider
qualification, Chromium integration, external fixtures, the config compile-
failure matrix, or vendor packaging-size measurements.

### M6 Context Integration

This repair round carries M3b `1dc9993` through M4 `4e016ca` and M5 `c3719a3`
into M6. The only conflict is append-only verification-record text, resolved
without discarding either history. M6's crates, dependency manifests/lockfile,
workflow configuration, scripts, runtime manifest, configuration and data are
identical to the freshly tested M5 head. No duplicate Rust execution is claimed.
Fresh format, whitespace, raw-JSON, roster and batch-documentation guards pass
on M6. Its design, implementation plan and roadmap are unchanged from the
committed `3135652` planning update; the new master-spec privacy/targeting
clarifications are retained. All integration changes are committed together;
no M6 document is left uncommitted by this round. This does not mark M6's new
implementation or deployment-qualification tasks complete.
