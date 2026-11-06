# Trusted Server Fastly Local-Provider Contract

Date: 2026-11-05

Status: Approved contract and delivery decomposition, merged through [PR #152](https://github.com/IABTechLab/auctioneer/pull/152) at `9b83deaa680ad8e867169db3b4ebb195b8a1b33c`. Package-specific API/configuration designs, implementation review and resource/serving acceptance remain required. No integration implementation or deployment is claimed.

Tracking: [Auctioneer #34](https://github.com/IABTechLab/auctioneer/issues/34), under [Direct Trusted Server Integration](https://github.com/IABTechLab/auctioneer/issues/8).

## 1. Decision And Scope

Link Auctioneer into the actual Trusted Server Fastly Compute application and invoke its existing auction engine in process. The first host delivery supports banner/display demand from the build-discovered executable catalog defined in section 3. Auctioneer bidder qualification covers Axum, Cloudflare, Fastly and Spin; the first Trusted Server host integration remains Fastly. Passing one adapter is not a substitute for the other adapters' qualification.

Extend Trusted Server's production, plan-backed auction execution with a typed local-execution contract. Reuse its routing, configuration, consent collection, host services, outer orchestration and creative delivery. Both standalone and embedded Auctioneer use the same preparation, policy resolution, bidder registry, per-bidder privacy enforcement and engine.

The outcome of #34 is a reviewed, source-pinned integration contract and an actionable decomposition. It does not implement the bridge (#51), complete the adapter qualification/serving work (#72-#75 and #88-#90), retire current publication restrictions, or demonstrate the actual linked host (#76). Those outcomes must not be credited from approval of this document.

### Required Boundaries

- One Trusted Server runtime configuration and its existing EdgeZero validation/push/extraction path. No second Auctioneer configuration file, private store, loader, cache or startup policy model.
- No Auctioneer service, sidecar, loopback request, nested HTTP App or runtime-loaded executable plugin.
- No handwritten bidder registry or per-bidder branches in the host bridge. Reuse the existing discovered implementations and typed bidder settings.
- Preserve installed workload limits, resource ceilings and shared accounting. This contract approves no new dependency, numeric limit, reserve or budget equation.
- Reuse build-time bidder discovery. Remove the current `native_only` field and target-dependent bidder filtering as an accepted cutover, not as a bypass of qualification. No replacement per-adapter bidder allowlist is introduced; schema admission, privacy, endpoint safety, request provenance and cancellation remain mandatory.

Full video/native/AMP delivery, response-currency conversion, remaining catalog migration and live publisher qualification remain with their existing owners. Supporting a bidder's display path does not claim its other media or complete source parity.

## 2. Source Baseline And Actual Gaps

The baseline is immutable. A later source change requires an explicit contract delta and refreshed affected evidence, not a mutable branch-tip citation.

| Component                          | Pinned Source                                                             | Relevant Ownership                                                                      |
| ---------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Auctioneer                         | `f307289269f7c510d86647a9f6b673058b74e746`                                | Shared direct runtime policy, canonical preparation, engine and bidder implementations  |
| Trusted Server                     | `51730bdaf841e4d7bb96c2258e5e98169bdff9a7`                                | Production provider-plan compiler, routing, orchestration, Fastly services and delivery |
| Auctioneer EdgeZero dependency     | `775366fdac923135e8430b1e3dffebedc648ef7c`                                | Current typed CLI/configuration and bounded outbound integration                        |
| Trusted Server EdgeZero dependency | tag `v0.0.11`, resolved commit `87456c708ffe9edf86b3a0f8b06dfd15d60b07b5` | Actual host SDK dependency, not assumed API-compatible with Auctioneer's pin            |

Verified existing behavior:

1. Auctioneer's `auctioneer-exchange::run_auction` and `auctioneer-openrtb::prepare` are public. However, effective account/configuration resolution, plan assembly and service/resource binding still occur inside the HTTP-specific `auction_inner` path in `auctioneer-core`. Public engine access alone is not a reusable application preparation boundary.
2. Trusted Server's live `AuctionOrchestrator::from_plan` constructs `GenericOpenRtbProvider` instances. `register_provider` is `pub(crate)` and `#[cfg(test)]`; it is not a production registration API.
3. Trusted Server's `ProviderConfig` requires an endpoint and its plan compiler accepts remote protocol/profile declarations. A local executor cannot be represented honestly by a fabricated HTTPS endpoint.
4. Trusted Server already has one typed Settings path, a compiled bidder-to-provider routing plan, Fastly runtime services and split dispatch/collection. These are foundations to extend, not missing systems to rebuild.
5. Current bidder manifests include 13 executable factory declarations marked `native_only`. The publication generator and config-data build filter their public bindings from guest targets. These are source-baseline restrictions to remove through qualification, not the desired permanent eligibility model. Implementation files or candidate-factory compilation alone do not establish guest serving.
6. Trusted Server currently chooses provider candidates by decoded CPM, retains the first equal-priced candidate, and reconstructs delivery targeting. Its `Bid` has no first-class targeting map. Auctioneer's local selection can prefer deals and has its own stable ordering; passing all local bids through the unchanged host reducer would not preserve that selection.
7. Split host execution carries pending HTTP operations keyed by backend name. The current Fastly deadline collector does not accept an arbitrary local auction future merely because it is wrapped in a pending-request container.
8. Build-time discovery already exists: `auctioneer-bidder/build.rs` scans bidder-owned manifests and generates the registry, settings, endpoints and conformance wiring. The shared catalog/discovery code validates public definition-to-factory ownership. This contract reuses that system rather than inventing a registry or treating historical delivery lists as its source of truth.
9. Schema selection is not yet uniform: `schema_staging_build.rs` still reads manually selected delivery-list inputs, and schema admission/initialization in the bidder and core bootstrap paths has native-only behavior. The qualification cutover must remove those selection and execution differences as well as the manifest flag; changing registry generation alone would leave an unusable guest validator path.

These observations come from committed source inspection. They are not new runtime, allocation or hosted test results. Source references are collected in section 13.

## 3. Discovered Catalog And Adapter Qualification

### One Build-Time Source Of Truth

Enumerate the whole catalog through the existing bidder-owned manifests, complete import/provenance inventory and validated identity/alias relationships, including entries that have no Rust implementation. Reconcile local definitions into that inventory. This is complete source coverage, not a second routable registry or a requirement to retain reference-only inventory in the deployed application.

Discover executable implementations from `crates/auctioneer-bidder/src/bidders/*/bidder.toml` with `factory = true`, and derive implemented public identities/aliases through their declared, validated metadata-to-factory bindings. Preserve identity case and alias ancestry. Report every catalog entry for every adapter: a reference-only identity with no Rust implementation/public-definition declaration is `skipped` with reason `not_implemented`, never omitted or counted as qualified. This includes an unported source alias even if its capture shares an owner with existing Rust code; source association is not implementation. Once a Rust public definition declares an executable binding, a dangling factory, missing generated registration, validation, resources or target support is failed/blocked, not eligible for that skip.

Skipped entries do not become serving definitions. Runtime configuration cannot activate them or turn their reference metadata into an executable bidder; reject that attempted activation through the existing validation/resolution path before bidder I/O.

For a selected Cargo feature graph, the required serving/qualification set is every applicable implemented public identity/alias, before operator activation or target filtering. Derive it from source declarations, never a generated registry that could already have lost a row. Existing feature requirements choose the applicable verification lane; a disabled security feature or an operator-disabled bidder is not an unimplemented bidder.

Do not introduce a second registry, handwritten bidder list or delivery-cohort manifest. Historical capture origin and attribution remain provenance only, not a qualification partition or serving predicate. Existing feature-owned security boundaries such as `insecure-bidders` remain intact and are tested in their applicable feature lanes; they are not adapter-selection controls. Operators enable/disable already-compiled bidders through the single runtime configuration, without rebuilding the binary.

At the pinned source revision there are 262 bidder manifests, 17 executable implementations and 18 declared public definitions across feature lanes. Report these different populations separately; manifest counts are not public-identity counts. These are observations, not hardcoded acceptance counts: a later bidder or alias must enter complete inventory reporting automatically, and an implemented public declaration must enter generated registration and qualification. Build discovery remains build-time work, not runtime plugin loading. Do not add another full qualification run to every local build; the existing mandatory acceptance owners enforce the release contract.

### Four-Adapter Contract And Flag Retirement

For Axum, Cloudflare, Fastly and Spin independently, prove that actual metadata, generated bindings and the serving registry equal the discovered required set for the same Cargo features. Qualification covers each implementation and public alias, actual parameter validation, admitted protocol behavior, privacy, errors, cancellation, bounded work/storage, initialization and target-applicable resource fit. Native tests, metadata enumeration, compile-only checks and another adapter's measurements cannot substitute for executed target acceptance. The hosted Spin resource contract remains the selected Akamai Functions contract, not an assumed generic Spin ceiling.

The accepted implementation removes `native_only` from bidder manifests, manifest/config models, publication generation, registry declarations, storage accounting and target filters. Reject the retired field rather than accepting a legacy alias. Do not replace it with per-adapter lists, special-case bidder names or a new target flag. Derive schema selection from the same discovered definitions and bidder-owned schema/provenance declarations, and replace native-only schema availability/initialization/admission with the qualified adapter-bound path. Reuse the current schema compiler and typed code generation rather than building a second validator. Retiring a publication flag does not retire resource or privacy enforcement.

Qualify the actual proposed production artifacts after making the candidate cutover locally, before merging or publishing it. This avoids requiring the old filtered registry to prove behavior it cannot expose, without accepting a direct-factory bypass as evidence. If any implemented adapter/identity cell lacks evidence or fails, the release cutover remains blocked; do not hide that cell, trim the discovered set or declare only a native release complete. Verified `skipped: not_implemented` entries remain visible but do not block qualification of the implemented set. This permits completing qualification without claiming the entire catalog has been implemented. Future bidder implementations carry the same four-adapter contract before release.

Use exact set comparisons with an independent source/provenance walk and existing discovery reconciliation. Whole-catalog report coverage and implemented serving coverage are separate sets, both checked exactly. Missing or extra identities, stale aliases, an unvisited definition, a newly implemented factory left marked skipped, an unjustified skip, a reintroduced target predicate or use of a delivery list must fail a control. Preserve security-feature separation and prove both positive serving and refusal paths; all-refusal results cannot qualify a bidder.

### Host Delivery And Bounded Coverage

The initial Trusted Server integration remains Fastly-first and banner/display-only. Its required display identities are derived from the same executable public definitions and their declared display capabilities, not another list. A bidder's other media capabilities do not imply new host player, renderer or cache support. Resource/portability qualification does not claim live vendor authorization or complete media-delivery parity.

Exercise every implemented display identity individually and cover that complete execution set across deterministic batches within existing call/impression limits. Unimplemented entries remain explicit report rows and produce no bidder I/O. Also exercise the installed concurrency boundary and one-over refusal. The catalog may exceed one auction's call limit; do not require every implemented identity in one request, increase a cap to fit the catalog, or reduce coverage to fit one batch. The same actual artifact and source-bound inventory must cover all batches, with exact sent-impression and alias attribution.

Use existing implementation-declared endpoint, region, settings and secret contracts. No vendor-issued credential or live traffic is required for deterministic fixture acceptance; live authorization remains a separate requirement. Do not invent a user-sync declaration or callback behavior for an identity that has none.

## 4. Component And Dependency Ownership

| Owner                                         | Responsibility                                                                                                                                 |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Trusted Server plan compiler and orchestrator | Provider identity, local/remote assignment, outer scheduling, mediation, final provider competition and host delivery                          |
| Trusted Server configuration integration      | One accepted Settings snapshot containing the shared Auctioneer operator policy and existing host secret/resource references                   |
| Auctioneer shared preparation                 | Canonical request validation/normalization, effective host/account/bidder policy, alias resolution and construction of the engine plan/context |
| Auctioneer engine and discovered bidders      | Bidder fan-out, per-bidder privacy projection, protocol behavior, bid admission, local targeting and typed results                             |
| Fastly host/SDK integration                   | Actual bounded transport, host clocks, cancellation/abandonment behavior, reference/service capabilities and terminal response ownership       |
| Host bridge                                   | Typed adaptation between these existing contracts; no alternate auction, privacy, configuration or protocol implementation                     |

Keep the portable Auctioneer engine free of Trusted Server and provider SDK dependencies. Keep the local-executor implementation outside Trusted Server's neutral orchestration types; its core needs an execution contract, not bidder names or Auctioneer internals. Start within existing packages. A new package is not a prerequisite and needs a concrete dependency/ownership justification if later proposed.

The production execution representation distinguishes remote transport from a local executor. Only remote execution carries endpoint/backend/profile transport data. Provider identity, bidder routing and logical timeout remain common plan concerns. The final public Rust signatures and serialized field names are defined in the reviewed bridge/host implementation design; this contract fixes ownership and observable behavior, not a pretend SDK API that already exists.

## 5. One Configuration And Shared Preparation

The operator supplies one Trusted Server configuration through its existing typed EdgeZero flow. The local-provider declaration carries the shared Auctioneer operator policy as a typed part of that Settings model. Secret values stay in existing host secret stores; configuration contains the established references, not copied credentials.

Use Auctioneer's actual policy declarations, discovered bidder-owned settings and common validators/defaults. Do not maintain a second host-side copy of the Auctioneer policy schema, use untyped JSON as an escape hatch, or require an Auctioneer-specific push invocation. The same authoring and resolved-secret validation boundaries must apply inside the host root.

Application preparation must become reusable outside the HTTP action:

1. The HTTP adapter retains ownership of HTTP body/header reads and final serialization.
2. Both callers use the existing canonical preparation and the shared effective-policy/plan resolution. Do not copy `auction_inner` into the bridge or expose the HTTP handler as an internal RPC.
3. The embedded caller supplies admitted typed input, an accepted policy snapshot, original timing and real host services/reservations. A typed input is not automatically bounded: its depth, width, construction and retained storage still need the existing applicable checks.
4. The prepared request owns or borrows exactly the state needed for execution. Drop unrelated account overlays before fan-out as on the existing path. Keep the accepted policy/secret/resource owner alive for every borrow.

Installed host extraction/admission/trust guards continue to bound their own inputs and cannot be raised by the policy being extracted. Immutable catalog/reference data may remain shared; operator/account policy must not be cached process-globally or leak across Settings snapshots.

SDK input hardening tracked by #142 is nonblocking follow-on work in EdgeZero. It is not permission to add another loader, and it does not waive runtime extraction or host admission requirements.

## 6. Typed Request And Permission Mapping

The bridge consumes only slots assigned to the local executor by the production routing plan. It must preserve canonical request/impression identity and cannot rediscover demand by scanning the raw browser payload after routing.

| Input                            | Contract                                                                                                                                                                   |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Publisher/account                | Resolve from the trusted host configuration and established account-selection rules; client fields cannot choose another tenant or bypass disabled/required-account policy |
| Slots and parameters             | Preserve exact slot identity, offered banner formats, floor metadata and selected bidder parameters; validate with the same canonical preparation and bidder validators    |
| Bidder/alias ownership           | Each requested identity has one provider owner; resolve the existing alias ancestry and runtime activation rules without expanding a second handwritten bidder list        |
| Consent                          | Carry the authoritative request-bound raw signals/provenance needed by Auctioneer; host decoded permission is not a replacement for per-vendor/purpose enforcement         |
| EC, UID and EID values           | Use only the host's selected, permission-gated sources; do not open a second cookie/store path or restore identifiers removed upstream                                     |
| Device, geo and first-party data | Preserve the existing disclosure classification and activity gates; a host-provided field does not gain permission by bypassing HTTP parsing                               |
| Request mode/debug               | Set trusted entrypoint/context explicitly; public request fields cannot manufacture host trust or bypass account/debug permission                                          |

Effective disclosure is the intersection of host restrictions and Auctioneer's existing per-bidder policy. Host denial cannot become a grant through remapping; host permission cannot override Auctioneer denial. Missing, malformed and conflicting signals retain the common resolver's precedence and fail-closed behavior rather than an embedded-only interpretation.

Do not stringify and reparse a whole HTTP request to cross this boundary. Necessary bidder wire serialization remains owned by the existing protocol path. Preserve missing/null/value distinctions and original extension-shape provenance where the engine consumes them; a typed reconstruction must not falsely claim raw-wire provenance.

## 7. Production Execution And Lifecycle

The intended flow is:

```text
Trusted Server accepted Settings and host request
  -> existing compiled provider plan and demand routing
  -> local executor with assigned input and original deadline
  -> shared Auctioneer preparation and run_auction
  -> typed local results and completion
  -> existing Trusted Server competition and creative delivery
```

The local executor is a production execution kind in the live plan, not an entry in the test-only provider map. Its implementation must cover Settings-to-plan compilation, target validation, live orchestrator construction and renderer/capability registration, not just implement the existing provider trait. Unavailable execution support, missing resources and unqualified guest definitions fail before bidder dispatch.

Preserve both ordinary auction execution and the existing split dispatch/collect path used around publisher-origin work. Starting local work must launch eligible bidder operations without waiting for their complete auction result before unrelated providers/origin work can progress. Collection joins the already-started operation; it cannot restart it or send the same bidder twice.

An owned pending local operation retains its admitted request, snapshot and reservations across collection. Both ordinary execution and split execution use this same start/collect/abandon lifecycle. A local operation is not a `PlatformPendingRequest` unless a reviewed production execution representation actually supports it; the current Fastly HTTP-handle collector does not. Do not fabricate a network response/pending backend solely to fit a remote-only type, or use a blocking executor that prevents its own async work from progressing. Retain existing early available responses and the host's selected completed-snapshot behavior on deadline expiry, with no additional network wait for unfinished work. Deadline equivalence is about the original bounded wait and retained completed results, not a new claim about upstream arrival-time eligibility.

Use the original host auction deadline, capped by the existing provider/engine policy. Define one explicit monotonic-domain mapping; cloning context, entering Auctioneer or entering collection must not reset the start time or provide a fresh timeout. Already-expired work performs no bidder dispatch, credential lookup or newly funded parsing.

Cancellation/drop releases local admission and funded owners exactly once. Distinguish provider-request abandonment from actual backend cancellation: do not promise Fastly can cancel a request where its SDK only permits abandonment. No background operation may retain request policy or charged memory after its owning completion without an explicit accounted host lifetime.

## 8. Host Services And Combined Resource Ownership

Reuse the existing portable interfaces for clock/RNG, bidder fan-out, secret/credential access, reference data and reservations. The bridge must carry real ownership, not install permissive defaults because an in-process caller is trusted.

The Fastly path requires the bounded outbound semantics used by the production Auctioneer path: exact per-call attribution, original deadlines, encoded/decoded/body/header limits, decoder/work admission and cancellation/completion rules. `DevFanout`, post-buffer body-size checks or an adapter capability constant alone cannot establish this contract.

The pinned host and engine currently use different EdgeZero revisions. The bridge design must record the actual compatible SDK/API path and prove its service handoff. Do not cast request contexts across SDK versions, assume duplicated SDK types are interchangeable, or create a second independent transport/admission pool. An SDK alignment or narrowly scoped host hook belongs to the reviewed implementation work, not an unrecorded prerequisite.

The actual linked Trusted Server application is the accounting/qualification subject. Account once for shared immutable data and explicitly for every additional live owner: host Settings and request data, bridge construction, registry/schema/reference residency and initialization, concurrent engine work, host creative transformations, output buffers and retained completion state. An Auctioneer standalone total is not the combined application's budget.

Response/creative ownership lasts through Trusted Server's final serialization and terminal egress, not merely until the engine future returns. The bridge cannot release a reservation while host delivery still holds its funded storage. Preserve the shared checked accounting model, installed limits and required headroom. Any changed reserve, workload, ceiling or equation requires separate review with independent evidence.

## 9. Typed Results, Competition And Errors

Return existing typed engine data through a reviewed, lossless projection, not a serialized Auctioneer HTTP response or an unbounded JSON metadata tunnel. The shared result must expose the actual local winner/tie facts from the same engine reduction; the current `BidResponse` return alone is not a claim that a separate typed selection contract already exists. Keep provider identity distinct from requested bidder/alias and returned upstream seat. Preserve request/slot/bid identity, admitted price/currency, media, creative, notification fields and applicable targeting/delivery metadata.

There are two explicit decision scopes:

- Auctioneer owns bid validity, per-bidder policy and local auction/targeting semantics for its assigned demand.
- Trusted Server owns competition between provider results, mediation and final creative delivery.

For outer competition, the local provider offers Auctioneer's selected candidate per slot, not a new CPM-only reduction over all its accepted bids. Keep the other admitted local candidates and per-bidder outcomes available through the bounded typed result for their existing response/diagnostic purposes; they are not additional outer contenders. This preserves local deal preference and tie order without teaching the host reducer Auctioneer's selection rules. Existing host mediation may select among the offered provider candidates; it does not re-enter the local bidder set. A change to global cross-provider selection policy is outside this contract.

Do not run a second implementation of bidder validation, price bucketing or Auctioneer targeting in the bridge. Extend the host's typed delivery projection to carry the selected local bid's targeting, rather than hiding it in metadata that current consumers ignore. Both `auction::formats` for direct `/auction` delivery and the publisher's `build_bid_map_with_auction_id` path must consume it. Remote providers keep their existing targeting projection. The selected bid's identity, dimensions, notifications and any host-added auction identifier remain explicit; a losing local candidate cannot leave winner-scoped targeting on another provider's winner.

Compare standalone and embedded equivalence at the shared engine boundary using identical inputs and resolved policy: accepted local candidates, selected local winner/ties and targeting must agree. Host floors, configured mediation and creative processing remain additional explicit outer decisions, not an embedded-only change to engine policy. Exercise their rejection/selection outcomes separately; do not claim final delivery is byte-identical to standalone when those host decisions differ.

Preserve the host's existing creative safety/rewrite behavior and delivery-rejection reporting. This contract adds no automatic runner-up retry when the selected creative is rejected; it must not report that creative as delivered or turn delivery failure into clean no-bid.

The initial contract retains existing supported currency behavior; it introduces no response FX capability. Do not compare incompatible currencies as if their numeric amounts were interchangeable. Preserve applicable provider-specific display metadata and existing browser-auction output rather than inventing creative/cache/renderer features. A required bidder whose valid display result cannot be delivered is not accepted merely because its request was sent.

Capability registration must follow the compiled local execution declaration and actual typed delivery requirements, not only a remote profile name. At the pinned host revision, APS renderer support is registered when an APS profile exists; placing its demand under a sole local provider does not automatically retain that registration. The bridge must use Auctioneer's actual APS bidder contract, not feed it through the host's different remote protocol parser. Demonstrate compatible display delivery, and explicitly wire any required existing typed renderer capability into the local plan. Do not invent a renderer from the bidder name or accept arbitrary executable descriptors. This is a named #76 delivery requirement, not an assertion that the current integration already supplies it.

| Outcome                                         | Required Handling                                                                                                |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Invalid host configuration/routing or admission | Refuse before affected provider I/O; do not reinterpret as an empty successful auction                           |
| Invalid request/account or fatal signal input   | Preserve the shared validation/policy outcome and bounded diagnostic; do not retry through a second ingress path |
| Valid no-bid                                    | Preserve no-bid as a distinct successful outcome                                                                 |
| Bidder-local failure                            | Keep valid sibling bids and exact attribution; no whole-response abort or fabricated no-bid success              |
| Local provider execution failure                | Retain the provider error category; independent remote providers may continue under existing host policy         |
| Deadline/cancellation/overload                  | Preserve the original cause and resource cleanup; do not silently restart or manufacture a new budget            |
| Creative/delivery rejection                     | Preserve host delivery diagnostics; do not report an undeliverable bid as delivered                              |

Expose only established bounded/sanitized diagnostics. Do not log or return raw consent, IDs, credentials, rejected config values or upstream-controlled error strings. Existing debug, observation and notification permissions remain in force.

## 10. Acceptance And Evidence

These are required future controls, not executed results. Keep unit tests inline, conformance with its owner, and process/allocator isolation where existing state requires it. Shared infrastructure tests use capable fake bidders/services. Real bidder protocol cases stay in the bidder-owned fixture/conformance layout; artifact membership coverage is derived from build discovery rather than duplicated shared test branches.

| ID  | Observable Control                                                                                                                                                                                                                                                                                                                                   | Primary Owner                            |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| C1  | Both standalone and embedded callers reach the same preparation/engine policy with deterministic clock/RNG and captured bidder replies; vary account, alias, privacy and settings independently                                                                                                                                                      | #51                                      |
| C2  | The real compiled host plan starts local execution; test-only registration, a fake endpoint and loopback dispatch are absent                                                                                                                                                                                                                         | Host plan task linked to #51             |
| C3  | One unchanged host artifact uses two isolated runtime policies and secret references; changes/invalid input cannot leak across host/account snapshots                                                                                                                                                                                                | #51 and actual host acceptance #76       |
| C4  | Whole-catalog reports cover every identity/alias and adapter, explicitly skipping only verified missing implementations; every implemented identity has positive serving in its feature lane, exact source/metadata/binding/registry coverage and no `native_only` or replacement target filter                                                      | #46, #88-#90 and existing adapter owners |
| C5  | All implemented identities are covered across bounded deterministic execution batches using the same artifact; unimplemented rows cause no bidder I/O, and installed concurrency/one-over refusal retain exact impression and alias ownership without changing limits or duplicating dispatch                                                        | #76 and existing adapter owners          |
| C6  | Direct `/auction`, split publisher dispatch/collect and current page-bids callers use the same live execution path; delayed origin/sibling responses cannot restart the deadline or starve local work; expired collection retains the permitted completed snapshot without further waits                                                             | Host plan task and #76                   |
| C7  | Host deny/engine allow and host allow/engine deny both remain denied at the appropriate boundary; inspect serialized bidder calls for ID/geo/FPD/TID removal and retained context                                                                                                                                                                    | #51 and #76                              |
| C8  | Under/at/over installed byte/node/call/work caps, zero remaining deadline, delayed headers/body, partial bid failure, cancellation and abrupt drop retain correct outcomes and release owners exactly once                                                                                                                                           | #51 with existing transport owners       |
| C9  | Local deal preference and equal-price ordering survive outer competition; both direct and publisher delivery use selected-local targeting and exact bidder/alias identity; remote winners have no stale local keys; creative rejection does not produce false success; APS display/capability registration works without a remote-profile assumption | #76                                      |
| C10 | Each adapter's actual artifact, SDK/features/config identities, applicable stack and resource owners satisfy its fit/serving requirements; the linked Trusted Server Fastly artifact also accounts for combined memory/egress ownership; another adapter or compile-only pass is not a replacement                                                   | #72-#75, #88-#90 and #76                 |

Each negative control must reach the branch it claims to test. Assert before-state presence for removals, nonempty dispatch/candidate populations, and exact covered membership. Do not hold independent controls equal, compare an output only against itself, or infer serving from publication metadata. Reuse matching current evidence instead of making a second test harness for the same boundary.

Use existing adapter, Fastly/Viceroy, Rust qualification, transport and bidder conformance owners where they match the boundary. Actual Cloudflare Worker and Spin component execution need their applicable runtime controls, not an inferred pass from the WASI publication test. Real component execution, deterministic fixture dispatch, allocation observations, worst-case accounting and hosted qualification are separate evidence classes. The implementation plan must name exact commands, feature lanes and dispatched targets; `--all-targets` does not satisfy missing `required-features`.

Hosted corruption/restoration, live bidder authorization and publisher rollout retain their existing acceptance owners (#99/#100/#102 and the selected pilot). No private deployment or temporary config mutation is authorized by this contract.

## 11. Dependencies And Focused Delivery Tasks

Do not implement all architecture work in the #34 contract PR. Keep the existing integration epic and reuse these owners:

| Work                                                        | Owner / Relationship                                                                                                                                                                                     |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reviewed source-pinned host contract and decomposition      | #34; this document                                                                                                                                                                                       |
| Shared typed preparation and host bridge                    | #51; split implementation into reviewable preparation and service/result-binding tasks without inventing another configuration API task                                                                  |
| Production local executor in Trusted Server's compiled plan | [Trusted Server #1505](https://github.com/IABTechLab/trusted-server/issues/1505) coordinates the host-owned work linked to #51; reuse its existing scope before proposing a narrower implementation task |
| Shared discovery/schema qualification and flag retirement   | #46/#47 with the existing build/discovery owners; no new registry or delivery-cohort source                                                                                                              |
| Actual per-adapter resource fit                             | #72 Axum, #73 Fastly, #74 Cloudflare and #75 hosted Spin, including their existing schema, decoder, lifetime and SDK-fact prerequisites                                                                  |
| Qualified guest schema/registry serving                     | #88 Fastly, #89 Cloudflare and #90 Spin; coordinate the common flag-retirement cutover only after the four-adapter candidate artifacts pass                                                              |
| Actual linked host artifact and discovered display demand   | #76, after #51 and applicable qualification/serving acceptance; the first host artifact remains Fastly                                                                                                   |
| Selected hosted/publisher deployment evidence               | Existing selected-path/pilot issues; not completion of unrelated providers                                                                                                                               |

Qualification/serving requirements gate the accepted uniform catalog and flag retirement, not writing/reviewing #34 or independent bridge design. Their open requirements must not be hidden until final integration. A staged artifact may remain explicitly unqualified and fail closed, but cannot be labeled a qualified adapter or an accepted host integration. The wider Auctioneer bidder matrix does not add Trusted Server integration on its other hosts to this initial Fastly delivery.

### Contract Tracking Reconciliation

The source revisions in section 2 and the planning merge above are unchanged. The existing [Trusted Server #1505](https://github.com/IABTechLab/trusted-server/issues/1505) already owns SDK alignment, host services and local-provider integration; no duplicate integration issue is needed. Its umbrella completion references Auctioneer #34/#51/#76/#91, so do not make those issues depend unconditionally on completion of that umbrella: that would introduce a dependency cycle. Record a concrete reviewed host prerequisite on its owning task when one is established, while keeping the umbrella as coordination.

The following reconciles existing ticket acceptance with the approved contract. It is not evidence that the corresponding production change or qualification has passed. A ticket's existing priority, size, status and assignment are not changed by this map.

| Existing Owner       | Required Acceptance Reconciliation                                                                                                                                                                                                                                             |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| #34                  | Accept the source-pinned contract and decomposition independently of downstream delivery; planning merge alone does not close it.                                                                                                                                              |
| #51                  | Review the shared preparation/result API and lifetime owners before code; preserve one runtime policy, actual local selection, original deadlines and host/engine privacy intersection.                                                                                        |
| #46/#47              | Derive schema selection from discovery rather than delivery cohorts; qualify residency, initialization, work, scratch and stack without treating unimplemented source identities as callable.                                                                                  |
| #72-#75              | Qualify every declared implemented identity/alias on the corresponding actual adapter artifact, using exact complete-catalog reports and explicit `skipped: not_implemented` rows; preserve target-specific fit prerequisites.                                                 |
| #88-#90              | Remove `native_only` and target-based publication/schema-admission selection through the common qualified cutover, not by a direct-factory bypass or a replacement allowlist. All four adapter candidates must pass before that cutover is accepted.                           |
| #76                  | Require the shared bridge and actual Fastly serving path; prove local execution, original-deadline start/collect/abandon, typed targeting in direct and publisher delivery, and combined host-resource ownership. Cloudflare/Spin host integration is not added to this issue. |
| Trusted Server #1505 | Reuse the existing host-owned SDK/local-executor/integration scope. Freeze the concrete API/configuration and independently review its implementation there; no test-only registration or fabricated remote endpoint.                                                          |

The 12 existing Auctioneer ticket bodies above have been updated and read back against this contract. Their types, status, priority, size, assignment and milestones were preserved; no issues were created. #76's native blocked-by links now name #51 and #88 for the selected Fastly host. This records tracking reconciliation, not downstream acceptance. Keep unrelated acceptance criteria and project fields intact in future updates. Whole-catalog four-adapter release acceptance remains distinct from the Fastly host's own dependency path; do not add Cloudflare or Spin host dependencies solely because bidder portability is required.

Trusted Server #1336 concerns wider protocol modularity. This contract keeps the new executor boundary independent of a wholesale rewrite; record and coordinate any actual interface change without assuming that entire workstream blocks Fastly integration. Trusted Server #1484 concerns multi-provider collection on Cloudflare/Spin and is not a Fastly prerequisite. Revisit that relationship only if the selected host interface changes.

#135 is superseded by #51/#76. #142 remains EdgeZero-owned nonblocking CLI hardening. Neither becomes a new parallel configuration system or an additional integration gate. Existing priorities/sizes are not changed by this document; candidate task creation and cross-repository ownership require approved issue tracking rather than a local status ledger.

## 12. Review And Publication Gates

At AG's explicit request, this written contract and its [delivery plan](../plans/2026-11-05-trusted-server-fastly-local-provider.md) are reviewed together in one planning PR. This changes the document-review sequence, not the implementation or merge gates. The planning PR must contain both documents, use `Refs #34`, and remain distinct from subsequent implementation PRs. Implementation requires approval and merge of the planning PR under PRfection. Do not close #34 or claim downstream acceptance merely by drafting or opening the plan.

Record exact executed documentation checks separately from the future C1-C10 evidence. The full prescribed current-head suite is required at the planning/implementation review handoff; an unchanged-code baseline or targeted document check is not that full suite. Do not weaken tests or required checks to accelerate the handoff.

## 13. Primary Source References

- [Auctioneer policy composition and shared validation](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-core/src/config/mod.rs), [canonical preparation](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-openrtb/src/prepare/mod.rs), and [portable engine](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-exchange/src/run/mod.rs).
- [Current HTTP preparation/resource assembly](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-core/src/handlers/auction.rs) and [production bounded fan-out binding](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-core/src/host_primitives/edgezero_proxy.rs).
- [Existing build-time discovery and generated registry](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-bidder/build.rs), [shared discovery/reconciliation](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-config/src/bidder_discovery.rs), and [current publication predicate to retire](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-config/src/codegen/mod.rs).
- [Current schema-selection build inputs](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-config/src/schema_staging_build.rs), [native-only schema admission](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-bidder/src/schema/native.rs), and [installed schema-funding gate](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/crates/auctioneer-core/src/host_primitives/gates.rs).
- [Trusted Server production plan/compiler](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/auction/plan.rs), [orchestrator](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/auction/orchestrator.rs), and [existing provider path](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/auction/provider.rs).
- [Host request/result types](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/auction/types.rs), [formats/delivery](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/auction/formats.rs), [publisher targeting/delivery](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/publisher.rs), and [APS renderer registration](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/integrations/aps.rs).
- [Host service container](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/platform/types.rs), [HTTP service contract](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-core/src/platform/http.rs), and [Fastly deadline/HTTP-handle implementation](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/crates/trusted-server-adapter-fastly/src/platform/deadline.rs).
- [Trusted Server SDK lock](https://github.com/IABTechLab/trusted-server/blob/51730bdaf841e4d7bb96c2258e5e98169bdff9a7/Cargo.lock), [Auctioneer SDK lock](https://github.com/IABTechLab/auctioneer/blob/f307289269f7c510d86647a9f6b673058b74e746/Cargo.lock), and the [approved product embedding boundary](../../../vision.md#two-delivery-modes).
