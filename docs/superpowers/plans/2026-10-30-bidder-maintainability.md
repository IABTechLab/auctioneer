# Bidder Maintainability Implementation Plan

> **For agentic workers:** Use the executing-plans or subagent-driven-development workflow. Preserve protocol behavior and existing test coverage.

**Goal:** Repair the guest test builds and make priority bidder implementations easier to extend without repeating shared infrastructure.

**Architecture:** Keep ordinary Rust submodules within every implemented bidder owner. The root retains factory/trait wiring; request and response modules own their inline unit tests. Add parameter or projection modules only for real responsibilities. Extract only status and request-metadata behavior whose contracts genuinely match. Keep provider-specific transforms and intentional error differences local. The user explicitly selected the uniform request/response layout for all 17 implementations on 2026-10-30.

**Tech Stack:** Existing Rust workspace, Serde, generated bidder discovery and bounded request/response APIs. No new dependencies.

## Constraints

- This executes the approved review recommendations, not a protocol redesign.
- Unit tests remain inline. Owner conformance remains under `tests/` with ordinary modules and existing discovery.
- No handwritten source includes, extra public production hooks, ignored tests or broad lint exemptions.
- Do not change captured fixture/provenance bytes, workload limits, resource coefficients, SDK pins or historical qualification artifacts. Verification found two pre-existing overlong authored manifest notes (Rubicon and Unruly): wrap only those prose lines, prove the parsed TOML unchanged, and update only their local factory/conformance manifest checksums. Factory cases and external captures must remain unchanged.
- SDK outbound accounting and absolute guest resource qualification remain open independently of these repairs.

## Tasks

- [x] Separate the native filesystem-backed published-catalog conformance modules from portable fake-based auction tests. Update the stale `JsonField` fixture without adding a compatibility API.
- [x] Split MSFT by cohesive responsibilities, retaining every existing unit and owner-conformance control.
- [x] Split Rubicon by cohesive responsibilities, retaining every existing unit and owner-conformance control.
- [x] Apply the same root/request/response contract to all other implemented bidders. The full before/after explicit owner-test inventory matches for all 17 owners (716 declarations, including 559 formerly at their roots); final execution remains below.
- [x] Extract the standard status gate with inline tests for matching contracts; retain deliberate status/error differences. Final execution remains below.
- [x] Audit all request constructors and reuse only exact shared endpoint/header/sink behavior. The [audit](../../testing/bidder-shared-contract-audit.md) records retained differences.
- [x] Provide a reusable, non-discovered bidder template and update the owner rules to require request/response modules for every implemented bidder, as explicitly selected by the user.
- [x] Run native bidder/exchange tests, owner conformance and source-import isolation; strict lint across native and the three real guest CI scopes; guest conformance builds; formatting and diff checks.
- [x] Review the integrated diff and update issue/adoption records with actual results, then commit and push.

## Verification

Use `CARGO_INCREMENTAL=0`, `--locked --offline -j2` and the existing source-checkout environment. Keep heavy Cargo commands sequential. Native socket tests need loopback access; guest checks prove compilation only, not hosted deployment acceptance.

1. `cargo test -p auctioneer-exchange --test construction_bound --test run_auction`
2. `cargo test -p auctioneer-bidder --lib`
3. `cargo test -p auctioneer-bidder --test test_isolation` and the discovered owner-conformance targets affected by module relocation.
4. Native strict Clippy with the existing `conformance-suite` feature.
5. For Cloudflare/`wasm32-unknown-unknown`, Fastly/`wasm32-wasip1` and Spin/`wasm32-wasip2`: the unchanged CI command selecting adapter, exchange and privacy with `--all-targets --features conformance-suite -- -D warnings`; then `cargo test --no-run` for each adapter's `fanout_conformance` on its actual target.
6. `cargo fmt --all -- --check` and `git diff --check`.

Record failures and final outcomes without relabeling prior failed runs as successful.

## Verification Record

The source tree was frozen before final execution. Static inventories retain
all 716 explicit owner test declarations, including the 559 previously at the
roots. Independent read-only review also compared those 559 unit bodies,
ignoring formatting, and reported no behavior or ownership findings.

During verification, the new duplicate-coverage metadata fixture incorrectly
expected duplicate IDs to be accepted. It now separately pins ordered unique
coverage and the existing duplicate refusal. APS's exact source-membership
inventory now includes the two new Rust files. Native strict lint exposed
move-related imports, DTO field visibility and test setup; these were repaired
without broad exemptions. The pre-existing manifest-prose failure was fixed
by the two value-preserving wraps described above; the all-manifest 100-column
check was not relaxed. A separate Rust/TOML/JSON comparison against `HEAD`
confirmed both decoded manifests and all factory cases unchanged, excluding
only the required local manifest checksum field.

Verified scopes:

| Scope | Result |
| --- | --- |
| Bidder library, secure defaults | 1,146 passed, 0 failed, 9 existing ignored |
| Bidder library, `insecure-bidders` | 1,157 passed, 0 failed, 9 existing ignored |
| Owner request-allocation conformance | 67 passed, 0 failed |
| Owner sealed-response conformance | 32 passed, 0 failed |
| Registry/source conformance, separate secure and `insecure-bidders` runs | 25 passed in each, 0 failed |
| Workspace source-import and bidder isolation | 16 passed, 0 failed |
| Exchange `--all-targets --features insecure-bidders` | 641 passed across 36 targets, 0 failed, 1 existing ignored |
| Native bidder/exchange `--all-targets --features insecure-bidders` strict Clippy | Passed; no new exemption |
| Cloudflare, Fastly and Spin unchanged all-target adapter/exchange/privacy CI strict-Clippy scopes | All three passed |
| Each guest adapter's feature-enabled `fanout_conformance` with `--no-run` on its actual target | All three compiled and linked |
| Raw-JSON fixture gate and its rejection controls | Passed; unchanged total of 192 fixtures across 55 files |
| Cargo formatting, explicit changed-Rust-file formatting and diff whitespace | Passed |

Guest checks are compilation/linking evidence, not guest execution or hosted
provider acceptance. The known archive tombstone notices and transitive
`proc-macro-error2` future-incompatibility warning remain. No threshold, SDK
pin, feature ownership or captured/historical qualification artifact changed.
Absolute guest fit, selected-validator qualification and SDK resource-owner
correspondence remain open independently of this refactor.

## Publication

Implementation commit `ae6c9fbbcf095ee4eb43807775fb08e773f1576c` was pushed to
`origin/bidder-catalog`. Against that committed source state, the exact CLI
test `bidders::resources::tests::report_uses_normalized_baked_inputs_and_all_runtime_owned_terms`
rebuilt and passed (1 passed, 0 failed, 711 filtered out). This checks the
reporter's current source/input binding; it is not a full CLI-suite run or
resource requalification. The follow-up publication note changes documentation
only; runtime source, SDK pins and historical evidence remain unchanged.

## Shared Fake Follow-Up

The first refactor omitted the shared fake because it lives outside serving
discovery. The user requested the same layout for it, without promoting it into
the catalog. This follow-up preserves the public fixture API and all capability
knobs: root factory/trait wiring and counters, ordinary request/response modules,
and a controls module for synthetic metadata and configuration types. Helper
units stay inline; public-contract and allocation conformance remain under the
crate's `tests/`. Assets, feature gates, serving registration and limits remain
unchanged.

- [x] Add the same layout check for the shared fake. Observe its original
  one-target failure for the missing request module, then split by responsibility.
- [x] Keep every existing contract/allocation test. Add four focused inline helper
  units for metadata identity, media correlation and browser-config envelopes.
- [x] Correct a pre-existing comparison fixture that decoded one sealed response
  twice. The exact failure was reproduced with the original unsplit source.
  Each comparison now gets its own receipt, with identical payload, node cap and
  assertions; no production decode gate is weakened.
- [x] Make synthetic workspace installation copy the whole owned fake module tree
  through the existing recursive copier. Strengthen its control to require exact
  path-set and byte equality; observe it fail when only the old root is installed.
- [x] Repair the scratch library's stale `configured_registry` reexport. Preserve
  the real admission wrapper verbatim and add a regression that first fails on
  the duplicate import. The actual feature-off/on/off executable controls and all
  six invalid registration cases pass: 19 child commands, 150.90 seconds, sampled
  peak logical scratch size 893,538,595 bytes, unchanged watchdog and size limits.
- [x] Verify bidder, core and exchange consumers, native and guest lint, source
  isolation and formatting; record exact execution scopes.
- [x] Update the issue record, commit and push the follow-up.

Follow-up verification uses the same locked, offline, nonincremental environment
and sequential two-job Cargo commands as above:

| Scope | Result |
| --- | --- |
| Bidder library, secure defaults | 1,150 passed, 0 failed, 9 existing ignored |
| Shared fake allocation conformance | 8 passed, 0 failed |
| Shared fake public contract | 17 passed, 0 failed |
| Registry/source conformance, including both scratch-workspace regressions | 27 passed, 0 failed |
| Workspace source-import and bidder isolation | 16 passed, 0 failed |
| Core library with `conformance-suite` | 570 passed, 0 failed |
| Exchange `--all-targets --features insecure-bidders` | 641 passed across 36 targets, 0 failed, 1 existing ignored |
| Native bidder/core/exchange `--all-targets --features conformance-suite` strict Clippy | Passed after correcting three test-helper lint issues, without exemptions |
| Cloudflare, Fastly and Spin unchanged all-target adapter/exchange/privacy CI strict-Clippy scopes | All three passed |
| Cargo formatting, explicit changed-Rust-file formatting and diff whitespace | Passed |
| Raw-JSON fixture gate and its rejection controls | Passed; unchanged total of 192 fixtures across 55 files |

The scratch executable control above is additional to those suite counts.
Independent read-only review reported no API or behavior findings. Earlier
negative layout, module-copy and wrapper checks failed on their original
implementations; the final source-conformance run includes their passing forms.
The repeated-response failure was reproduced against the original unsplit fake,
not dismissed as a refactor failure. All original public capability fields,
defaults and counters are preserved. Guest lint is compilation evidence only;
no hosted or resource qualification is claimed, and the existing dependency
future-incompatibility warning remains open.
