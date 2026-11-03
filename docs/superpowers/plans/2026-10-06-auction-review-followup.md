# Auction Review Follow-up Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** Use test-driven-development and verification-before-completion. Independent repairs may run in parallel with disjoint file ownership.

**Goal:** Correct the seven findings in the latest bounded fanout review without reducing capability scope or changing accepted provider deferrals.

**Architecture:** Preserve denied identities through splitting. Resolve independent transport/decoded caps across host, account, bidder and alias layers into the auction plan, then clamp each cap to the installed envelope. Preflight request-derived response bookkeeping before impression-ID copies and charge its bounded peak separately. Keep SDK-owned routing and adapter conversion.

**Tech Stack:** Rust, pinned EdgeZero `dfd439c`, Cargo tests, allocation regressions, shell acceptance guards and provider target checks.

## Constraints

- Work in `.claude/worktrees/bounded-fanout`; preserve existing uncommitted repairs.
- No compatibility shims are required. No commits, pushes or capability merges are requested.
- Unit tests stay beside implementation; process/allocation/lifetime tests remain integration tests.
- Tests describe behavior, not capability numbers. Preserve required wire identifiers and attribution.
- Warn-mode budgets and hosted/raw-ingress/egress qualification remain accepted boundaries.

## Tasks

### 1. Request-Derived Bookkeeping

Files: `auctioneer-exchange/src/{run,response}.rs`, a focused bookkeeping module if needed, `auctioneer-runtime/src/budget.rs`, allocation integration tests.

- [x] Add a failing many-alias/large-impression-ID regression through `run_auction`.
- [x] Preflight the aggregate bookkeeping population before copying IDs; omit status rows when not requested.
- [x] Fund its bounded wire/string and node peak separately, including response assembly scratch.
- [x] Verify valid boundary traffic, refusal, checked arithmetic and narrowed envelopes.

### 2. Alias Routing And Catalog

Files: `auctioneer-exchange/src/{split,alias}.rs`, config-overlay tests, `auctioneer-core/src/handlers/auction.rs` and catalog tests.

- [x] Reproduce disabled request/host aliases shadowing compiled names.
- [x] Deny dropped aliases before direct-registry fallback in all parameter locations.
- [x] Apply the same identity bounds to catalog eligibility as auction routing.
- [x] Verify ordinary compiled bidders and valid aliases still participate.

### 3. Scoped Response Budgets

Files: config schema/resolution, exchange budgets/plan/call construction, outbound policy bridge and related fixtures.

- [x] Demonstrate account/bidder cap fields are currently rejected.
- [x] Implement both caps with 1 KiB..=100 MiB validation and host/account/bidder precedence.
- [x] Preserve per-field alias-own then base fallback; clamp only against the funded envelope and emit code 10020 when needed (10019 is reserved for stale CMP-list warnings).
- [x] Thread both caps to production transport/decode enforcement; missing resolved entries are invariant errors.
- [x] Test defaults, boundaries, precedence, alias inheritance, distinct caps and actual outbound policy values.

### 4. Provider Logging

Files: Cloudflare/Spin logging and entrypoints, shared helpers only as needed, focused logging tests.

- [x] Add failing environment-driven ordinary/boot-filter regressions.
- [x] Resolve configured ordinary levels before bootstrap while admitting mandatory boot WARN/ERROR independently.
- [x] Recheck ERROR/OFF/DEBUG and the real provider targets after integrated budget changes.

### 5. Acceptance Reliability

Files: `scripts/check-roster*.sh`, roster section of the bounded fanout plan, node-budget integration fixture, Fastly CI if locally runnable tests can be wired.

- [x] Make the auction fixture safe under parallel test execution without weakening admission assertions.
- [x] Require the enumerated roster count to equal the declared total; reconcile counts and add arbitrary deletion fixtures.
- [x] Wire the existing local Fastly logging process regressions into CI rather than relying on ignored default tests.

## Final Verification

- [x] Review integrated changes and spec/plan consistency.
- [x] Formatting, diff, fixture, roster and batch-rebase guards.
- [x] Focused tests, repeated parallel auction tests, full workspace and fanout conformance.
- [x] Strict native Clippy and actual Cloudflare/Fastly/Spin target checks.
- [x] Report evidence and any remaining limitations; do not imply hosted qualification.

## Integration Review

Two independent read-only reviews found no new actionable issues in the
auction/resource or provider/alias/acceptance repairs. Source-aware budget
resolution also prevents publisher-less requests from selecting
`[accounts.unknown]`; a publisher explicitly named `unknown` still selects
that configured row. Both cases are covered through the real handler.
Account-selection fixtures use `serde_json::json!`, and exchange config-overlay
fixtures share the response-budget helper rather than accidentally supplying
zero caps.

Fresh focused verification passed 208 core, 91 exchange and 103 runtime
unit tests, six real-handler tests, 27 config-overlay tests and the focused
auction/construction/allocation regressions. The 111 fanout conformance
cases passed. Seven Fastly guest cases passed through two Viceroy-driven
host tests, and CI now runs them explicitly. Formatting, strict workspace
Clippy, ShellCheck, actionlint, all three provider target checks, and the
roster/batch/JSON-fixture guards passed. The five-test admission fixture
also passed 100 repeated executions with five test threads.

Both settled-source workspace sweeps completed successfully:

```sh
cargo test -q --workspace --all-targets --exclude auctioneer-config --locked --offline
cargo test -q --workspace --all-targets --features insecure-bidders --exclude auctioneer-config --locked --offline
```

Config verification was split out: all 55 build-failure harness cases
passed in the earlier full-workspace run, and the settled-source config
library, snapshot, codegen, GVL, smoke and URL-safety targets passed in a
fresh focused run. The earlier full-workspace run compiled a core snapshot
before the account-selection repair was integrated and failed that new
regression; the fresh handler and both settled-source workspace runs
passed it. The unchanged build-failure harness was not repeated.

All-target sweeps also execute the cold-GVL benchmark in debug mode;
its timing is not evidence for the release/production-hardware target.
Warn-only over-ceiling budgets and hosted, raw-ingress and BestEffort
response-egress qualification remain accepted boundaries, not newly
claimed guarantees. These repairs are uncommitted and unpushed in the
bounded fanout worktree; no changes were propagated to later capability branches.
