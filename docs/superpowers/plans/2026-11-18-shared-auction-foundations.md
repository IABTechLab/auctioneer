# Shared Auction Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the approved common currency, multibid, UID and protocol-mode contracts through separate, testable changes owned by the existing child issues of #36.

**Architecture:** Extend the one operator policy and settings-first auction pipeline. Capture policy/mode/rates/identity once, select recipient controls before copying, scrub after UID selection, and validate/convert/rank bids before targeting. Keep concrete host I/O in core, pure shared transformations in exchange and protocol code with its bidder; preserve existing funding, absolute deadlines and one additive accounting model.

**Tech Stack:** Rust, Serde, existing `validator` patterns, bounded JSON/field metadata, existing EdgeZero configuration and secret bindings, existing guarded `f64` arithmetic, and `quote!`/`syn::parse2` for any changed generated Rust. No new crate, dependency, SDK extension or matcher is approved.

---

## Contract, Scope And Execution Rules

- Specification: [Shared Auction Foundations](../specs/2026-11-18-shared-auction-foundations-design.md), user-approved at `b9c6d158cd8d30ed0e667edd2e713cd1dc728e18` on 2026-11-18. The current review revision also incorporates the user's later hard-cut instruction; its changed reader/decoder and corpus-migration requirements need current-head PR acceptance before implementation.
- Planning baseline: `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`. Every implementation unit starts from the then-current default branch after this planning PR merges; this document does not authorize execution.
- [#54](https://github.com/IABTechLab/auctioneer/issues/54) owns currency. [#55](https://github.com/IABTechLab/auctioneer/issues/55) and [#56](https://github.com/IABTechLab/auctioneer/issues/56) share the multibid/mode activation. [#57](https://github.com/IABTechLab/auctioneer/issues/57) owns the trust decision; [#79](https://github.com/IABTechLab/auctioneer/issues/79) owns auction UID projection. [#59](https://github.com/IABTechLab/auctioneer/issues/59) remains a later FPD design, not implementation in this package.
- Use separate implementation worktrees/PRs for independent units, never this planning branch. Do not stack production changes into the spec/plan PR. A unit may contain tested internal staging commits, but its public configuration/reader/serving changes activate atomically.
- Preserve the user's prohibition on local Cargo, tests, Clippy, fmt, CI scripts and mutations. Commands below are instructions for authorized hosted execution, not recorded results. Read-only source inventories and Git diff checks are allowed. Request publication/mutation authorization before remote writes; old approvals of mutation batches for other issues do not apply.
- Reuse existing test binaries and fake bidders. Pure units stay inline. Do not add a real bidder to shared fixtures, merge allocator/process-global test binaries, add handwritten source imports or create a workflow-source snapshot in Rust.
- Do not lower workloads, increase ceilings, change the additive budget, add legacy aliases or invent a reserve. Stop the affected activation at a failed resource/layout gate; independent work may continue without claiming that gate passed.
- Keep commands and observations distinct. A filtered test must actually dispatch the named case; report its full name, count and actual result line. A failure at an earlier parser/configuration gate is not evidence for the later branch named by a test.
- Apply the user's 2026-11-18 hard-cut instruction to changed behavior: no legacy wire reader, fixed-USD decoder fallback or compatibility wrapper remains. The existing global mode still normalizes only its four reviewed semantic differences within the same implementation. Newly invalid external fixtures receive precise checked refusals under the reviewed test-contract migration, not a skip or an extra accepting parser.

## File And Test Ownership

All existing paths below were inspected at the planning baseline. `Create` means a proposed file, not code that already exists. Re-inventory on rebase rather than trusting line numbers or retaining compatibility code to preserve a stale plan.

| Area | Production owner | Test owner |
| --- | --- | --- |
| Rates and application format | Modify `crates/auctioneer-config/src/{currency,policy,lib}.rs`; core `src/config/{validation,mod}.rs` and `src/auction/{prepare,operation}.rs`; CLI `src/application.rs` | Config inline units and `tests/root_shape.rs`; core `tests/config_reference_examples.rs`, `tests/auction_config_lifecycle.rs`; CLI inline `application::config_contract` |
| Shared amount arithmetic | Generalize `crates/auctioneer-bidder/src/currency.rs`; update its public re-exports, `src/request_info.rs` and all actual protocol consumers found by inventory | Inline currency units; existing bidder-owned conformance remains with each bidder |
| Response admission/currency | Create `crates/auctioneer-exchange/src/{bid_admission,currency}.rs`; modify `src/{lib,context,response}.rs`, `src/run/{mod,resource}.rs` | Inline units; `tests/bid_impression_correlation.rs`, `tests/response_charge.rs`, `tests/response_projection.rs`, `tests/run_auction/main.rs` and its ordinary module tree |
| Source-currency preservation | Modify `crates/auctioneer-bidder/src/bidders/{unruly,yahoo_ads,sovrn,sharethrough,triplelift}/response.rs` and any further currency-discarding decoder found on rebase; reuse shared `bidders/media_type.rs` | Existing inline response units and each bidder's own `tests/conformance.rs`; shared exchange/handler tests still use fake bidders |
| Currency-control admission | Modify `crates/auctioneer-openrtb/src/ext/request_prebid.rs`, core `src/auction/prepare.rs` and the existing bounded ingress/tree builder as actually present | Inline typed/raw visitor controls, core `tests/auction_config_lifecycle.rs`, exchange `tests/source_preflight.rs`; raw lexical cases use the existing fixture gate |
| Mode | Create `crates/auctioneer-config/src/protocol_mode.rs`; modify `src/{types,policy,source,lib}.rs`, `src/codegen/mod.rs`; core `src/config/policy_host.rs` | Inline config/core units, config `tests/root_shape.rs`, config-data `tests/{codegen,product_defaults_snapshot}.rs`, core `tests/config_reference_examples.rs` |
| Multibid input and output | Create `crates/auctioneer-openrtb/src/ext/multibid.rs` and `crates/auctioneer-exchange/src/multibid.rs`; modify OpenRTB `src/ext/{mod,request_prebid}.rs`, exchange `src/split/{mod,root}.rs`, `src/run/{mod,resource}.rs`, `src/response.rs` | Inline parser/rank units; OpenRTB `tests/sample_requests_round_trip.rs` checked-outcome migration; exchange `tests/{source_preflight,bid_impression_correlation,response_charge,determinism}.rs`, `tests/targeting_prefix/main.rs` and its ordinary module tree |
| UID wire controls and projection | Modify `crates/auctioneer-openrtb/src/ext/misc.rs`, `src/types/{user,field}.rs`; create exchange `src/identity.rs`; modify `src/split/{mod,root}.rs`, `src/scrub/{decision,fields}.rs` only as required by classification | Inline units; exchange `tests/{source_preflight,disclosure_model_coverage,disclosure_policy_diff,privacy_policy_composition,projection_scrub_bound}.rs` |
| Cookie I/O and shared auction | Create `crates/auctioneer-core/src/usersync/keys.rs` and `src/auction/identity.rs`; modify `src/usersync/mod.rs`, `src/handlers/usersync_keys.rs`, `src/auction/{mod,prepare,operation,services,audit}.rs`, `src/handlers/auction.rs` | Inline key/identity units; core `tests/{usersync_integrity,usersync_handlers,privacy_outbound,auction_config_retention,response_retention,clock_domain}.rs` |
| Original timing and host cooperation | Modify exchange `src/{tmax,context}.rs`, `src/run/mod.rs`, core `src/auction/{prepare,operation,services,audit}.rs` and `src/handlers/auction.rs`; reuse `src/host_primitives/{request_clock,registry}.rs` and the actual adapter factories | Existing exchange `tests/tmax.rs`, core `tests/{clock_domain,config_deadline}.rs`, Cloudflare `tests/fanout.rs`, all changed future-layout/resource owners |
| Disclosure metadata | Modify `crates/auctioneer-config-data/data/{supported-extension-catalogue,request-disclosure-policy}.toml` for the new typed identity controls; use existing metadata derive | Existing field/disclosure units and config-data `tests/disclosure_codegen.rs`; never rewrite the frozen oracle to hide a policy regression |
| Accounting and host handoff | Existing `crates/auctioneer-runtime/src/budget.rs`, core `src/host_primitives/resource_profile.rs`, four adapter `src/envelope.rs` owners, CLI resource paths only when a reviewed delta requires them | Core `tests/{catalog_resource_profile,boot_resource_charges,config_peak_charge}.rs`; exchange `tests/{response_factors,projection_charge}.rs`; four adapter `tests/boot_envelope.rs`; Axum `tests/resource_profile.rs`; CLI `tests/bidders_resources.rs` and `tests/resource_boot_parity/main.rs` |
| Reference and documentation | Modify `auctioneer.toml`, `README.md`, `docs/roadmap.md`, relevant `docs/deployment/` instructions and `docs/testing/resource-proof-ownership.md`; add current evidence only when executed | Existing exhaustive config-example checks; no new per-issue integration target or workflow gate |

`handlers/usersync_keys.rs` currently collapses all bounded-read, keyring and deadline errors into `SigningUnavailable`. It is not the typed reader required by the new auction contract. The plan reuses its actual read once, before that route-specific mapping; it does not copy the reader or send auctions through its egress deadline.

At this baseline `AuctionServices` and `AuctionContext` do not expose the installed `DecodeCooperator`; transport decoding owns its current borrow. Task 2 therefore includes wiring the existing trait through both shared owners, with explicit caller supply and no global fallback for embedded use. Reuse #244's wiring if it has landed. HTTP obtains the installed adapter cooperator; native callers use the existing no-op implementation. Update both context constructors and every inventoried caller, not a deprecated overload. Before any affected activation, orchestration must await it at the reviewed bounded CPU checkpoints and before subsequent dispatch, then re-read the original deadline. Pure transforms remain synchronous only for a proved bounded chunk; if they need chunking, freeze that actual work cadence and suspended-future layout in the same proof package.

## Delivery Units And Atomic Boundaries

| Unit | Tasks | Merge/activation rule |
| --- | --- | --- |
| Common inventory and proof preparation | 1-2 | No serving activation or numeric change from an inventory alone. |
| Currency delivery, #54 | 3-5 | Schema rename, operation capture, floor callers, response conversion, builder currency and active examples land together. External import has its own prerequisite design gate in Task 5; #54 stays open until that acceptance is delivered. |
| Multibid and mode, #55/#56 | 6-7 | One delivery PR may close both only when every criterion passes. Do not publish an inert mode, a full-list forward or a ranker behind an unreviewed flag. |
| Trust review then UID delivery, #57/#79 | 8-9 | Record the trust/recovery decisions first. Activate shared input, conditional key read, negative-state decision, recipient projection and final privacy scrub together. |
| Cross-unit acceptance and closeout | 10-11 | Each preceding unit already has its own full acceptance; this is interaction verification, not permission to defer tests. |

Do not infer new native GitHub dependency links from this table. The user approved a shared contract, not automatic editing of issue criteria or project fields.

## Task 1: Rebase, Inventory And Establish Hosted Evidence

**Files:** All owners in the table, read-only initially; specification/plan and current evidence only for verified record updates.

- [ ] Read repository/crate guidance, the approved spec, #54/#55/#56/#57/#79/#59 and current related #180/#192 criteria. Read active PRs before choosing a delivery branch.
- [ ] Fetch the actual default branch and record its SHA, `.tool-versions`, lockfile, SDK revision and prerequisite source objects. Never refresh or suppress an expired GVL/GCL archive without separate approval.
- [ ] Obtain the full hosted baseline on that exact base, including explicit compiler/evidence owners. Record links, commands, exit states and real summaries; do not reuse an older binary's evidence as a new head's result.
- [ ] Inventory all current callers/types/fixtures using `FloorCurrencyConfig`, `FloorConversion`, `floor_currency`, `ResponseInput`, `AuctionServices`, `AuctionContext`, both `with_request_start` constructors, `AuctionInput`, `prepare_auction`, `user.buyeruid` and `ExtRequestPrebid.multibid`. Include CLI examples, generated code, borrowed account validation, fake bidders and adapter test fixtures. Record actual per-owner counts before edits; text-reference counts are not failing-test counts.
- [ ] Audit every implemented response decoder for currency clearing, literal replacement and retained nonempty monetary responses. The verified baseline has `parsed.cur.clear()` in exactly five owners: `unruly`, `yahoo_ads`, `sovrn`, `sharethrough` and `triplelift`. Inventory their fixed-USD tests as well as the common empty-to-USD classifier; distinguish legitimate no-bid/default branches from a nonempty declared code being erased. Do not treat the five-name list as an exemption for a new sibling found on rebase.
- [ ] Inventory the configured sample-request corpus at its actual pinned revision, currently `0ef82cbb44f00b15290d02883c315e0f5a109536`, and the committed roots in `auctioneer-openrtb/tests/sample_requests_round_trip.rs`. Its present `SKIP = []` and unconditional parity contract must become the explicit checked-outcome migration in Task 6, not be silently bypassed. Record source revision/path/digest, exact newly invalid field/category and accepted/refused counts before editing; source-reference counts are not executed rejection evidence.
- [ ] Separately inventory valid-intent bid helpers/fixtures missing `id`, `impid` or `crid`, including the existing builders in `bid_impression_correlation.rs` and `run_auction/main.rs`. Keep deliberate invalid-price/identity cases distinct from tests whose declared purpose needs an otherwise valid bid. Record actual affected/failing cases, not an assumed exhaustive list of these two examples.
- [ ] Classify active inputs separately from immutable captures/historical profiles. Migrate current constructors/readers together; preserve historical bytes and their original schema claims. Do not bulk-replace every `schema_version`, currency label or identifier in the repository.
- [ ] Reconcile the actual application format at each activation. At this baseline it is 1; #244 independently proposes 2. Choose the next format relative to the rebased implementation base, and require a fresh review if a concurrent cutover invalidates that choice. Do not reserve #244's number, silently accept two formats or revive removed `accounts` fields.
- [ ] Record the test-first hosted sequence and obtain authorization to publish the implementation PR/checkpoints. A tests-only failing checkpoint may be pushed only with explicit approval; no local mutation or automatic batch of diagnostic PRs is authorized.

**Expected checkpoint:** A complete source/caller inventory and passing or explicitly blocked hosted baseline. No new production capability is claimed.

## Task 2: Freeze The Changed Ownership And Resource Package

**Files:** Core `src/auction/{audit,services,operation}.rs`, exchange `src/run/resource.rs`, runtime `src/budget.rs` and the existing resource test owners; no limit change is assumed.

- [ ] Write test-first layout/multiplicity controls for the actual proposed types: accepted currency code, rate owner, multibid plan and ordinals, UID disposition/selected hints, key-read handle, frozen `TmaxResolution`, borrowed cooperator and suspended preparation/execution futures. Unwired type definitions may be staged, but not a public accepting reader without its consumer.
- [ ] Wire the existing cooperator capability through shared services/context and HTTP/embedded fixtures before the first new CPU-stage activation. Pair a real frozen-clock positive control with a no-op negative/control source; ready I/O and an executor-only yield must not satisfy the host-refresh assertion. Do not postpone this plumbing until cross-unit Task 10.
- [ ] Build the accounting ledger from existing limits, not the happy-path fixtures. Include rate-table ownership once per retained operation, full input/control validation work, group expansion, potential label planning, ranking scratch, conversion annotations, signed-cookie decode and coherent keyring lifetime.
- [ ] Keep cookie/header work separate from body JSON and auxiliary-route funding. Existing cookie input caps are 64 fields, 32 KiB total headers, 32 KiB encoded input, 24 KiB decoded input and 4,096 nodes; these are not the 4 KiB complete output-cookie ceiling. The keyring uses its existing 4,096-byte/eight-key limits. Do not reduce any of these declarations to fit a fixture or charge the auxiliary pool for auction work.
- [ ] Add positive controls and independent expected terms before measuring. Vary rates, recipients, calls, candidates, header bytes and cookie nodes independently. Record both peak retained storage and work/allocation evidence; a valley allocation is not proved by an unchanged process peak.
- [ ] Run the audit/layout/resource owners in checkpoint R, including actual adapter constants and strict/boot parity. Preserve the single additive total and target-specific facts. A known guest fit failure remains a readiness blocker, not a failing-test expectation to erase.
- [ ] Submit exact changed charges, formulas, layouts, work cadence and source assumptions for independent review and user approval before installing a new reserve or enabling the new state/I/O. If actual production types differ from the reviewed package, return here rather than refresh expected numbers from the new formula alone.

**Stop gate:** No new numeric reservation, changed limit, SDK contract or provider-readiness assertion is approved by this plan. An unfunded unit remains disabled by not landing its activation, not by a new operator bypass.

## Task 3: Generalize The Existing Currency Model And Arithmetic

**Files:** Config `src/{currency,policy,lib}.rs`; bidder `src/{currency,request_info,lib}.rs` and inventoried imports/callers; their inline unit modules.

- [ ] Add independent literal arithmetic cases before generalizing the helper. Use actual source/target codes, nonidentity rates and different directions so USD identity cannot hide multiplication/division errors.
- [ ] Generalize the model to `CurrencySnapshot` and the borrowed helper to `CurrencyConversion`; retain `expires_at`, `usd_per_unit`, implicit USD identity and the 256-rate cap. Rename current imports/re-exports/callers together; do not retain deprecated type aliases.
- [ ] Provide one `convert_amount(amount, source, target)` arithmetic implementation and make the existing floor-to-USD method delegate to it. The protocol caller still chooses floor refusal scope. Do not duplicate rate validation in a second response-only converter.
- [ ] Reuse existing strict object/optional-value decoding and validation. Reject duplicate decoded rate keys, USD overrides, bad shapes and invalid rates without echoing the rejected table. Rate absence is not a zero/identity rate.
- [ ] Validate the captured table once at operation construction, using the supplied time. Preserve exact same-currency identity after price admission, exclusive expiry and the reviewed checked `f64` policy. Check both multiply and divide intermediates for nonfinite values and positive underflow; zero-price deals must not trip a positive-underflow guard.
- [ ] Exercise these prospective unit assertions with the proposed new API; the snippet is an expected test contract, not executed evidence:

```rust
let snapshot = CurrencySnapshot {
    expires_at: 2_000,
    usd_per_unit: BTreeMap::from([
        ("EUR".to_owned(), 1.25),
        ("GBP".to_owned(), 2.0),
    ]),
};
let conversion = CurrencyConversion::new(Some(&snapshot), 1_000);
assert_eq!(conversion.convert_amount(4.0, "EUR", "USD"), Ok(5.0));
assert_eq!(conversion.convert_amount(6.0, "USD", "GBP"), Ok(3.0));
assert_eq!(conversion.convert_amount(4.0, "EUR", "GBP"), Ok(2.5));
```

- [ ] Add one-before/at/one-after expiry controls, missing source/target rates independently, same-currency bit parity, zero-price deal conversion, negative/nonfinite programmatic input, overflow and positive underflow. Keep the source admission and conversion tests distinct so an earlier policy refusal cannot mask the arithmetic branch.
- [ ] Run checkpoint C on the authorized hosted head; retain all original floor unit/protocol cases. Commit the tested internal generalization only if it is buildable and still unwired from new public configuration until Task 4.

## Task 4: Convert Responses And Activate The Currency Hard Cut

**Files:** Create exchange `src/{bid_admission,currency}.rs`; modify `src/{lib,context,response}.rs`, `src/run/{mod,resource}.rs`; core `src/auction/{prepare,operation}.rs`, `src/config/validation.rs`; OpenRTB `src/ext/request_prebid.rs` and the actual bounded ingress owner; the inventoried bidder-owned response modules and inline/conformance tests; operator model and active examples; existing exchange/core/CLI test owners.

- [ ] Add a real shared-handler red test with source prices in the opposite order from converted prices. Supply different response currencies from two fake bidders/call slots and assert the exact winner, response `cur`, converted CPM, original-price annotations and targeting bucket. Include valid `id`/`impid`/`crid` fields so viability cannot mask currency behavior.
- [ ] Before changing exchange conversion, add bidder-owned red controls for each inventoried currency-erasing decoder: absent/empty defaults, explicit USD, explicit EUR/GBP and an invalid nonempty code. Assert the exact source code and unchanged source amount at `BidderResponse`, not just a shared fake's result. Replace the obsolete fixed-USD expectation with preservation assertions and an explicit regression against the old reset; keep status, sent-coverage, media and valid-sibling assertions. Remove the production resets in the same activation as conversion, with no old-behavior flag. Preserve provenance captures; record the intentional departure from the pinned reference's fixed-USD behavior locally.
- [ ] Reuse or implement #192's single shared bid-admission owner before conversion/winner selection. Preserve zero-price bids only with a nonempty deal ID, and preserve valid siblings. A simple `price > 0` filter is wrong. Keep floor eligibility out of this predicate.
- [ ] Migrate only inventoried valid-intent fixtures/builders to include the required fields; retain deliberate viability negatives, before-state assertions, valid siblings and zero-price deals. Re-run each earlier correlation/currency/rejection case and prove it still reaches the branch it names, rather than passing because `crid` is absent. Do not bulk-edit immutable reference captures, remove assertions or reduce populations/raise caps for green. If adding required fields invalidates an exact-size resource fixture, retain its original refusal/admission evidence and review a separate valid dense control under unchanged limits instead of silently presenting fewer bids as the old positive proof.
- [ ] Resolve the selected currency once from the request's ordered `cur`: absent/empty means USD; otherwise validate all codes and choose the first. Freeze that accepted code for response assembly and recipient request metadata; do not choose a currency per returned bidder.
- [ ] Integrate duplicate-preserving currency-control admission into the existing ingress before generic-map insertion, not merely before root projection. Reject repeated decoded `ext`, `prebid`, `currency`, `rates` and `usepbsrates` slots at their exact control-bearing paths, including equal/null values and escaped-equivalent keys. In the current direct parse, use typed control visitors before `ExtRequestPrebid.other` can absorb `currency`; if #244's generic tree is present, use its existing bounded admission builder before collapse. Retain one admitted result, not a second whole-request parser or an old `Value` fallback.
- [ ] Reject the presence of `ext.prebid.currency.rates` or `usepbsrates`, including null, with a fixed invalid-request error. Add raw lexical red controls for an override-bearing first `currency` followed by an empty second one, the reverse order, duplicate override members and escaped-equivalent keys; assert zero dispatch in both modes. Separately test two otherwise-admissible empty `currency` objects and repeated `ext`/`prebid` ancestors with no override, so the presence refusal cannot mask the duplicate guard. A single empty currency control and same-named unrelated nested data must remain valid positive controls, ruling out blanket parent rejection. Typed input checks its supplied state without claiming to recover external lexical history.
- [ ] Capture one projected rate-table owner in prepared state and borrow it into the shared exchange. Keep one captured time; do not clone the table per bidder or fetch it in `process_one_result`.
- [ ] In the actual call-result path, retain source currency until each admitted bid is converted, then merge candidates. Missing/unusable conversion affects that call's monetary candidates, not unrelated calls, valid sibling bidders or bounded browser-auction configurations.
- [ ] Add the independent literal regression `source EUR 4`, `EUR = 2 USD/unit`: decoder output remains EUR 4, a USD target gets 8, and an EUR target remains 4 rather than the old-reset result 2. Cover both the bidder-local source boundary and the shared conversion/handler boundary without importing a real bidder into shared test owners. Inventory all remaining decoder replacements again before activation; a fake-only pass is insufficient.
- [ ] Set/clear exchange-owned `origbidcpm`/`origbidcur` exactly as specified, replacing provider claims. Move creative/extension storage instead of cloning it into parallel candidate trees. Recheck the reviewed response-growth charge before additional allocations.
- [ ] Extend `ResponseInput` with the accepted output currency and update every inventoried builder caller, including direct allocation/conformance callers. The builder emits that code when bids exist and the existing empty code when no bids exist. No intermediate commit may advertise a foreign output currency while preserving unconverted prices.
- [ ] Use Task 2's shared cooperator at the reviewed conversion/annotation work checkpoints, then re-read the original deadline before targeting/output. Account for any new suspension layout. Do not leave the first CPU-stage activation depending only on a later Task 10 test.
- [ ] Replace only the active operator `floor_currency` field with `currency`, update the single supported application-format reader, CLI/config validators, fake current fixtures, exhaustive reference key lists and examples atomically. Reject the old field even under the new format; no alias, dual table, loader or compatibility feature survives.
- [ ] Test fresh/stale configured-policy admission separately from low-level conversion without a snapshot. Add two-call mixed-source-currency, unconvertible-first-request-currency, provider-forged annotation, cancellation and retained-result controls.
- [ ] Run checkpoints C, E, H and R and the full acceptance matrix F. Record the actual schema cutover and readiness limitations. Preserve historical captures/old evidence bytes; new active-artifact evidence gets a new identity, not edited old measurements.
- [ ] Commit/publish the tested currency unit only under authorization. Keep #54 open if Task 5's external ingestion criterion is not yet met.

## Task 5: Review And Deliver External Rate Publication Separately

**Files:** Initially the #54 design/plan and existing CLI `src/application.rs` configuration boundary, read-only. The producer's exact new files are defined by its prerequisite design, not guessed here.

- [ ] Prepare #54's scoped producer refinement: operator-side external import/refresh produces one complete `CurrencySnapshot` and publishes it through existing typed validate/diff/push. Serving keeps the same single publication and no network refresh or secondary cache.
- [ ] Freeze the actual source schema, allowed URL/redirect/auth behavior, maximum bytes/nodes/rates/depth, timeout, wall-clock source/expiry calculation, unknown-code policy and failure/publication atomicity. Existing limits are not evidence of an unreviewed external parser's bound.
- [ ] Obtain explicit approval for the concrete CLI/API shape, any dependency or numeric reserve, and reviewed producer spec/plan before writing that code. If approval is absent, stop this task without blocking independent manual-snapshot conversion work or claiming the import criterion delivered.
- [ ] Implement only the approved producer, with literal external fixtures, malformed/stale/oversize/error/cancellation controls and proof that failure cannot publish a partial table or extend an old expiry. An earlier fetched table is not silently republished as fresh after a source failure.
- [ ] Verify the actual existing config flow and byte-identical snapshot data reaching the current operator format; no private direct-store publication path. Run its approved complete checkpoint and full hosted matrix before #54 closure.

## Task 6: Add Typed Multibid And The Single Global Mode

**Files:** Create config `src/protocol_mode.rs`, OpenRTB `src/ext/multibid.rs`, exchange `src/multibid.rs`; modify config `src/{types,policy,source,lib}.rs` and `src/codegen/mod.rs`, core `src/config/policy_host.rs`, OpenRTB `src/ext/{mod,request_prebid}.rs` and `tests/sample_requests_round_trip.rs`.

- [ ] Add schema/model red tests for the exact values `strict` and `reference_compatible`, absent host value, explicit null, unknown strings and wrong types. The default is strict; invalid input must not silently inherit it. Keep diagnostics fixed if a token is pasted into a mode field.
- [ ] Use existing `deserialize_optional_value` in `HostPolicy`, an effective `ProtocolMode` enum in `HostConfig`, and existing `requested_policy_host` composition. Capture the effective value through that host owner; do not add a redundant independent context flag or process-global cache.
- [ ] Update product-default generation with `quote!`/`syn::parse2` and its exact source tests. Preserve workspace dependency metadata and generated-only includes. Do not introduce string-concatenated Rust or change unrelated generated files.
- [ ] Implement the one typed wire parser before generic-map insertion: require integer `maxbids`, reject unknown/duplicate fields and malformed member types, and retain selector presence/array order for the later normalizer. Keep absent/null/empty-array semantics. Range, selector conflict, name/group ownership and prefix rules are semantic normalization, not a mode-dependent second decoder. Remove the untyped `Value` pass-through; do not add a permissive legacy representation or camelCase alias to retain obsolete positive assertions.
- [ ] Migrate `sample_requests_round_trip.rs` explicitly to exhaustive parity-or-precise-refusal outcomes under the new model contract. Preserve all source bytes, root guards and populations. At the current source revision, `supplementary/multi-bid-error.json` contains a first rule with no `maxbids`; enumerate it by revision/path/digest and require the multibid missing-required-field refusal, not arbitrary `is_err()`. No known-name lookup, fixture-only registry or normalized response expectation is needed to reach that structural error. Use a structurally valid counterpart to prove the named boundary; acceptance of the original must fail its negative assertion. Every other discovered input must still round-trip or have a separately reviewed, exact rejection entry. Reject stale/unused rejection entries, unknown failures, skipped rows and accepted/refused population drift; include refusal rows in the total exercised count.
- [ ] Split the synthetic inline `passthrough_fields_round_trip` fixture: use a complete lowercase `maxbids`/fake-selector rule as the positive multibid case while retaining its unrelated `data`/`bidderconfig` parity assertions; add a negative camelCase-key control with every required lowercase field otherwise valid, so missing `maxbids` cannot mask the unknown-field guard. Add independent absent/null/noninteger `maxbids` and duplicate-key cases. The exact invalidity, not historical acceptance, is the regression contract.
- [ ] Add one normalization pass parameterized by the captured mode. Enforce known public names, selector presence/type rules and unique ownership; compatible mode performs only the four listed normalizations. Bound warning/plan growth before allocating; capacity refusal is not a truncated-success warning list.
- [ ] Add exact reserved override-path guards for raw and typed input before root projection. Include null and host-equal values. Use the existing strict operator/account model boundaries, and only the specified OpenRTB paths; a same-named bidder parameter is not an override.
- [ ] If #244 has introduced a generic tree before typed decoding, integrate these same duplicate-sensitive visitors at that tree's admission boundary before map collapse. Do not claim `from_value` can recover duplicate history, and do not maintain two normalizers for raw and typed inputs.
- [ ] Run checkpoints M, C and R for the staged types, including the full configured upstream corpus and both the precise rejection rows and unchanged parity rows. Record their separate actual populations and aggregate exercised total. Do not publish mode activation alone: Tasks 6-7 form one accepting delivery.

Task 6's verified source anchor is `supplementary/multi-bid-error.json` at the upstream revision recorded in Task 1, SHA-256 `e0706ceaeea56bee5f0bdc09939a5b4d4dfbcf5f085cae062151a450c0c357d4`. Its eight input rules include exactly one missing-`maxbids` row, at index 0. Under the proposed required-field reader, this is the one enumerated structural refusal, not a request to reproduce its reference-normalized response.

The read-only inventory found 53 upstream JSON files and seven committed model/capture files. Expected Task 6 outcomes are 52 upstream parity rows plus one precise refusal, and seven committed parity rows: 59 parity outcomes + 1 refusal = 60 exercised files, with zero skips. These are implementation expectations, not executed new-reader results. Re-inventory after rebase; changed inputs or additional refusals require explicit source/category review, not an automatically extended error allowlist.

## Task 7: Project, Rank And Limit Multibid Without Reassigning Identity

**Files:** Exchange `src/multibid.rs`, `src/split/{mod,root}.rs`, `src/run/{mod,resource}.rs`, `src/response.rs`, existing targeting units; core capture only as needed. Existing fake supports explicit request coverage and source response currencies.

- [ ] Add a shared fake transport red test with two distinct public recipients/aliases and different multibid rules. Assert full sent-body equality, selected single-recipient array, no competitor control and unchanged parameter/alias precedence from #232.
- [ ] Select the normalized rule before copying the root. Validate the full control first, but retain rule state only for actual participants. A base rule never fills an alias's absent rule.
- [ ] Build the bounded prospective targeting-label plan during control admission for ranks 2 through the rule's maximum; test all potentially emitted key families using the current prefix/truncation policy. Reject collisions before dispatch, and reuse the accepted plan at output. Do not create real registry aliases from labels.
- [ ] Apply the reviewed cooperator cadence to control/group/label work and ranking orchestration. Refresh the host turn and recheck the captured deadline before dispatch/output. Do not assume transport decoding's private cooperator also covers these CPU stages.
- [ ] Rank admitted, converted candidates per public bidder/impression using deal preference, descending CPM, original call slot and original bid ordinal. Keep deterministic order independent of response completion and preserve the existing tie order between bidders.
- [ ] Limit only explicitly configured groups and rebuild/move their vectors in rank order before overall-winner calculation. Leave valid-bid retention unchanged without a rule. Preserve source-seat association and valid siblings; never resolve a stale winner index after truncation.
- [ ] Implement primary/extra targeting and `targetbiddercode` from accepted labels. A missing prefix suppresses only extra bidder-specific labels, not valid extra wire bids or the overall winner. Overwrite provider attribution claims without rewriting the authoritative output seat.
- [ ] Add independently varied cases for 1/2/9 requested limits against 0/1/2/9/10 candidates, multiple calls, aliases, impressions and media. Include zero-price deals, invalid highest bidders, stable equal-price ties, prefix/truncated collisions and hostile provider labels.
- [ ] Pair every compatible normalization with strict rejection and unchanged shared safety negatives. Demonstrate at least one real strict/compatible output difference on the handler, not merely equality to the same enum constructor.
- [ ] Update the single exhaustive configuration reference and current docs. Run M, E, H, R and F. Commit/publish the atomic #55/#56 unit under authorization; keep privacy, unknown-feature and target eligibility rules identical in both modes.

## Task 8: Record UID Trust Decisions And Add The Wire Controls

**Files:** #57/#79 review records after authorization; OpenRTB `src/ext/misc.rs`, `src/types/{user,field}.rs`, exchange new `src/identity.rs`, disclosure catalogue/policy and current metadata tests.

- [ ] Record #57's reviewed integrity-only hint boundary and intended-consumer requirements. Reconcile the approved cookie-negative-state policy with #180's relevant acceptance without claiming its unrelated privacy-default work complete. Do not invent UID ownership, replay protection or account authentication from a MAC.
- [ ] Inventory current user/extension consumers and classify new typed `user.ext.prebid.buyeruids` through existing field metadata/disclosure ownership. Preserve unrelated user context and unknown-field fail-closed behavior; do not blanket-allow a prebid object.
- [ ] Add a bounded typed UID-map visitor that rejects duplicate decoded keys and invalid values before insertion. Preserve a present empty exact map value as a terminal clear. A top-level empty `buyeruid` is different: it permits the next source. Null/non-string control values get a fixed diagnostic, not raw-value interpolation.
- [ ] For raw top-level null handling, use an appropriate presence-aware field or existing non-null Serde pattern in the owning model; do not let `Option<String>` erase a forbidden null before validation. Typed input validates the representation actually supplied; it cannot prove what an external caller already discarded.
- [ ] Write pure selection units with three distinct source values and exact alias/family metadata. Exercise explicit clears, base/alias non-fallback, shared-family membership and discovered UID exclusions. No central switch over real bidder names.
- [ ] Consume/remove the complete buyer-UID map before recipient copying. Carry only selected hints and an explicit suppression disposition, not a complete cookie/map per recipient. Preserve the caller's owned input.
- [ ] Run I, M, E and R on the staged model/projection and disclosure changes. Stop #79 activation if the trust/recovery decision or resource package is not accepted; do not introduce a bypassing public helper.

## Task 9: Share Bounded Key Reading And Activate UID Projection

**Files:** Create core `src/usersync/keys.rs` and `src/auction/identity.rs`; modify the existing core auction, handler and usersync owners, exchange identity/split/scrub wiring, and their existing tests.

- [ ] Extract one bounded key-read primitive into `usersync/keys.rs`, retaining typed failures before route mapping. Provide a concrete reader built from the existing SDK `BoundSecretStore` and host `MonotonicClock`, not a second secret-loader mechanism or a new service trait for pure selection.
- [ ] Let `handlers/usersync_keys.rs` remain only its existing route facade, mapping shared typed errors to the sync routes' existing `SigningUnavailable`. Retain every existing sync-route assertion. There must be exactly one backend read/keyring-parse implementation, not two readers kept for compatibility.
- [ ] Add an optional borrowed reader to shared services and optional bounded cookie/header input usable in both `AuctionInput` variants. Keep settings first. If preparation is still synchronous, replace that one API with its async form and migrate all callers atomically; if #244 already did so, extend it. Never retain a deprecated sync wrapper or a host-only preparation path.
- [ ] Before awaiting any key read, capture the existing effective `TmaxResolution` from the canonical request, effective host and original `request_start`, intersecting any outer deadline. At this baseline only the outer cap is checked in preparation and exchange resolves `tmax` later; fix that ownership explicitly in `tmax.rs`, shared prepared state, context and `run/mod.rs`. If #244 already supplies a frozen resolution, reuse it rather than resolve again or widen its cutoff. Preserve its bidder-visible value and source; remove the late recomputation path for prepared operations.
- [ ] Pass that resolution's absolute execution cutoff into the key read with `Deadline::at_instant` in the existing common monotonic domain, including when `outer_deadline` is `None`. Do not pass the sync route's fresh egress allowance, the caller's outer cap alone or a newly anchored `tmax`. HTTP and embedded hosts supply the same clock source to the reader and shared services, including Task 2's cooperator.
- [ ] Implement the spec's ordered negative-state table before positive source selection: bounded header admission, host opt-out, genuinely no eligible UID recipients, absent cookie, denied host read with all-source suppression, one permitted verify/decode, typed key failure, integrity failure, signed opt-out, then live-family fallback. An explicit hint or absent family must not hide signed opt-out.
- [ ] Reuse existing privacy permission primitives with the auction's captured applicability signal for host storage access; do not reuse a sync-only applicability ladder. A bidder exception cannot grant host storage permission.
- [ ] Choose the UID using the pure selection owner, then apply the final composed per-recipient scrub. No later adapter/finalizer reads the old cookie/map to restore it. Existing device/user context and non-ID policy remain unchanged.
- [ ] Add pending-provider and failure controls with counters: permitted read exactly once, opt-out/absent/ineligible/denied-read zero polls, unavailable store distinct from invalid cookie, rotation, expiry boundaries, cancellation drops the provider future and returns all funding. Put explicit higher-priority hints and no family in signed-opt-out controls.
- [ ] Add deadline rows with no outer cap: a pending key provider must stop at request `tmax`, and an immediately-ready provider followed by CPU work must observe expiry after real host cooperation. Assert no subsequent bidder dispatch, unchanged bidder-visible `tmax`/source and restored funding. Separately test a shorter outer cap so it cannot mask an absent auction cutoff.
- [ ] Prove negative cookie state suppresses every source while contextual bidding remains otherwise possible. Independently vary GDPR, activities, COPPA, LMT and applicable US policy, with before-state presence assertions and shared-family recipients having different permissions.
- [ ] Verify actual generated-App handler output and typed/raw embedded parity in existing owners. Keep cookie response writes out of auctions. Run I, E, H, R and F before committing/publishing the atomic #79 delivery under authorization.

## Task 10: Verify Interactions, Snapshots And Retained Funding

**Files:** Existing core `tests/{auction_config_lifecycle,auction_config_retention,response_retention,clock_domain,privacy_outbound}.rs`; exchange determinism/correlation/resource tests; owning inline units.

- [ ] Add a retained-operation update control: operation A captures mode/rates/identity, the next publication changes them, A finishes coherently and B sees the update. Avoid accidentally reusing a process-global cached policy or asserting against the same resolver twice.
- [ ] Exercise currency-before-ranking with a rule of two, an invalid high-price candidate, a zero-price deal and two fake aliases with independent privacy decisions. Verify full monetary output, targeting labels, attribution and actual sent request bodies, not just candidate count.
- [ ] Keep completed success/failure results alive across another admission. Verify the right receipt remains held through projection/drop and all resources are restored afterward. Do not hardcode expected totals using the same changed charge constant.
- [ ] Run frozen-clock cooperation and expiry before each subsequent dispatch, including CPU-only normalization/ranking between ready I/O operations. Preserve actual clock/source attribution and absolute cutoff; an unrelated timeout is not the branch's positive control.
- [ ] Re-run resource/layout owners against final compiled types after every ownership or future-shape change. Return to Task 2 for a changed proof package; no post-measurement threshold tuning or dropping a summand to obtain fit.
- [ ] Confirm unresolved FPD remains unforwarded and that #59 can consume the captured mode/public identities without changing their trust boundary. Do not implement new FPD merges or weaker disclosure policy in this delivery.
- [ ] Verify the hard cut has no decoder erasing a nonempty source currency, old rate-field alias, untyped multibid pass-through or permissive legacy control parser. Keep historical bytes and precise refusal evidence, not executable compatibility paths. Independently review any newly discovered sibling; do not use an allowlist of the original five decoders as a permanent exception.

## Task 11: Publish Evidence And Close Only Delivered Issues

**Files:** Relevant current docs/evidence and existing GitHub issues/PRs, after authorization.

- [ ] Record each delivery's exact reviewed base/head, full hosted workflow links, commands and verbatim result summaries, plus source/resource/target limitations. An unstarted, skipped or missing-prerequisite job is not passing evidence. Record the model-corpus accepted/refused populations and exact migrated categories, so a hard cut cannot be presented as unchanged parser acceptance.
- [ ] Read every review thread, including later replies/outdated anchors; reply per thread as CREATOR with fixed/won't-fix/disagree and actual evidence. Leave resolutions to REVIEWER.
- [ ] Obtain current-head review and the user's merge decision/account exception where required. Merge only that authorized delivery; never merge planning and implementation together or bypass failed checks administratively.
- [ ] Read back the merge and issue state. Close only implemented, accepted child scope; leave #54 open for an undelivered producer and keep #57/#180/#59 work truthful. Planning publication alone does not close #36 or any child.
- [ ] Clean only the phase's owned, safely removable worktree/branch after merge readback. Keep other cycles and unowned files. Never run routine target cleaning as a verification prerequisite.

## Validation Checkpoints

These commands are expected hosted instructions, not executed results. Recheck target existence and required features on the implementation base. Focused checkpoints supplement, never replace, the full required workflow matrix. Do not run these locally under the current user rule.

### C: Configuration And Arithmetic

```sh
cargo test --locked -p auctioneer-config --lib --test root_shape --features build-support
cargo test --locked -p auctioneer-bidder --lib
cargo test --locked -p auctioneer-config-data --test codegen --test product_defaults_snapshot --test disclosure_codegen
cargo test --locked -p auctioneer-core --lib --test config_reference_examples --test auction_config_lifecycle
```

Expected: actual new arithmetic/policy tests dispatch, all valid snapshots/examples pass and invalid controls return their intended categories. No exact aggregate case count is predicted here.

### M: Wire Models And Mode/Disclosure

```sh
cargo test --locked -p auctioneer-openrtb --all-targets
cargo test --locked -p auctioneer-exchange --lib --test source_preflight --test disclosure_model_coverage --test disclosure_policy_diff --test privacy_policy_composition --test projection_scrub_bound
cargo test --locked -p auctioneer-bidder --test test_isolation
```

Expected: typed/raw control admission, duplicate-preserving negative controls, exact mode paths and recipient/metadata privacy tests actually exercise their new branches. Generic `Value` fixtures alone cannot prove duplicate-key refusal. `auctioneer-openrtb --all-targets` includes the migrated sample corpus with every discovered input classified exactly once: unchanged parity or its specifically enumerated new refusal, never a skipped failure.

### E: Exchange Monetary And Recipient Behavior

```sh
cargo test --locked -p auctioneer-exchange --test bid_impression_correlation --test bidder_call_correlation --test determinism --test run_auction --test targeting_prefix --test response_charge --test response_projection
```

Expected: valid siblings and zero-price deals survive; converted prices decide winners; multibid labels cannot change seat identity; independent aliases do not inherit controls or UID hints. Shared fixtures use fake identities.

### I: UID Integrity And Lifetime

```sh
cargo test --locked -p auctioneer-core --lib --test usersync_integrity --test usersync_handlers --test privacy_outbound --test auction_config_retention --test response_retention --test clock_domain
```

Expected: original sync behavior stays covered, auction error classification remains typed, negative-state ordering and conditional read counts are observable, and retained funding/cancellation controls restore capacity exactly once.

### H: Shared Handler And Actual Native Wire

```sh
cargo test --locked -p auctioneer-core --features conformance-suite --test bidder_handler_conformance --test fanout_conformance --test native_bidder_onboarding
cargo test --locked -p auctioneer-adapter-axum --features insecure-bidders --test privacy_wire
cargo test --locked -p auctioneer-cli --all-targets
```

Expected: the same captured policy reaches standalone/embedded flows and real fake-bidder sockets. The CLI graph also checks its exhaustive application reference. Real-bidder protocol tests are not moved into shared code.

### R: Accounting, Layouts And Adapter Facts

```sh
cargo test --locked -p auctioneer-core --lib auction::audit::tests
cargo test --locked -p auctioneer-core --test catalog_resource_profile --test boot_resource_charges --test config_peak_charge
cargo test --locked -p auctioneer-runtime --lib budget::tests
cargo test --locked -p auctioneer-runtime --test term_exhaustiveness
cargo test --locked -p auctioneer-exchange --test response_factors --test projection_charge --test response_projection
cargo test --locked -p auctioneer-adapter-axum --test resource_profile --test boot_envelope
cargo test --locked -p auctioneer-adapter-cloudflare --test boot_envelope
cargo test --locked -p auctioneer-adapter-fastly --test boot_envelope
cargo test --locked -p auctioneer-adapter-spin --test boot_envelope
cargo test --locked -p auctioneer-cli --test bidders_resources --test resource_boot_parity
```

Expected: reviewed multiplicities/independent bounds and the final layouts are exercised. The explicit runtime budget and term-exhaustiveness commands are mandatory before activation when their production accounting/types change, not deferred to a later broad run. Existing explicit nonqualified targets remain nonqualified; a test expecting a recorded fit failure is not an assertion of deployment readiness.

### F: Full Hosted Acceptance

Use the complete current `.github/workflows/test.yml`, `format.yml`, `ci-cloudflare.yml`, `ci-fastly.yml` and `ci-spin.yml`. Preserve all mandatory native feature graphs, explicit compiler/source/config-failure/producer owners, guest builds/executed publication checks, strict linting and vendor package/size checks. Broad `--all-targets` alone misses required-feature and `test = false` owners; do not substitute it for the matrix. Keep existing once-owned producer scheduling and exclusions unchanged.

The ordinary lint commands include both workspace graphs and the conformance-feature graph:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features insecure-bidders -- -D warnings
cargo clippy -p auctioneer-core -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin --all-targets --features conformance-suite -- -D warnings
```

Obtain actual full-matrix evidence on each fixing commit. Read hosted logs, record the precise commands/statuses and new case dispatch, and keep independently supplied GitHub execution distinct from an agent's own checks. If additional hosted mutation controls are needed to prove a critical branch, obtain approval for that scope first; no local mutations or automatic extra PR batch is implied.

## Planning Verification Record

This document is an ordered plan, not a report of executed currency/multibid/UID behavior. Its named existing owners and test roots are subject to read-only existence checks and advisory plan review. Record those actual checks separately from the expected hosted checkpoints above. Publication and implementation remain separate authorization gates.
