# EdgeZero Shared Policy Update

Update the existing milestone stack to EdgeZero PR 275 at
`91f4af38821de3454dafff75803ea6fea65efb8b`, replacing
`950cb81d406d0dc269baf94314547329a4c1cef0`. Preserve SDK-owned transport,
ingress, config resolution and egress. Do not enable the new optional
reusable application lifecycle or change the production resource caps.

## Checklist

- [x] Check the upstream PR head, Auctioneer remote branches and worktree state.
- [x] Update all six direct SDK pins and the eight locked SDK packages,
      without refreshing unrelated dependencies.
- [x] Review shared batches, response-body policy and provider/lifecycle
      changes against Auctioneer's actual integration paths.
- [x] Run fresh native integration, strict Clippy, resource guards,
      conformance and provider-target compilation checks.
- [x] Push M3b and synchronize M4, M5 and M6 without changing main.
- [x] Verify SDK-update remote heads and report merge readiness and CI gaps.

## CI Status At Start

M3b PR 15 at `7d0fa9957408b4516286723ede828875928254fb` has failed checks,
but the jobs have no executed steps. The GitHub check annotation states
that jobs were not started because recent account payments failed or the
spending limit needs increasing. This is a runner/account block, not a
test failure; do not call that CI green. EdgeZero PR 275's returned checks
were successful at the new pin. Upstream checks do not replace Auctioneer
integration verification.

## Review And Verification

Two independent source reviews found no new upstream integration blocker.
Shared batches preserve indexed terminal observations, sampled timestamps,
cancel/drop ownership and driver EOF validation; cutoff now releases pending
work before reporting termination. The extracted response-body policy keeps
the previous ordering of declared-length checks, encoded caps, recognized
decoding, decoded caps and rechunking. Adapter deadlines and final collection
remain adapter-owned. Fastly keeps its provider-specific batch driver.

Auctioneer still uses single-request application lifecycles. Fastly's custom
entrypoint retains `App::build`, configured hooks, manifest registries and
SDK-owned delivery. Config push/diff now validate the selected adapter
case-insensitively; standalone validation remains cross-adapter. Auctioneer's
runtime-only push projection is unchanged. Config extraction, codec versions
and decoder resource contracts are unchanged.

The sole review finding was Auctioneer's stale SDK revision assertion in
`codec_pins.rs`. The existing dependency-free test was compiled directly
with Rust 2024 and the real crate manifest path: the revision assertion
failed against the new lockfile while the codec tuple passed. Its revision
constant was updated without changing the six codec version assertions.
Both assertions pass after that update. Fresh workspace/all-target
compilation also passes with the locked SDK graph. Fresh Cargo results are
recorded below; the codec versions and resource caps were not changed.

Previous-pin test results remain historical and are not evidence for this
SDK update. Local compilation does not imply hosted qualification or release
performance. The already accepted raw-ingress and response-egress provider
limitations are not newly introduced blockers.

## Executed Checks

All commands used the new locked SDK revision and offline dependencies.

| Check                                                                              | Result                                                |
| ---------------------------------------------------------------------------------- | ----------------------------------------------------- |
| M3b workspace/all-target compilation                                               | Passed                                                |
| Core/exchange libraries and SDK-sensitive integration targets                      | 439 passed                                            |
| Core/exchange libraries, codec guard and config allocation with `insecure-bidders` | 350 passed                                            |
| Real TCP cancellation and production-wave transport probes                         | 9 passed                                              |
| Core and three adapter fanout conformance targets                                  | 111 passed                                            |
| CLI all-target tests                                                               | 7 passed, 2 existing tests ignored                    |
| Cargo codec/revision guard, default features                                       | 2 passed                                              |
| Strict workspace/all-target Clippy                                                 | Passed                                                |
| Strict conformance-feature/all-target Clippy                                       | Passed                                                |
| Cloudflare `wasm32-unknown-unknown` compilation                                    | Passed                                                |
| Fastly `wasm32-wasip1` compilation                                                 | Passed                                                |
| Spin `wasm32-wasip2` compilation                                                   | Passed                                                |
| Formatting, whitespace, batch, roster and raw-JSON guards                          | Passed, including negative fixture suites             |
| Resolved SDK capability enumeration                                                | All 23 capabilities present; support levels unchanged |
| M4, M5 and M6 workspace/all-target compilation                                     | Passed                                                |
| M5 codec, user-sync lifecycle/resource/route integration targets                   | 24 passed                                             |

The 439-test run selected `--lib` and these integration targets from core
and exchange: `config_parse_factor`, `config_envelope_value_factor`,
`handler_resolution`, `edgezero_fanout`, `axum_production_strategy`,
`clock_domain`, `ingress_admission`, `egress_binding`, `config_deadline`,
`resolve_runtime_config`, `runtime_config_failclosed`, `write_budget_anchors`,
`bid_impression_correlation`, `expired_tmax_diagnostics` and `targeting_prefix`.
The feature-enabled run selected both libraries, `codec_pins` and
`config_parse_factor`. This is targeted SDK-delta verification, not a new
full-workspace sweep. The expensive config subprocess harness, release
performance/package-size qualification and hosted-provider tests were not
rerun. Previous broad-run evidence remains in its own dated record.

Initial downstream checks reused M5's target directory for M4 and failed
on stale M4 generated-config types and runtime exports. Clearing only the
`auctioneer-config` and `auctioneer-runtime` build artifacts restored M5 compilation;
M6 compilation passed with its own target directory. No source repair was
needed. Keep incompatible worktree build outputs separate for future checks.
M6 still has identical code, manifests, workflows and scripts to M5.

The SDK update was pushed as M3b `87bba91`, M4 `1c34df9`, M5 `bf8c974` and
M6 `66023dc`. Later commits synchronize this verification record only. Main
remains `e75881ec4590c6529c15f2f9df1f3afbf8bff7b8`. The upstream PR head was
rechecked after verification and still matched `91f4af3`.

## Merge Assessment

No further full review cycle is required for this dependency delta, and no
new merge-blocking source finding remains. M3b is code-ready on the evidence
above; that does not label every provider production-qualified.

M3b PR 15 at the pushed `87bba91` is mergeable but its checks are not green.
The new check annotation again confirms jobs were not started because of
account billing/spending limits. Restore Actions execution and rerun the PR
checks before normal merge acceptance. The targeted local results must not
be represented as a successful full CI run. M3b was not merged into main.
