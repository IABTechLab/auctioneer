# EdgeZero Update And Milestone Sync

> **For agentic workers:** Use verification-before-completion and requesting-code-review. Keep review tasks read-only and independent; synchronize milestone branches sequentially.

**Goal:** Update M3b to the current EdgeZero PR 275 revision, review its implementation against the spec and plan, and carry the integrated changes through M4, M5 and M6.

**Architecture:** Preserve the existing SDK-owned routing, ingress, outbound and egress paths. Treat upstream EOF and batch-finalization validation as stronger checks within that boundary. Merge the milestone stack in order, preserving each milestone's privacy, user-sync and hardening contracts.

**Tech Stack:** Rust, Cargo, Git worktrees, EdgeZero PR 275 at `950cb81d406d0dc269baf94314547329a4c1cef0`.

## Constraints

- Preserve outstanding M3b review repairs and unrelated root-worktree files.
- Use existing `m3b-fanout`, `m4-privacy`, `m5-usersync` and `m6-hardening` worktrees.
- Local commits and merges are needed to synchronize the branches. The user subsequently requested immediate checkpoint pushes; push milestone branches, not main, and keep pending verification explicit.
- Keep required wire identifiers and attribution. Preserve downstream document renames and milestone-specific behavior.
- No compatibility shims or reduction of accepted milestone scope.
- Existing warn-only budgets and provider-qualification deferrals remain explicit.

## Tasks

- [x] Confirm PR head and refresh Auctioneer remote refs; inspect all branch/worktree states.
- [x] Update six direct dependency pins and the lockfile; inspect upstream contract changes and migrate integration only where required.
- [x] Independently review auction/privacy/config/resource surfaces and adapter/lifecycle/acceptance surfaces against the current M3b spec and plan.
- [x] Verify M3b native tests, conformance, guards, strict Clippy and provider target builds; document findings and residual qualification gaps.
- [x] Commit the integrated M3b changes locally.
- [x] Merge M3b into M4; preserve and verify M4 privacy behavior.
- [x] Merge M4 into M5; preserve and verify M5 user-sync behavior and documentation renames.
- [x] Merge M5 into M6; preserve and verify M6 hardening contracts.
- [x] Confirm dependency identity and ancestry throughout the stack; report review findings, verification and code-checkpoint IDs.

## Evidence

GitHub reported PR 275 open at `950cb81d406d0dc269baf94314547329a4c1cef0`
on 2026-10-06. Relative to the previous `dfd439c` pin, the changes are
`9211183` (response EOF and batch finalization validation) and `950cb81`
(changelog). The user requested an immediate checkpoint push before the
fresh review and verification finished. Prior repair verification used
`dfd439c`; the initial checkpoint was explicitly published without claiming
verification of the new pin. Subsequent results are recorded below.

Checkpoint `d9c16a5` was pushed immediately as requested. Three independent
reviews covered auction/privacy/config, resource/lifecycle/acceptance and
all-adapter outbound/egress integration. Six open Auctioneer findings are
recorded in [the implementation review](2026-10-06-m3b-implementation-review.md);
the sync is not a merge approval. No new EdgeZero-owned defect was confirmed.

Seven unit regressions now cover errors after the final slot, late errors
in subsequent waves, pending clean EOF and cancellation during finalization.
All 45 wave tests passed in M3b and the merged M4 source. The new test-only
changes passed strict core Clippy. The M3b default workspace sweep, config
non-harness targets, 111 conformance cases, three provider target checks,
strict workspace Clippy and repository guards passed. The insecure-bidders
sweep also passed. Fresh Fastly guest artifact selection used Cargo's JSON
compiler output, and both ignored host logging tests passed (seven guest
scenarios). Hosted-provider qualification remains explicitly deferred.

## Downstream Verification

All four milestone branches were pushed throughout the work, rather than
waiting for the entire verification sweep. The tested code checkpoints are:

| Branch | Code checkpoint | Fresh verification                                                                                                                                                                                                                                                             |
| ------ | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| M3b    | `45e4267`       | Default and insecure-bidders native sweeps; config non-harness targets; 111 conformance cases; strict workspace Clippy; three provider-target checks; real Fastly guest logging regressions; repository guards                                                                 |
| M4     | `b652982`       | Default native sweep; config non-harness targets; strict workspace Clippy; Cloudflare, Fastly and Spin target checks; privacy and allocation guards; 45 wave tests                                                                                                             |
| M5     | `bc55c72`       | Default native sweep: 1918 passed; insecure-bidders: 1919 passed; config non-harness targets; 115 conformance cases; strict workspace Clippy; three provider-target checks; actual Chromium user-sync check; two shutdown regressions; repository guards and negative fixtures |
| M6     | `a101df9`       | Workspace all-targets check; exact source/manifests/scripts/workflows equality with M5; preservation of its separate hardening design                                                                                                                                          |

Native sweeps use `--workspace --all-targets --exclude auctioneer-config`, with
`--locked --offline`. Config coverage is run separately with
`test-harness-override`, selecting `--lib --test codegen --test gvl_archive
--test smoke --test url_safety`. The expensive child-build failure harness
was not rerun. The M5 native sweeps each report five ignored tests; the
separately executed conformance, Fastly guest and browser checks must not
be inferred from a native sweep alone. M6 has design changes, not a distinct
hardening implementation: its source is identical to M5, so its check is
not evidence that the planned debug work is implemented.

M4 conflict resolution retained sealed bidder requests, privacy ordering,
the measured projection allowance and independently prepaid bookkeeping.
M5 retained user-sync auxiliary and GVL populations. Independent runtime
and adapter calculations agreed on the combined totals. Test fixtures were
updated to reflect early supplement rejection, separate response parsing
and bookkeeping node charges, and M5's auxiliary-work floor; allocation,
no-fanout and cancellation assertions were preserved.

Sandboxed socket tests initially failed to bind loopback ports. Their
successful reruns used loopback-capable execution. The browser shutdown
tests also required the documented feature-enabled binary/config-example
build. Actual Chromium passed credentialed CORS, JSON preflight, controlled
callback redirects, Secure/SameSite=None cookie acceptance and visibility,
and denied-origin checks. This is cross-origin, same-site loopback evidence,
not cross-site or hosted-provider qualification.

GitHub's PR head was rechecked and remained `950cb81`. Six direct dependency
pins and all eight EdgeZero lock entries share that identity across the
stack. Each latest parent is an ancestor of its downstream milestone.
M5/M6 keep the renamed OpenRTB master spec and updated review links; main
remains at `e75881e`, with its pre-existing untracked files untouched.

The six open review findings remain open and were not fixed by integration
fixture changes. Their reproductions are source-traced, not executed. No
new EdgeZero-owned defect was confirmed; accepted warn-only budgets and
provider-qualification deferrals remain unchanged. This update and sync
does not constitute merge approval.
