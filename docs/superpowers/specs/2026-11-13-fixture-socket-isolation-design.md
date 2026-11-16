# Isolate CLI socket-lifecycle unit checks

## Status And Scope

Scope amendment for [Bug #168](https://github.com/IABTechLab/auctioneer/issues/168) and [Task #174](https://github.com/IABTechLab/auctioneer/issues/174), under [development reliability](https://github.com/IABTechLab/auctioneer/issues/12). The original three-case specification and plan merged in [PR #169](https://github.com/IABTechLab/auctioneer/pull/169), commit `cc18818d9e742ea017b3b771c0cb485ef3f1e053`. On 2026-11-15 the user requested a combined #168/#174 delivery, then approved continuing from the written amendment at local commit `629ef40c0bf3a4b27262f630717506de4b429776` to revise the plan and publish the pair for review. This amendment covers six cases, one harness-owned test helper and verification through the existing Linux owners. It retains the previously approved child runtime, cleanup and capture bounds. Written-scope approval and the original three-case review do not authorize implementation or merge of this expansion.

**Hosting-cost amendment, 2026-11-15:** During PR #201 repairs, the user declined macOS GitHub runners because of their cost. Remove the proposed macOS job and required-check setting; retain the full Linux acceptance matrix and run the finite checker fixtures in its existing CLI lane. macOS execution is deferred and unverified. This does not turn Linux results into macOS evidence or waive PR #162's recorded acceptance failure.

The accompanying [implementation plan](../plans/2026-11-13-fixture-socket-isolation.md) replaces the merged base's three-case instructions with this combined scope. Review both revised documents together in one docs-only planning PR. Neither issue closes when planning merges. Implementation then belongs in one separate PR from updated `main`, referencing the merged amendment and delivering both issues.

The repair does not waive [PR #162](https://github.com/IABTechLab/auctioneer/pull/162)'s failed local check, approve that PR, change its code or resolve its review threads. Its full acceptance and current-head review remain separate gates.

The reviewed standard-library child ownership, bounded stderr diagnostics and directly awaited cleanup scope remain unchanged. Do not reopen those decisions or change the production process supervisor to combine the issues.

## Amendment Decisions

The combined implementation uses one helper, not two independently maintained runners. A fixture-local owner with wider ad hoc visibility would leave the native port test depending on a sibling subsystem; duplicating the runner would duplicate its cleanup and dispatch proofs. Place the private test-only owner at their existing common ancestor, `qualification/harness/mod.rs`, instead. Keep every original test and body at its current source owner.

The original Linux matrix and test selections remain mandatory. Add only the finite checker-fixture step to the existing CLI lane. Do not add a macOS runner, another job or repeated full Cargo suites. Keep the cohort script available for separately authorized focused verification; the user's GitHub-only execution instruction still forbids local CI checks during this delivery. This adds no crate, Cargo test target, production API or dependency.

## Problem And Evidence

Three inline CLI unit cases assert that a fixture listener is closed after EOF, cancellation or explicit stop. Other tests in the same libtest process spawn native programs concurrently. On macOS, currently locked Mio 1.2.4 sets close-on-exec separately from creating the socket. A concurrent child spawn can inherit the descriptor in that interval, retaining the listening socket after the fixture drops its own descriptor. Accepted sockets can encounter the same ownership issue.

The reviewer observed a post-shutdown connection succeed in PR #162's full insecure-feature CLI test process. The recorded result was `775 passed; 1 failed; 3 ignored`. Its EOF case ended with `unwrap_err()` receiving a successful `TcpStream`. A sibling server-stop failure was also reproduced at the base; the exact protocol failure was not, so its pre-existence at that base is not established.

A historical bounded diagnostic using the then-locked Mio 1.2.1 listener, without changing repository source, produced:

| Diagnostic condition | Completed checks | Connection after dropping listener |
| --- | ---: | --- |
| No child-spawn load | 100,000 | Not observed |
| Eight same-process spawn workers | 11,554 | Observed |
| Same spawn load in another process | 100,000 | Not observed |

At the observed failure, a diagnostic child still held the listening descriptor. All diagnostic children were stopped and reaped. This is evidence for cross-test descriptor inheritance, not a deterministic CI reproducer or proof that fixture shutdown itself is defective.

Six later unmodified CLI-binary runs each reported `776 passed; 0 failed; 3 ignored`. They establish neither a repair nor permission to disregard the reviewer's failure. The regression controls for this repair must test the isolation boundary deterministically, not require that a scheduling race happen.

### Additional Exposures Now In Scope

Three more tests in the same CLI unit binary and application feature graph depend on descriptor closure. The original plan intentionally left them to #174. This amendment proposes including all three in the same implementation, without representing their exposure as an observed failure:

| Owner, relative to `crates/auctioneer-cli/src/` | Full libtest selector | Exposed observation |
| --- | --- | --- |
| `qualification/harness/native/system.rs` | `qualification::harness::native::system::tests::reserve_port_releases_the_loopback_listener_before_returning` | `reserve_port` drops its loopback listener, then the test immediately rebinds that exact port; an inherited listener can prevent rebind |
| `qualification/harness/fixture/server/mod.rs` | `qualification::harness::fixture::server::tests::both_socket_populations_are_bounded_without_unbounded_waiting_tasks` | The server drops an over-capacity accepted stream and the client must observe `NetworkError` or `Disconnect`; an inherited accepted stream can delay EOF until the existing client timeout instead |
| `qualification/harness/fixture/server/mod.rs` | `qualification::harness::fixture::server::tests::header_timeout_releases_connection_capacity_without_dispatch` | The client must observe EOF within its existing 1,800-ms read deadline after the native fixture's 1-second header timeout; Hyper's error path drops the accepted stream without `shutdown()`, so an inherited copy can keep the read open |

REVIEWER verified all three code paths and demonstrated the listener and accepted-stream inheritance shapes with external models. All three actual tests passed in the reported second-round CLI run and hosted jobs; none is claimed to have failed. For the header-timeout case, the source path is Hyper 1.12.0's `poll_read_head` returning `HeaderTimeout`, through `Server::recv_msg` and `poll_catch`: the error returns before `poll_inner` reaches `poll_shutdown`, so dropping the socket does not shut down an inherited copy. The inventory covers post-close connection assertions, exact-port rebinds, EOF reads and client-observed server drops across the CLI's source and integration tests. Recheck these candidates and their owning assertions, not just a count of grep matches:

```sh
rg -n 'TcpStream::connect|TcpListener::bind|read_to_end|Category::NetworkError|Category::Disconnect' crates/auctioneer-cli/src crates/auctioneer-cli/tests
```

The public `qualification_harness` closure checks bind their listeners inside spawned CLI children, not in the parent test process; the HTTP disconnect case drops the client side. They are different ownership paths, not permission to expand the closed case set. Each proposed isolated child must remain free of native process spawning. Do not claim the search proves that every socket-related test in the repository needs or receives isolation.

## Chosen Boundary

Run exactly these six existing cases in fresh exact-test child invocations of the same compiled CLI unit-test executable:

| Owner | Full libtest selector |
| --- | --- |
| `qualification/harness/fixture/protocol.rs` | `qualification::harness::fixture::protocol::tests::private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof` |
| `qualification/harness/fixture/protocol.rs` | `qualification::harness::fixture::protocol::tests::malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener` |
| `qualification/harness/fixture/server/mod.rs` | `qualification::harness::fixture::server::tests::delay_applies_to_responses_and_stop_joins_held_connections_before_returning` |
| `qualification/harness/native/system.rs` | `qualification::harness::native::system::tests::reserve_port_releases_the_loopback_listener_before_returning` |
| `qualification/harness/fixture/server/mod.rs` | `qualification::harness::fixture::server::tests::both_socket_populations_are_bounded_without_unbounded_waiting_tasks` |
| `qualification/harness/fixture/server/mod.rs` | `qualification::harness::fixture::server::tests::header_timeout_releases_connection_capacity_without_dispatch` |

Paths in this table are relative to `crates/auctioneer-cli/src/`. The names, attributes, feature gates, original bodies and assertions stay with these inline tests. Their parent invocations remain selected by the ordinary suite. They delegate the original body to the exact child; they do not become ignored tests or silently return without verification.

Add one private, inline `#[cfg(test)] mod socket_test_support` under the existing `qualification/harness/mod.rs`. Its own unit controls also stay inline. Give only the case type and runner entry points the visibility needed within the harness subtree; supervision, report parsing and capture internals stay private to the helper. The fixture protocol/server tests and native system test call it through ordinary module paths. There is no shared library API, cross-crate include, production process framework or separate helper implementation. The existing `application` gate continues to own the qualification harness; maintenance-only and non-test builds acquire no helper code or application test dependency.

The parent suite remains parallel. The selected child runs one test with one libtest thread and does not launch native subprocesses. Existing Tokio tasks inside that test are retained. For these six cases only, this separates listener creation from unrelated native spawning without globally serializing the suite or changing fixture shutdown. Source review must confirm the full bodies and their called helpers still satisfy the no-descendants assumption; selecting a test name alone does not prove it.

## Parent And Child Contract

1. Represent the six selectors as a closed test-only set, with exactly one mapping per case and no duplicate selector. Independently compare that mapping against the six literal names above, and verify every owning wrapper selects its corresponding case. The helper's negative-control entry points are likewise fixed test names, not arbitrary commands or filters. No executable path, shell fragment or selector comes from operator input.
2. Invoke `std::env::current_exe()` directly with `--exact`, the full selector, `--test-threads=1`, `--nocapture` and `--color=never`. Do not invoke Cargo, a shell, a second build or a guessed binary path.
3. Use the child-only marker `AUCTIONEER__TEST__FIXTURE_SOCKET_CASE`, with no legacy alias. Clear inherited environment and pass only that marker. The selected pure-Rust loopback cases require neither source credentials nor operator configuration. No global environment mutation occurs in the parallel parent.
4. In the child, validate the marker, exact selector and single-test arguments before constructing fixture resources. A malformed marker, mismatched selector or recursive spawn attempt fails. In the parent, do not construct or poll the fixture body. A deferred body factory separates parent delegation from child execution.
5. Execute the original case body exactly once in the matching child. Panic, assertion failure and runtime error remain failures. Do not catch an assertion and convert it to success.
6. A successful exit is necessary but insufficient. Validate the child's bounded stdout: exactly one `running 1 test` record, exactly one success for the requested full selector and one complete successful summary with one passed, zero failed, zero ignored and zero measured. The filtered count can vary with the binary's test roster. Reject zero-test, mismatched, duplicated, ignored, truncated and malformed result records.
7. Consume successful child output in the parent. Do not forward nested `test result:` summaries into the outer suite's accounting. On a child-exit failure, the parent diagnostic must identify the fixed case and failure category and include the child's bounded stderr, preserving its panic location and message. Read it only after confirmed reap through the existing bounded capture path; an oversized or unreadable capture still refuses as that error, without bypassing its bounds. Do not attach the child's stdout to any failure diagnostic. No diagnostic may retain or forward more than 16,384 bytes of child stderr; other capture/cleanup failures remain explicit rather than inventing missing panic details.

No original test assertion is replaced by a process exit or output marker. The child report proves dispatch; the retained case body still proves closure and the protocol/lifecycle contract.

## Runtime, Capture And Cleanup

The outer execution watchdog remains **20,000 ms per child**, followed by a separate **2,000 ms kill/reap fence** on failure or timeout. The EOF case already contains three successive five-second waits. The capacity case also contains successive five-second observation waits and 20,000-ms held-request timeouts. The outer watchdog bounds the entire invocation and startup; it is not the sum of those independent maxima, a promise that every inner timer can fully elapse, or an extension to any original wait. Its expiry is an explicit parent failure, never an accepted client timeout or a shortcut around later assertions. Preserve the 1,800-ms header EOF deadline, 20,000-ms held-request timeouts, all existing observation waits, the 40-ms delay assertion and immediate post-stop checks. Do not retune either inner or outer values to make a run pass; any necessary change returns to design review.

Anchor the execution deadline before spawn and check it immediately after spawn returns. Synchronous OS spawn and other synchronous system calls are not preempted by Tokio timers; this is an observed deadline, not a hard real-time OS guarantee. Startup must not reset the child budget. Anchor the separate cleanup deadline before requesting termination and do not reset it while polling for reap.

Use `std::process::Command` and retain its returned `std::process::Child` immediately in one private owner, before any fallible post-spawn work. Use the existing Tokio timers to poll `try_wait()` and the existing `tempfile` dependency for captures. Do not convert the child to `tokio::process::Child`: currently locked Tokio 1.54.1 spawns the OS process before fallible exit-notification registration, and installs its kill-on-drop guard only afterward. This ordering remains the same as the historical 1.52.3 implementation examined for the initial design. An error at that intermediate stage can lose the handle to an already-running child. No such registration or ownership-transfer stage is needed for regular-file captures and nonblocking `try_wait()` polling.

Give the private owner a best-effort kill-on-drop fallback, but successful cleanup still requires observing the direct child's exit/reap through `try_wait()`. Do not use blocking `wait()`, blocking wait threads, detached cleanup/reader tasks or an orphan queue. The selected children and finite negative-control children do not create subprocess descendants, so this is direct-child ownership, not a claim of arbitrary process-tree supervision.

Capture stdout and stderr in private temporary regular files. While waiting, observe their lengths and reject either stream above **16,384 bytes**. After exit, read each from the beginning with at most **16,385 bytes** (cap plus one sentinel) and refuse overflow before parsing. Retained accepted output is at most 32 KiB combined. On every successful or refused return, remove the files through their scoped owner after confirmed reap; if the cleanup fence expires, report cleanup failure instead of claiming that ordering was achieved.

The bounded reap and post-reap capture-removal contract applies to directly awaited invocations that return success or refusal. All six wrappers must not spawn, abort, time out or drop the helper future externally. Parent panic, caller cancellation, runtime teardown and process termination are outside that awaited guarantee: dropping the owner only requests termination best-effort and does not prove bounded reap or post-reap capture removal. Such a path produces no verified success receipt. This helper is not a cancellation-safe general-purpose process supervisor.

These are memory/read limits and monitored file-size limits for trusted compiled test cases, **not an operating-system disk quota**: a writer can overshoot between observations. Negative controls emit finite cap-plus-one payloads rather than an unbounded disk flood. Neither success nor overflow handling waits for pipe EOF.

The existing production `native_control::process::Supervisor` was considered. It always captures pipes and waits for reader cleanup; a pipe descriptor inherited by an unrelated same-process child can prolong EOF independently of the selected child's exit. Extending that production API or weakening its cleanup fence is outside this repair. Private regular-file capture avoids introducing that second dependency on descriptor-inheritance timing. Do not refactor other subprocess owners here.

Any spawn, wait, capture, report-validation or cleanup error fails the parent case. After standard-library spawn returns a child, every subsequent error, including supervision setup failure, keeps that same owner and enters the cleanup path. On an active-child refusal, request termination and poll `try_wait()` within the cleanup fence before returning failure. A failed kill request must not bypass the reap attempt. If the operating system does not confirm reap, report cleanup failure; never claim successful cleanup. There are no retries, port-release sleeps, accepted post-stop connections or fallback success paths.

The controls must distinguish forced termination from natural exit. For setup failure, timeout and each active-stream overflow, use a finite child that signals readiness through a capture file and then remains alive for the existing 20,000-ms hold. The injected execution deadline plus the unchanged cleanup fence must be shorter than that hold. Confirm `try_wait()` still reports a live child before injecting the setup fault or judging the active overflow. On Linux and macOS, require a termination-signal exit status as well as observed reap; a natural successful exit inside the cleanup fence is not equivalent evidence. Test the bounded file-reading component used after confirmed reap directly with completed regular files at 16,383, 16,384 and 16,385 bytes. Do not claim that an overflow-writing child naturally exits before the monitor observes its output: the monitor can win that race. Any additional finite child that writes and exits may assert size refusal, not which observation path won. These controls introduce no production limit or unbounded writer.

Private cleanup fault controls must also cover a failed kill request followed by confirmed reap, an exhausted reap fence, and capture-removal failure. The first must prove polling continued; the second must report cleanup failure ahead of the original setup/timeout/overflow reason, retaining that original cause; the third must report a capture-cleanup failure after confirmed reap. Use finite completed-state/clock controls for an unconfirmed-reap outcome, not an abandoned real process. Explicit checked removal is required: `TempDir` drop alone ignores removal errors and cannot establish this contract. These controls exercise the same private finalization path as the real runner, not a parallel test-only cleanup algorithm.

## Assertions That Must Survive

| Case | Retained independent observations |
| --- | --- |
| EOF | Valid ready protocol/port; connection succeeds before EOF; stats command and exact reply keys; input EOF joins serve; a new connection fails after join; exact ready JSON |
| Invalid frames/cancellation | Every malformed-input canary is refused with the existing fixed error and no output; interrupt 143 yields `interrupted`; serve joins; a new connection fails; output reaches EOF |
| Delay/stop | Response delay is at least 40 ms; held request exists; pre-stop counters are checked; stop joins; stopped state is exactly `(false, 0, 1, 0)`; held request is not successful; new connection fails; exact address can be rebound |
| Port reservation | The returned port is at least 1024 and not 8787; rebinding the exact port succeeds; the bound listener reports that same port |
| Both capacity populations | Both native and provider loop rows remain; capacity is exactly 2 and 8 respectively; held counts reach capacity and every held task stays pending before and after the over-capacity request; that request is `NetworkError` or `Disconnect`, never `Timeout`; calls/dispatches equal capacity; stop releases every hold; the stopped `(running, active, completed, disconnected)` tuple is `(false, 0, 0, capacity)` for native and `(false, 0, 0, 0)` for provider; every joined held task is not successful |
| Header timeout | EOF is observed within the existing 1,800-ms `read_to_end` deadline; no request was dispatched; the next real request succeeds; stop completes. Keep this error/drop path independent from the over-capacity rejection case |

Review the original and wrapped bodies directly, including every loop row and before-state assertion. Preserve existing test names and gates in the before/after roster. New helper controls are additive, not substitutes for these observations.

CREATOR owns a one-time, Git-only body-preservation record for the implementation PR, and REVIEWER independently checks it. Record the full pre-cutover implementation-base SHA before editing, then inspect unnormalized Git function-context diffs for all three owning source files against that exact object. Record the exact commands, output and a six-case verdict covering attributes and complete deferred bodies. This direct source review is permitted locally by the user's CI-only execution constraint; it runs no test, script or compiled comparison program. Do not claim automatic token equality, ignore whitespace inside literals, or substitute the current wrapper-shape guard for historical comparison. No committed golden source, new baseline environment variable or runtime Git dependency is needed. The plan specifies the commands and evidence owner.

## Deterministic Regression Controls

- Prove parent delegation does not construct or poll the body factory, and matching child dispatch executes it exactly once. Include a positive witness so an unconditional early return cannot pass.
- Run the real exact child with an intentionally stale selector. Its zero-test successful exit must be rejected. No roster rename may silently stop exercising a case.
- Assert literal expected decisions for good, failed, ignored, wrong-selector, duplicate-summary, zero-test and truncated libtest records. No newly ignored test is needed to produce an ignored-report control.
- Exercise actual child assertion failure, independently varied active stdout/stderr overflow, and a child that exceeds a short injected test deadline. The active controls must prove forced termination and reap under the runtime contract above, not merely a returned error or natural exit. Direct regular-file boundary controls exercise the same reader used after confirmed reap without a live child or monitor race; they are component coverage, not a claim that the real runner selected a particular post-exit branch. For the assertion-failure control, require the parent's diagnostic to contain the child's panic location and fixed panic message from bounded stderr, and to contain neither its stdout canary nor a nested `test result:` summary.
- Inject a private supervision-setup failure after a real child has been spawned, retained and confirmed alive. Require the setup refusal, forced termination and observed OS exit/reap for that same child before capture removal. Add the kill-error/reap-fence/removal-failure controls above. The injections must not add a production hook or release ownership to Tokio registration.
- Check the test-helper policy values against independent literals and use cap-minus-one, cap and cap-plus-one controls. Keep all six cases' inner deadlines independent from the short helper timeout test.
- Reject malformed/mismatched child markers without entering the fixture body or recursively spawning. The closed child test modes must perform no native process spawn.
- The fix must be sensitive to removal of delegation/body execution checks and result validation. A compilable passthrough helper must fail the parent-does-not-enter-body control; malformed/zero reports must fail if report validation is bypassed. Independently pin the required report-control row identities so deleting a row fails a roster assertion. Isolate witness validation with an otherwise valid status/report receipt and correct zero parent counters, varying only the missing witness. Rejecting an accept-all candidate demonstrates a whole-class defect, not independent sensitivity of every parser branch. Do not call compile errors or repeated stochastic passes a red/green regression proof.

## Hosted Verification Ownership

Current `test.yml` runs its native jobs on `ubuntu-latest`. Its existing standalone CLI, default workspace and insecure workspace owners execute the six wrappers and helper controls. Keep their complete test selections and ordinary parent parallelism. Add the finite cohort-checker fixtures to the existing CLI lane without another Cargo build, repeated full test suite or new runner.

The retained focused cohort command establishes the actual test roster, requires each of the six full selectors exactly once and nonzero helper-control dispatch, then executes that selected parent cohort with ordinary libtest parallelism. Each selected real wrapper launches one exact single-threaded child. Do not globally set `RUST_TEST_THREADS`, run six direct bodies in place of their wrappers, or use six independent zero-match-permitted Cargo filters as acceptance. Record selected names, outer summaries, exit status and the tested source SHA when that command is separately authorized. Reject missing or ignored selected cases and selection/count mismatches rather than accepting a successful zero-test exit.

Read tool versions from `.tool-versions` and dependencies from the locked workspace. Preserve the download-only cache policy and do not introduce a compiler/artifact cache. Keep substantial selection/result-checking shell beside the workflow under `.github/workflows/scripts/test/`, reusing compatible existing result checks. A missing runner, setup failure or unexecuted required Linux job is blocked verification, not a pass. Existing job and child limits remain unchanged.

There is no added macOS job or required-check context. Preserve every existing required context and strict up-to-date policy; change no repository setting. Removing the proposed runner follows the user's hosting-cost decision, not a claim that the platform-specific mechanism has been tested. Keep macOS verification outstanding under #174 and PR #162's acceptance decision until a separate authorized run establishes it.

The user has directed CREATOR not to run CI checks locally. Keep local work to source inspection and Git/document review; execute the complete existing Linux test/lint/build matrix in GitHub at the implementation head. Do not present historical local diagnostics, pre-repair passes or Linux jobs as new macOS repair evidence. All behavioral negative controls remain required in the existing Linux owners; the hosting-cost amendment does not remove assertions or permit skipped controls.

Harness source and executable bindings change when the implementation is committed, even for test-only source changes. Preserve historical evidence bytes and record new evidence against the new head where required; do not rewrite old digests to make them current. PR #162's source-bound evidence is refreshed only after the combined fix is integrated there.

## Acceptance And Verification

1. The implementation changes only the harness-owned test helper, the six existing inline unit cases, focused documentation, colocated checker/cohort scripts and a finite checker step in the existing Linux CLI owner. Add no macOS job. Existing Linux test selections are unchanged. No production logic, dependency/pin, workload limit, resource ceiling, private frame or runtime environment interface changes.
2. All six original unit cases remain inline, discoverable and selected in their existing native feature lanes. Each parent invocation verifies exactly one matching child case; assertions and internal timeouts are unchanged. No whole fixture or native module is serialized or moved to another Cargo target.
3. The helper's negative controls fail closed and observe cleanup, including forced termination of active children and the kill/reap/removal failure paths. Required report rows cannot disappear silently, and the witness-only refusal is independently exercised. A stale selector cannot pass, and no child output inflates the outer test count.
4. Execute the full CLI binary in the existing hosted default and insecure lanes, without globally setting `RUST_TEST_THREADS` or reducing suite parallelism. Record dispatched names and actual results. Any declared repetition cohort records all results; no retry-until-green acceptance. Keep focused cohort execution available for separately authorized verification, not an added hosted requirement.
5. Execute the complete prescribed GitHub matrix at the implementation head: workspace/secure/standalone CLI lanes, explicit once-owned compiler/producer owners, strict all-target Clippy, source-import gate, fixture gates and adapter checks. Run the finite checker fixtures in the existing CLI lane. Missing or failed required jobs are not passing results.
6. Keep the confirmed macOS failure and historical inheritance diagnostic separate from implementation verification. macOS execution is deferred at the user's direction and remains unverified; Linux success does not establish it. All six Linux parent cases must execute their original bodies in verified children. PR #162's platform acceptance decision remains separate.
7. Merge the reviewed scope-amendment planning PR before implementing in a separate bug-fix PR delivering both #168 and #174. After that fix lands on `main`, integrate it into PR #162 without force-push, refresh that PR's source-bound evidence and full checks, and obtain review of its resulting head. A failure of any of the six cases remains reportable and blocking under PR #162's gate; merely including it in this design does not dismiss or repair it. Neither planning merge nor the original PR #169 approval authorizes those later merges or transfers an older approval to a changed head.
8. Record the Git-only preservation review against the pre-cutover implementation base and require REVIEWER to verify all six bodies. The complete existing required matrix must pass at the reviewed head. Record the hosting-cost amendment and deferred macOS verification explicitly. Change no repository settings; planning approval does not authorize implementation merge.

## Non-Goals

No fixture server, transport or SDK changes; no replacement socket implementation; no global spawn lock; no suite-wide serialization; no new `#[ignore]`, broad-lane skips or removed assertions; no extra compiler/artifact cache; no recapture of historical evidence; no unrelated changes to PR #166's dependency-upgrade cycle; no declaration that PR #162's unresolved verification gate is satisfied.
