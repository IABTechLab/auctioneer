# EdgeZero Bounded Ingress And Config Update

**Scope:** M3b only, based on Auctioneer `8a489821c5776295e1cc0e655a8ebdb73cde4531`.
Preserve existing Fastly local-config edits. No downstream merge or hosted deployment.

**SDK:** [EdgeZero PR #275](https://github.com/IABTechLab/edgezero/pull/275),
`ea66eff045512cccc60646d6ec339e83ff44fed7`, replacing
`91f4af38821de3454dafff75803ea6fea65efb8b`.
PR #285 is unrelated and is not the dependency source.

## Migration

- [x] Pin all six direct SDK dependencies and resolve all eight SDK packages together.
- [x] Migrate config fixtures to shared UTF-8 `ConfigValue`; retain allocation assertions.
- [x] Bind native SDK ingress/config environment settings to funded startup allowances,
      using `run_app_with_preflight`, not another listener or config loader.
- [x] Enable Cloudflare `enable_weak_ref`, required by bounded config reader finalization.
- [x] Verify native builds, affected integration/allocation/TCP tests, fanout conformance,
      provider-target compilation, formatting and strict Clippy.
- [x] Independently review the config fixture migration and native integration delta.
- [x] Record completed verification results, including full-suite environment failures.
- [ ] Obtain a complete workspace test result in an environment supporting the child
      build harnesses; the restricted container run below is not a full-suite pass.

## Acceptance Boundaries

| Item | Implemented In This SDK Revision                                                                                                                                                                                    | Remaining Evidence                                                                                                                                                                                                  |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1   | Native HTTP/1 connection permits acquired before parser creation; joint raw-head/field caps, bounded parser storage, absolute head deadline and keep-alive expiry. HTTP/2 is disabled.                              | Auctioneer uses these bounds through the SDK entry point. Exact-artifact load, RSS and latency qualification remain Q2; this does not establish provider raw-ingress guarantees.                                        |
| Q3   | Shared config values; finite immutable Axum startup snapshots and aggregate registry/staging budgets; Cloudflare bounded BYOB reads and reader cleanup; Fastly bounded host-ABI reads and chunk/envelope preflight. | Spin host/backend read-allocation ceiling remains open. Provider buffering, allocator/RSS overhead, GC and synchronous interruption are not universally certified. Native config updates require a process restart. |
| Q4   | Native real-TCP suppression, stall/disconnect, completion and recovery regressions; opt-in provider qualification applications, finite runner and correlated ownership validator.                                   | Hosted execution with authorized isolated targets and exact telemetry remains open. The runner is tooling, not a hosted result. Response egress remains `BestEffort`; host handoff is not proof of client delivery. |

These supersede the earlier qualification ledger only for the concrete implementation
boundaries above. They do not turn all Q1/Q3/Q4 items into unconditional provider guarantees.
Q2 remains an explicit Auctioneer M6 requirement: tune and qualify the exact serving artifact,
including config residency and cold GVL parses. An over-ceiling warning is not acceptance.

## Verification Results

The old native parser allowance fails the new SDK-default allocation regression.
The original Cloudflare manifest fails the TOML-parsed `enable_weak_ref` assertion.
Native budget totals after the repair are `167243707392` bytes, including
`435879936` bytes for the conservative parser population and an explicit
`117440512`-byte SDK snapshot startup/residency reserve alongside the historical
`27328512`-byte measured baseline. This is not a fresh RSS measurement. Application
capacity/decoder caps are unchanged; the row still warns over its 512000000-byte
ceiling. The production boot integration imports the actual row instead of an
outdated handwritten fixture.

The SDK revision/JSON/parser/codec lockfile guards pass. Locked Cargo metadata,
formatting, diff whitespace, batch-API, raw-JSON and roster guards pass. Two
independent read-only reviews found no actionable issue in the config fixture
migration or native SDK integration, and independently recomputed the totals.

Fresh Linux verification used Rust 1.95.0 in disposable arm64 containers, with
the source tree and existing dependency cache mounted read-only. Native test
linking used the toolchain's bundled LLVM linker after the default linker proved
slow. This changes only the verification environment, not repository build flags.

| Check                                                                                                   | Result                                                                         |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Native serving-binary unit tests, including SDK preflight/parser charges and logging                    | 10 passed                                                                      |
| Native baseline-probe unit tests                                                                        | 9 passed                                                                       |
| Actual production boot-envelope installation                                                            | 9 passed, including its imported pure envelope checks                          |
| Cloudflare library tests, including the parsed deployment flag                                          | 11 passed                                                                      |
| Config allocation/retention regression binary                                                           | 14 passed; allocation assertions retained                                      |
| Runtime config resolution, fail-closed behavior, deadline and degraded logging                          | 2 + 19 + 9 + 1 passed                                                          |
| Real-handler bounded path, auxiliary bodies, admission retention and node budget                        | 6 + 6 + 4 + 5 passed                                                           |
| Preboot/pending-read probes, no-envelope fallback and write-budget anchors                              | 1 + 4 + 1 + 3 passed                                                           |
| SDK/codec/JSON/parser lockfile guards                                                                   | 2 passed                                                                       |
| Core plus Cloudflare/Fastly/Spin fanout-conformance binaries                                            | 30 + 27 + 27 + 27 passed                                                       |
| Native real-socket cancellation and bounded wave transport                                              | 3 + 11 passed                                                                  |
| Foundation crates and Cloudflare tests/conformance on `wasm32-unknown-unknown`                          | Locked offline check passed                                                    |
| Foundation crates and Fastly tests/conformance on `wasm32-wasip1`                                       | Locked offline check passed                                                    |
| Foundation crates and Spin tests/conformance on `wasm32-wasip2`                                         | Locked offline check passed                                                    |
| `cargo clippy --workspace --all-targets --features conformance-suite --locked --offline -- -D warnings` | Passed after fixing new-test shadowing, absolute paths and shift operand types |

The focused core/config, boot and four conformance binaries were executed directly
from the new SDK's Linux build to avoid waiting for unrelated test links. The
native serving/baseline, Cloudflare library and real-socket tests also executed
under the broad Cargo invocation. Provider-target checks include tests and enable
`conformance-suite`; they compile the provider deployment targets, not hosted runs.

`cargo test --workspace --all-targets --features conformance-suite --locked --offline`
compiled all targets, but its run failed at both tests in
`auctioneer-bidder/tests/insecure_bidders_snapshot.rs`. Their child Cargo builds do not
inherit the parent's `--offline` flag and attempted crates.io index access with
networking disabled. The diagnostic was DNS/network failure, not the expected
feature-gate diagnostic; the positive control failed for the same reason. This
is not a full-workspace green result. Other build-failure harnesses also require
writable fixture/target paths under the source tree. Normal CI or a separately
prepared writable harness environment must supply the complete result. The two
Fastly Viceroy-dependent logging tests were ignored as declared, not executed.

Initial macOS checking and test attempts were deliberately stopped after prolonged
dynamic-library/signature delays or while queued behind another session's build.
The old native artifact did run and fail the intended allowance assertion. The
other session's workspace-check process was not stopped or counted as our result.
Neither interrupted commands nor historical results under the previous SDK pin
count as passing evidence for this revision. No hosted deployments were performed.
