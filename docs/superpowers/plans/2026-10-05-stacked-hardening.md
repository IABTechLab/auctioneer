# Stacked Milestone Hardening Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans or superpowers:subagent-driven-development. Preserve the branch order below; do not implement later-stage features on earlier branches.

**Goal:** Correct the concrete ownership/privacy/configuration gaps on their owning branches and carry verified changes forward to M6.

**Architecture:** Keep the existing EdgeZero generated App, runtime extractor, sealed response API, and typed privacy policies. Add bounded regression evidence without introducing a router, config loader, signing protocol, or workspace crate.

**Tech Stack:** Existing Rust workspace, serde JSON fixtures, existing native Axum binary, and Playwright for actual-browser evidence.

## Branch Order

- [x] M3b: protect sealed transport payloads from derived Debug disclosure.
- [x] Verify M3b; commit and merge `m3b-fanout` into `m4-privacy`. M3b fix `b1561c0`; merge `9b6c8a2`. M4's sealed/non-Clone API was retained and bidder 52/52, compiler guards 2/2, baseline scrub 40/40 and strict bidder Clippy passed.
- [x] M4: make hostile privacy fixtures reusable and preserve the existing exhaustive composition oracle. No new production privacy defect was demonstrated.
- [x] Verify M4; commit and merge `m4-privacy` into `m5-usersync`. M4 fix `1bad3ff`; merge `045fd8c`, without conflicts. Scrub 40/40 and strict exchange Clippy passed on M4; input-only extraction passed independent compliance and quality review.
- [x] M5: extend actual-App invalid/outage/deletion/recovery coverage and add repeatable actual-browser sync evidence through the existing Axum entry point.
- [x] Verify M5; commit and merge `m5-usersync` into `m6-hardening`. M5 fix `5367ab0`; M6 merge `74c8bfa`, without conflicts. Independent compliance/quality re-review passed after demonstrated SIGTERM and permission-removal fixes; 153 focused Rust tests, Chromium flow, and both shutdown regressions passed.
- [x] M6: reconcile the evidence inventory and resource/debug implementation requirements with the inherited fixes. Do not enable debug as part of this follow-up. The M6 design now distinguishes inherited formatting/privacy/sync/browser evidence from new projection, prepaid debug accounting, allocation/concurrency, and live-provider work.

## Task 1: Transport Debug Safety

Files: `crates/auctioneer-bidder/src/types.rs` and `crates/auctioneer-bidder/src/meta.rs`.

- [x] Add colocated tests asserting exact safe normal/pretty Debug output for `BidderHttpRequest`, `BidderHttpResponse`, and `OutboundRequestMeta`; include body bytes, credential-bearing headers/URLs, IDs, and the raw metadata pool.
- [x] Run `cargo test -p auctioneer-bidder --lib --locked --offline` and demonstrate the new assertion fails with derived formatting. Baseline 47 passed; all three new regressions failed by exposing secrets before the fix.
- [x] Replace all three unsafe derived Debug implementations with status/count-only formatting. Never expose method/URL/header/ID contents, bodies, or raw-pool offsets. Correct the metadata safety comment and preserve existing fields, accessors, and cloning behavior, including the non-Clone response seal.
- [x] Rerun the library tests and `cargo test -p auctioneer-bidder --test response_body_is_sealed --locked --offline`. Library 50/50 and compiler guards 2/2 passed; strict bidder library/test Clippy and formatting passed.

## Task 2: Privacy Corpus and Composition

Files: `crates/auctioneer-exchange/src/scrub/composition_tests.rs`, test-only fixture/support files under `crates/auctioneer-exchange/tests/`, and M4 coverage evidence.

- [x] Preserve the independent wire oracle, 256 decision combinations, idempotence, and shadow-field coverage; do not duplicate already-covered behavior.
- [x] Extract the existing adversarial request builder to `crates/auctioneer-exchange/tests/support/privacy_fixture.rs` for future debug projection tests. Keep outbound-policy expected values separate from unconditional debug omission requirements.
- [x] Test representative hostile shapes and restrictive-policy combinations before changing production behavior. The existing 40 scrub tests pass before and after extraction, including all 256 decision combinations and shadow variants; no production scrub change was needed.
- [x] Run focused scrub tests and strict scoped Clippy; update exact evidence references. See `2026-09-29-m4-coverage.md`; 40/40 scrub tests and strict exchange library/test Clippy passed.

## Task 3: Sync Configuration Lifecycle

Files: `crates/auctioneer-core/tests/support/usersync_app.rs`, `crates/auctioneer-core/tests/usersync_precedence.rs`, and a behavior-named integration test if needed.

- [x] Use the existing envelope/extractor path and counted request-bound store. Add missing, corrupt/integrity-invalid, and unavailable states without production test hooks.
- [x] Exercise healthy policy, policy replacement, invalid replacement, outage, deletion, and recovery across cookie_sync, setuid, and getuids.
- [x] Assert one read per request, existing status/Retry-After mappings, no stale CORS permission on extraction errors, and no unauthorized Set-Cookie.
- [x] Rerun lifecycle, precedence, route, and flow tests; fix production code only after a failing behavioral test. Lifecycle 5/5, precedence 23/23, routes 15/15, flow 4/4 and strict scoped Clippy passed. No new production defect was demonstrated.

## Task 4: Browser Sync Evidence

Files: `scripts/check-usersync-browser.mjs`, its shutdown regression and pending-launch fixture under `scripts/tests/`, `docs/testing/usersync-browser.md`, and test-only `crates/auctioneer-adapter-axum/examples/usersync_browser_config.rs`; reuse `crates/auctioneer-adapter-axum/src/main.rs` unchanged.

- [x] Start the real native binary with an isolated valid runtime envelope, a controlled publisher origin, and no real bidder credentials. The example uses typed validation and the existing envelope builder; HTTP loopback origins require the existing development feature.
- [x] Exercise browser-enforced credentialed CORS and preflight, setuid navigation/cookie acceptance, getuids visibility, and denied-origin behavior. Chromium 153.0.8010.12 passed genuine 204/403 preflights, controlled callback, Secure/SameSite=None storage/delivery, and a healthy allowed-origin control after denial.
- [x] Record what actually ran. A controlled callback is not a real bidder redirect certification, and a loopback cookie test is not proof of third-party cookie acceptance on all browsers/providers. See `2026-09-30-m5-verification.md` and `../../testing/usersync-browser.md`.
- [x] Keep the harness repeatable, bounded, and cleanup-safe; do not add a Auctioneer-local router or a test handler to production. A rejecting local proxy blocks external browser traffic without interception; processes, contexts, sockets, and temporary storage are cleaned up. Deterministic SIGTERM/startup and cleanup-permission regressions both failed before their fixes, then passed 2/2.

## Verification and Integration

- [x] Run focused tests and strict Clippy for changed Rust targets, formatting, and whitespace checks. M5 parent reruns: 153 Rust tests, actual Chromium flow, and the SIGTERM shutdown regression passed; scoped strict Clippy, formatting, Node syntax, whitespace, and raw-JSON checks passed. This is not a full workspace/provider rerun.
- [x] Commit only owned changes. Merge serially with non-destructive git commands and preserve later milestone implementations when resolving conflicts. Ordered merges are `9b6c8a2`, `045fd8c`, and `74c8bfa`; the later M4 seal and M5 sync handlers/caps were retained.
- [x] After each merge inspect the combined diff and rerun affected tests. Keep implementation proof, actual-browser proof, and hosted-provider qualification distinct. Final M6 results are recorded below; independent read-only merge/documentation review found no blockers.
- [x] Reconcile M6 with the final inherited commits; retain its explicit debug resource charge and on/off/denied allocation/concurrency qualification tasks rather than charging unused debug work in earlier milestones.

## Final M6 Merge Verification

Verified in `.claude/worktrees/m6-hardening` after merge `74c8bfa`:

- `cargo test -p auctioneer-bidder -p auctioneer-exchange --lib --locked --offline`:
  52 bidder and 119 exchange tests passed, including the safe formatters and
  exhaustive privacy composition. The two sealed-body compiler guards had
  also passed on the identical inherited M5 source.
- `cargo test -p auctioneer-core --test usersync_config_lifecycle --test usersync_precedence --test usersync_flow --test usersync_routes --locked --offline`:
  5 + 23 + 4 + 15 tests passed. Final merged Rust total: 218; no failures.
- The feature-enabled Axum binary and config example rebuilt successfully.
  `check-usersync-browser.mjs` passed with real Chromium 153.0.8010.12;
  the separate shutdown regression passed 2/2. The latter uses a synthetic
  pending browser only to test cleanup, not to claim actual-browser behavior.
- Strict all-targets Clippy passed for bidder, exchange, core and Axum with
  `insecure-bidders` and `-D warnings`. Formatting, whitespace, Node syntax,
  the raw-JSON gate (98 fixtures in 39 files), and nine new evidence paths
  plus the verification anchor passed.
- Git ancestry confirms M3b is contained in M4, M4 in M5, and M5 in M6.
  M6's crates, scripts and Cargo manifests/lockfile are identical to M5
  `5367ab0`; the M6 follow-up is documentation only.

No full workspace/WASM/provider matrix was rerun for this follow-up, and no
hosted qualification or deployment is claimed. Debug projection, prepaid
debug resource accounting, allocation/concurrency measurements and new live
provider evidence remain M6 implementation work. Main was not changed, and
these local commits/merges were not pushed.
