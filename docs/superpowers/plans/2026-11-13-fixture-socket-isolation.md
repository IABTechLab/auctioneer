# Fixture socket isolation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make three inline CLI socket-closure checks independent of unrelated child spawning without weakening their assertions or serializing the surrounding suite.

**Architecture:** A private fixture-owned test helper invokes the existing CLI unit binary with one closed exact selector. It defers fixture construction to that child, monitors private regular-file captures, verifies one successful test, and explicitly reaps the child. Serving code and the existing production process supervisor are unchanged.

**Tech Stack:** Existing Rust/libtest, Tokio process/time, `tempfile`, and ordinary inline unit modules; no new dependency, crate, Cargo target or source include.

---

## Contract And Execution Gate

Issue: [#168](https://github.com/IABTechLab/auctioneer/issues/168). Approved specification: [fixture socket isolation](../specs/2026-11-13-fixture-socket-isolation-design.md). The maintainer approved the written specification on 2026-11-13; this plan is proposed for review, not implementation evidence. Every result below is **expected**, unless explicitly identified as previously executed baseline evidence.

Publish the spec and plan together in a separate planning PR with `Refs #168`. After review and explicit merge authorization, merge that planning PR, then create a fresh implementation worktree from updated `origin/main`. Do not implement on the planning branch or fold this repair into PR #162 before its own implementation has landed. Use @superpowers:test-driven-development, @superpowers:verification-before-completion and the PRfection CREATOR thread/merge rules during implementation.

The diagnostic in the spec establishes an interference mechanism, not a deterministic reproduction of the exact protocol failure at main. Do not recreate the race in CI or rerun a failing suite until it happens to pass. PR #162's recorded failure stays an acceptance blocker until the independently reviewed repair is integrated and that resulting head is verified.

## File Ownership

| File | Intended implementation change |
| --- | --- |
| `crates/auctioneer-cli/src/qualification/harness/fixture/mod.rs` | One private inline `#[cfg(test)] mod test_support`, with its own inline tests; selectors, child command, capture/cleanup and dispatch checks live together |
| `crates/auctioneer-cli/src/qualification/harness/fixture/protocol.rs` | Wrap only the two named protocol cases from the spec; keep their names, gates, bodies and assertions inline |
| `crates/auctioneer-cli/src/qualification/harness/fixture/server/mod.rs` | Wrap only the named delayed/held-connection stop case; retain its body and inline assertions |
| `docs/testing/development.md` | Briefly explain exact-child ownership, fixed test-only bounds and the unchanged parallel parent suite |

The spec and this plan remain committed contracts. No implementation edits to `Cargo.toml`, `Cargo.lock`, `.github/workflows/`, `native_control/process.rs`, other test owners or historical evidence are authorized. All implementation Rust remains inside the existing `#[cfg(test)]` blocks. The helper needs fixture-local visibility only; do not expose a library API to reach these private tests.

## Task 1: Freeze The Roster And Current Failure Evidence

**Read:** The four owner files above, `AGENTS.md`, `crates/AGENTS.md`, and the acceptance/prerequisite sections of `docs/testing/development.md`.

- [ ] Record the implementation base SHA, clean worktree state, enabled Cargo features and installed tools. Use one existing target directory and one heavy Cargo job at a time; do not clean build outputs or disable incremental builds as preparation.
- [ ] Install/acquire the existing prerequisites from the development guide, including the pinned SDK/source objects. Missing source/tool prerequisites fail instead of skipping a test. Set any local target-directory or tool paths privately; never commit machine paths or credentials.
- [ ] List the actual CLI unit roster in both existing feature graphs:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli -- --list
cargo test --locked --offline --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli -- --list
```

**Expected:** Every full selector in the spec occurs exactly once per graph. Record the total roster and existing ignored cases before adding controls; a zero-match filter is failure.

- [ ] Run the existing owning filter once in each graph before modifying source:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli qualification::harness::fixture::
cargo test --locked --offline --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli qualification::harness::fixture::
```

**Expected:** The original three cases dispatch. Preserve any failure as baseline evidence; a pass is not a repair. The baseline planning run at main `7f6773e` already executed the default broad workspace: 245 summaries, 6,565 passed, zero failed, 19 ignored, including CLI `776 passed; 0 failed; 3 ignored`. Those are past baseline results, not promises about the future implementation.

- [ ] Save the original three functions, including attributes and complete bodies, for the final preservation comparison. Use source as data or a direct diff; do not add handwritten imports, raw JSON snapshots, generated Rust strings or duplicate golden bodies.

## Task 2: Specify And Test Exact-Child Dispatch

**Modify/test:** Only the new inline `fixture::test_support` module in `crates/auctioneer-cli/src/qualification/harness/fixture/mod.rs`.

- [ ] Add a closed case representation for the three full selectors in the spec and fixed helper-control entry points. The normal entry takes a deferred `FnOnce() -> Future<Output = ()>` body; it does not require `Send` or construct fixture resources before role selection. Return enough private execution information to distinguish a verified parent invocation from an executed child, without exposing raw output outside tests.
- [ ] Add literal transcript controls before implementing report acceptance. A minimal successful stdout control has this structure (the actual selector must be one of the closed names):

```text
running 1 test
test <full selector> ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 779 filtered out; finished in 0.01s
```

The filtered count is an arbitrary test literal, not a pinned total or the current roster size. Test another count independently. Assert exact acceptance/refusal for failed/ignored/zero results, wrong names, missing or duplicated records, truncated summaries, trailing records and invalid UTF-8. Do not use `is_err()` where the helper has distinct refusal categories.

- [ ] Start with a compiling permissive dispatch-check stub, run the new report tests and record the negative rows failing. Implement complete record validation: one run announcement, one requested-case success, one successful 1/0/0/0 summary, valid numeric filtered/duration fields, no extra result records. Whitespace-only separators are not extra test records.
- [ ] Add child-role controls: clear environment, pass only `AUCTIONEER__TEST__FIXTURE_SOCKET_CASE`, validate its closed value and the exact single-test arguments, and reject malformed/mismatched controls before calling the factory. Use `Command::env_clear()`/`Command::env()` on the child, never process-global `set_var`.
- [ ] Add a parent/child delegation control using the same entry path as the real cases. The body factory increments a local construction counter; its future increments a separate poll counter. In child mode both must equal one; in parent mode both must stay zero. Emit a fixed completion witness on **stderr** from inside the successfully executed probe body, and require exactly that witness in the parent's bounded receipt. Stderr avoids interleaving witness text into libtest's stdout status line. An unconditional parent return cannot supply that positive witness.
- [ ] Run a compiling passthrough entry that directly awaits the body: the parent counter/role control must fail. Record that red result before replacing passthrough with real isolation in Task 3. A missing symbol or compiler failure is not this red proof.

Run throughout this task:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli qualification::harness::fixture::test_support::tests::
```

**Expected:** Nonzero dispatched helper tests; report controls pass after validation, while the new delegation probe fails until Task 3. Do not commit a broken checkpoint; commit Tasks 2 and 3 together once their controls pass.

## Task 3: Implement The Bounded Test-Only Child Owner

**Modify/test:** The same inline `fixture::test_support` owner. Do not modify the production supervisor.

- [ ] Build the command directly from `std::env::current_exe()` with `--exact`, the closed selector, `--test-threads=1`, `--nocapture`, `--color=never`, null stdin and kill-on-drop. Clear inherited environment as specified in Task 2. Use no shell, Cargo invocation, guessed executable or native subprocess inside a selected child.
- [ ] Create a private scoped temporary directory with distinct stdout/stderr regular files. Attach file handles to the child. Keep readable ownership that can start at byte zero after exit: cloned descriptors share offsets, so explicitly seek or reopen appropriately. Do not create capture pipes or background reader tasks.
- [ ] Supervise child wait with one monotonic 20,000-ms execution deadline. While it runs, periodically check both file lengths. Use an observation interval no greater than 10 ms without extending the absolute deadline. A length above 16,384 bytes refuses that stream. The interval does not grant a disk quota; a finite writer may overshoot before observation.
- [ ] On any active-child refusal, request termination without cancelling the ownership guard, then wait with a separate 2,000-ms cleanup fence. On every path observe whether `try_wait`/`wait` confirmed exit and reap. Surface cleanup failure if not; do not turn kill-on-drop or a successful signal into a reap receipt. Spawn failures have no child to reap.
- [ ] After confirmed exit, read each regular file from its beginning with a 16,385-byte maximum, and reject cap-plus-one before parsing. A successful status plus the strict report check from Task 2 is required. Accept at most 16,384 bytes per stream; consume successful nested reports instead of reprinting them into the outer harness. Explicitly close/remove temporary captures after reap. Cleanup or file-operation failure remains failure.
- [ ] Exercise the real runner with fixed inline helper modes, all inside tests that perform real parent assertions when not in child mode:

| Control | Required independent observation |
| --- | --- |
| Positive exact dispatch | Actual child status success, exactly one requested test, probe witness present once, parent construction/poll counters both zero |
| Static nonexistent selector | Actual child exits successfully after zero tests; runner refuses it as dispatch failure |
| Assertion failure | Actual child fails; runner refuses with child-exit failure and confirms reap |
| Timeout | A child waits beyond a short injected test deadline; runner refuses timeout and confirms reap within the cleanup fence |
| Stdout overflow | Finite 16,385-byte stdout payload; runner refuses output size and confirms reap |
| Stderr overflow | Independently varied finite 16,385-byte stderr payload; runner refuses output size and confirms reap |
| Capture boundaries | Private regular-file read controls at 16,383, 16,384 and 16,385 bytes; exact first two lengths accepted, third refused |
| Invalid child control | Unknown value or selector/argument mismatch refuses before the fixture factory; no recursion |

For raw capture-boundary tests use the file-reading component, not a child payload whose extra libtest status bytes would change the measured total. The intentionally absent selector is fixed test data, not an arbitrary normal dispatch API. Helper child modes are closed and must not spawn native descendants. Do not add ignored helper tests or a dummy test that only returns successfully in the ordinary suite.

- [ ] Pin actual helper defaults against literals `20_000`, `2_000` and `16_384`, independently of the implementation constants. Keep timeout injection private and separate from defaults; never shorten or extend the three original fixture waits to accelerate this control. Retain at most cap-plus-one per stream on refusal as well as success.
- [ ] Re-run the helper filter until the implementation satisfies the already-written tests; inspect every failed run rather than retrying unchanged. In a controlled local mutation, bypass child-body execution or dispatch validation separately: the probe or malformed/zero-report controls must fail. Restore the fix and re-run. Never mutate another worktree or claim a compile failure is a killed behavioral mutant.
- [ ] Commit the now-passing fixture-local helper and controls with no semantic prefix or byline.

**Expected:** All new helper controls pass; the Task 2 passthrough red is closed by actual isolation. Parent success cannot come from an empty probe, a stale selector, a nonzero child or unconfirmed cleanup.

## Task 4: Delegate The Three Original Cases Without Rewriting Them

**Modify/test:** `crates/auctioneer-cli/src/qualification/harness/fixture/protocol.rs` and `crates/auctioneer-cli/src/qualification/harness/fixture/server/mod.rs`.

- [ ] Wrap each of the three named functions around the helper, placing the **entire existing body** inside the deferred closure. Use this structural pattern, substituting the owning closed case; keep the original test name and attributes:

```rust
fixture_test_support::run(SocketCase::Eof, || async {
    // The original body is retained here, without replacing any assertion.
})
.await
.unwrap();
```

This is an integration pattern, not permission to replace the body with the comment. The private helper import uses ordinary module paths. No socket, control channel, configuration, timer or task may be constructed before the helper chooses parent versus child.

- [ ] Compare each wrapped inner body with Task 1's original body, ignoring only indentation. Independently check every row of the spec's assertion table: EOF ready/stats/closure, malformed-frame loop/no-echo/interrupt/EOF, and delay/held-request/state/closure/rebind. Reconfirm the 5-second waits, 40-ms delay and immediate `unwrap_err()` checks are intact.
- [ ] Repeat both roster commands from Task 1. Require the original three names and gates exactly once; added helper controls are the only new cases. Ensure no new `#[ignore]`, skip, Cargo target, source import, feature or global test-thread override appears.
- [ ] Run the fixture filters from Task 1 in both feature graphs, then full binaries:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli
cargo test --locked --offline --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli
```

**Expected:** All selected tests execute and pass with ordinary parent parallelism. The three wrappers each verify a child result but add only their ordinary single outer test result. Do not predict a future numeric total by subtracting existing cases.

- [ ] Add the short test-ownership explanation to `docs/testing/development.md`, including the scoped marker, child-only single-thread selection, 20-second/2-second bounds, 16-KiB stream caps and explicit lack of a hard disk quota. Keep the normal focused commands and full CI owners unchanged.
- [ ] Commit the case delegation and focused documentation after the owning checks pass.

## Task 5: Full Verification And Separate Bug-Fix Handoff

**Read/execute:** The unchanged current workflows and `docs/testing/development.md`. No source edits during source-identity, artifact or measurement runs.

- [ ] On macOS, run the complete CLI binaries above in both feature graphs. Declare any repetition count before starting, keep every result, and stop on failure rather than accepting the first later green run. Do not change normal parent parallelism.
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

- [ ] Execute the full existing secure-package lane, explicit hermetic compiler/producer owners, boot parity, feature-sensitive metadata/core checks, WASM execution/build/package and browser/adapter checks at the implementation head. The current workflows remain the source of truth for their commands and prerequisites; do not hand-copy or narrow their selection. The approved verification-location boundary permits expensive unchanged owners to run in the complete hosted matrix, but list the location, tested SHA, command and actual result for each. A default broad Cargo run alone is not full acceptance, and a queued/unrun check is not passing.
- [ ] Publish a separate implementation PR with `Closes #168`, the merged plan SHA, the preserved-roster/body comparison and exact successful/refused command output. Ask for independent review; do not resolve finding threads, waive failures or merge without the PRfection current-head gates.
- [ ] After explicit approval/authorization and readback of that bug-fix merge into `main`, integrate the landed fix into PR #162 without force-push. Recompute its current source-bound evidence, rerun its full checks and request review of its new head. PR #162's earlier hosted success, resolved findings and unmodified reruns are not approval of that new head.

**Expected final state:** The isolated repair is delivered on main with all original assertions retained and complete acceptance evidence. Bug #168 closes only when its implementation lands; the separate review/merge decision for PR #162 remains with its reviewer and maintainer.
