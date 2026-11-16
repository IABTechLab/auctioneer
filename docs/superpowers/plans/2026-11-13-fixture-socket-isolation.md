# CLI socket isolation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Isolate the six existing #168/#174 CLI socket-lifecycle cases from unrelated child spawning, retaining complete assertions, internal deadlines and ordinary parent-suite parallelism.

**Architecture:** One private harness-owned test helper invokes the existing CLI unit binary with one closed exact selector. It immediately retains the standard-library child, defers the original body to that child, monitors private regular-file captures, verifies one successful dispatch, and confirms exit/reap. A focused macOS CI job exercises the six parents and helper controls in both feature graphs; the complete existing Linux matrix remains unchanged. Serving code and the production process supervisor are unchanged.

**Tech Stack:** Existing Rust/libtest, `std::process`, Tokio time, `tempfile`, existing test dependencies `syn`/`quote` for Rust-source assertions, and colocated Bash workflow scripts. No new dependency, crate, Cargo target, handwritten source import or production API.

---

## Contract And Execution Gate

Issues: [#168](https://github.com/IABTechLab/auctioneer/issues/168) and [#174](https://github.com/IABTechLab/auctioneer/issues/174). Specification: [CLI socket isolation](../specs/2026-11-13-fixture-socket-isolation-design.md). The original three-case pair merged in PR #169 at `cc18818d9e742ea017b3b771c0cb485ef3f1e053`. The user approved continuing from the written six-case amendment on 2026-11-15 to this revised plan and a docs-only planning PR. This replaces the merged three-case plan, not its historical evidence. Every result below is **expected**, not executed evidence.

The reviewed immediate standard-child ownership, directly awaited cleanup scope, 20,000-ms execution watchdog, separate 2,000-ms reap fence and 16,384-byte per-stream cap remain unchanged. Combining the issues is not permission to retune these bounds or use the production process supervisor. The review repairs add active-termination and cleanup-failure controls, an independently pinned report-row roster, a Git-only preservation-review owner, and an explicit approval-gated macOS required-check step. They authorize no production implementation or repository-settings mutation.

Publish the spec and plan together in a separate planning PR with `Refs #168` and `Refs #174`. After current-head review, full hosted checks and explicit merge authorization, create a fresh implementation worktree from updated `origin/main`. Deliver both issues in one separate implementation PR. Do not implement on this planning branch or fold the repair into PR #162 before the implementation has landed. Use @superpowers:test-driven-development, @superpowers:verification-before-completion and PRfection CREATOR thread/merge rules.

The user directed CREATOR not to run CI checks locally. Every Cargo/test/fmt/lint/script command below is a **future hosted command**, not a direction to run it on the shared workstation. Local actions are source inspection, document review and ordinary Git operations only. Full verification remains mandatory in GitHub, and absent/stopped/failed jobs are not passing. Hosted compiling counterexample controls in Task 3 retain the independent red/green requirement without publishing a broken outer suite or mutating another worktree.

The diagnostic in the spec establishes an interference mechanism, not a deterministic reproduction of the exact protocol failure at main. Do not recreate the race in CI or rerun a failing suite until it happens to pass. PR #162's recorded failure stays an acceptance blocker until the independently reviewed repair is integrated and that resulting head is verified.

This amendment adds #174's exact-port rebind, capacity-population and header-timeout EOF cases to the original three. The spec names all six full selectors and their observations. It distinguishes #174's exposures from measured failures and does not claim an exhaustive proof that no other test could be exposed. Scope inclusion does not repair or dismiss a failure; every original assertion still executes in a verified child.

## File Ownership

| File | Intended implementation change |
| --- | --- |
| `crates/auctioneer-cli/src/qualification/harness/mod.rs` | One private inline `#[cfg(test)] mod socket_test_support`, its closed map, runner/captures, strict report parser and inline controls |
| `crates/auctioneer-cli/src/qualification/harness/fixture/protocol.rs` | Wrap only the two named protocol cases from the spec; keep their names, gates, bodies and assertions inline |
| `crates/auctioneer-cli/src/qualification/harness/fixture/server/mod.rs` | Wrap the delay/stop, capacity-populations and header-timeout cases; preserve both native/provider loop rows |
| `crates/auctioneer-cli/src/qualification/harness/native/system.rs` | Wrap only the port-reservation/rebind case; retain its complete inline body |
| `docs/testing/development.md` | Briefly explain exact-child ownership, fixed test-only bounds and the unchanged parallel parent suite |
| `.github/workflows/test.yml` | Add one focused macOS job; existing Linux jobs, feature selections and gates remain unchanged |
| `.github/workflows/scripts/test/verify-cli-socket-isolation.sh` | New hosted cohort selector/executor, roster/name accounting and reuse of the existing summary checker |
| `.github/workflows/scripts/test/check-cli-socket-isolation-fixtures.sh` | Finite selection/report controls for the cohort checker; no compiler builds or substitute test results |

The spec and this plan remain committed contracts. No edits to package manifests, the lockfile, SDK pins, the fixture serving implementation, `native_control/process.rs`, other Rust test owners or historical evidence are authorized. All implementation Rust remains test-only. Expose only the case type and runner within the harness subtree; supervision, capture and parser internals stay private to the helper. Keep the existing `application` gate on `qualification` and `cfg(test)` gate on the helper; do not add a library export, handwritten source import or maintenance-build dependency. `fixture/mod.rs` needs no change. Task 6 separately proposes adding one required status-check context in repository settings; that is not a file change and needs explicit user approval before an authorized maintainer performs it.

## Task 1: Freeze The Roster And Current Failure Evidence

**Read:** The four Rust owners above, `AGENTS.md`, `crates/AGENTS.md`, the CLI feature declarations, and the acceptance/prerequisite sections of `docs/testing/development.md` and current workflow/scripts.

- [ ] Record the implementation base SHA, the landed amendment SHA, clean worktree state and enabled features. Inspect changes since the base before reusing source lines or dependency citations. Do not clean shared build outputs or run a local build as preparation.
- [ ] In each hosted owner, acquire the existing pinned prerequisites from their owning declarations. Missing tools/source objects fail instead of skipping a test. Do not commit machine paths or credentials, and do not copy Linux-only provider installers into the focused macOS job.
- [ ] List the actual CLI unit roster in both existing feature graphs:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli -- --list
cargo test --locked --offline --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli -- --list
```

**Expected:** Every full selector in the spec occurs exactly once per graph. Record the total roster and existing ignored cases before adding controls; a zero-match filter is failure.

- [ ] For baseline diagnostics, the existing fixture filter does not select the native port case. If a hosted pre-repair diagnostic is needed, add its exact native selector rather than claiming the fixture filter covers all six:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli qualification::harness::fixture::
cargo test --locked --offline --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli qualification::harness::fixture::
```

**Expected:** These filters dispatch the five fixture-owned cases, not the native-system case. Task 5's cohort selects all six. Retain every diagnostic failure; a pre-repair pass is not a fix. Historical planning/reviewer runs stay attributed to their exact commits and executors, not reused as new implementation evidence.

- [ ] Before editing any wrapper, record `git rev-parse HEAD` as the full `PRESERVATION_BASE` SHA in the implementation PR. That immutable commit is the original-source input for Task 4's Git-only comparison; it is not the later PR head or an automatically moving merge base. Confirm `git cat-file -e "$PRESERVATION_BASE^{commit}"` succeeds. Preserve the original six functions, attributes and bodies in that object; do not commit duplicate golden bodies, source imports, raw JSON snapshots or generated Rust strings. If another reviewer lacks that object, fetch the recorded SHA explicitly from `origin` and record the fetch before comparison; a shallow checkout is not assumed to contain it.
- [ ] Recheck the spec's exposure inventory and all six complete call paths. Exact-port rebinds, client-observed server drops and header-timeout EOF reads require ownership review; a grep count is not a proof. Selected child bodies must launch no OS subprocess; preserve Tokio tasks.
- [ ] Freeze the independent variant/selector map below. Compare against literal names, not values generated by the implementation:

| Variant | Function suffix under `qualification::harness::` |
| --- | --- |
| `ProtocolEof` | `fixture::protocol::tests::private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof` |
| `ProtocolMalformed` | `fixture::protocol::tests::malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener` |
| `FixtureStop` | `fixture::server::tests::delay_applies_to_responses_and_stop_joins_held_connections_before_returning` |
| `PortRebind` | `native::system::tests::reserve_port_releases_the_loopback_listener_before_returning` |
| `SocketCapacity` | `fixture::server::tests::both_socket_populations_are_bounded_without_unbounded_waiting_tasks` |
| `HeaderTimeout` | `fixture::server::tests::header_timeout_releases_connection_capacity_without_dispatch` |

## Task 2: Specify And Test Exact-Child Dispatch

**Modify/test:** Only the new inline `socket_test_support` module in `crates/auctioneer-cli/src/qualification/harness/mod.rs`.

- [ ] Add a closed case representation for the six full selectors in Task 1 and separate fixed helper-control entry points. The normal entry takes a deferred `FnOnce() -> Future<Output = ()>` body; it does not require `Send` or construct resources before role selection. Return private execution information distinguishing a verified parent from an executed child; expose no arbitrary selector/command API or raw output outside tests.
- [ ] Add literal transcript controls before implementing report acceptance. A minimal successful stdout control has this structure (the actual selector must be one of the closed names):

```text
running 1 test
test <full selector> ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 779 filtered out; finished in 0.01s
```

The filtered count is an arbitrary test literal, not a pinned total or the current roster size. Test another count independently. Assert exact acceptance/refusal for failed/ignored/zero results, wrong names, missing or duplicated records, truncated summaries, trailing records and invalid UTF-8. Do not use `is_err()` where the helper has distinct refusal categories.

- [ ] Write an independent report-contract driver before the real parser, then implement complete validation: one run announcement, one requested-case success, one successful 1/0/0/0 summary, valid numeric filtered/duration fields, no extra result records. Whitespace-only separators are allowed. Task 3 runs that same driver against a compiling accept-all candidate and the real parser; do not publish a permissive stub as successful verification.
- [ ] In `report_control_roster_is_complete`, compare the report table's row IDs against this separately written literal set, rejecting duplicates or omissions. Delete each row in turn from an in-memory copy and require the roster guard to reject it; the unmodified table must pass. These are table controls, not source mutations or extra compiler runs:

```text
success, success_other_filtered, success_whitespace,
failed, ignored, measured, zero_tests, multiple_tests, wrong_selector,
missing_run, duplicate_run, missing_case, duplicate_case,
missing_summary, duplicate_summary, truncated_summary, trailing_result,
invalid_utf8, invalid_filtered, invalid_duration
```

Each of these 20 rows has its own literal expected decision. Keep that expectation independent of the parser and compare every actual verdict against it. Rejecting the accept-all candidate does not by itself pin an individual row's continued presence.
- [ ] Add child-role controls: clear environment, pass only `AUCTIONEER__TEST__FIXTURE_SOCKET_CASE`, validate its closed value and the exact single-test arguments, and reject malformed/mismatched controls before calling the factory. Use `Command::env_clear()`/`Command::env()` on the child, never process-global `set_var`.
- [ ] Add a parent/child delegation control using the same entry path as the real cases. The body factory increments a local construction counter; its future increments a separate poll counter. In child mode both must equal one; in parent mode both must stay zero. Emit a fixed completion witness on **stderr** from inside the successfully executed probe body, and require exactly that witness in the parent's bounded receipt. Stderr avoids interleaving witness text into libtest's stdout status line. An unconditional parent return cannot supply that positive witness.
- [ ] Write the parent/child contract driver before the runner. Task 3 must exercise its guard failure against compiling passthrough and empty-success candidates as hosted negative controls, then require the real runner to satisfy the same independent driver. Missing symbols or compiler failures do not count.
- [ ] In `witness_only_candidate_is_refused`, give the private completed-receipt validator a successful status, a complete valid one-case report and correct zero parent construction/poll counts. Supply the fixed stderr witness exactly once for the positive control, then vary only its absence for the negative control and require the exact missing-witness refusal. The otherwise-valid empty-success candidate must reach that same refusal, not fail because status/report fields were also omitted. Keep the real-child positive dispatch in the driver too; this completed-state control is not a substitute for it.
- [ ] Add a pure Rust-source guard using the existing `syn`/`quote` dev-dependencies and `include_str!` as data. Independently assert that each of the six literal owning functions is one directly awaited helper wrapper, selects its matching case and defers its complete body. Reject work before delegation, a passthrough body, missing/duplicate owners and ignored/disabled wrappers. Add in-memory parsed-source controls removing a wrapper, changing its case and disabling it, plus a positive check against the real source. This is not a handwritten source import or a workflow snapshot; it does not replace Task 4's original-body comparison.

Keep these required helper test names under `qualification::harness::socket_test_support::tests::`, independently of the runtime-discovered roster:

```text
closed_case_roster_matches_six_owned_tests
report_acceptance_requires_one_exact_success
child_role_rejects_invalid_controls_before_body
parent_delegation_defers_body_and_child_runs_it_once
owned_wrappers_delegate_before_constructing_resources
wrapper_guard_rejects_missing_mismatched_and_disabled_cases
zero_test_child_success_is_refused
child_assertion_failure_preserves_only_bounded_stderr
post_spawn_setup_failure_retains_owner_until_reap
timeout_refuses_and_observes_reap
stdout_and_stderr_overflow_are_independent_refusals
capture_reads_pin_cap_minus_one_cap_and_plus_one
helper_policy_defaults_match_independent_literals
contract_controls_reject_passthrough_empty_success_and_accept_all
cleanup_polls_after_kill_error
cleanup_fence_failure_overrides_original_refusal
capture_removal_failure_is_reported
report_control_roster_is_complete
witness_only_candidate_is_refused
```

Additional owner-local controls are allowed and must be selected too. Deriving the mandatory list from discovery would not detect deletion or rename.

Future hosted focused commands:

```sh
cargo test --locked -p auctioneer-cli --bin auctioneer-cli qualification::harness::socket_test_support::tests::
cargo test --locked --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli qualification::harness::socket_test_support::tests::
```

**Expected:** Nonzero helper dispatch and every required control accounted for. Tasks 2-4 form one coherent implementation checkpoint: the source guard also needs all six wrappers. Keep the tests written first, but do not publish a broken checkpoint or claim tests have passed before hosted execution.

## Task 3: Implement The Bounded Test-Only Child Owner

**Modify/test:** The same inline `socket_test_support` owner at the harness common ancestor. Do not modify the production supervisor.

- [ ] Build a `std::process::Command` directly from `std::env::current_exe()` with `--exact`, the closed selector, `--test-threads=1`, `--nocapture`, `--color=never` and null stdin. Clear inherited environment as specified in Task 2. Use no shell, Cargo invocation, guessed executable or native subprocess inside a selected child.
- [ ] Create a private scoped temporary directory with distinct stdout/stderr regular files. Attach file handles to the child. Keep readable ownership that can start at byte zero after exit: cloned descriptors share offsets, so explicitly seek or reopen appropriately. Do not create capture pipes or background reader tasks.
- [ ] Retain the returned `std::process::Child` immediately in one private owner before any fallible post-spawn operation. Do not transfer it to `tokio::process::Child` or an orphan queue: locked Tokio can fail during registration after OS spawn, before installing its drop guard. Add a best-effort kill-on-drop fallback to the private owner without treating it as observed reap.
- [ ] Anchor one monotonic 20,000-ms execution deadline before spawn and check it immediately after spawn returns; synchronous OS spawn is not preemptible and must not receive a fresh budget afterward. Poll `Child::try_wait()` and both capture lengths with existing Tokio timers at intervals no greater than 10 ms, without extending the absolute deadline. A length above 16,384 bytes refuses that stream. Use no blocking `wait()` or blocking wait thread. The interval does not grant a disk quota; a finite writer may overshoot before observation.
- [ ] On every post-spawn setup error or active-child refusal, retain the same owner, request termination, and poll `try_wait()` with a separate absolute 2,000-ms cleanup fence anchored before the kill request. A kill error must not bypass polling for reap. On every returning path observe whether `try_wait()` confirmed exit and reap. Surface cleanup failure if not; do not turn dropping a handle, a kill flag or a successful signal into a reap receipt. Do not assume a fallible post-spawn operation failed before a child existed.
- [ ] Add only private operation/clock seams around this same finalization path. In `cleanup_polls_after_kill_error`, inject a kill error followed by observed exit/reap and assert that polling still occurred and the original refusal was retained with the kill error as context. In `cleanup_fence_failure_overrides_original_refusal`, supply finite wait results and a controlled clock that exhausts the fence; assert cleanup failure is primary and the original refusal is retained as its cause. Do not leave an actual child unreaped to test that branch or implement a separate test-only finalizer.
- [ ] After confirmed exit, read each regular file from its beginning with a 16,385-byte maximum, and reject cap-plus-one before parsing. A successful status plus the strict report check from Task 2 is required. Accept at most 16,384 bytes per stream; consume successful nested reports instead of reprinting them into the outer harness. For child-exit failure, include the fixed case/category and the bounded stderr in the parent error so the original panic location and message survive. Never embed child stdout in a failure message. Oversized/unreadable capture remains an explicit refusal, not a bypass for diagnostics. Explicitly close/remove temporary captures after reap. Cleanup or file-operation failure remains failure.
- [ ] In `capture_removal_failure_is_reported`, inject removal failure after confirmed reap into the real finalization path and assert the capture-cleanup category plus its original cause, never success. Include a positive removal witness and assert removal is not attempted before the reap receipt. Use explicit checked cleanup such as `TempDir::close()` rather than relying on its error-ignoring destructor. Pure ordering controls do not remove real files belonging to an unreaped process.
- [ ] Keep every wrapper and ordinary helper control directly awaiting the owner through its success/refusal return. Do not externally abort, time out or drop that future, or introduce detached cleanup. Document that parent panic, caller cancellation, runtime teardown and process termination have only best-effort drop cleanup, not bounded reap or post-reap capture-removal guarantees, and produce no verified success receipt.
- [ ] Exercise the real runner with fixed inline helper modes, all inside tests that perform real parent assertions when not in child mode:

| Control | Required independent observation |
| --- | --- |
| Positive exact dispatch | Actual child status success, exactly one requested test, probe witness present once, parent construction/poll counters both zero |
| Static nonexistent selector | Actual child exits successfully after zero tests; runner refuses it as dispatch failure |
| Assertion failure | Actual child fails; runner refuses with child-exit failure and confirms reap; parent diagnostic contains the child's panic location and fixed panic message from stderr, but neither the independent stdout canary nor any nested `test result:` summary |
| Post-spawn setup failure | Retain the real child, observe its readiness and `try_wait() == None`, then inject setup failure; require the setup refusal, signal-termination status and observed reap of that same child before capture removal |
| Timeout | A ready, live child holds for the existing 20,000 ms, longer than the injected execution deadline plus the unchanged cleanup fence; require timeout refusal, signal-termination status and observed reap within the cleanup fence |
| Active stdout overflow | Ready child emits a finite 16,385-byte stdout payload and holds for the existing 20,000 ms; confirm it is alive at overflow, then require size refusal, signal-termination status and observed reap |
| Active stderr overflow | Vary only the overflowing stream independently; require the same live-child and forced-termination/reap observations, not a natural exit |
| Kill error | Private finalization-operation fault reports kill error, then confirmed reap; observe polling and the original refusal with kill-error context |
| Reap fence exhausted | Controlled finite wait/clock inputs never confirm reap before the fence; cleanup failure overrides the original refusal while preserving its cause |
| Capture removal failure | After confirmed reap, injected checked removal fails; capture-cleanup failure is reported and ordering is observed |
| Capture boundaries | Call the same bounded file-reading component used after confirmed reap on completed regular files at 16,383, 16,384 and 16,385 bytes; exact first two lengths accepted, third refused, with no live child or monitor |
| Invalid child control | Unknown value or selector/argument mismatch refuses before the fixture factory; no recursion |

For live-child controls, signal readiness through a bounded regular-file capture, not a new pipe, port or unbounded output loop. Keep the other stream below its independent cap. On Linux/macOS use the standard-library Unix exit-status extension to assert signal 9 from the owner's kill request as well as observed reap. A naturally successful exit is insufficient. The finite 20,000-ms hold exceeds the injected deadline plus cleanup fence; no original case's waits or helper defaults change. For raw capture-boundary tests use the actual file-reading component, not a child payload whose extra libtest status bytes would change the measured total. These deterministic controls cover that component's cap-plus-one refusal, not the real runner's post-exit branch selection. A finite child that writes oversized output and exits may be refused by either the active monitor or the post-reap reader; do not add a separate required child mode or claim that natural exit deterministically selects the latter. No new observation-order seam is needed. The intentionally absent selector is fixed test data, not an arbitrary normal dispatch API. Helper child modes are closed and must not spawn native descendants. Do not add ignored helper tests or a dummy test that only returns successfully in the ordinary suite.

- [ ] Pin defaults against independent literals `20_000`, `2_000`, `16_384`. Keep timeout injection private; never change any of the six original cases' waits to accelerate a control. Retain at most cap-plus-one per stream on refusal as well as success.
- [ ] Implement compiling sensitivity controls in the helper's inline unit contract drivers. The driver that exercises the real runner/parser must reject three fixed bad candidates: a passthrough constructing/polling the parent body, empty success with no real child witness, and an accept-all report predicate. Execute the bad candidate to an observed guard failure and assert that failure; the real implementation must satisfy the same independent driver. Use finite fixed child/control cases or pure completed-return assertions, never abandonment of an active helper future.
- [ ] Record those expected candidate failures as hosted negative-control evidence, not a claim that source-rewrite mutations were executed. Task 2's independently pinned row roster detects missing report rows; its witness-only receipt detects removal of the witness check without other invalid fields masking it. The three whole-class bad candidates alone do not prove individual parser-branch mutation sensitivity. All controls compile as part of the existing CLI binary and run in both full Linux lanes and the focused macOS owner. No source-mutation script, new compiler build plan, ignored test or intentionally red outer CI job is needed.
- [ ] Keep closed helper entry tests meaningful in the ordinary suite: without a child marker they invoke the parent runner and assert the expected result, not simply return. Selected child branches execute only their finite behavior and launch no native descendants.
- [ ] Commit the complete helper/controls with Task 4's six delegations, without a semantic prefix or byline. Tests remain unverified until their actual hosted results are read back.

**Expected:** All new helper controls pass; the Task 2 passthrough red is closed by actual isolation. Parent success cannot come from an empty probe, a stale selector, a nonzero child or unconfirmed cleanup. Post-spawn setup failure cannot lose the standard child handle; no stronger cancellation guarantee is implied.

## Task 4: Delegate All Six Original Cases Without Rewriting Them

**Modify/test:** The two protocol cases, three server cases and one native-system case named in Task 1. No other test owner is refactored.

- [ ] Wrap each of the six functions around the helper, placing the **entire existing body** inside the deferred closure. Use this structural pattern, substituting the owning case; retain the original test name and attributes:

```rust
socket_test_support::run(SocketCase::ProtocolEof, || async {
    // The original body is retained here, without replacing any assertion.
})
.await
.unwrap();
```

This is an integration pattern, not permission to replace the body with the comment. The private helper import uses ordinary module paths. No socket, control channel, configuration, timer or task may be constructed before the helper chooses parent versus child.

The wrapper directly awaits `run`; do not put an outer timeout around it or hand it to an abortable task. The helper owns the execution deadline and cleanup fence. Preserve the original inner timeouts within the deferred body.

- [ ] CREATOR executes the Git-only comparison below and publishes its actual commands/output and a six-case verdict in the implementation PR; REVIEWER independently verifies it. Set `PRESERVATION_BASE` to Task 1's recorded full SHA and `IMPLEMENTATION_HEAD` to the reviewed head. These are shell values for ordinary Git inspection, not runtime environment interfaces or committed placeholders. Run locally or in a reviewer checkout; no CI script, compiler, test program or automatic token comparison is involved. If the base object is absent, first execute and record `git fetch --no-tags --depth=1 origin "$PRESERVATION_BASE"`. Missing base/source output blocks this review.

```sh
git cat-file -e "$PRESERVATION_BASE^{commit}"
git rev-parse "$IMPLEMENTATION_HEAD^{commit}"
git show "$PRESERVATION_BASE:crates/auctioneer-cli/src/qualification/harness/fixture/protocol.rs"
git show "$PRESERVATION_BASE:crates/auctioneer-cli/src/qualification/harness/fixture/server/mod.rs"
git show "$PRESERVATION_BASE:crates/auctioneer-cli/src/qualification/harness/native/system.rs"
git diff --no-ext-diff --no-textconv --function-context "$PRESERVATION_BASE" "$IMPLEMENTATION_HEAD" -- crates/auctioneer-cli/src/qualification/harness/fixture/protocol.rs crates/auctioneer-cli/src/qualification/harness/fixture/server/mod.rs crates/auctioneer-cli/src/qualification/harness/native/system.rs
```

**Expected:** Object lookup succeeds; source output contains all six original functions; diff shows only the six wrappers/imports and layout changes in those owners. Inspect every removed/added body line against the original, accepting indentation or Rustfmt layout only when statements, expressions, literal contents, loop rows and assertions are unchanged. Do not use `--ignore-all-space` or whitespace-normalized equality as proof: it could conceal changed string contents. Record one verdict for each literal selector in Task 1, including attributes, gates and complete body. This is an explicit human/author source-review record, not a machine assertion or executed mutation proof. The `syn`/`quote` source guard in Task 2 remains a current-source wrapper check, not historical preservation. Unrelated baseline-to-head changes in an owner must be explained or reviewed separately, not silently normalized away.

- [ ] Check the spec's full assertion table independently, including both capacity rows and stopped tuple order `(running, active, completed, disconnected)`, which differs from the delay case's ordering. Preserve reserved-port bounds/exclusion, exact rebind and `local_addr`; preserve header EOF/zero-dispatch/recovery independently from capacity rejection.
- [ ] Reconfirm five-second observation waits, 20,000-ms held requests, 1,800-ms header EOF deadline, 40-ms delay and immediate `unwrap_err()` checks. The outer 20-second watchdog is a whole-invocation fence, not a sum of independent maxima. Its expiry is failure, not permission to skip assertions. Any needed limit change returns to design review, not after-the-fact tuning.
- [ ] Repeat both roster commands from Task 1. Require all six original names/gates exactly once; helper controls are the only new cases. No new `#[ignore]`, broad-lane skip, Cargo target, source import, feature or parent test-thread override is permitted.
- [ ] Run the fixture filters from Task 1 in both feature graphs, then full binaries:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli
cargo test --locked --offline --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli
```

**Expected:** All six wrappers verify their exact child but add only one ordinary outer result each. The fixture filter alone is not six-case acceptance; Task 5 selects the native case too. Do not predict a future numeric total by subtracting existing tests.

- [ ] Add the short explanation to `docs/testing/development.md`: six names/owners, immediate standard-child ownership, scoped marker, child-only single-thread selection, unchanged bounds, bounded stderr diagnostics without nested stdout, directly awaited cleanup limitation and lack of an OS disk quota. Link both joint issues and the macOS command owner, without copying the full acceptance matrix.
- [ ] Commit the case delegation and focused documentation after the owning checks pass.

## Task 5: Add The Focused macOS Hosted Owner

**Create:** `.github/workflows/scripts/test/verify-cli-socket-isolation.sh` and `.github/workflows/scripts/test/check-cli-socket-isolation-fixtures.sh`. **Modify:** Add only the focused macOS job in `.github/workflows/test.yml`; all current Linux jobs remain unchanged.

- [ ] Give the cohort script one required lane argument, `default` or `workspace-insecure`; reject every other value. Use Bash indexed arrays/portable loops compatible with the hosted macOS shell, not associative arrays, GNU-only process tools or a new interpreter dependency.
- [ ] Map lane to these fixed Cargo arrays:

```sh
# default
cargo_args=(--locked -p auctioneer-cli --bin auctioneer-cli)
# workspace-insecure
cargo_args=(--locked --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli)
```

- [ ] Enumerate that exact graph with `cargo test "${cargo_args[@]}" -- --list --color=never`, saving the listing under `RUNNER_TEMP`. Refuse command failure, duplicate test names or a roster missing any of the six full selectors or 19 literal required helper names in Task 2.
- [ ] Build `selected` from the six literal full names plus every discovered test under `qualification::harness::socket_test_support::tests::`. Require exactly six real cases, all 19 required controls, no duplicates and no selected name outside those owners. The required list must not be derived from discovery itself; that would miss deletion/rename. Select additional helper controls too.
- [ ] Run the entire selected cohort in **one parent process**, with multiple exact filters after Cargo's separator:

```sh
cargo test "${cargo_args[@]}" -- --exact --color=never "${selected[@]}"
```

Pinned libtest accepts `[FILTERS...]` as OR, and `--exact` makes each literal exact. Cargo sees no extra positional filters before `--`. Do not add parent `--test-threads=1` or set `RUST_TEST_THREADS`; only the helper's children are single-threaded. Capture the log with `pipefail` so `tee` cannot hide a failed Cargo/test command.

- [ ] Compute `filtered = total_roster - selected_count`. Reuse `check-libtest-result.sh LOG FILTERED 1 SELECTED_COUNT`; additionally require one `running SELECTED_COUNT tests`, exactly one successful status line for every selected full name, no extra successful names and no failed/ignored selected case. Keep ordinary pretty output without color so the existing anchored summary grammar applies. A matching summary alone is not dispatch-name coverage.
- [ ] Add finite selection/report fixture controls: valid cohort; missing/renamed real case; missing required helper; duplicate roster name; unknown lane; zero selected/helper cases; ignored/failed result; wrong passed/filtered count; missing/wrong/duplicate successful name; extra/nested summary; and truncated/invalid report. A positive fixture must pass the same checker; do not print fixture summaries into broad test accounting. Separate fixture checking from Cargo invocation so these controls need no compiler build.
- [ ] Add job ID `cli-socket-isolation-macos` with the exact stable `name: cargo test (cli-socket-isolation-macos)` in the existing Tests workflow on `macos-latest`, retaining both triggers and read-only permissions. The `name:` is the required check-run context, not merely the YAML job ID; do not make it matrix-dependent. Use the existing CLI lane's declared safety limit (currently 75 minutes) for the job and run the two feature lanes sequentially; no new runner matrix or independently tuned timeout. Get Rust from `.tool-versions` through the existing setup pattern and keep download-only caching with implicit compiled caching disabled. Reuse the existing `df -h` reporting before and after the two graphs so disk exhaustion remains diagnosable.
- [ ] Acquire the locked prerequisites needed to compile the existing CLI/workspace graph through normal checkout/Rust/fetch setup. Do not copy Linux-only yq/Wasmtime/provider installers: the selected bodies and helper controls need none of them. If compilation proves an additional prerequisite necessary, report it and use its existing source of truth, never skip a selected case.
- [ ] In that job, run the finite checker controls, then both lane commands below. Record source SHA, OS/toolchain, actual names/summaries and exit status; retain failure logs. No `continue-on-error`, success fallback, unchanged-run retry or condition that omits the owner for an implementation change is allowed.

**Complete future hosted entry commands:**

```sh
bash .github/workflows/scripts/test/check-cli-socket-isolation-fixtures.sh
bash .github/workflows/scripts/test/verify-cli-socket-isolation.sh default
bash .github/workflows/scripts/test/verify-cli-socket-isolation.sh workspace-insecure
```

**Expected:** One complete parallel parent cohort per graph, all six real names and every required helper control accounted for, zero failed/ignored selected tests, and refusal of every finite bad fixture. Default and workspace-insecure must remain distinct build graphs. No Linux success or historical macOS pass substitutes for this current-head owner.

## Task 6: Full Verification And Separate Bug-Fix Handoff

**Hosted verification:** The current workflows plus the added macOS owner and `docs/testing/development.md`. No source edits during source-identity, artifact or measurement runs. All Cargo/test/fmt/lint/script commands remain hosted-only under the user's instruction. Ordinary Git/GitHub inspection is read-only and may be local; the separately approved ruleset operation belongs to an authorized maintainer, not the read-only workflow token.

- [ ] After the implementation's first green `cargo test (cli-socket-isolation-macos)` run, ask the user for explicit approval to add that exact context as a required check to the active `main` ruleset. Then an authorized maintainer adds only that context using the current GitHub Actions integration, preserving all existing contexts, enforcement and `strict_required_status_checks_policy`. This is a separately approved repository-settings operation, not an automatic consequence of planning/implementation approval. Do not change settings in this planning PR or before the proposed job exists. Record the approval and settings readback on the implementation PR. Without approval and readback, implementation merge remains blocked even if the job is green; the job is not advisory.
- [ ] Read back the rule and PR checks with the commands below. Verify the ruleset is active for `main`, contains the exact new context alongside every prior context, and retains strict up-to-date enforcement. The currently observed ruleset ID is `26424451`; re-discover the applicable rule if its ID changes. Confirm the new check passed at the reviewed implementation head, not just an earlier run. A missing context or result is blocked, never a success fallback. CREATOR's merge checklist and REVIEWER's current-head acceptance require this verification until the required rule has been installed, not instead of installing it.

```sh
gh api repos/IABTechLab/auctioneer/rulesets/26424451 --jq '{enforcement,conditions,rules}'
gh pr checks "$IMPLEMENTATION_PR" --repo IABTechLab/auctioneer --required
gh pr view "$IMPLEMENTATION_PR" --repo IABTechLab/auctioneer --json headRefOid,statusCheckRollup
```

**Expected:** Active `main` ruleset includes `cargo test (cli-socket-isolation-macos)` without dropping any earlier required check; the check appears in required results and passes at `IMPLEMENTATION_HEAD`. Retain the actual output. This acceptance prerequisite is not claimed to have executed in the planning PR.

- [ ] Run Task 5's focused macOS cohort in both graphs and retain the complete existing full CLI/default/insecure Linux owners. If a repetition cohort is needed, declare its count before starting and keep every result, rather than accepting a later green retry. Do not change parent parallelism or claim full macOS CLI execution from a focused cohort.
- [ ] Run formatting, strict unit-aware lint, source-import and documentation/fixture gates:

```sh
cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo clippy --locked --offline --workspace --all-targets --features insecure-bidders -- -D warnings
cargo test --locked --offline -p auctioneer-bidder --test test_isolation
bash scripts/check-raw-json-fixtures.sh
bash scripts/check-batch-rebase.sh
bash scripts/check-roster.sh
git diff --check
```

**Expected:** Every command exits zero; the import gate dispatches its actual tests and reports no handwritten source imports. Existing upstream future-incompatibility notices are recorded, not suppressed or repaired in this issue.

- [ ] Run the unchanged complete broad workspace commands, keeping the two already once-owned producer exclusions and no new exclusion:

```sh
cargo test --locked --offline --workspace --all-targets --no-fail-fast -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked --offline --workspace --all-targets --features insecure-bidders --exclude auctioneer-config-data --no-fail-fast -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked --offline -p auctioneer-cli --all-targets -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked --offline -p auctioneer-cli --no-default-features --all-targets
```

- [ ] Publish the separately authorized implementation PR against `main` with `Closes #168`, `Closes #174`, the merged planning-amendment SHA, Task 4's six-case Git-only preservation record, roster record and macOS required-check approval/readback status. State hosted verification as pending until it executes; an author-side source review is not a test run. Request current-head independent review, and leave thread resolution to REVIEWER.
- [ ] Inspect the full current-head Tests, Format, Cloudflare, Fastly and Spin runs plus the added macOS owner. Existing secure-default, full workspace/feature/CLI, maintenance-only, strict all-target lint, once-owned compiler/producer, boot parity, source/import, resource/artifact, browser and guest/provider checks remain exactly as selected by their workflow owners. No broad Cargo command alone is full acceptance. Record hosted commands, exit codes and actual result lines, distinguishing expected child/control failures from outer-suite failures. Missing prerequisites, stopped jobs and absent macOS execution are blocked, not passing.
- [ ] Bind new current evidence to the changed source/executable where required. Preserve all historical capture and frozen-profile bytes; do not rewrite old digests to make them current. PR #162's source-bound evidence is refreshed only after the combined fix is integrated there.
- [ ] After explicit approval/authorization and readback of the combined implementation merge into `main`, record its merge SHA and verify both issue states. Planning alone closes neither. Integrate the landed fix into PR #162 without force-push, refresh its source-bound evidence/full hosted matrix and request review of that new head. A failure of any of these six cases remains reportable and blocking until independently addressed; scope inclusion does not waive it. Earlier hosted success and resolved findings do not approve a changed #162 head.

**Expected final state:** One reviewed combined repair on main, #168 and #174 closed by implementation, all six original test bodies/assertions preserved, complete current-head Linux/macOS evidence, and PR #162 awaiting its own refreshed acceptance/merge decision. This plan authorizes no later merge and transfers no older approval.
