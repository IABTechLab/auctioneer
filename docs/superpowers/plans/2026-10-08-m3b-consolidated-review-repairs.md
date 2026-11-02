# M3b Consolidated Review Repairs

Scope: the independently reported implementation findings A-G, the subsequent
numbered findings 1-12, and related warning/targeting/verification issues.
This is not a provider certification or
a reduction of the existing privacy contract. All edits are in `m3b-fanout`.

## Repair Decisions

- A: `PassID=false` clears the complete flattened `User.other` and
  `UserExt.other` maps, not a blacklist of current identifier spellings.
  Typed consent and independently
  permitted geo remain intact. `PassID=true` preserves unknown fields.
  Colocated scrub and real auction/bidder-wire regressions cover both sides.
- B: Cloudflare and Spin match Fastly: malformed flags or conflicting health
  inputs log a token-free diagnostic and resolve to `Closed`. `/status` and
  auction dispatch remain available. A conflicting installed policy/scope
  remains an error; fallback must not overwrite immutable installation.
  Process-isolated tests exercise the production policy helper and real routes.
- C: Wrangler declares the runtime's non-secret environment keys and documents
  secret-only token provisioning. Manifest tests derive store keys from the
  actual application's declarations and pin safe health/logging defaults.
- D: only the native adapter proves the shipped row, reading production
  `limits()` and `target()`. Generic runtime fixtures explicitly test synthetic
  accounting examples. The native proof includes the 1,048,576-byte parser
  transient, all derived terms, exact-demand acceptance, and one-byte-under
  rejection. Its provisional oversized total is not a production certificate;
  resource-profile qualification remains the explicit M6 follow-up.
- E: remove unused profile installation, storage, and lookup APIs. Keep the
  adapter-owned static capability declarations and expectation checks, labeled
  as declarations. Runtime decoding uses response metadata; scheduling and
  funding use concrete adapter primitives and the resolved envelope. No fake
  consumer or new SDK guarantee is introduced to justify dead state.
- F: core owns the shared algorithm conformance suite. Provider binaries must
  import their own adapters and exercise their timer/cooperator with a controlled
  client. This proves local adapter wiring, not actual hosted transport caps or
  cancellation. Deployment-target compilation separately covers WASM bindings.
- G: serialize the AppNexus placement transform through borrowed Serde views
  directly into the capped writer. Validate placements before reserving a call;
  preserve unrelated fields, namespace replacement, and caller immutability.
  A separate allocator-instrumented integration binary checks large unknown
  trees with tiny and admitting caps without charging fixture construction.
- Warning allocation: `GclStale` stays 10019; typed `OutboundBudgetClamp` is 10020. Only the auction-owned privacy warning group feeds deployment counters.
- Targeting: reject reversed custom price ranges before advancing the implicit
  minimum; use the existing medium fallback when no valid ranges remain.
- CI: add strict Clippy for each adapter's actual WASM target, including tests
  and conformance features. Do not enable CodeQL automatically while repository
  access remains explicitly unavailable in its workflow configuration.
- Verification portability: CLI test fixtures use the OS temporary directory,
  independent of Cargo target-path configuration and the source checkout's
  writability. Health JSON fixtures use `json!`, with test-only dependencies;
  no raw-fixture allowlist exemption was added.
- Socket verification: deadline expiry may occur before a connection exists.
  Keep the 50 ms deadline/classification assertions, and separately synchronize
  on flushed response headers, prove the client send is still pending, drop it,
  and require peer-observed closure while its pooled client remains alive.
  The cancellation fixture owns and joins its server task before assertions;
  watchdog expiry aborts and joins rather than silently losing task errors.
  A subsequent full-width healthy wave checks permit recovery.

## Verification

Retained host-readable logs:
`/private/tmp/auctioneer-m3b-review-20261007/`.
Retained container: `auctioneer-m3b-review-20261007` (no automatic removal).
Pinned Linux Rust 1.95.0, two build jobs, four test threads, 6 GiB memory limit;
bundled LLD is used to reduce link memory. Sources and dependency caches are
read-only, build outputs use container volumes, and child Cargo processes inherit
offline mode. This separates a build's exit status from container lifecycle.
The dependency pin under verification is EdgeZero
`ea66eff045512cccc60646d6ec339e83ff44fed7`; no newer SDK revision is inferred.

- Confirmed red: scrub unit regression and real bidder-wire regression both
  failed on the original implementation, including `sha256email` reaching wire.
- Confirmed green: exchange unit tests (115), privacy integration (9), targeting
  integration (8); final workspace verification also covers the nested
  `UserExt.other` regression.
- `workspace-verified.log`: final locked native workspace/all-targets run with
  `conformance-suite`, excluding the unchanged configuration crate: 1,504
  passed, zero failed, four top-level ignored declarations.
- Configuration targets in `workspace-final.log`: all 219 tests passed,
  including all 55 compile-failure cases (1,523 seconds). That initial overall
  workspace command failed on subsequently repaired targets, so its overall
  exit status is not being presented as green. Configuration code did not
  change between that run and the final workspace rerun. Together these split
  default-feature results cover 1,723 passing test executions.
- `workspace-insecure-verified.log`: the alternate `insecure-bidders` plus
  `conformance-suite` workspace run passed 1,506 tests, zero failed, four
  top-level ignored declarations. `config-insecure-verified.log` passed the
  configuration crate's 163 feature-sensitive tests using the same target list
  and `test-harness-override` feature as CI. The feature-independent
  compile-failure harness was not duplicated.
- In each workspace run, two ignored CLI probes are deliberately executed by
  their parent tests with isolated process environments. The two ignored
  Fastly Viceroy logging drivers and their three WASM-only child probes were
  not executed locally; no hosted lifecycle qualification is claimed.
- `conformance-verified.log`: 30 shared algorithm tests and eight adapter-wiring
  tests per provider passed (54 total).
- `allocation-verified.log`: all three allocator regressions passed. At a
  32-byte cap, both 64- and 2,048-node unknown trees measured the same 3,977-byte
  peak and 4,043 cumulative bytes. Exhausted call budget measured zero allocated
  bytes. The admitting 2 MiB cap measured a 2,101,924-byte peak, within the cap
  plus the explicit construction allowance.
- Strict WASM Clippy passed for Cloudflare `wasm32-unknown-unknown`, Fastly
  `wasm32-wasip1`, and Spin `wasm32-wasip2`. All three conformance WASM
  binaries linked successfully (`wasm-cloudflare-link.log`,
  `wasm-fastly-link.log`, and `wasm-spin-link.log`). These binaries were
  compiled, not executed in provider hosts.
- `clippy-verified-final.log`: strict native workspace/all-targets Clippy with
  `conformance-suite` passed. `doc-tests-verified.log`: the workspace doc-test
  command succeeded; there are currently no executable doc tests.
- Both roster gates and both raw-JSON fixture gates passed, including negative
  gate fixtures. The roster remains 139; raw-fixture budgets remain unchanged
  at 89 fixtures across 35 files. The batch-API documentation gate,
  `cargo fmt --all -- --check`, and `git diff --check` also passed.
- Earlier failing verification logs remain retained. They exposed native
  health-fixture setup, adapter yield-observation assumptions, read-only CLI
  fixture paths, the obsolete five-copy AppNexus expectation, socket timing,
  and strict test lint failures. Final commands above completed successfully
  after the fixes; no failed command is silently treated as a pass. The
  retained container remained running with `OOMKilled=false`.

## Numbered Follow-up Repairs

- 1: pre-header deadline cleanup requires either no observed request or a
  nonempty all-`ClientDrop` outcome. The counter is read after joining the
  fixture task. Restoring the old vacuous predicate fails the new regression;
  the established-body cancellation proof and 50 ms deadline are unchanged.
- 2: reproduced the strict stale-GCL build failure, then ran the transactional
  archive refresh against IAB sources without changing freshness or retention
  policy. The generated archive contains current GVL v180 and the October 8
  GCL. No applicable Auctioneer instruction prohibited running the refresh tool.
- 3: ignore only the Fastly CLI's generated `.fastly.toml.edgezero-lock`.
- 4: reject overlapping/interior-gapped custom price ranges and numerically
  unsafe increments. Direct custom constructors receive the same validation
  and precision cap. Invalid configurations use medium granularity; an
  explicitly nonzero first minimum retains its documented no-bucket interval.
  Unit and auction-targeting regressions cover the reported shapes.
- 5: declare Fastly's local config flag and environment-backed secret store.
  Missing/empty tokens intentionally resolve closed without a store-read error;
  a development token selects bearer authentication. No operator credential
  is checked in. Portable manifest tests and actual Viceroy SDK tests cover both
  cases; CI explicitly drives the host-only test driver.
- 6: AppNexus and Mocktioneer classify response hints and the matching
  single-format impression rather than hardcoding banner. Ambiguous,
  conflicting, malformed, or unrequested formats are rejected. AppNexus's
  bidder-specific codes are distinct from OpenRTB `mtype`; audio remains
  unsupported by that adapter. No creative caching/video delivery parity is
  implied by correct format classification.
- 7: asymmetric backend/blob caps and one-limit-at-a-time changes prove
  independent config funding attribution and enforced byte boundaries. The
  total measured peak remains a combined bound, not a per-phase allocator proof.
- 8: caller fixtures are outside allocation measurement. Funded metadata and
  fixed control scratch replace AppNexus's loose allowance; response rejection
  is bounded by the installed cap plus 256 bytes of fixed error scratch.
  Fixed-cap cross-size comparisons and clone/unbounded negative controls cover
  both paths; response allocator accounting handles failed/replaced allocations.
- 9-10: scrub documentation matches the catch-all ID repair; denied Geo clears
  both `Geo.other` and opaque `Geo.ext` in user/device locations while preserving
  typed country and rounded coordinates. Allowed-Geo controls and actual bidder
  wire tests prove the permission boundary.
- 11-12: run both raw-JSON fixture gates in the workspace test workflow and
  protect test/format workflows alongside provider packaging. Regression
  mutations cover required conformance features, deployment targets, strict
  Clippy, disabled jobs/steps, ignored failures, and comment-only commands.

Follow-up verification logs use the `followup-` prefix in the retained directory
above. The earlier verification counts describe the original A-G repair, not
this expanded patch. Final follow-up verification is recorded separately below.

## Follow-up Verification

- `followup-workspace.log`: locked workspace/all-targets tests with
  `conformance-suite`, excluding `auctioneer-config`: 1,547 passed across 124 test
  binaries, zero failed, five top-level ignored declarations. Two ignored CLI
  child probes are driven by their parent tests. The three ignored native
  Fastly host drivers were not run by that command.
- `followup-config-verified.log`: 164 config codegen, archive/hash, URL-safety,
  smoke, and baked-default tests passed against the refreshed archive. The
  unchanged 55-case compile-failure harness was not rerun in this follow-up;
  its earlier result is recorded separately above.
- Post-lint reruns passed: 76 bidder unit tests with `insecure-bidders`, six
  AppNexus allocation tests, and 40 core allocation/config-factor/workflow
  tests. Logs are `followup-bidders-final.log`,
  `followup-appnexus-allocation-final.log`, and `followup-core-final.log`.
- `followup-cancellation-green.log`: all 13 real-socket transport tests
  passed. Restoring the vacuous predicate fails the new regression in
  `followup-cancellation-red.log`.
- Geo scrub unit and bidder-wire tests, unsafe price configurations, and media
  classification regressions failed before their fixes. The exchange rerun
  passed 124 unit, ten privacy-wire, and eight targeting tests; the final
  workspace run also covers the expanded unsafe-granularity targeting table.
- Strict native workspace/all-targets Clippy with `conformance-suite` passed
  (`followup-clippy-native-pass.log`). Strict deployment-target Clippy passed
  for Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1`, and Spin
  `wasm32-wasip2` (`followup-clippy-{cloudflare,fastly,spin}-pass.log`).
  Bidder insecure-feature Clippy and core insecure/conformance Clippy passed
  in `followup-clippy-insecure-pass.log` and
  `followup-core-insecure-clippy-verified.log`.
- The final insecure-feature end-to-end rerun exposed a preexisting fixture
  defect: `TestClockHandle` overrides wall time only, leaving handler CPU
  timing on a running monotonic clock (11 ms versus the direct path's 10 ms).
  The fixture now supplies the existing `TestMonotonicClockHandle` from the
  same frozen clock; the exact byte comparison is unchanged. Both end-to-end
  tests and all three clock-domain tests passed in
  `followup-mocktioneer-wire-verified.log`. The full alternate-feature workspace
  was not rerun in this follow-up.
- Both actual Fastly SDK guest health tests passed in Viceroy 0.21.0 against
  the local manifest: unset token resolves closed, and a development token
  requires the matching bearer (`followup-fastly-sdk-{unset,token}.log`).
  Neither emitted a store-read error. These guest cases were executed directly;
  the native wrapper driver is wired into CI but was not executed locally.
- Both raw-JSON fixture gates, both roster gates, and the batch documentation
  gate passed. Counts remain 89 raw fixtures across 35 files and 139 roster
  members. Formatting and whitespace checks passed. The generated Fastly lock
  file is confirmed ignored by its exact path.

## External Limits

GitHub run 37740020082, job/check 113188330140, reports: jobs were not started
because recent account payments failed or the spending limit needs increasing.
The job has no steps, so its failure is not a compile/test verdict. Account
billing must be repaired by an authorized owner; local tests do not make hosted
CI green or supply unavailable branch protection. No account settings changed.

Hosted provider lifecycle runs and Spin host config allocation evidence remain
open under the existing SDK update record. CodeQL remains manual-only: its
workflow records unavailable repository access. An authorized owner must verify
access and restore automatic triggers when enabled; Rust lint does not replace
security analysis. This repair does not change account settings or protection.
