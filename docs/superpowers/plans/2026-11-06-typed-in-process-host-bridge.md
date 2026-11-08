# Typed In-Process Host Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this approved plan task-by-task. Steps use checkbox syntax. This document is not implementation authorization.

**Goal:** Deliver #51's shared HTTP/embedded preparation, single typed engine outcome and funded completion ownership without adding another configuration or auction path.

**Architecture:** Both caller kinds enter one explicit-context prepared operation with accepted Auctioneer settings first and auction input second. That operation owns request-scoped resolved state, builds borrowed engine views only inside its consuming future, and moves existing receipts with typed results. The existing reducer supplies selection facts; deterministic wire output is a consuming projection, not another execution API.

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

Move the existing portable disclosure scope from core `src/handlers/request_disclosure_audit.rs` to `src/auction/audit.rs`, preserving its inline tests and bounded logger implementation. Update ordinary module declarations and actual handler/bootstrap/adapter/test references; do not compile the old source through an include or leave a second guard implementation. Migrate `.github/workflows/test.yml`'s exact `handlers::request_disclosure_audit::tests` filter to `auction::audit::tests`, the subprocess test's hardcoded `--exact` selector to its new qualified name, and the relocated `include_str!("auction.rs")` source probe to its real handler path. Verify the new filter lists and executes the same nonzero audit cases and the subprocess executes exactly one child case; an exit-zero empty run is not acceptance. This is a selector/path migration only, with no lane removal, assertion weakening or workflow-snapshot gate. The funded state owns this scope, not a borrowed external observer. Explicit SDK binding also moves the existing `src/handlers/bidder_credentials.rs` loader into `host_primitives/bidder_credentials.rs` and exposes its bound-store/clock constructor beside `host_primitives/edgezero_proxy.rs`; no alternative transport or credential reader is added.

Extend existing targets named in each task. Wire-focused callers consume the new explicit projection; typed-result controls assert engine facts directly. Ordinary shared fixtures use `fake_` identities and capable existing fake bidders; real protocol captures remain bidder-owned. No production bidder is a shared stand-in.

## Task 1: Freeze Observable Baseline And Caller Inventory

**Files:** Existing core `tests/{handler_resolution,resolve_runtime_config,auction_config_lifecycle,auction_config_retention,privacy_config_parity,response_account_resolution}.rs`; exchange `tests/{determinism,response_projection,bid_impression_correlation,bidder_call_correlation}.rs`, `tests/targeting_prefix/`; corresponding current implementation modules.

- [x] Inventory every `run_auction` caller and `TypedBid` constructor using `rg -n 'run_auction\(|TypedBid\s*\{' crates`. Record actual feature gates and test counts from Cargo selection, not a guessed count in this plan.
- [x] Extend current characterization tests for wire/account error precedence, default versus named account selection, equal-price/deal selection, disabled winner targeting, multiple bids per bidder/imp, browser-only results and bidder errors with no bids. Keep expected values independent of the future shared helper.
- [ ] In the existing targets, add the future equivalence and retained-selection assertions as the API lands in subsequent tasks; first prove the baseline oracles reject deliberately wrong local expected values without changing production source.
- [x] Run the baseline commands below. Expected: actual nonzero tests and green current behavior. Capture failures before editing; missing prerequisites are blocked verification, not baseline bugs or permission to weaken controls.
- [x] Commit only self-contained green characterization changes. Do not claim the still-missing shared API is tested by those controls.

```sh
cargo test --locked -p auctioneer-core --test handler_resolution --test resolve_runtime_config --test response_account_resolution
cargo test --locked -p auctioneer-exchange --test determinism --test targeting_prefix --test response_projection --test bid_impression_correlation --test bidder_call_correlation
```

Executed checkpoint `d4adbd2`: both command selections above passed with `--offline`, `CARGO_BUILD_JOBS=2` and the existing shared target directory. The baseline dispatched core counts `8/4/3` and exchange counts `8/2/2/2/23`; the final characterization dispatch was core `8/4/4` and exchange `8/2/2/2/26`, all with zero failures and zero ignored tests. Temporarily substituting a wrong account targeting key and wrong deal winner each produced `0 passed; 1 failed` in its named test; both substitutions were restored and the complete selections rerun successfully. Clippy for all three changed test targets passed under `-D warnings`. Independent internal specification and quality reviews passed after adding literal price-key coverage. These are baseline controls, not shared-API or full-matrix acceptance.

## Task 2: Add Owned Receipt Backing To The Existing Ledger

**Files:** Runtime `src/auction_budget.rs`; inline unit tests. Extend existing core `tests/auction_budget_concurrency.rs` only for cross-component ownership controls after the bridge exists.

- [x] Add failing unit cases named `owned_reservations_share_the_original_ledger`, `owned_prepayment_rolls_back_each_failed_phase` and `owned_receipts_survive_the_callers_budget_handle`. Independently vary each phase charge and ledger ceiling; verify the positive live allocation/count before checking release.
- [x] Run `cargo test --locked -p auctioneer-runtime --lib auction_budget:: -- --list`, confirm each new name, then execute the same filter. Expected red: missing owned operations or failed ownership assertions, never zero matched tests.
- [x] Add the private borrowed/shared budget owner and owned prepayment to the existing `Reservation`/`AuctionCharges`. Share the atomic reserve/check/drop implementation. Do not derive `Clone` for receipts, construct a second budget from the same limit, widen capacity or add an unsafe lifetime conversion.
- [x] Test move/drop, partial failure, mixed borrowed/owned receipts on the same ledger, under/at/over cap, capacity overflow and cancellation/unwind. Drop an external `Arc` and prove the live receipt still owns its exact charge. Charge layout/control-block ownership in the later resource checkpoint; do not call it zero-cost.
- [x] Run runtime units and lint, retain actual output and commit the green ownership primitive without changing numeric costs. If its required storage cannot fit current funded owners, stop activation and return to resource review.

```sh
cargo test --locked -p auctioneer-runtime --lib auction_budget::
cargo test --locked -p auctioneer-runtime --lib
cargo clippy --locked -p auctioneer-runtime --lib --tests -- -D warnings
```

Executed checkpoint `9271dfb`: the initial test build failed with 16 missing-owned-method diagnostics. After implementation, the listed module filter executed `18 passed; 0 failed`, the complete runtime library executed `272 passed; 0 failed; 0 ignored`, and Clippy passed with `--lib --tests -- -D warnings`; verification used the same offline/shared-target setup. Independent internal specification and quality reviews passed. On the native ABI, the receipt grew from 24 to 32 bytes at alignment 8, adding 48 bytes across six receipts. This checkpoint introduces no numeric cost change or serving cutover: funding the receipt/control-block layouts and guest qualification remain open in Task 7.

## Task 3: Retain Upstream Seat Provenance Once

**Files:** Bidder `src/types.rs`, `src/bidders/media_type.rs`, actual direct decoder construction sites; inline units and existing owner-local conformance; exchange `tests/{response_projection,response_factors}.rs` as affected.

- [x] Add red shared-classifier cases for absent/empty seat, two different seats, multiple bids sharing one seat, valid siblings around a bad bid and repeated response calls. Assert requested bidder and source seat differ in the before-state.
- [x] Extend `TypedBid` with optional shared immutable source-seat provenance. Establish one shared string owner per decoded nonempty source seat before flattening; do not clone the string for each bid or synthesize the alias/provider ID. Retain canonical missing/empty behavior.
- [x] Update every actual constructor discovered by Task 1. Use the shared helper for common response handling; source-specific decoders capture only information their protocol actually contains. Do not add a names-based registry or a hand-maintained sibling exception list.
- [x] Add resource controls varying seat byte length and bid population independently; compare one versus many shared references and prove the value is not copied per bid. Include the shared owner/control block and `TypedBid` layout in current decoder/response accounting. Wire projection must continue to use the requested bidder/alias seat.
- [x] Run current bidder/response owners and compile all callers, preserving assertions and feature gates. Record the discovered migration set and commit only when no constructor silently loses observable provenance.

```sh
cargo test --locked -p auctioneer-bidder --lib
cargo test --locked -p auctioneer-bidder --test fake_bidder_contract
cargo test --locked -p auctioneer-exchange --test response_projection --test response_factors --test response_charge
cargo check --locked --workspace --all-targets
```

Executed native checkpoint: the red shared-classifier build produced 12 missing-field diagnostics. All ten discovered construction literals were migrated: the shared classifier, four direct protocol decoders, the fake bidder, the winner fixture, two response-factor fixtures and the call-correlation fixture. The final bidder selection executed `1205 passed; 0 failed; 9 ignored` in the library, followed by allocation `67`, response conformance `32`, fake contract `18` and source isolation `16`, each with zero failures and zero ignored cases. The four exchange response/correlation owners executed `2/16/19/3`, all passing. Native workspace all-target compilation passed before the final test-only repairs; final bidder/exchange Clippy checked both libraries and ordinary test targets with `--lib --tests -- -D warnings`. Verification used the same offline/shared-target setup. The pre-existing nine ignored source/environment cases are not claimed as executed acceptance.

The allocation sweep independently varied seat byte lengths `0/8/1024/32768` and bid populations `1/32/256`. It measured the current 432-byte native `TypedBid` and actual shared-owner allocations `0/24/1040/32784`, including control blocks, and checked retained, peak and cumulative work against unchanged funding with the existing margin. Independent size deltas and pointer ownership reject per-bid text copies. The previous bare-bid no-copy heuristic initially failed after wrapper growth; its structural margin is now preserved after accounting for the compiler-checked additional metadata and one measured owner. Exact cumulative creative-size controls separately distinguish one parse allocation from an actual deep-clone positive control. No funded floor, ceiling, margin or capture was changed. The existing marketplace capture also now distinguishes upstream seats from its marketplace payload. Internal specification and quality reviews passed. This checkpoint does not implement the engine outcome, resource-retaining bridge, guest qualification or concrete host handoff in later tasks.

## Task 4: Make The Existing Engine Return Its Typed Outcome

**Files:** Exchange `src/{lib,response,outcome}.rs`, `src/run/mod.rs`; every existing caller from Task 1, including the current core handler. Existing `run_auction`, `determinism`, `targeting_prefix`, correlation and response allocation owners.

- [x] Add red outcome tests for retained selection when winner keys are disabled, nondefault/truncated targeting prefixes, equal prices, deal preference, repeated impression matches and multiple bids in one bidder vector. Assert candidate presence and the exact selected bid index, not only a price.
- [x] Make `apply_targeting` return/move its existing impression-aligned winners instead of dropping them. Construct private `AuctionOutcome` from the existing owned typed response material, retained request/impression identity, selection and finalized permitted debug; add read-only typed candidate/diagnostic/browser views. Admit the bounded retained identity owner before allocating it. No second winner reducer or reconstructed targeting map.
- [x] Change the existing `run_auction` return type and all early exits to that outcome. Add its consuming `into_bid_response` projection around existing `build_response`; preserve ID, timestamps, error/nonbid behavior, debug deadline/permissions, media and full bid objects.
- [x] Migrate every current wire-oriented caller explicitly to the projection in this same green checkpoint. Do not add `run_auction_with_*` compatibility alternatives or clone all bids to satisfy borrowing. Ensure the debug capture is finalized once while its clock/context is still valid.
- [x] Compare typed selections and the unchanged wire projection against independent expected results. Prove browser-only success, bidder-local errors and timeout diagnostics survive no-winner exits. Run allocation/retention controls, commit the green hard cut and preserve low-level caller funding responsibilities.

```sh
cargo test --locked -p auctioneer-exchange --lib targeting::winner::
cargo test --locked -p auctioneer-exchange --test run_auction --test determinism --test targeting_prefix --test bid_impression_correlation --test bidder_call_correlation
cargo test --locked -p auctioneer-exchange --test response_charge --test response_projection --test response_factors --test code12_attribution
cargo test --locked -p auctioneer-core --test handler_resolution
cargo check --locked --workspace --all-targets
```

Executed native outcome checkpoint: the missing public outcome produced the expected red build. The final exchange command selection (library plus the ten outcome/response owners, including expired diagnostics) executed `390 passed; 0 failed; 0 ignored`. The four migrated core caller owners executed `21 passed; 0 failed`, and source-import isolation executed `16 passed; 0 failed`. Workspace all-target compilation and strict core/exchange library/test Clippy passed; the final receipt-test-only lint also passed. Exact synthetic receipts now include the measured 64-byte native impression/selection slots and their retained names, while the original allocation-margin oracles, production limits and additive funding remain unchanged. Their one-byte-under refusal and parser-invocation assertions are preserved. Static internal specification and quality reviews found no remaining contract gap. These controls do not complete the shared preparation/funding API or the final feature/guest/host matrix.

## Task 5: Factor Explicit Preparation, Restrictions And Timing

**Files:** Proposed core `auction/{mod,services,prepare}.rs`; current config helpers/settings; exchange context/tmax/run and privacy decision/classified-projection owners. Tests stay inline or in existing `handler_resolution`, `privacy_outbound`, `clock_domain`, `construction_deadline` and provenance targets.

- [x] Freeze the reviewed public records from spec sections 4-6 in code, including the complete same-source resource tuple, one owned funding owner carrying the already-acquired capacity permit, borrowed JSON/typed input, five host ceilings and original start/outer deadline. Private constructors prevent arbitrary `Derived` data from standing in for the canonical derivation. Move the existing permit; do not install or acquire another pool/slot for the bridge.
- [x] Expose exactly `prepare_auction(settings: &AuctioneerAppConfig, input: AuctionInput<'_>, host: AuctionServices<'host>, funding: AuctionFunding)` with the spec's lifetime/return contract. Both standalone and embedding callers pass settings first and auction input second. The host binding borrows only its accepted Auctioneer subtree; do not accept full TS settings, add a host-settings dependency, duplicate the policy in request parameters, fetch config again or add an alternative embedding entry point. Include the settings-first call in public API examples and compile those examples with the implementation.
- [x] Add red tests for missing/insufficient resources, mismatched bindings, spent outer cap before any parse/credential/dispatch, and independently varied host ID/geo/FPD/TID/fetch ceilings. Keep other ceilings permissive in each single-denial control so no sibling masks it.
- [x] Implement input admission before ownership transfer/copy. Preserve selective raw discovery and standalone precedence; share the ordered account-selection rule and `resolve_selected_account` with programmatic input, with no whole JSON tree or serialize/reparse. Disabled/default/named account behavior must be explicit.
- [x] Resolve host/account/aliases/scoped budgets/features/timeouts using existing explicit-context helpers. Project only request-required bidder settings/floor/account state from the accepted policy; do not retain a full config clone or read ambient boot/primitive state. Carry honest original extension provenance.
- [x] Intersect host ceilings with existing per-bidder decisions immediately before fetch filtering/classified scrubbing, without rewriting consent. Extend the existing tmax resolution with the absolute outer cap; preserve bidder-visible tmax and the original monotonic domain.
- [x] Run the exact owners below and prove the host/engine-denial cross-product and independent cap boundaries. Verify negative controls actually reach their named branches, then commit without activating a second preparation path.

```sh
cargo test --locked -p auctioneer-core --lib
cargo test --locked -p auctioneer-core --test handler_resolution --test response_account_resolution --test node_budget_on_the_auction_path --test original_request_extension --test privacy_wire_signal_400
cargo test --locked -p auctioneer-core --test privacy_outbound --test clock_domain
cargo test --locked -p auctioneer-exchange --test construction_deadline --test privacy_policy_composition --test disclosure_policy_diff --test request_provenance
cargo test --locked -p auctioneer-privacy --lib
```

### Preparation Checkpoint

The settings-first API, explicit resource/services records, request-owned projection and funded result owners are implemented locally. Raw and programmatic account discovery share the same six-path table. Host restrictions intersect the existing decision, and the absolute outer deadline does not rewrite bidder-visible tmax. Registry construction now records the selected implementation independently; preparation rejects a missing, reordered, duplicate or substituted service binding before shared input parsing or projection. The generated-registry compiler fixture retains that identity too and keeps its existing feature/factory controls.

Internal specification and code-quality reviews found and corrected the binding substitution, stale compiler scaffold, and retained diagnostic-capacity cases. Main-executed negative controls failed on the intended assertions before correction. The post-correction library run recorded:

```text
test result: ok. 1206 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 1.56s
test result: ok. 660 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
test result: ok. 291 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.87s
```

These are bidder, core and privacy library results, respectively, not a complete implementation acceptance run. The final eight core handler owners separately passed `19/3/8/5/2/6/8/4` cases, and exchange's library plus construction-deadline, disclosure-policy, privacy-composition and provenance owners passed `297/14/17/54/6`. The generated-registry compiler controls passed three cases. Core's documentation examples passed one compile case and two compile-fail cases. Strict Clippy passed for core, exchange, privacy, bidder and config with `--features auctioneer-config/build-support --lib --tests -- -D warnings`. The existing nine bidder ignores remain subject to their owning CI lanes. The admitted-handler fixture extends the existing capture owner and retains the original admission grant and paired terminal envelope; the extraction-only helper remains unadmitted.

At preparation checkpoint `1db7de2`, HTTP still used its pre-cutover assembly, and the audit module had not moved. That checkpoint's operation/result ownership scaffolding did not close Task 6's handler equivalence, cancellation and terminal-transfer requirements. Receipt/control-block, registry-entry and funded-failure storage still need Task 7's layout/accounting audit, guest checks and concrete SDK handoff; the preparation checkpoint does not qualify those costs or change any workload ceiling, numeric reserve or budget equation.

## Task 6: Build The Owned Operation And Cut HTTP Over

**Files:** Proposed core `auction/{operation,result}.rs`; `src/handlers/auction.rs` and existing health/error/audit consumers. Extend existing lifecycle, retention, handler and response targets, not a new HTTP/App fixture program.

- [x] Add red shared/HTTP equivalence cases in `handler_resolution`, using the same fake responses and deterministic independent clock/RNG instances. Exercise the settings-first call with typed Auctioneer subtrees; compare full candidate sets, selected identity, targeting and errors. Hold auction input fixed while varying the passed subtree, then hold that subtree fixed while varying input; use two isolated host policies on the same executable so hidden or ignored settings cannot pass.
- [x] Implement consuming `PreparedAuction::run` without an external audit argument. Construct borrowing `ResolvedAccount`/`AuctionContext` locals inside the future; move the owned audit scope, receipts and acquired capacity permit with successful completion and destroy request/policy state first. Keep read-only result views and a guarded consuming wire projection; no raw data/receipt/permit escape.
- [x] Keep transport acquisition, SDK config resolution, HTTP encoding/body checks, error rendering, no-store, GVL observation and egress formatting/deadline binding in their existing wrapper. Replace duplicated shared assembly with the new operation. Standalone prepayment remains before config extraction. Move its existing auction permit into the funding owner; preserve separate terminal/egress ownership. Release auction capacity only after its data is destroyed or transferred into that existing bounded terminal owner, never merely because the future returned.
- [x] Exercise drop before poll, suspension in credential/fanout work, successful completion, retained typed result, wire-projection failure and final delivery release. Prove exactly-once observation before receipt/permit release in every success/error/cancellation branch, including the migrated HTTP path. Preserve its Success-after-response-construction and Error-on-refusal behavior. A bounded heap-owning failure keeps funding until rendered/dropped; fixed prepayment refusal allocates nothing. Assert the ledger is nonzero while data survives and returns exactly to baseline after drop.
- [x] Add a nonempty retained-result-at-capacity control in the existing concurrency/retention owners: fill the accepted population, drop execution futures and external ledger handles, and prove a further admission refuses while completions survive. Destroy one completion and prove exactly one slot returns. Independently cover heap-owning failure and consuming projection; measure retained data before testing release, so empty results cannot make the assertion vacuous. Preserve the additive equation and existing population limit.
- [x] Execute the relocated audit filter and process-isolated exact child case, checking their actual nonzero/one-case result lines as required by the selector migration above. Re-run the handler source-order probe against its new relative path; do not replace it with a comment or silently remove its owning CI command.
- [x] Re-run config snapshot change/recovery and unrelated-overlay retention with large versus small independent policy populations. No process-global operator cache, cross-host alias/secret leak or permanent full-policy `Arc` is allowed.
- [x] Verify the HTTP mapper preserves status/body/headers and no-bid versus fatal errors. Document that actual TS start/collect scheduling and terminal attachment remain with #1505/#76, not a pass from these core fixture tests. Commit the green shared cutover.

```sh
cargo test --locked -p auctioneer-core --test handler_resolution --test resolve_runtime_config --test privacy_config_parity
cargo test --locked -p auctioneer-core --test auction_config_lifecycle --test auction_config_retention --test auction_budget_concurrency --test response_retention
cargo test --locked -p auctioneer-core --test admission_holds_through_auction --test handler_wave --test clock_domain --test write_budget_anchors
```

### HTTP Cutover Checkpoint

The HTTP wrapper now enters the same settings-first preparation and consuming execution path. It moves its original admission permit into owned funding, prepays before configuration and HTTP debug decoding, drops the full accepted policy and raw input before fanout, and retains funding through projection, rendering and terminal-owner transfer. The conformance seam no longer accepts missing or borrowed phase receipts. Its fake registries record their selected implementations explicitly.

The disclosure scope moved to `auction/audit.rs`; ordinary module imports, all four adapter references, current-source identity paths, the CI filter and the exact subprocess selector moved with it. The existing 18 audit test names and logging assertions are preserved. Normalization limits have one implementation in the shared resource owner, with both prior boundary controls colocated there. Runtime bidder settings now have only request-scoped projection; the obsolete whole-policy move path is removed. Its synthetic owner tests now distinguish the one required copy from allocation-free alias/base lookup, vary unrelated populations independently, and retain exact dispatch reconciliation.

Internal specification review approved the ownership/cutover slice. Quality review found prepayment ordering of encoded debug queries; the added positive-control allocation test reproduced `951` versus `927` requested bytes on the unfunded path before correction and passed after decoding moved behind prepayment. Further static review approved the corrected ownership and destruction order, subject to executed verification. No reviewer approved whole-issue readiness.

Before the requested SDK update, the final native core-owner command against `775366fdac923135e8430b1e3dffebedc648ef7c` executed the library and 16 named integration targets with `--locked --offline --no-fail-fast`: **744 passed, zero failed, one existing environment-dependent ignored case**. The library result was:

```text
test result: ok. 664 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
```

That run includes actual suspension in credential and fanout work, unadmitted/funding refusal controls, exact typed/HTTP wire equality across two accepted policies and two independently varied requests, and nonempty completion/wire/failure retention at the existing two-operation fixture capacity. It also reruns configuration recovery, unrelated-policy retention, privacy, account precedence and write anchors. Strict Clippy passed for core, exchange, privacy, bidder and config, including the conformance feature, with `--lib --tests -- -D warnings`; no lint allowance was added. These are checkpoint results, not the complete current-head CI matrix.

Prepared state and the HTTP execution frame are boxed only after prepayment to avoid oversized async frames. Their exact heap/layout charges, owned ledger/control-block and registry-entry costs, remaining synthetic compiler execution, feature/guest checks and the real host-SDK handoff remain Task 7 work. The resource review identified that the new fixed owners are not yet explicitly included in the construction charge, including prepared/failure coexistence; prepayment and passing allocation tests do not close that accounting gap. No numeric reserve, workload limit, resource ceiling or budget equation changed. Trusted Server #1510 remained open when checked on 2026-11-06; this checkpoint is not a claim of linked-host acceptance or target fit.

### Requested SDK Update

The maintainer requested the latest EdgeZero PR #275 in this implementation. GitHub readback on 2026-11-06 resolved the immutable head `598f0a2a490eb83dd31c06c024a0cefa8e049628`, 60 commits ahead of the preceding pin. The lockfile changed only the eight EdgeZero Git packages; decoder/parser package versions and `.tool-versions` remain unchanged. Active CLI/provider resource identities, the CI checkout locator and the independent pin test move with the manifest. Historical measured evidence keeps its original bytes and source identity.

Source review found unchanged core configuration-store, monotonic-time, terminal-egress and public outbound contract files and unchanged CLI source. The SDK changes routed original-body length preflight and provider scheduling/cancellation implementations, with additional conformance controls. Axum's allocation documentation now explicitly distinguishes its Rust 1.95 measurements from the current Rust 1.98.1 toolchain. Compilation or prior-pin results do not renew those measurements or establish hosted qualification. The new pin requires current-head workspace, native/guest and real SDK-object regression verification; all earlier checkpoint counts above remain attributed to the preceding revision.

## Task 7: Close Resource, Feature And Source-Ownership Controls

**Files:** Existing runtime budget/receipt units, bidder response/conformance owners, exchange `tests/{response_charge,response_factors}.rs`, core retention/concurrency owners and their normal inline modules. Do not change frozen numeric thresholds or add workflow-text snapshots.

- [x] Audit the exact owner/layout delta: owned receipt handles/control blocks and acquired capacity permit; programmatic copied request; selected settings; retained winners; source seats; typed/debug material; transient consuming wire projection; failure/drop paths. Charge each allocation once against an existing prepaid owner and keep its receipt and population permit for its real lifetime. Verify the reviewed terminal-storage transfer without releasing either owner before its last funded data.
- [x] Add independently varied size/population cases and under/at/over resource boundaries. Check peak, retained and cumulative measurements in their existing isolated binaries; do not equate a passing peak with absence of a copy or derive the expected bound from the implementation constant under test.
- [ ] Run source/ownership and feature lanes below. Preserve separate Cargo graphs and real test counts. Core conformance requires `conformance-suite`; native success does not establish guest/host fit. Any needed new reserve/limit/equation returns to review before adoption.
- [ ] Close the concrete SDK service handoff with host #1510: use the common frozen revision from #1507/#1508 and accepted #1509 contract, review source/API differences, then execute real SDK-object binding controls in `handler_wave`, `clock_domain` and credential tests, plus #1510's reviewed host-owned commands. Verify the existing wave scheduler/client, shared gates, exact deadlines and bounded secret reads. The host-owned tests must invoke the exposed API with only the borrowed Auctioneer settings subtree first, auction input second, and verify independent unrelated-TS-settings and Auctioneer-policy changes. Keep the implementation PR/issue incomplete if that leaf evidence is blocked; do not substitute fake fanout tests or add a compatibility transport.
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

### Ownership Accounting Checkpoint

The existing fixed construction term now includes the compiled prepared-state payload, the funded failure payload and bounded diagnostic, the compiler-derived boxed HTTP execution future, and one shared ledger allocation. Receipt handles are included in their enclosing owners, not charged as six separate ledger allocations. The failure and prepared-state boxes are conservatively simultaneous. On the measured aarch64-macOS test build, these additions are 19,144 bytes; that snapshot is target-scoped, while production costs derive from the actual compiled layouts. The ledger layout follows the pinned Rust 1.98.1 implementation and an independent allocation control measures one 32-byte owner with allocation-free receipt clones. No arbitrary reserve, workload limit, resource ceiling, or additive equation changed.

Funding rejects each underdeclared fixed/scratch/per-scope construction cost before creating receipts or the audit scope. The independent minus-one/exact/plus-one controls reproduced a grant before the check and passed after it. Separate internal specification and quality reviews accepted this ownership slice. The complete core target checkpoint executed **94 top-level targets, 1,182 passed, zero failed, one existing environment-dependent ignore**, including 666 library cases. Eleven legacy test targets initially bypassed actual admission; their fixtures now retain the original ingress and paired terminal owners, without weakening their response, disclosure, timing, or accounting assertions. The no-boot control separately retains the extractor's exact 500 refusal and asserts the HTTP admission refusal. Strict Clippy passed across all five affected packages and all targets with the conformance feature and `-D warnings`, without new exemptions.

The sole cold native catalog control now retains the actual configured registry through schema initialization and the entire evaluation sweep. It measured 17 registry entries retaining 2,105 bytes, combined catalog residency of 50,049 bytes, temporary initialization of 27,073 bytes, a 77,122-byte simultaneous peak, and 273,819 cumulative requested bytes. These fit the unchanged 16-MiB resident and 8-MiB initialization tested reservations. Metadata-pointer and implementation-binding checks execute outside the measurement window. This is native allocation evidence, not a worst-case schema proof, guest residency qualification, or combined-host fit claim.

The once-owned generated-settings compiler lane subsequently passed 24 outer cases, its exact 32-case copied-source contract, catalog-size sweeps at 1/256/512 definitions, and its explicit strict Clippy target. Core's current conformance command passed 8 bidder-handler, 35 fanout, and 8 native-onboarding cases. These executed checks do not complete the final feature/guest/provider matrix or the concrete Trusted Server #1510 handoff.

### Draft Integration Candidate

On 2026-11-07, the maintainer authorized publication of a draft implementation candidate so Trusted Server can build its host-owned handoff against the actual API. This explicitly permits an incomplete integration handoff, not a ready-for-review or merge claim. The draft targets the default branch, references #51 without closing it, and records its tested source identity and actual check results. It must retain the outstanding acceptance gates rather than turn compilation, partial suites, or fake services into linked-host evidence.

Trusted Server #1510 remained open at publication preparation. Its host-owned commands and compatible final SDK/service identity are still required, alongside the remaining full feature/guest/provider acceptance matrix and independent review. Publication neither completes #51 nor authorizes merging, removing draft status, changing resource limits, or waiving a failing check.

The default-workspace CI test command was subsequently executed with its existing two once-owned producer exclusions and `--no-fail-fast`. It exited 101: 6,510 passed, six failed, and 20 existing ignores across 245 libtest summaries; a separate benchmark target also executed. Five failures were independent native accounting snapshots that still omitted the newly charged 19,144-byte fixed owner delta. Their corrections add that literal once per auction and reduce the expected remaining margin accordingly, without changing a production cap or reading the production cost constant as the expectation. Focused rerun results belong to the implementation PR.

At draft publication, one genuine acceptance regression remained: `sparse_request_copies::tests::denied_alias_status_rows_are_bounded_before_impression_ids_are_copied` previously accepted 61 denied aliases with a 4-KiB impression ID, but retaining the typed outcome's identities exhausted its materialization bookkeeping budget. Its existing positive assertion was deliberately unchanged. This blocked standalone acceptance independently of the external host handoff; the repair below must restore that accepted case without raising a cap or blessing rejection.

### Review-Readiness Scope And Bookkeeping Repair

On 2026-11-07, the maintainer authorized marking the implementation ready for code review once Auctioneer's implementation, local verification and CI are green, while retaining Trusted Server #1510 as an explicit merge/acceptance gate. This supersedes the earlier draft-only publication restriction, not the host-owned tests, independent review, issue-completion or merge requirements. Do not close #51 or claim linked-host acceptance from Auctioneer's tests.

The bookkeeping repair preserves the existing funded ceiling `F = 4 * (B + 384 * N)` and the independent wire-metadata byte/node quotas `B/N`. Wire metadata still reserves `M = 4 * (used_bytes + 384 * used_nodes)`. Typed outcome identity storage instead reserves one allocation-derived upper bound `I`: each impression's ID payload and `String`/`Option<Winner>` slots, plus each participating bidder name times its covered impressions. The winner vector moves into the outcome; it is not duplicated. Charging all participating names conservatively bounds the surviving selections rather than claiming an exact retained measurement.

Both reservation paths check `M + I <= F` before changing any counters. Constructor arithmetic and prospective additions are checked; overflow and refusal leave accounting unchanged. Typed identities no longer consume the separate wire quota or receive its four-representation multiplier. There is no new reserve, debug borrowing, population, cap, or change to the runtime budget equation. Independent static review accepted this decomposition and verified the original 512-byte wire-boundary inputs `206/242/184` against the pre-implementation source. The two new regression tests first executed with `9 passed; 2 failed` before the correction. Green allocation, original sparse-acceptance and full-suite results must be recorded separately after execution.

The public SDK credential-loader control similarly first failed with exactly one `E0432` unresolved import for `auctioneer_core::host_primitives::CredentialLoader`. Its repair relocates the existing implementation and preserves its inline tests, lazy construction, original clock/deadline and bounded 4-KiB store read. That real SDK-object test is an Auctioneer-side contract control, not the unimplemented Trusted Server host handoff.

The first green-checkpoint attempt passed all 346 exchange cases, including the preserved sparse acceptance and original allocation margins. Core passed its 40 selected integration cases and 665 library cases; its one failure detected the actual compiler-owned HTTP frame growing by 16 bytes for the new counters. Production's existing layout-derived construction charge already includes that growth. Independent native owner and Axum/CLI accounting snapshots now use the observed 19,160-byte addition once per auction, rather than the prior checkpoint's 19,144 bytes. Expected remaining margin decreases by the same population-scaled delta; no production cost constant, reserve, cap, or equation was edited to repair this snapshot. These are intermediate results, not a complete suite pass.

### Repair Verification Before Final-Head Publication

The next complete default-workspace command used the existing locked/offline `--workspace --all-targets --no-fail-fast` lane with its unchanged two once-owned producer exclusions. It exited 101 with **6,517 passed, two failed and 20 existing ignores across 245 libtest summaries**, plus the existing benchmark target. Both failures were exact synthetic receipt fixtures in `run_auction`: the targeting parser-invocation boundary and denial-row exact-minus-one boundary still multiplied typed identities by four and assigned them eight metadata nodes. Their calculations now use the reviewed `M + I` decomposition; parser-count expectations, exact-minus-one refusals, all 128 denial identities, visibility/expiry combinations, caps and ledger-release assertions are unchanged. Independent static review accepted the corrected calculations.

After those corrections and the unchanged strict lint policy, the complete repair checkpoint exited zero:

- Exchange library and `response_charge`, `response_factors`, `run_auction`, `sparse_request_copies`: **361 passed**, zero failed/ignored.
- Core library and six lifecycle/HTTP/SDK-object owners: **706 passed**, zero failed/ignored, including all nine relocated credential units and the external bound-store/clock control.
- Axum library: **24 passed**; CLI `bidders_resources`: **13 passed**, zero failed/ignored.
- Core conformance with `conformance-suite,insecure-bidders`: **8/35/8 passed**, zero failed/ignored; onboarding reconciled 300 actual dispatch cases.
- Core documentation: **one compile example and two compile-fail controls passed**.
- Workspace `cargo clippy --all-targets -- -D warnings`, formatting, raw JSON budgets, all raw-fixture gate rejection controls and `git diff --check`: **exit zero**, with no new lint exemptions. The raw-fixture count remains **203 across 56 files**.

These are current repair-checkpoint results, not a claim that the final-head full matrix has passed. Publish the verified repair while retaining draft status, then execute the complete default-workspace lane at the committed head alongside the full unchanged CI matrix. Do not mark ready for code review until both finish green. The first failed round and its focused repair are not substituted for that complete rerun.

At committed head `9f756459f78ef5247641e4a284fe754d3afde000`, the complete default-workspace command subsequently exited zero: **6,519 passed, zero failed and 20 existing ignores across 245 libtest summaries**, plus the existing benchmark target. All **25 CI checks succeeded** at that same head, including the separately owned evidence producers. Their actual result lines reported one passed, zero failed/ignored for each producer. This closes the preceding bookkeeping failures without weakening their assertions. It is evidence for that head, not for subsequent revisions or the linked host.

### Private Projection Funding Handoff

`build_response` documents its serialization errors as defensive: its error/warning maps have string keys and primitive typed entries, response times are `u32`, and its required `prebid` object is constructed locally before lookup. No admitted material that reaches `CompletedAuction` has been identified that can produce this defensive error. The maintainer nevertheless requested an executable private funding-handoff control rather than waiving the planned error case.

The existing production success/error match now belongs to `CompletedResponse::from_projection`, visible only inside the auction module. `CompletedAuction::into_wire_response` always uses that handoff; there is no public injection API, alternate configuration or extra allocation. The owner-local audit unit supplies a heap-owning projection error to that same match. Its existing observation probe checks the original weak ledger, full charge and occupied admission slot at observation time; the test also checks retained error contents, exactly one observation, continued refusal while the failure lives, ledger destruction and exactly one returned slot after drop. The existing nonempty success-projection and capacity-retention integration controls remain unchanged.

Before the handoff was added, the new unit's Cargo invocation exited 101 with `E0599`: no `from_projection` associated function on `CompletedResponse`. The new unit then passed alone, and the complete core library plus six lifecycle/HTTP/SDK-object targets passed **707 cases**, zero failed/ignored. The unchanged nonempty consuming-projection and retained-failure controls both passed. The conformance feature command passed **8/35/8 cases**, including all 300 onboarding dispatch cases, and workspace strict Clippy exited zero with no new allowances. The compiled-layout control still accepts the same independent 19,160-byte native owner addition; this extraction changes no storage cost.

Independent static review found no issues in the private handoff or corrected new test. Final-head matrix results belong in the implementation PR after execution; the previous head's pass does not qualify a subsequent revision. This private control does not manufacture an admitted request that reaches the defensive error, waive any existing test, close #51 or replace the real host handoff.

## Full Verification And Planning Handoff

Prepare sources, SDK root, locked dependencies and pinned runner tools using [development/test ownership](../../testing/development.md). One heavy Cargo job at a time, existing shared outputs and `CARGO_BUILD_JOBS=2` locally; no routine target clean or disabled incremental builds. The workflows remain authoritative for full acceptance:

- `.github/workflows/test.yml`: secure/default/insecure/CLI, once-owned compiler/config/evidence producers, source probes, actual adapter artifacts, conformance and cross-target checks.
- `.github/workflows/format.yml`: formatting and both applicable lint modes.
- `.github/workflows/ci-fastly.yml`, `ci-cloudflare.yml`, `ci-spin.yml`: their existing package/size/runtime regression owners, not proof of the future linked host.

Do not replace these with a bare workspace test, silently remove once-owned producers, claim a missing provider prerequisite passed or rerun an old artifact under a new source identity. Apart from the explicit audit selector/path migration, this plan changes no workflow scheduling, lanes or gates. Targeted commands above supplement the complete final-head suite. Exact code/API additions remain subject to planning approval; this planning PR changes documents only.

- [x] Independently review the whole spec, fix material findings and preserve the maintainer's joint-review gate. Final document-review verdict: Approved, iteration 3; this is not maintainer or formal GitHub approval.
- [x] Independently review this whole plan against the spec and actual source/test owners; fix contradictions or unexecutable task order before handoff. Final document-review verdict: Approved, iteration 2; retained-capacity ownership and audit-selector migration findings are addressed.
- [x] Validate Markdown, source/file links, B1-B10 mapping and existing Cargo targets/features. Executed: formatter checks; 17 immutable source-file and five relative links; 28 planned Cargo commands and 50 existing named test selections/features. These checks do not compile proposed APIs or execute future controls.
- [x] Obtain publication authorization, commit only the two documents, and open one non-draft PR against the actual default branch with `PRfection phase: plan`, `Refs #51`, both links, base identity and honest validation state. Do not use an issue-closing reference.
- [ ] Execute and inspect the full current-head acceptance matrix. Keep missing/failed checks nonpassing. Read back open/non-draft/head/base state before requesting REVIEWER; do not claim readiness from partial checks.
- [ ] Stop for joint review, independent approval and authorized planning merge. No production implementation starts in this PR, and #51 remains open at the planning merge.
