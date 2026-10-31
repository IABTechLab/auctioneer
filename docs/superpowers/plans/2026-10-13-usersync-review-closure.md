# User Sync Review Closure Implementation Plan

> **For agentic workers:** Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` for the tasks below. Keep write sets disjoint and run mutation probes only in independent temporary source copies.

**Goal:** Close the confirmed user-sync review defects, turn the privacy reference corpus into behavioral evidence, and implement the approved cookie-integrity boundary in M5.

**Architecture:** Keep privacy in `auctioneer-privacy`, shared config validation in `auctioneer-config`, and HTTP/cookie integration in `auctioneer-core`. Use the existing EdgeZero handler, bounded secret-store, admission, clock, and egress paths. Signing protects issued cookie state; it does not authenticate a callback UID or substitute for consent.

**Tech Stack:** Rust, serde, url, HMAC-SHA256, existing EdgeZero secret registry, native HTTP/browser and provider compilation checks.

## Task 1: Independent Privacy Inputs

**Files:** `auctioneer-privacy/src/sync/{mod,permission}.rs`, `tests/sync_{sources,permissions}.rs`, `tests/data/sync/README.md`.

- [x] Add account opt-in cases with host GDPR on, host TCF off, account/global versus web opt-in, and malformed/denied/allowed consent. Drive real permission APIs.
- [x] Exercise host-name exclusion with the basic-vendor and vendor-exception lists separately populated, including the empty and NUL names.
- [x] Drive every reference corpus row through real Auctioneer extraction; explicitly name intentional differences rather than comparing the corpus to itself or updating its reference outputs.
- [x] Remove the unused sync `ccpa_enforce` field and ladder; preserve the documented absence of a CCPA sync-denial stage and existing USP propagation.
- [x] Verify individual branch mutations fail these tests. Run `cargo test -p auctioneer-privacy --all-targets --locked --offline`.

## Task 2: Callback And Template Contracts

**Files:** `auctioneer-config/src/usersync.rs`, `build_support.rs`, `auctioneer-core/src/usersync/template.rs`, inline unit tests and existing HTTP/config integration targets.

- [x] Reproduce account-key macro injection, parameter count growth, malformed literal percent/UTF-8 encoding, build/render disagreement, and valid alias priority rejection before changing behavior.
- [x] Reject macros in callback query keys. Check actual rendered callback account/bidder/UID fields and reserve room for an inserted account.
- [x] Use the same lexical rules in build validation and rendering. Match callback validation to the receiver's strict query encoding and count rules.
- [x] Align priority-name validation with accepted bounded alias names.
- [x] Vary GPP, USP and SID inputs independently in both callback and bidder URL rendering. Assert encoding and independently guard endpoint scheme/host/port/userinfo/fragment checks without relaxing production config validation.
- [x] Run focused config/core targets; verify dropped encoding and individual endpoint checks are detected by isolated mutations.

## Task 3: Issued-Cookie Integrity

**Files:** `docs/superpowers/specs/2026-10-13-usersync-cookie-integrity-design.md`, root/crate manifests, `auctioneer-core/src/usersync/{cookie,signed,evict}.rs`, shared key loading and all three sync handlers; adapter manifests/test support and browser fixtures; `auctioneer-runtime/src/aux_work.rs`.

- [x] Review the approved design before implementation. No separate SDK, plugin loading, key-bearing runtime config, or provider-specific handler routes.
- [x] Add red tests for caller-generated unsigned cookies, payload/key-ID/MAC/version tampering, unknown/retired keys, rotation, oversized keyring/envelope and redacted errors.
- [x] Implement a versioned HMAC-SHA256 envelope using a proven crypto crate. Keep the bounded inner JSON codec and reference fixtures as payload qualification, not claims of signed-wire parity.
- [x] Load one bounded atomic keyring from the default EdgeZero secret store per request under the existing absolute deadline. Defer errors to the UID-reading stage so earlier opt-out/account errors retain their order.
- [x] Use the signed reader for every production UID read and sign every successful write after transactional eviction. Account for the complete envelope in size limits.
- [x] Replace future-expiring empty UID values with explicit Path/Domain-matched browser deletion and test deletion followed by readback.
- [x] Add real-App/native-wire tests for missing secrets, rotation, tampering, callback/readback, opt-out precedence and cancellation. Update provider provisioning and browser fixtures without committing real keys.
- [x] Measure full handler allocation paths and fund bounded secret/keyring/signing work in the auxiliary derivation.

## Task 4: Diagnostics And Guard Quality

**Files:** sync handler error mapping/tests; `auctioneer-config/tests/disclosure_codegen.rs`; existing CI guard and secure-user-sync script.

- [x] Map SID/GPP resource limits to a fixed resource-limit response, independently of malformed input.
- [x] Port the non-ancestral flattened-JSON-map regression without its prohibited commit trailer. Do not rewrite historical commits.
- [x] Remove or explain the redundant cookie minimum-length check while retaining a guard for bounded formatting work.
- [x] Pin Fastly guest-build/execution steps in the CI contract and require the secure script to execute its selected test rather than accepting zero matches.
- [x] Correct the unimplemented/disabled signing claim and update M5/master/roadmap ownership; preserve existing `ISSUES.md` edits for the publication commit below.

## Task 5: Integration And Publication

- [x] Run formatting, strict native Clippy, secure/default and insecure core/privacy/config suites, conformance, fixture gates, native signed-cookie tests and actual-browser acceptance.
- [x] Compile/lint affected WASM adapters; distinguish existing guest artifacts from fresh builds and live-provider qualification.
- [x] Document provider secret materialization/cancellation limits; a bounded accepted value is not universal host peak or interruption evidence.
- [x] Run a fresh self-review of all changed boundaries; record red/green and mutation evidence with unique log paths.
- [x] Commit the verified fixes without co-author/byline trailers and push `m5-usersync` after this repair round.

Published implementation and `ISSUES.md` repair statuses: `a1cbeb298fe540d64bd02794a8be33c17d8f12d0`.
The [verification record](2026-09-30-m5-verification.md#cookie-integrity-and-review-closure-october-13)
contains fresh suite totals, mutation evidence and deployment qualifications.
