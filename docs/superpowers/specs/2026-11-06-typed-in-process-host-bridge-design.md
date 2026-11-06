# Typed In-Process Host Bridge

Date: 2026-11-06

Status: Proposed for joint specification/plan review. The maintainer requested both documents before the review handoff. This authorizes preparing the proposal, not its public APIs, production implementation, publication without permission, resource changes or merge. No approval exception from an earlier PR applies here.

Tracking: [#51](https://github.com/IABTechLab/auctioneer/issues/51), under [Direct Trusted Server Integration](https://github.com/IABTechLab/auctioneer/issues/8). Parent contract: [Trusted Server Fastly local provider](2026-11-05-trusted-server-fastly-local-provider-design.md), accepted through PRs #152/#153. [Implementation plan](../plans/2026-11-06-typed-in-process-host-bridge.md).

## 1. Outcome And Boundaries

Expose one application-level preparation/execution boundary that both the existing HTTP handler and a host-supplied in-process caller use. Preserve the existing OpenRTB preparation, configuration resolution, discovered registry, privacy enforcement and auction engine. Return admitted typed candidates, the actual selected local candidate per impression, targeting and typed diagnostics under a retained resource owner.

This is Auctioneer's portable bridge package, not the actual Trusted Server executor. Trusted Server #1505 owns its live compiled-plan/local-execution representation and SDK/service alignment; #76 owns the linked Fastly application, outer competition and final delivery. Four-adapter schema/fit/serving qualification remains with #46/#47, #72-#75 and #88-#90. No current native-only restriction is removed here.

No second policy schema, loader, signer, private store, HTTP application, registry or engine is introduced. There is no new workspace crate, dependency, numeric reserve, workload limit, resource ceiling, response FX support, player or renderer. This proposal intentionally changes internal/public Rust return types rather than carrying a second compatibility execution API. Existing standalone wire behavior remains the regression contract.

## 2. Source Baseline

Auctioneer source: `87e329fe88f3a1c15a4d298d026607971b3065ee`. Trusted Server interface baseline remains `51730bdaf841e4d7bb96c2258e5e98169bdff9a7`; this package changes no host files. The different SDK revisions recorded by the parent contract are not assumed interchangeable.

Verified current behavior, not an implementation of this proposal:

- `handlers::auction::auction_response` prepays six phases before SDK config extraction, holds audit/admission owners and captures the original request start. Private `auction_inner` still combines input preparation, effective policy, plan/service assembly, `run_auction` and HTTP serialization.
- Public explicit-context `resolve_policy_host`, `resolve_policy_account`, `resolve_policy_budgets` and `ConfigValidationContext` already exist. Reuse them; do not add another configuration resolution mechanism or claim their presence already admits a host operation.
- Raw account discovery runs before typed request validation. `AccountSelection::Default` differs from a named `unknown` account; `resolve_selected_account` does not itself reject disabled accounts.
- `AuctionContext` borrows its owners. Persisting it beside owners in an allegedly owned struct would be self-referential. `Reservation` also currently borrows `AuctionBudget`; returned `BidResponse` contains no receipt.
- `apply_targeting` already computes impression-aligned `Winner` values. Selection uses optional deal preference, then strictly greater price, retaining the first equal-priced candidate in bidder/bid traversal order. The vector is discarded after targeting.
- `ResponseInput` contains typed bids, diagnostics, timing and browser-auction output. `build_response` consumes it, moves bids into wire seats and discards classified media. A response-plus-sidecar alone would not retain every typed outcome.
- `TypedBid` has no upstream-seat provenance. Wire `SeatBid.seat` is manufactured from the requested bidder/alias. It is not evidence of an upstream seat or a host provider identity.

## 3. Approach

Use a shared owned prepared operation with short-lived borrowed engine views. Both caller kinds enter that operation after their own transport/configuration acquisition. The operation owns only its normalized request, resolved application state and required settings, plus its receipts; immutable services are supplied explicitly. Its consuming execution future creates `ResolvedAccount` and `AuctionContext` locally, never as self-borrowing fields.

Alternatives considered:

| Approach                                                         | Decision                                                                                                                    |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Shared operation and typed engine outcome                        | Recommended: one policy implementation and one explicit resource lifetime, without an HTTP dependency in the API.           |
| Publish `auction_inner` or wrap a synthetic `RequestContext`     | Rejected: couples embedding to SDK extraction, ambient boot state and HTTP output; risks a second App or fake request.      |
| Have each host assemble low-level `AuctionContext` independently | Rejected: repeats account, alias, privacy, budget and settings policy; public engine access is not application equivalence. |

Normal standalone configuration loading and SDK validation remain where they are. The bridge consumes the already accepted `AuctioneerAppConfig` subtree of the host's single runtime policy. No per-operation store fetch or config fallback exists in the shared boundary. Host acceptance must invoke the existing shared validator with its actual catalog/defaults/funded context; successful validation is not a memory reservation or proof of target readiness.

## 4. Proposed API And Ownership

The following signatures are the proposed public contract, not existing code or compiled evidence. Supporting field declarations stay private except explicit input/service records; names may be refined during review without adding another execution route.

```rust
pub fn prepare_auction<'host>(
    settings: &AuctioneerAppConfig,
    input: AuctionInput<'_>,
    host: AuctionServices<'host>,
    funding: AuctionFunding,
) -> Result<PreparedAuction<'host>, AuctionFailure>;

impl PreparedAuction<'_> {
    pub async fn run(self) -> Result<CompletedAuction, AuctionFailure>;
}

impl CompletedAuction {
    pub fn outcome(&self) -> &AuctionOutcome;
    pub fn into_wire_response(self) -> Result<CompletedResponse, AuctionFailure>;
}
```

The settings-first parameter order is part of the exposed API contract. Trusted Server borrows only the accepted Auctioneer settings subtree from its single host-owned `Settings` value and passes that typed `&AuctioneerAppConfig` as the first argument, followed by the auction input. It does not pass the complete Trusted Server settings object, untyped JSON, a config path/source selector or a separately loaded Auctioneer config. This parameter is the accepted operator policy already described above, not another settings model. The same entry point and ordering apply to standalone callers; there is no embedding-specific overload or policy hidden in auction parameters or services.

Proposed host call, where `auctioneer_settings` is that borrowed subtree and the other arguments are request-scoped:

```rust
let prepared = prepare_auction(auctioneer_settings, auction_input, services, funding)?;
let completed = prepared.run().await?;
```

The host owns the subtree's placement in its `Settings` model; this example does not claim that the integration field or bridge exists today. Auctioneer depends only on its own shared config type, not a Trusted Server settings type.

`AuctionInput` has two representation constructors, not two policy pipelines: borrowed bounded JSON bytes for HTTP, and a borrowed programmatic `BidRequest` for embedding. Both carry borrowed `RequestHeaders`; debug request and trusted `EntryPoint` come from the caller's explicit context, never payload-created trust. Shared construction admits the representation before allocating its owned normalized request. A rejected programmatic tree remains the caller's owner and is not recursively dropped by a failed ownership transfer.

`AuctionServices` borrows the matching discovered registry/bindings, portable fanout, clock/RNG, GVL/GCL, host restriction source and optional credential source. It also names product defaults and a private-construction `AuctionResourceProfile`. That profile carries the existing derivation tuple: resolved isolate limits, platform constants, outbound metadata bytes, construction costs, neutral compiled-default byte count and catalog costs. Its constructor calls the existing `derive_isolate_budget_with_baked_config`; callers cannot supply an arbitrary `Derived` literal as proof. The installed schema allowance remains an explicit qualified fact, not inferred from a nonzero cost. Standalone projects this tuple from its installed owner; embedding supplies its accepted host tuple. This creates no new resource equation, gate or operator-config source, and does not certify the combined host application's fit. The shared path never reads `boot_gates()`, `registry_handle()`, primitive globals or `DevFanout` as an ambient fallback.

`AuctionFunding` is a non-cloneable owner of the six prepaid existing phases and the already-acquired auction-capacity permit. It is created before configuration acquisition in the standalone wrapper, and before any bridge-owned projection in the host caller. The wrapper moves the existing portable `AdmissionPermit` out of its ingress lease; the host supplies the corresponding permit from its accepted shared admission owner. Moving it between states does not acquire admission, transport permits or another ledger. Missing, wrong-kind or under-capacity funding refuses before its funded work. Existing HTTP terminal/egress owners remain separate and are not replaced by this portable permit.

The funded state also owns the existing disclosure observation scope; `run` does not borrow an audit from an external caller. Move that portable guard into the shared auction owner without changing its numeric-only logging/once semantics. Its field/drop order observes cancellation while receipts are still live. Preparation/execution failures finalize Error before releasing charged work; successful execution transfers the unfinished scope with the completed owner. HTTP finalizes Success only after its existing successful response construction, and Error on projection/serialization refusal. An embedding caller explicitly finalizes its local work before releasing/abandoning completion; unfinished drop finalizes Cancelled. Finalization does not release funding, and repeated finalization/drop cannot observe twice. Error values retaining a bounded heap diagnostic retain its receipt owner; prepayment refusal uses fixed allocation-free diagnostics.

`PreparedAuction` owns the normalized request, selected `ResolvedAccountOwned`, effective host/plan, needed bidder settings, floor snapshot, timeout overrides and original timing. It contains no `AuctionContext` field and no full config clone. Preparation borrows the accepted policy only long enough to project the request-owned state; the standalone wrapper drops its extracted policy before fanout. Add a bounded settings projection using existing generated settings types; copy only entries required by resolved requested/base identities, not unrelated accounts or the whole policy. Retained host Settings are independently host-owned, not an additional operation-owned snapshot.

`CompletedAuction` owns the typed engine outcome and the funding for its retained data, including the auction-capacity permit. Per-auction receipts alone do not constrain the process-wide population: the existing additive model funds at most `max_concurrent_auctions` such owners. Retaining a completion, a heap-owning failure or a transient consuming projection therefore retains that same acquired permit until its per-auction storage is destroyed, or until a reviewed prepaid transfer places the surviving data under an existing separately funded population. No second pool or arbitrary host promise substitutes for the acquired owner.

The completed owner exposes borrowed views, not `into_parts` or an unguarded owned result. `CompletedResponse` similarly holds the consuming wire projection and its funding through HTTP serialization. The HTTP path may release auction capacity only after the typed/projection owners are gone and the remaining bytes have moved into the existing bounded terminal/egress owner; preserve its terminal-delivery behavior rather than holding or releasing a permit based only on a handler return. A host that retains or transforms a result must retain that completed owner, or establish the same reviewed prepaid transfer before releasing it. Host-created creative copies are not funded merely because the source was funded.

### Owned Receipts Without Self-Reference

Propose extending the existing receipt implementation with an owned budget handle: a private borrowed-or-`Arc<AuctionBudget>` owner inside `Reservation`, and an owned prepayment entry on `AuctionCharges`. The borrowed entry continues to use the same accounting implementation. Owned receipts remain non-cloneable; cloning the ledger handle does not create capacity. Reserve/check/drop use the same atomic ledger and `ChargeKind` rules. Partial prepayment failure releases every earlier receipt.

This is an ownership proposal for review, not permission to add a second ledger, change a bound, or claim an unmeasured layout is free. The resulting receipt/frame layouts and reference-count allocations must be included in the existing resource controls before implementation is accepted. A failed bound returns for review; no threshold is adjusted until green. No `unsafe`, leaked owners, lifetime transmute or self-referential pinning helper is needed.

Services may remain borrowed for the host operation's declared lifetime. The execution future is request-scoped and need not be `Send` or `'static`; the actual host executor must support that contract or present a separately reviewed ownership adaptation. Completion does not borrow `PreparedAuction` or its stack-local budget. Nothing here claims the current Trusted Server HTTP pending-handle type can already hold this future.

## 5. Admission, Policy And Error Precedence

Preserve the standalone ordering, not merely the final happy path:

1. Prepay, obtain the already accepted policy/services and reject a spent outer host deadline before new bridge-owned parsing or credential work.
2. Retain HTTP transport checks and bounded body acquisition in the wrapper. Shared JSON admission validates syntactic node/depth limits and captures original extension shape before account discovery.
3. Resolve account identity before canonical typed validation. Preserve the six existing ordered publisher paths and the distinction between default and explicitly named accounts. Raw selective discovery remains the owner of raw spelling/duplicate handling; no whole `Value` tree is added.
4. Reject the selected disabled account, then parse/construct typed input and run the existing `prepare`. Apply its impression/header limits and structural/privacy-field rules.
5. Apply fatal wire-signal validation before alias/plan errors, then existing activation/alias, scoped budget, sampling, timeout, settings and feature resolution.
6. Build temporary engine views and enter the existing per-bidder privacy/projection and dispatch sequence.

Programmatic input receives borrowed depth/node/serialized-width admission before copying. Account discovery uses the same path precedence and `resolve_selected_account` merge, without a serialize/reparse round trip. Factor the shared account-selection rule instead of retaining an independent handwritten policy walker. Raw malformed input has no typed equivalent: equivalence is required where both representations can express the same input, while wire-only syntax/error controls stay mandatory.

Typed-only input has unknown original extension provenance; it cannot manufacture wire evidence from normalized content. Existing source-construction, missing/null/value and bidder-owned parameter validators stay active. Requested registry/bindings must match; no bidder identity is hardcoded in shared code or fixtures.

The host must not reintroduce identifiers/consent that it removed. Its restrictions and the engine's vendor/purpose/activity policies compose by intersection. Preserve source/provenance-sensitive TCF/GPP precedence and existing ID/geo/FPD/TID scrubbing. This bridge adds no alternative consent interpretation, cookie reader or account-level permission grant.

Propose a synchronous, request-bound `HostRestrictionSource` taking requested identity and canonical implementation and returning five typed ceilings: allow fetch, IDs, precise geo, user FPD and transaction IDs. It performs no I/O. Each ceiling is intersected with the corresponding engine decision immediately before fetch filtering or classified projection; a host ceiling cannot grant an engine-denied permission. `allow_precise_geo = false` uses the existing geo-denial transform, not a new coordinate algorithm. The HTTP wrapper supplies an explicit unrestricted source; embedding must supply its host source, with no ambient default. Denials remain separate from decoded consent/legal-basis facts, so no fake consent signal is constructed. Host restrictions on data outside these five dimensions remain the host's ingress responsibility and must not be undone by later enrichment/projection.

`AuctionFailure` preserves preparation, account-disabled, privacy, unavailable-capability, resource, deadline and internal categories using existing errors and bounded diagnostics. HTTP maps them to its current status/body/header behavior. Host execution must not turn a fatal error into clean no-bid. Bidder-local errors and valid siblings remain a successful typed auction outcome with diagnostics. Empty candidates, `nbr`, and optional nonbid rows alone are not a complete success classification.

## 6. Original Deadline And Cancellation

Carry both the original monotonic admission instant and an optional absolute outer-host deadline in the explicit service/timing input. Standalone supplies no additional outer cap and retains its current behavior. Extend the existing `AuctionContext`/tmax resolution so the single effective auction deadline is `min(existing request/host tmax deadline, outer host deadline)` when present. Keep bidder-visible tmax semantics distinct from this execution cap.

Read the same clock domain throughout. An already-spent outer cap refuses before new bridge parsing, schema work, credential access or dispatch. Recheck after preparation and before every existing dispatch boundary; a queue wait or collection never grants a new allowance. Existing per-call limits still cap the common auction deadline. A success-write deadline may still begin at egress entry; it does not extend auction work.

`run(self)` is consumed once. Polling that future starts execution; constructing it is not proof of dispatched work. The host's later start/collect integration must pin/poll this same future to make progress, retain it during unrelated origin work and collect it without restarting. Dropping unpolled preparation or a suspended future abandons work and releases its owners exactly once. A typed deadline/drop outcome does not promise actual upstream cancellation on a platform that only supports abandonment.

## 7. One Typed Engine Outcome

Change the existing `run_auction` return to a private-construction `AuctionOutcome` instead of adding a second runner. It owns the already-admitted `ResponseInput` material, the retained impression-aligned selection vector and finalized permitted debug output. It also owns the request/impression identifiers needed to associate every selection and non-selection after the prepared request is destroyed; a vector of positions without those owners is insufficient. Admit and charge this bounded retained identity storage before allocation. Both ordinary and early-timeout/no-live exits produce this same representation. Preserve existing timestamps, diagnostics, conditional nonbid rows and browser output.

Expose read-only candidate/bidder views over the existing `TypedBid` storage. A selected view resolves the original winner's bidder-vector index, not an impression-filtered index, a sorted-price position or a parsed targeting key. Preserve `preferdeals`, stable equal-price order and existing RNG consumption. Do not invent dealpriority-based selection, a tie-reason field or another reduction. An impression with no selection stays explicitly unselected, including browser-only outcomes.

`AuctionOutcome::into_bid_response` is the sole consuming projection to the existing deterministic `build_response`. It moves candidates once, preserves wire ordering/extensions and attaches already-finalized debug under the same permission/deadline. It does not run selection, price bucketing or targeting again. All current wire-oriented callers migrate explicitly; no compatibility runner remains. The core funding wrapper prevents this projection from silently detaching the data's resource owner.

Per-bidder views expose existing errors/warnings, dispatched status, timing, construction diagnostics and browser configs as typed data. Do not infer clean no-bid from empty bids when errors or deadline diagnostics exist. These are the current engine facts, not a newly complete per-impression transport-outcome ledger. Keep USD-only response admission; request-floor conversion is not response FX.

### Attribution And Seat Provenance

Keep separate identities for requested bidder/alias, canonical implementation and returned upstream seat. A Trusted Server provider ID belongs to the host and is not inferred from any of them. The actual bid/imp IDs and complete existing `Bid` object, including notifications and extensions, are borrowed/moved rather than reconstructed from a lossy subset.

Add optional source-seat provenance to `TypedBid`, captured before the existing upstream-seat flattening. Share one immutable owner per decoded upstream seat across its bids; do not clone a possibly large seat string per bid. Missing/empty canonical seat remains explicitly absent; a protocol with no seat supplies none, never the requested alias as a substitute. Update shared classification and any source decoder that bypasses it, discovered by actual construction-site search rather than a bidder allowlist. Keep the standalone wire seat unchanged. This new retained owner requires resource accounting and protocol-owner conformance before #51 can claim lossless seat projection.

## 8. Resource And Lifetime Acceptance

Keep the existing checked additive model and all caps/headroom. The exact same derivation inputs supply service limits and prepaid charges; narrower policy is clamped against those facts. Do not install an independent capacity pool for an embedded call or infer funding from a nonzero schema allowance alone.

The ownership audit must cover configuration projection, normalized input copy, selected settings, owned receipt storage, retained winner indices, upstream-seat sharing, finalized debug, typed outcome and wire projection. Data may move but may not survive the receipt and capacity owner funding it. Holding all six phases and the acquired capacity permit until completion release is the conservative initial implementation; release individual phases only after their last owners demonstrably die. Cancellation/audit must run before funding is released. Field destruction order must release outcome/data before the capacity permit and receipts. A nonempty completion retained at the existing admission limit must prevent another operation, even after its execution future and caller's ledger handle have been dropped; releasing or validly transferring that completion restores exactly one slot.

Measure retained, peak and cumulative allocation independently in the existing isolated owners, under real nonzero populations and original limits. Add independent size/population sweeps, source-sharing controls and under/at/over receipt tests. Assert installed charges rather than computing expected results from the same new cost constant. Do not claim that an unchanged peak proves a new copy is free. No complete target fit, compiled stack proof or linked-host residency is obtained from these checks.

## 9. Acceptance Controls

All are future controls until implemented and executed. Shared tests use capable `fake_` bidders/services. Unit tests stay inline; existing integration/conformance/allocation targets own cross-module behavior and process isolation.

| ID  | Required Observable Result                                                                                                                                                                                                                               | Existing Owner To Extend                                                                                                                               |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| B1  | Both callers pass accepted `&AuctioneerAppConfig` first and auction input second to the same entry point; preparation/engine results agree with account, aliases, settings and privacy varied independently; standalone wire output stays equal.         | core `handler_resolution`, `privacy_config_parity`; exchange `determinism`                                                                             |
| B2  | Raw node/depth refusal precedes disabled account; disabled account precedes typed validation; fatal privacy input precedes alias errors; default/named account selection and scoped caps stay distinct.                                                  | core `handler_resolution`, `node_budget_on_the_auction_path`, `privacy_wire_signal_400`, `response_account_resolution`                                 |
| B3  | Two isolated accepted policies/services on one executable cannot leak aliases, settings, floor snapshots, secret references or source state; unrelated extracted policy is gone before fanout.                                                           | core `auction_config_lifecycle`, `auction_config_retention`, `resolve_runtime_config`                                                                  |
| B4  | Selected candidate is exactly the original reducer's choice for deals, equal prices, multiple bids per bidder/imp and disabled winner targeting; other admitted bids remain inspectable without becoming outer contenders.                               | exchange winner units, `targeting_prefix`, `bid_impression_correlation`, `bidder_call_correlation`                                                     |
| B5  | Missing/no-bid, bidder errors, timeout, unavailable services, browser-only success and non-USD bid refusal preserve distinct current facts and valid siblings through typed and wire projection.                                                         | exchange `run_auction`, `response_charge`, `response_projection`, `code12_attribution`                                                                 |
| B6  | Requested alias, implementation and optional shared upstream seat stay distinct; missing provenance is never fabricated; one large seat reused by many bids allocates once.                                                                              | bidder shared classifier units/owner conformance; exchange `response_projection`, `response_factors`                                                   |
| B7  | Before/at/after original outer deadline, preparation delay, queueing and suspended collection cannot restart work or access secrets on an expired entry; dropping each state releases once.                                                              | core `clock_domain`, `handler_wave`, `admission_holds_through_auction`; exchange `construction_deadline`                                               |
| B8  | Owned/borrowed receipts share one ledger; rollback and observation/drop ordering hold; nonempty retained completion/failure/projection keeps its acquired capacity permit until destruction or prepaid transfer, refusing another operation at capacity. | runtime `auction_budget` units; core `auction_budget_concurrency`, `response_retention`, `auction_config_retention`, `admission_holds_through_auction` |
| B9  | Host-deny/engine-allow and host-allow/engine-deny never disclose beyond their intersection, using independently varied ID/geo/FPD/TID restrictions and serialized bidder calls.                                                                          | core `privacy_outbound`; exchange `privacy_policy_composition`, `disclosure_policy_diff`                                                               |
| B10 | No HTTP context, second loader, ambient operator fallback, real-bidder test stand-in or handwritten source import enters the shared boundary; every migrated caller is compiled in its feature/target lane.                                              | core `dependency_purity`; bidder `test_isolation`; unchanged full CI matrix                                                                            |

Each negative case must reach its claimed branch. Use independent inputs, before-state presence, nonempty dispatch/valid-sibling populations, exact test counts and mutation/revert controls. Existing tests that only compare config lowering or publication metadata are not embedded execution evidence.

## 10. Review And Delivery

The spec and plan are prepared together at the maintainer's explicit request; both remain proposals until joint review. Implement only after the planning PR is approved and merged under the full PRfection gate. No previous head-specific exception applies. Keep #51 open when that planning PR merges.

The implementation PR must include B1-B10 and the resource/lifetime review, not postpone bridge correctness to #76. If the existing bounds cannot cover the proposed ownership changes, return with measured/derived evidence for review rather than waive acceptance. Actual host execution, guest eligibility and hosted/publisher evidence retain their existing owners; this package cannot close them from fixture or native results.

### Concrete Host-Service Handoff

The selected compatibility path is a single frozen EdgeZero PR #275 revision in both applications, not an adapter over Trusted Server's older HTTP transport. [Trusted Server #1507](https://github.com/IABTechLab/trusted-server/issues/1507) owns its SDK/lifecycle cutover and waits on the qualified candidate in #1508. [#1509](https://github.com/IABTechLab/trusted-server/issues/1509) owns host-side contract agreement; [#1510](https://github.com/IABTechLab/trusted-server/issues/1510) owns the concrete service handoff and depends on those leaves. #1505 remains coordination, never an umbrella completion prerequisite.

Auctioneer currently pins `775366fdac923135e8430b1e3dffebedc648ef7c`, while the host cutover names development candidate `8f82547b` and a later qualified frozen merge candidate. This spec does not assert those revisions or the current host tag are compatible. Before #51 is accepted as the host bridge, record the final common immutable revision, review its source/API delta and execute the handoff against it. Adopting a different pin requires the normal dependency review and refreshed full evidence; this planning proposal makes no dependency edit or qualification claim. Portable implementation work can proceed independently, but fake-service tests alone cannot close this handoff gate.

| Required Input          | Selected Concrete Mapping And Existing Owner                                                                                                                                                                                                                                                                            |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bounded outbound        | Host's shared EdgeZero `HttpClient` enters `EdgeZeroFanout::with_wave_scheduler` with its actual timer, decode cooperator, strategy, resolved limits and shared stream gate. No `PlatformHttpClient` compatibility shim, `DevFanout`, second pool or post-buffer-only cap.                                              |
| Time                    | Preserve the host EdgeZero monotonic admission instant/deadline; use the existing `RequestClock`-style portable clock view with the same instant type and one explicit wall-clock capture.                                                                                                                              |
| Credentials             | Explicitly construct the existing bounded credential loader from host-selected `BoundSecretStore` and monotonic clock, rather than extracting another config/secret registry inside preparation. Keep deadline/byte limits and delayed lookup after admission.                                                          |
| Policy and restrictions | Host's existing Settings validator accepts the shared Auctioneer policy subtree; #1510 passes a borrowed `&AuctioneerAppConfig` subtree first to `prepare_auction`, auction input second, and explicit services/funding after them. No complete TS settings object, second loader or policy copy in request parameters. |
| Metadata/references/RNG | Matching discovered registry/bindings and immutable GVL/GCL from Auctioneer; explicit request RNG from the host binding, never an operator-policy singleton or missing-service fallback.                                                                                                                                |
| Funding                 | The same profile/ledger/permit owners used by the accepted host binding. This bridge does not install a second concurrency/transport pool or certify combined-host memory.                                                                                                                                              |

Extend the existing core `handler_wave`/`clock_domain`/credential controls for real SDK-object-to-portable-service handoff, not only a fake `BidderFanout`. Together with host #1510's owning tests, verify exact revision/type compatibility, bounded actual SDK calls, the same clock/secret references, refusal before forbidden I/O and unchanged installed limits. The host tests must also exercise the settings-first subtree call, proving that changes to unrelated TS settings do not become Auctioneer input, while changes to the passed Auctioneer subtree affect the appropriate auction policy. Those host-side test commands and final candidate identity must be recorded by #1510's reviewed implementation; do not invent them here. #51 cannot claim completed host-service binding until that leaf evidence is accepted. Actual live local execution and final Fastly delivery remain #76/#1505 work.

## 11. Source References

- [HTTP assembly and egress](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-core/src/handlers/auction.rs), [accepted SDK extraction](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-core/src/config/runtime.rs), [explicit validation context](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-core/src/config/validation.rs).
- [Account resolution](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-core/src/config/resolve.rs), [explicit host policy](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-core/src/config/policy_host.rs), [scoped budgets](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-core/src/config/budgets.rs), [settings retention](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-core/src/config/bidder_settings.rs).
- [Engine and targeting](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-exchange/src/run/mod.rs), [selection](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-exchange/src/targeting/winner.rs), [typed response assembly](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-exchange/src/response.rs), [borrowed engine context](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-exchange/src/context.rs).
- [Current receipt lifetime](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-runtime/src/auction_budget.rs), [shared budget model](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-runtime/src/budget.rs), [bid/source types](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-bidder/src/types.rs), [shared response classification](https://github.com/IABTechLab/auctioneer/blob/87e329fe88f3a1c15a4d298d026607971b3065ee/crates/auctioneer-bidder/src/bidders/media_type.rs).
- [Host's existing pending HTTP contract](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/platform/http.rs) and [live remote provider](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/auction/provider.rs) explain why an actual host-owned executor change remains necessary.
