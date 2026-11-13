# Isolate fixture socket-closure unit checks

## Status And Scope

Planning contract for [Bug #168](https://github.com/IABTechLab/auctioneer/issues/168), under [development reliability](https://github.com/IABTechLab/auctioneer/issues/12). The user approved scoped process isolation and a separate spec/plan PR, then approved this written specification and its precise test-only bounds on 2026-11-13. The [implementation plan](../plans/2026-11-13-fixture-socket-isolation.md) accompanies this contract; implementation starts only after plan review and the separate planning PR's merge.

The repair does not waive [PR #162](https://github.com/IABTechLab/auctioneer/pull/162)'s failed local check, approve that PR, change its code or resolve its review threads. Its full acceptance and current-head review remain separate gates.

## Problem And Evidence

Three inline CLI unit cases assert that a fixture listener is closed after EOF, cancellation or explicit stop. Other tests in the same libtest process spawn native programs concurrently. On macOS, the locked Mio listener creation sets close-on-exec separately from creating the socket. A concurrent child spawn can inherit the descriptor in that interval, retaining the listening socket after the fixture drops its own descriptor. Accepted sockets can encounter the same ownership issue.

The reviewer observed a post-shutdown connection succeed in PR #162's full insecure-feature CLI test process. The recorded result was `775 passed; 1 failed; 3 ignored`. Its EOF case ended with `unwrap_err()` receiving a successful `TcpStream`. A sibling server-stop failure was also reproduced at the base; the exact protocol failure was not, so its pre-existence at that base is not established.

A bounded diagnostic using the locked Mio 1.2.1 listener, without changing repository source, produced:

| Diagnostic condition | Completed checks | Connection after dropping listener |
| --- | ---: | --- |
| No child-spawn load | 100,000 | Not observed |
| Eight same-process spawn workers | 11,554 | Observed |
| Same spawn load in another process | 100,000 | Not observed |

At the observed failure, a diagnostic child still held the listening descriptor. All diagnostic children were stopped and reaped. This is evidence for cross-test descriptor inheritance, not a deterministic CI reproducer or proof that fixture shutdown itself is defective.

Six later unmodified CLI-binary runs each reported `776 passed; 0 failed; 3 ignored`. They establish neither a repair nor permission to disregard the reviewer's failure. The regression controls for this repair must test the isolation boundary deterministically, not require that a scheduling race happen.

## Chosen Boundary

Run only these three existing cases in fresh exact-test child invocations of the same compiled CLI unit-test executable:

| Owner | Full libtest selector |
| --- | --- |
| `qualification/harness/fixture/protocol.rs` | `qualification::harness::fixture::protocol::tests::private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof` |
| `qualification/harness/fixture/protocol.rs` | `qualification::harness::fixture::protocol::tests::malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener` |
| `qualification/harness/fixture/server/mod.rs` | `qualification::harness::fixture::server::tests::delay_applies_to_responses_and_stop_joins_held_connections_before_returning` |

Paths in this table are relative to `crates/auctioneer-cli/src/`. The names, attributes, feature gates, original bodies and assertions stay with these inline tests. Their parent invocations remain selected by the ordinary suite. They delegate the original body to the exact child; they do not become ignored tests or silently return without verification.

Add one private, inline `#[cfg(test)]` helper module under the existing `qualification/harness/fixture/mod.rs`. Its own unit controls also stay inline. It is not a new production API, crate, integration binary, process framework or shared source import. The existing `application` feature continues to own the fixture module; maintenance-only builds acquire no application test dependency.

The parent suite remains parallel. The selected child runs one test with one libtest thread and does not launch native subprocesses. Existing Tokio tasks inside that test are retained. This separates listener creation from unrelated native spawning rather than globally serializing the suite or changing fixture shutdown.

## Parent And Child Contract

1. Represent the three fixture selectors as a closed test-only set. The helper's negative-control entry points are likewise fixed test names, not arbitrary commands or filters. No executable path, shell fragment or selector comes from operator input.
2. Invoke `std::env::current_exe()` directly with `--exact`, the full selector, `--test-threads=1`, `--nocapture` and `--color=never`. Do not invoke Cargo, a shell, a second build or a guessed binary path.
3. Use the child-only marker `AUCTIONEER__TEST__FIXTURE_SOCKET_CASE`, with no legacy alias. Clear inherited environment and pass only that marker. The selected pure-Rust loopback cases require neither source credentials nor operator configuration. No global environment mutation occurs in the parallel parent.
4. In the child, validate the marker, exact selector and single-test arguments before constructing fixture resources. A malformed marker, mismatched selector or recursive spawn attempt fails. In the parent, do not construct or poll the fixture body. A deferred body factory separates parent delegation from child execution.
5. Execute the original case body exactly once in the matching child. Panic, assertion failure and runtime error remain failures. Do not catch an assertion and convert it to success.
6. A successful exit is necessary but insufficient. Validate the child's bounded stdout: exactly one `running 1 test` record, exactly one success for the requested full selector and one complete successful summary with one passed, zero failed, zero ignored and zero measured. The filtered count can vary with the binary's test roster. Reject zero-test, mismatched, duplicated, ignored, truncated and malformed result records.
7. Consume successful child output in the parent. Do not forward nested `test result:` summaries into the outer suite's accounting. Failure diagnostics identify the fixed case and failure category; any retained child diagnostic is subject to the capture bound.

No original test assertion is replaced by a process exit or output marker. The child report proves dispatch; the retained case body still proves closure and the protocol/lifecycle contract.

## Runtime, Capture And Cleanup

The proposed outer execution watchdog is **20,000 ms per child**, followed by a separate **2,000 ms kill/reap fence** on failure or timeout. The EOF case already contains three successive five-second waits; the outer watchdog adds a bound around the whole case and process startup, not extra time to any original wait. Preserve every existing internal timeout, the 40-ms delay assertion and immediate post-stop checks.

Use the existing workspace-managed Tokio process facilities and `tempfile` dependency only in the private test helper. Configure kill-on-drop as a fallback; successful cleanup still requires explicitly observing the direct child's exit/reap. Do not leave blocking wait threads, detached reader tasks or test servers behind. The selected children and finite negative-control children do not create subprocess descendants, so this is direct-child ownership, not a claim of arbitrary process-tree supervision.

Capture stdout and stderr in private temporary regular files. While waiting, observe their lengths and reject either stream above **16,384 bytes**. After exit, read each from the beginning with at most **16,385 bytes** (cap plus one sentinel) and refuse overflow before parsing. Retained accepted output is at most 32 KiB combined. Remove the files through their scoped owner after the child is reaped on every successful or refused path.

These are memory/read limits and monitored file-size limits for trusted compiled test cases, **not an operating-system disk quota**: a writer can overshoot between observations. Negative controls emit finite cap-plus-one payloads rather than an unbounded disk flood. Neither success nor overflow handling waits for pipe EOF.

The existing production `native_control::process::Supervisor` was considered. It always captures pipes and waits for reader cleanup; a pipe descriptor inherited by an unrelated same-process child can prolong EOF independently of the selected child's exit. Extending that production API or weakening its cleanup fence is outside this repair. Private regular-file capture avoids introducing that second dependency on descriptor-inheritance timing. Do not refactor other subprocess owners here.

Any spawn, wait, capture, report-validation or cleanup error fails the parent case. On an active-child refusal, request termination and explicitly wait within the cleanup fence before returning failure. If the operating system does not confirm reap, report cleanup failure; never claim successful cleanup. There are no retries, port-release sleeps, accepted post-stop connections or fallback success paths.

## Assertions That Must Survive

| Case | Retained independent observations |
| --- | --- |
| EOF | Valid ready protocol/port; connection succeeds before EOF; stats command and exact reply keys; input EOF joins serve; a new connection fails after join; exact ready JSON |
| Invalid frames/cancellation | Every malformed-input canary is refused with the existing fixed error and no output; interrupt 143 yields `interrupted`; serve joins; a new connection fails; output reaches EOF |
| Delay/stop | Response delay is at least 40 ms; held request exists; pre-stop counters are checked; stop joins; stopped state is exactly `(false, 0, 1, 0)`; held request is not successful; new connection fails; exact address can be rebound |

Review the original and wrapped bodies directly, including every loop row and before-state assertion. Preserve existing test names and gates in the before/after roster. New helper controls are additive, not substitutes for these observations.

## Deterministic Regression Controls

- Prove parent delegation does not construct or poll the body factory, and matching child dispatch executes it exactly once. Include a positive witness so an unconditional early return cannot pass.
- Run the real exact child with an intentionally stale selector. Its zero-test successful exit must be rejected. No roster rename may silently stop exercising a case.
- Assert literal expected decisions for good, failed, ignored, wrong-selector, duplicate-summary, zero-test and truncated libtest records. No newly ignored test is needed to produce an ignored-report control.
- Exercise actual child assertion failure, a finite over-cap stdout payload, a separately varied over-cap stderr payload, and a child that exceeds a short injected test deadline. Assert failure and observed reap, not merely a returned error.
- Check the production test-helper policy values against independent literals and use cap-minus-one, cap and cap-plus-one controls. Keep the three fixture deadlines independent from the short helper timeout test.
- Reject malformed/mismatched child markers without entering the fixture body or recursively spawning. The closed child test modes must perform no native process spawn.
- The fix must be sensitive to removal of delegation/body execution checks and result validation. A compilable passthrough helper must fail the parent-does-not-enter-body control; malformed/zero reports must fail if report validation is bypassed. Do not call compile errors or repeated stochastic passes a red/green regression proof.

## Acceptance And Verification

1. The implementation changes only the existing fixture-owned test code and focused test documentation. No production logic, dependency/pin, workload limit, resource ceiling, private frame or runtime environment interface changes.
2. All three original unit cases remain inline, discoverable and selected in their existing native feature lanes. Each parent invocation verifies exactly one matching child case; assertions and internal timeouts are unchanged.
3. The helper's negative controls fail closed and observe cleanup. A stale selector cannot pass, and no child output inflates the outer test count.
4. Run the owning fixture filter and full CLI binary in the default and existing insecure workspace feature graphs, without globally setting `RUST_TEST_THREADS` or reducing suite parallelism. Record dispatched names and actual results. Any declared repetition cohort records all results; no retry-until-green acceptance.
5. Run the unchanged prescribed workspace/secure/standalone CLI lanes, explicit once-owned compiler/producer owners, strict all-target Clippy, source-import gate, fixture gates and adapter checks from the current workflows. Local execution and hosted execution are distinct evidence. Under the existing verification-location decision, expensive unchanged owners may execute in the complete GitHub matrix, but missing or failed checks are not passing results.
6. Keep the confirmed macOS failure and deterministic inheritance diagnostic separate from implementation verification. Exercise the repair on macOS as well as the unchanged hosted Linux matrix; neither alone establishes the other's result.
7. Merge the reviewed planning PR before implementing in a separate bug-fix PR. After that fix lands on `main`, integrate it into PR #162 without force-push, refresh that PR's source-bound evidence and full checks, and obtain review of its resulting head. This planning document does not authorize any of those merges or transfer an older approval to a changed head.

## Non-Goals

No fixture server, transport or SDK changes; no replacement socket implementation; no global spawn lock; no suite-wide serialization; no new `#[ignore]`, skips or removed assertions; no extra compiler/artifact cache; no change to PR #166's dependency-upgrade cycle; no declaration that PR #162's unresolved local verification gate is satisfied.
