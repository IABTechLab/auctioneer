# Factory And Metadata Feature Controls

Date: 2026-10-19
Branch: `m7-bidder-catalog`, based on `f1f17343`
Scope: the declaration/feature reconciliation part of M7 Task 6.

## Contract

The bidder build consumes the config crate's data-only metadata declarations.
The dependency direction remains bidder to config; config never imports bidder
implementations or executes constructors. All metadata declarations participate,
including declarations disabled in the current feature mode.

Every metadata implementation must have a discovered factory declaration. Its
optional feature must match exactly: ungated/gated, gated/ungated and different
feature names are all mismatches. Each associated optional feature must contain
the explicit `auctioneer-config/<same-feature>` forwarding edge in the bidder
manifest. An unrelated edge, weak optional-dependency forwarding or another
local feature is not that declared ownership contract.

Several independent public bases may share one implementation. Comparisons use
the explicit implementation key, not a public-name conversion or central bidder
list. An unused factory remains legal until metadata is promoted. The scanner
already owns duplicate factory declarations, valid keys, manifest membership and
fixed-root source containment; the metadata compiler owns duplicate public rows
and scalar validation.

This build-time declaration check is not the strict runtime catalog join.
Actual target-feature membership, alias policy, cached startup failures,
constructor receipts and removal of the existing generic fallback remain later
acceptance work. No source definition is promoted and no endpoint, default,
workload limit, accounting coefficient or dependency revision changes here.

## Regression Ownership

Pure reconciliation tests live beside `build_reconcile.rs` and run through the
existing `discovery_codegen` target. They independently vary the metadata and
factory feature predicates, the forwarding edge and the public/implementation
names. A scanner-level control changes the actual temporary manifest rather
than passing a separately fabricated forwarding map alone.

Initial pure-helper RED: six tests ran; four failed because the permissive stub
accepted missing factories, mismatched predicates and absent forwarding. The
same six tests passed after implementation. The workflow guard separately
failed on the missing required command before its workflow step was added;
the full 103-test workflow target then passed.

The isolated native target copies only owned Rust/TOML source and build files,
uses original dependency source paths, clears inherited build overrides and
keeps one temporary target directory through the entire sequence. Per-command
and whole-suite watchdogs, bounded file-backed output and process-group
termination/reaping protect every child-result path. The fixture's lean native
development profile is not a production artifact or resource measurement.

It executes real factories and compares compiled membership and metadata in
feature off/on/off builds. Five independent mutations then remove a factory,
change either side of gated/ungated ownership, change the required feature,
or remove forwarding. Each must fail with its intended build diagnostic;
restoring the source must recover without cleaning. The initial hook-less
sequence really compiled the invalid Mocktioneer and missing-forwarding cases,
then failed the test. After wiring, all five received their intended refusals
and every recovery plus the final executable passed.

Quality review identified that generating a fresh offline child lock alone
could select another cached dependency version. The harness now seeds the
workspace lock, uses offline `cargo update --workspace` to preserve dependency
pins, and verifies every materialized package name/version/source/checksum
against the reviewed root lock before any build. The first seeded
`generate-lockfile` attempt was correctly refused by this check: five helper
tests passed but the real target failed before building. The replacement
materialization passes without weakening verification or adding exceptions.
Four independently changed identity fields failed the permissive helper stub;
all five lock-helper controls pass after repair. This changes neither the
repository lockfile nor SDK pin.

## Verification Scope

All commands use Rust 1.98.1 with locked/offline resolution and two Cargo build
jobs. Counts below are per invocation, not disjoint totals. The selected test
scopes report zero failures and zero ignored tests.

| Scope                                                                                   | Result                                                                                       |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Bidder library, discovery compiler and public feature-snapshot targets, secure defaults | 156 passed: 129 + 24 + 3                                                                     |
| Same bidder targets with `insecure-bidders`                                             | 167 passed: 140 + 24 + 3                                                                     |
| Config `bidder_bindings`, `codegen`, `smoke`, secure defaults                           | 150 passed: 20 + 116 + 14                                                                    |
| Core library and workflow target, secure defaults                                       | 606 passed: 503 + 103                                                                        |
| Same core targets with `insecure-bidders`                                               | 606 passed: 503 + 103                                                                        |
| Isolated factory-build target with lock controls                                        | 6 passed; five intended build refusals, five same-cache recoveries, four executable controls |
| Bidder library guest checks, three ABIs in both feature states                          | Six passed                                                                                   |
| Workspace all-targets Clippy, `insecure-bidders,conformance-suite`, `-D warnings`       | Passed                                                                                       |
| Workspace formatting, diff and raw-JSON gate                                            | Passed; 168 counted fixtures across 50 files                                                 |
| Existing roster and batch-rebase checks                                                 | Passed                                                                                       |

The bidder scopes use `--lib --test discovery_codegen
--test insecure_bidders_snapshot`; discovery's native Unix-socket controls run
with local filesystem/socket permissions, not deployed provider access.
Core uses `--lib --test ci_workflows`. Log files are
`/private/tmp/m7-factory-bidder-{secure,insecure}.log`,
`/private/tmp/m7-factory-config.log` and
`/private/tmp/m7-factory-core-{secure,insecure}.log`.

The final native child target used
`cargo test --offline --locked -j2 -p auctioneer-bidder
--test factory_binding_build -- --nocapture` and finished in 50.77 seconds.
Its log is `/private/tmp/m7-factory-build-final.log`. Three initial executable
passes cover off/on/off; the fourth is the restored final executable. The
child's all-declarations views are independent of the outer test binary's
enabled feature mode. Expected live membership selects only the exact feature
enabled in that child, not every optional feature.

Guest commands use `cargo check --offline --locked -j2 -p auctioneer-bidder --lib
--target <ABI>`, separately with and without `--features insecure-bidders`,
for `wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`. They verify
compilation with the host/build dependency split, not linking or execution.
Logs are `/private/tmp/m7-factory-<ABI>-{secure,insecure}.log`. Final lint is
`cargo clippy --workspace --all-targets --offline --locked -j2
--features insecure-bidders,conformance-suite -- -D warnings`, recorded in
`/private/tmp/m7-factory-clippy-verified.log`. The earlier local Clippy failures
were repaired without adding lint exemptions. Existing GVL deletion notices
and the tracked `proc-macro-error2` future-compiler warning remain.

Spec review approved only this declaration-reconciliation slice. Quality review
required the child-lock repair and then approved it; parent verification above
independently reran the complete repaired target and final lint. No complete
workspace test run, fresh hosted CI run or deployed-provider test is claimed.

This record does not qualify schema compatibility, full catalog resource costs,
physical WASM memory, hosted deployment behavior or M7 completion.
