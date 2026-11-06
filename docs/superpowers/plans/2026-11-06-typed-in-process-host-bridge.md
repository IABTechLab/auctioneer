# Typed In-Process Host Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this approved plan task-by-task. Steps use checkbox syntax. This document is not implementation authorization.

**Goal:** Deliver #51's shared HTTP/embedded preparation, single typed engine outcome and funded completion ownership without adding another configuration or auction path.

**Architecture:** Both caller kinds enter one explicit-context prepared operation. That operation owns request-scoped resolved state, builds borrowed engine views only inside its consuming future, and moves existing receipts with typed results. The existing reducer supplies selection facts; deterministic wire output is a consuming projection, not another execution API.

**Tech Stack:** Existing Rust workspace, serde/validator, generated bidder settings, portable runtime/exchange interfaces and current SDK HTTP wrapper. Standard-library shared ownership only; no new dependency, crate, resource equation or numeric limit.

**Contract:** [Typed in-process host bridge](../specs/2026-11-06-typed-in-process-host-bridge-design.md). At the maintainer's request, this spec and plan are proposed together for joint review. The public API, receipt ownership proposal and implementation remain subject to approval and planning-PR integration; the parent #34 contract is not approval of these detailed APIs.

---

## Scope, Baseline And Review Gates

Base: Auctioneer `87e329fe88f3a1c15a4d298d026607971b3065ee`; host interface reference `51730bdaf841e4d7bb96c2258e5e98169bdff9a7`. Refresh before implementation and record a contract delta if either relevant interface changes. #34 is closed; reuse its accepted contract rather than reopen it. Keep #51 open when this planning PR merges.

Only Auctioneer's bridge is implemented here. Actual Trusted Server plan/executor, SDK alignment, final targeting consumers and linked Fastly acceptance remain Trusted Server #1505/#76. Schema qualification and uniform catalog publication remain #46/#47, #72-#75 and #88-#90. Do not remove `native_only`, change schemas/bidder routing, add provider APIs or start another issue because it is named here.

The concrete host-service gate is Trusted Server #1510, with #1507/#1508's qualified SDK cutover and #1509's host agreement, as specified in the design's final subsection. Portable changes need not wait for that work, but #51 cannot claim completed host binding from fake services or incompatible pins. Reconcile this selected leaf relationship before implementation tracking changes; never add #1505's completion as a cyclic blocker. No SDK revision change is authorized by this docs-only PR.

Read `AGENTS.md`, `crates/AGENTS.md` and [development/test ownership](../../testing/development.md). Preserve process/allocator isolation, feature lanes, unchanged workload and frozen bounds. Future tests below are requirements/expected outcomes, not executed results. New types/modules are proposed paths, not claims that they already exist.

## File And Test Ownership

| Responsibility                       | Modify Existing                                                                                                                       | Create                                                                                  |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Receipt owner, same ledger           | `crates/auctioneer-runtime/src/auction_budget.rs`                                                                                     | None; unit tests inline                                                                 |
| Source-seat provenance               | `crates/auctioneer-bidder/src/types.rs`, `src/bidders/media_type.rs`, discovered `TypedBid` construction sites and their owning tests | None; no central bidder-name switch                                                     |
| Typed outcome/projection             | `crates/auctioneer-exchange/src/{lib,response,context,tmax}.rs`, `src/run/mod.rs`, existing callers of `run_auction`                  | `crates/auctioneer-exchange/src/outcome.rs`                                             |
| Host permission ceilings             | `crates/auctioneer-privacy/src/decision.rs`, exchange `src/run/mod.rs`, `src/scrub/decision.rs` and context                           | Prefer inline definitions in the existing decision owner; no separate permission engine |
| Explicit services and resource facts | core `src/lib.rs`, `src/host_primitives/gates.rs` projection only                                                                     | `crates/auctioneer-core/src/auction/{mod,services}.rs`                                  |
| Shared admission/preparation         | core `src/config/{resolve,bidder_settings,budgets,policy_account,policy_host}.rs` where factoring is needed                           | `crates/auctioneer-core/src/auction/prepare.rs`                                         |
| Owned execution/results              | core `src/handlers/auction.rs`, existing error/health/audit consumers                                                                 | `crates/auctioneer-core/src/auction/{operation,result}.rs`                              |

`mod.rs` contains declarations/reexports and the compact facade, not another large handler. Combine a proposed small file into its owning module if it has no independent responsibility. Use normal Rust modules; generated includes remain the only source-import exemption. Do not introduce an integration crate or an external-path Cargo test target.

Move the existing portable disclosure scope from core `src/handlers/request_disclosure_audit.rs` to `src/auction/audit.rs`, preserving its inline tests and bounded logger implementation. Update ordinary module declarations and actual handler/bootstrap/adapter/test references; do not compile the old source through an include or leave a second guard implementation. Migrate `.github/workflows/test.yml`'s exact `handlers::request_disclosure_audit::tests` filter to `auction::audit::tests`, the subprocess test's hardcoded `--exact` selector to its new qualified name, and the relocated `include_str!("auction.rs")` source probe to its real handler path. Verify the new filter lists and executes the same nonzero audit cases and the subprocess executes exactly one child case; an exit-zero empty run is not acceptance. This is a selector/path migration only, with no lane removal, assertion weakening or workflow-snapshot gate. The funded state owns this scope, not a borrowed external observer. Explicit SDK binding also extends the existing `src/handlers/bidder_credentials.rs` constructor and `host_primitives/edgezero_proxy.rs`; no alternative transport is added.

Extend existing targets named in each task. Wire-focused callers consume the new explicit projection; typed-result controls assert engine facts directly. Ordinary shared fixtures use `fake_` identities and capable existing fake bidders; real protocol captures remain bidder-owned. No production bidder is a shared stand-in.

## Task 1: Freeze Observable Baseline And Caller Inventory

**Files:** Existing core `tests/{handler_resolution,resolve_runtime_config,auction_config_lifecycle,auction_config_retention,privacy_config_parity,response_account_resolution}.rs`; exchange `tests/{determinism,response_projection,bid_impression_correlation,bidder_call_correlation}.rs`, `tests/targeting_prefix/`; corresponding current implementation modules.

- [ ] Inventory every `run_auction` caller and `TypedBid` constructor using `rg -n 'run_auction\(|TypedBid\s*\{' crates`. Record actual feature gates and test counts from Cargo selection, not a guessed count in this plan.
- [ ] Extend current characterization tests for wire/account error precedence, default versus named account selection, equal-price/deal selection, disabled winner targeting, multiple bids per bidder/imp, browser-only results and bidder errors with no bids. Keep expected values independent of the future shared helper.
- [ ] In the existing targets, add the future equivalence and retained-selection assertions as the API lands in subsequent tasks; first prove the baseline oracles reject deliberately wrong local expected values without changing production source.
- [ ] Run the baseline commands below. Expected: actual nonzero tests and green current behavior. Capture failures before editing; missing prerequisites are blocked verification, not baseline bugs or permission to weaken controls.
- [ ] Commit only self-contained green characterization changes. Do not claim the still-missing shared API is tested by those controls.

```sh
cargo test --locked -p auctioneer-core --test handler_resolution --test resolve_runtime_config --test response_account_resolution
cargo test --locked -p auctioneer-exchange --test determinism --test targeting_prefix --test response_projection --test bid_impression_correlation --test bidder_call_correlation
```

## Task 2: Add Owned Receipt Backing To The Existing Ledger

**Files:** Runtime `src/auction_budget.rs`; inline unit tests. Extend existing core `tests/auction_budget_concurrency.rs` only for cross-component ownership controls after the bridge exists.

- [ ] Add failing unit cases named `owned_reservations_share_the_original_ledger`, `owned_prepayment_rolls_back_each_failed_phase` and `owned_receipts_survive_the_callers_budget_handle`. Independently vary each phase charge and ledger ceiling; verify the positive live allocation/count before checking release.
- [ ] Run `cargo test --locked -p auctioneer-runtime --lib auction_budget:: -- --list`, confirm each new name, then execute the same filter. Expected red: missing owned operations or failed ownership assertions, never zero matched tests.
- [ ] Add the private borrowed/shared budget owner and owned prepayment to the existing `Reservation`/`AuctionCharges`. Share the atomic reserve/check/drop implementation. Do not derive `Clone` for receipts, construct a second budget from the same limit, widen capacity or add an unsafe lifetime conversion.
- [ ] Test move/drop, partial failure, mixed borrowed/owned receipts on the same ledger, under/at/over cap, capacity overflow and cancellation/unwind. Drop an external `Arc` and prove the live receipt still owns its exact charge. Charge layout/control-block ownership in the later resource checkpoint; do not call it zero-cost.
- [ ] Run runtime units and lint, retain actual output and commit the green ownership primitive without changing numeric costs. If its required storage cannot fit current funded owners, stop activation and return to resource review.

```sh
cargo test --locked -p auctioneer-runtime --lib auction_budget::
cargo test --locked -p auctioneer-runtime --lib
cargo clippy --locked -p auctioneer-runtime --lib --tests -- -D warnings
```

## Task 3: Retain Upstream Seat Provenance Once

**Files:** Bidder `src/types.rs`, `src/bidders/media_type.rs`, actual direct decoder construction sites; inline units and existing owner-local conformance; exchange `tests/{response_projection,response_factors}.rs` as affected.

- [ ] Add red shared-classifier cases for absent/empty seat, two different seats, multiple bids sharing one seat, valid siblings around a bad bid and repeated response calls. Assert requested bidder and source seat differ in the before-state.
- [ ] Extend `TypedBid` with optional shared immutable source-seat provenance. Establish one shared string owner per decoded nonempty source seat before flattening; do not clone the string for each bid or synthesize the alias/provider ID. Retain canonical missing/empty behavior.
- [ ] Update every actual constructor discovered by Task 1. Use the shared helper for common response handling; source-specific decoders capture only information their protocol actually contains. Do not add a names-based registry or a hand-maintained sibling exception list.
- [ ] Add resource controls varying seat byte length and bid population independently; compare one versus many shared references and prove the value is not copied per bid. Include the shared owner/control block and `TypedBid` layout in current decoder/response accounting. Wire projection must continue to use the requested bidder/alias seat.
- [ ] Run current bidder/response owners and compile all callers, preserving assertions and feature gates. Record the discovered migration set and commit only when no constructor silently loses observable provenance.

```sh
cargo test --locked -p auctioneer-bidder --lib
cargo test --locked -p auctioneer-bidder --test fake_bidder_contract
cargo test --locked -p auctioneer-exchange --test response_projection --test response_factors --test response_charge
cargo check --locked --workspace --all-targets
```

## Task 4: Make The Existing Engine Return Its Typed Outcome

**Files:** Exchange `src/{lib,response,outcome}.rs`, `src/run/mod.rs`; every existing caller from Task 1, including the current core handler. Existing `run_auction`, `determinism`, `targeting_prefix`, correlation and response allocation owners.

- [ ] Add red outcome tests for retained selection when winner keys are disabled, nondefault/truncated targeting prefixes, equal prices, deal preference, repeated impression matches and multiple bids in one bidder vector. Assert candidate presence and the exact selected bid index, not only a price.
- [ ] Make `apply_targeting` return/move its existing impression-aligned winners instead of dropping them. Construct private `AuctionOutcome` from the existing owned typed response material, retained request/impression identity, selection and finalized permitted debug; add read-only typed candidate/diagnostic/browser views. Admit the bounded retained identity owner before allocating it. No second winner reducer or reconstructed targeting map.
- [ ] Change the existing `run_auction` return type and all early exits to that outcome. Add its consuming `into_bid_response` projection around existing `build_response`; preserve ID, timestamps, error/nonbid behavior, debug deadline/permissions, media and full bid objects.
- [ ] Migrate every current wire-oriented caller explicitly to the projection in this same green checkpoint. Do not add `run_auction_with_*` compatibility alternatives or clone all bids to satisfy borrowing. Ensure the debug capture is finalized once while its clock/context is still valid.
- [ ] Compare typed selections and the unchanged wire projection against independent expected results. Prove browser-only success, bidder-local errors and timeout diagnostics survive no-winner exits. Run allocation/retention controls, commit the green hard cut and preserve low-level caller funding responsibilities.

```sh
cargo test --locked -p auctioneer-exchange --lib targeting::winner::
cargo test --locked -p auctioneer-exchange --test run_auction --test determinism --test targeting_prefix --test bid_impression_correlation --test bidder_call_correlation
cargo test --locked -p auctioneer-exchange --test response_charge --test response_projection --test response_factors --test code12_attribution
cargo test --locked -p auctioneer-core --test handler_resolution
cargo check --locked --workspace --all-targets
```

## Task 5: Factor Explicit Preparation, Restrictions And Timing

**Files:** Proposed core `auction/{mod,services,prepare}.rs`; current config helpers/settings; exchange context/tmax/run and privacy decision/classified-projection owners. Tests stay inline or in existing `handler_resolution`, `privacy_outbound`, `clock_domain`, `construction_deadline` and provenance targets.

- [ ] Freeze the reviewed public records from spec sections 4-6 in code, including the complete same-source resource tuple, one owned funding owner carrying the already-acquired capacity permit, borrowed JSON/typed input, five host ceilings and original start/outer deadline. Private constructors prevent arbitrary `Derived` data from standing in for the canonical derivation. Move the existing permit; do not install or acquire another pool/slot for the bridge.
- [ ] Add red tests for missing/insufficient resources, mismatched bindings, spent outer cap before any parse/credential/dispatch, and independently varied host ID/geo/FPD/TID/fetch ceilings. Keep other ceilings permissive in each single-denial control so no sibling masks it.
- [ ] Implement input admission before ownership transfer/copy. Preserve selective raw discovery and standalone precedence; share the ordered account-selection rule and `resolve_selected_account` with programmatic input, with no whole JSON tree or serialize/reparse. Disabled/default/named account behavior must be explicit.
- [ ] Resolve host/account/aliases/scoped budgets/features/timeouts using existing explicit-context helpers. Project only request-required bidder settings/floor/account state from the accepted policy; do not retain a full config clone or read ambient boot/primitive state. Carry honest original extension provenance.
- [ ] Intersect host ceilings with existing per-bidder decisions immediately before fetch filtering/classified scrubbing, without rewriting consent. Extend the existing tmax resolution with the absolute outer cap; preserve bidder-visible tmax and the original monotonic domain.
- [ ] Run the exact owners below and prove the host/engine-denial cross-product and independent cap boundaries. Verify negative controls actually reach their named branches, then commit without activating a second preparation path.

```sh
cargo test --locked -p auctioneer-core --lib
cargo test --locked -p auctioneer-core --test handler_resolution --test response_account_resolution --test node_budget_on_the_auction_path --test original_request_extension --test privacy_wire_signal_400
cargo test --locked -p auctioneer-core --test privacy_outbound --test clock_domain
cargo test --locked -p auctioneer-exchange --test construction_deadline --test privacy_policy_composition --test disclosure_policy_diff --test request_provenance
cargo test --locked -p auctioneer-privacy --lib
```

## Task 6: Build The Owned Operation And Cut HTTP Over

**Files:** Proposed core `auction/{operation,result}.rs`; `src/handlers/auction.rs` and existing health/error/audit consumers. Extend existing lifecycle, retention, handler and response targets, not a new HTTP/App fixture program.

- [ ] Add red shared/HTTP equivalence cases in `handler_resolution`, using the same fake responses and deterministic independent clock/RNG instances. Compare full candidate sets, selected identity, targeting and errors; vary one policy dimension at a time and use two isolated host policies on the same executable.
- [ ] Implement consuming `PreparedAuction::run` without an external audit argument. Construct borrowing `ResolvedAccount`/`AuctionContext` locals inside the future; move the owned audit scope, receipts and acquired capacity permit with successful completion and destroy request/policy state first. Keep read-only result views and a guarded consuming wire projection; no raw data/receipt/permit escape.
- [ ] Keep transport acquisition, SDK config resolution, HTTP encoding/body checks, error rendering, no-store, GVL observation and egress formatting/deadline binding in their existing wrapper. Replace duplicated shared assembly with the new operation. Standalone prepayment remains before config extraction. Move its existing auction permit into the funding owner; preserve separate terminal/egress ownership. Release auction capacity only after its data is destroyed or transferred into that existing bounded terminal owner, never merely because the future returned.
- [ ] Exercise drop before poll, suspension in credential/fanout work, successful completion, retained typed result, wire-projection failure and final delivery release. Prove exactly-once observation before receipt/permit release in every success/error/cancellation branch, including the migrated HTTP path. Preserve its Success-after-response-construction and Error-on-refusal behavior. A bounded heap-owning failure keeps funding until rendered/dropped; fixed prepayment refusal allocates nothing. Assert the ledger is nonzero while data survives and returns exactly to baseline after drop.
- [ ] Add a nonempty retained-result-at-capacity control in the existing concurrency/retention owners: fill the accepted population, drop execution futures and external ledger handles, and prove a further admission refuses while completions survive. Destroy one completion and prove exactly one slot returns. Independently cover heap-owning failure and consuming projection; measure retained data before testing release, so empty results cannot make the assertion vacuous. Preserve the additive equation and existing population limit.
- [ ] Execute the relocated audit filter and process-isolated exact child case, checking their actual nonzero/one-case result lines as required by the selector migration above. Re-run the handler source-order probe against its new relative path; do not replace it with a comment or silently remove its owning CI command.
- [ ] Re-run config snapshot change/recovery and unrelated-overlay retention with large versus small independent policy populations. No process-global operator cache, cross-host alias/secret leak or permanent full-policy `Arc` is allowed.
- [ ] Verify the HTTP mapper preserves status/body/headers and no-bid versus fatal errors. Document that actual TS start/collect scheduling and terminal attachment remain with #1505/#76, not a pass from these core fixture tests. Commit the green shared cutover.

```sh
cargo test --locked -p auctioneer-core --test handler_resolution --test resolve_runtime_config --test privacy_config_parity
cargo test --locked -p auctioneer-core --test auction_config_lifecycle --test auction_config_retention --test auction_budget_concurrency --test response_retention
cargo test --locked -p auctioneer-core --test admission_holds_through_auction --test handler_wave --test clock_domain --test write_budget_anchors
```

## Task 7: Close Resource, Feature And Source-Ownership Controls

**Files:** Existing runtime budget/receipt units, bidder response/conformance owners, exchange `tests/{response_charge,response_factors}.rs`, core retention/concurrency owners and their normal inline modules. Do not change frozen numeric thresholds or add workflow-text snapshots.

- [ ] Audit the exact owner/layout delta: owned receipt handles/control blocks and acquired capacity permit; programmatic copied request; selected settings; retained winners; source seats; typed/debug material; transient consuming wire projection; failure/drop paths. Charge each allocation once against an existing prepaid owner and keep its receipt and population permit for its real lifetime. Verify the reviewed terminal-storage transfer without releasing either owner before its last funded data.
- [ ] Add independently varied size/population cases and under/at/over resource boundaries. Check peak, retained and cumulative measurements in their existing isolated binaries; do not equate a passing peak with absence of a copy or derive the expected bound from the implementation constant under test.
- [ ] Run source/ownership and feature lanes below. Preserve separate Cargo graphs and real test counts. Core conformance requires `conformance-suite`; native success does not establish guest/host fit. Any needed new reserve/limit/equation returns to review before adoption.
- [ ] Close the concrete SDK service handoff with host #1510: use the common frozen revision from #1507/#1508 and accepted #1509 contract, review source/API differences, then execute real SDK-object binding controls in `handler_wave`, `clock_domain` and credential tests, plus #1510's reviewed host-owned commands. Verify the existing wave scheduler/client, shared gates, exact deadlines and bounded secret reads. Keep the implementation PR/issue incomplete if that leaf evidence is blocked; do not substitute fake fanout tests or add a compatibility transport.
- [ ] Verify B1-B10 mapping is complete, all new tests have a current owning CI lane and no required case is silently skipped/ignored. Keep immutable captures and their pins unchanged.
- [ ] Run the complete unchanged CI matrix at the final implementation head and record actual commands, exit codes and result lines. Obtain independent review and explicit merge authorization; do not infer them from a positive comment or a previous PR's exception.

```sh
cargo test --locked -p auctioneer-bidder --test test_isolation
cargo test --locked -p auctioneer-core --test dependency_purity
cargo test --locked -p auctioneer-core --features conformance-suite --test fanout_conformance --test bidder_handler_conformance --test native_bidder_onboarding
cargo test --locked -p auctioneer-core --features conformance-suite,insecure-bidders --test native_bidder_onboarding
cargo clippy --locked -p auctioneer-runtime -p auctioneer-bidder -p auctioneer-privacy -p auctioneer-exchange -p auctioneer-core --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Full Verification And Planning Handoff

Prepare sources, SDK root, locked dependencies and pinned runner tools using [development/test ownership](../../testing/development.md). One heavy Cargo job at a time, existing shared outputs and `CARGO_BUILD_JOBS=2` locally; no routine target clean or disabled incremental builds. The workflows remain authoritative for full acceptance:

- `.github/workflows/test.yml`: secure/default/insecure/CLI, once-owned compiler/config/evidence producers, source probes, actual adapter artifacts, conformance and cross-target checks.
- `.github/workflows/format.yml`: formatting and both applicable lint modes.
- `.github/workflows/ci-fastly.yml`, `ci-cloudflare.yml`, `ci-spin.yml`: their existing package/size/runtime regression owners, not proof of the future linked host.

Do not replace these with a bare workspace test, silently remove once-owned producers, claim a missing provider prerequisite passed or rerun an old artifact under a new source identity. Apart from the explicit audit selector/path migration, this plan changes no workflow scheduling, lanes or gates. Targeted commands above supplement the complete final-head suite. Exact code/API additions remain subject to planning approval; this planning PR changes documents only.

- [x] Independently review the whole spec, fix material findings and preserve the maintainer's joint-review gate. Final document-review verdict: Approved, iteration 3; this is not maintainer or formal GitHub approval.
- [x] Independently review this whole plan against the spec and actual source/test owners; fix contradictions or unexecutable task order before handoff. Final document-review verdict: Approved, iteration 2; retained-capacity ownership and audit-selector migration findings are addressed.
- [x] Validate Markdown, source/file links, B1-B10 mapping and existing Cargo targets/features. Executed: formatter checks; 17 immutable source-file and five relative links; 28 planned Cargo commands and 50 existing named test selections/features. These checks do not compile proposed APIs or execute future controls.
- [ ] Obtain publication authorization, commit only the two documents, and open one non-draft PR against the actual default branch with `PRfection phase: plan`, `Refs #51`, both links, base identity and honest validation state. Do not use an issue-closing reference.
- [ ] Execute and inspect the full current-head acceptance matrix. Keep missing/failed checks nonpassing. Read back open/non-draft/head/base state before requesting REVIEWER; do not claim readiness from partial checks.
- [ ] Stop for joint review, independent approval and authorized planning merge. No production implementation starts in this PR, and #51 remains open at the planning merge.
