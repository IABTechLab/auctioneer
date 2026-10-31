# Auctioneer Roadmap

Updated: 2026-10-30
Status: Approved milestone ordering, including M7.5/M7.6/M7.7; future milestones are planned, not implemented

Auctioneer's next priority is bidder foundations and all 14 publisher-priority
flows under M7, Trusted Server integration/testing under M7.5/M7.6, then
additional demand-led bidder waves under M7.7 before the shared-parity program,
whose emphasis remains full video support. The user-approved
[2026-10-27 sequencing correction](#2026-10-27-post-pilot-bidder-expansion)
brings that expansion forward from M20; M20 retains remaining catalog closure.
The [2026-10-26 scope correction](#2026-10-26-scope-correction) still limits M7
to foundations and its 14 priority flows. The user-approved 2026-10-15
correction moves credentialed live execution and the limited-bidder publisher
pilot to M7.6 after M7.5; all other initial M1-M6 requirements and existing
privacy and security protections remain unchanged.

The [product vision](../vision.md) sets the OpenRTB-first direction and the
standalone/embedded delivery goals. The
[master design](superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md)
owns the initial behavioral contract and intentional differences. The
[architecture document](architecture.md) describes the current implementation;
this roadmap describes future work. New implementation needs a reviewed
specification and plan. M7.6 executes the retained M6 harness/runbooks and
acceptance contracts under section 8; it does not need a new implementation
specification merely to run those checks.

M6 implementation and local verification are merged into `main` through PR #19.
The [verification record](testing/evidence/m6-verification-20261015.md)
includes 45/45 passing native local acceptance rows and the retained limitations.
It grants no live publisher readiness or overall deployment qualification.

## 1. Priorities and Compatibility

1. Preserve the merged M6 hardening contracts and recorded evidence limitations
   without expanding them into the entire deferred feature backlog.
2. Preserve M7's delivered foundations and all 14 priority selected flows,
   including Sharethrough, Rubicon and APS, under the approved controlled-native
   resource contract below. The M7 branch's verification is recorded separately
   from R0 qualification and live publisher readiness.
3. Deliver direct Trusted Server embedding under M7.5, using the selected
   publisher's bidder set without waiting for full feature parity.
4. Execute hosted acceptance and a limited-bidder publisher pilot under M7.6,
   using the selected M7 migration wave and M7.5 integration/host setup, not
   waiting for full catalog closure or video parity.
5. Expand bidder coverage under M7.7 immediately after the selected Trusted
   Server/publisher test path passes, in smaller reviewed demand-led waves.
   Do not wait for M8-M19 or M20 to add bidders supported by current contracts.
6. Complete the foundations and delivery paths for video under M8-M11.
7. Complete the remaining shared parity surfaces under M12-M19.
8. Complete residual integrations and full-catalog reconciliation under M20,
   including integrations that need later shared features.

These numbers express priority and ownership, not a requirement to wait for
every unrelated deployment qualification before beginning the next design.
Work can overlap when its contracts and dependencies are settled. Privacy,
security, and correctness fixes are never postponed to preserve roadmap order.

The current behavioral reference is the pinned upstream commit
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`. M7 must record an exact catalog
revision and inventory instead of treating an approximate adapter count as its
completion criterion. Any baseline update must be explicit, with corresponding
fixtures and compatibility decisions. Newer standards or policy support must
identify their own reference versions rather than claim parity with an older
baseline that does not implement them.

Parity does not silently undo the master's intentional differences, such as
privacy hardening, HTTPS-only defaults, or deterministic output. Each owning
milestone must say which differences it closes and which it preserves.

## 2. Milestone Ownership

| Milestone                                                      | Scope                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Main dependencies                                                                                                                                                                                                                       |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **M5: User Sync Integrity**                                    | Existing sync endpoints, bounded cookie/selection/template/privacy contracts, plus versioned MAC envelopes, atomic secret-store rotation, unsigned-state rejection and browser-sized transactional writes. Signing does not authenticate callback UID ownership.                                                                                                                                                                                                                         | M4 privacy and existing EdgeZero handlers, secrets, admission and egress.                                                                                                                                                               |
| **M6: Parity Hardening**                                       | [M6 hardening design](superpowers/specs/2026-10-05-m6-parity-hardening-design.md) and [implementation plan](superpowers/plans/2026-10-07-m6-parity-hardening.md): bounded safe debug capture/enablement, current typed-overlay and structural-budget error precedence, snapshot/update/failure/recovery tests, explicit residual parity inventory, offline provider harness/artifact handoff/refusal tests and runbooks, and complete native Q2-A/B/C plus reusable Q2-E (section 10.1). | Initial M1-M5 contracts and applicable EdgeZero interfaces; local/staging acceptance is not live publisher-pilot readiness.                                                                                                             |
| **M7-R0: WASM Resource Envelope Remediation**                  | [Design](superpowers/specs/2026-10-17-m7-r0-wasm-resource-envelope-design.md) and [plan](superpowers/plans/2026-10-17-m7-r0-wasm-resource-envelope.md), included in the M7 PR: audit pre-existing over-ceiling envelopes, review explicit workload/lifetime decisions, and prove the same additive demand used at boot fits with a 6.25% headroom floor. Not catalog work or credentialed deployment execution.                                                                          | Existing runtime/core/provider equations and target-specific observations; may run alongside M7, but blocks the affected M7.6 deployment/readiness claims.                                                                              |
| **M7: Bidder Foundations and 14 Priority Flows**                | Complete inventory/import, automatic registration, aliases/configuration, schema/protocol/resource foundations and full selected-flow acceptance for all 14 priority identities, including Sharethrough/Rubicon/APS. Retain priority-local branch requirements, all-target catalog-delta gates and native fit. See the [scope correction](#2026-10-26-scope-correction). | Existing bidder, registry, privacy and runtime-config contracts. M20 full-catalog closure is not an M7 prerequisite; WASM runtime readiness additionally requires M7-R0. |
| **M7.5: Trusted Server In-Binary Integration**                 | Compile Auctioneer into the Trusted Server binary, invoke the shared engine without a Auctioneer HTTP hop, bridge host policy/resources and typed results into orchestration, and demonstrate a selected-bidder banner/display publisher flow.                                                                                                                                                                                                                                                   | Existing reusable engine and host contracts; the selected bidders, which may be supplied by an M7 migration wave. Full M7 closure and M8-M19 features are not prerequisites.                                                            |
| **M7.6: Limited-Bidder Publisher Pilot and Hosted Acceptance** | [Execution and acceptance](#8-limited-bidder-publisher-pilot-and-hosted-acceptance): credentialed real Cloudflare/Fastly/Spin deployed shared-handler execution, authorized isolated corrupt-config injection/recovery, selected actual deployment Q2-D qualification, and bounded limited-bidder rollout/rollback.                                                                                                                                                                      | M6 harness/runbooks/native acceptance, the selected publisher bidder migration wave, M7.5 integration/applicable host setup, and M7-R0 fit for affected WASM deployments. Neither M20 full-catalog closure nor video parity is required. |
| **M7.7: Post-Pilot Bidder Expansion**                          | Additional publisher-demand-led bidder waves after the selected Trusted Server/publisher test path passes. Freeze exact wave membership and requirements, implement real protocols through the existing foundations, and repeat composed standalone/embedded acceptance and affected resource/build gates. | M7 foundations and selected M7.5/M7.6 acceptance. Use supported shared contracts; later-feature-dependent rows retain explicit M20 ownership. No wait for M8-M19 or full catalog closure. |
| **M8: Stored Requests and Accounts**                           | Root and impression stored requests, merge precedence, independently stored account lookup, account-required policy, bounded storage access, caching/invalidation policy, and failure ordering.                                                                                                                                                                                                                                                                                          | Existing configuration and host-store interfaces.                                                                                                                                                                                       |
| **M9: Auction Foundations for Parity**                         | Currency conversion and rate freshness; multibid validation, rebuild, and limiting; auction ingestion of stored UIDs with explicit source precedence, alias/sync-family mapping, and privacy-safe bidder projection. One host-wide strict/reference-compatible runtime mode; see [Global Protocol Mode](#m9-global-protocol-mode).                                                                                                                                                                                                                                                                     | M5 identity codec/selection contracts; shared auction and host interfaces.                                                                                                                                                              |
| **M10: Creative Caching**                                      | Reference-protocol-compatible bid and VAST storage, cache IDs and public URLs, TTLs, targeting, bidder-precached creatives, bounded requests/responses, deadlines, and partial failures.                                                                                                                                                                                                                                                                                                 | Auction selection and targeting; an external cache service reached through EdgeZero.                                                                                                                                                    |
| **M11: Full Video Support**                                    | Instream/outstream workflows, `/openrtb2/video`, long-form/ad pods, duration rules, category translation, competitive separation, VAST delivery/tracking, and player/SSAI integration evidence.                                                                                                                                                                                                                                                                                          | M8 stored requests, M9 shared auction foundations, and M10 caching.                                                                                                                                                                     |
| **M12: First-Party Data**                                      | Request-level `ext.prebid.data`, `ext.prebid.bidderconfig`, EID permissions, bidder-specific FPD projection, merge precedence, and related activity controls.                                                                                                                                                                                                                                                                                                                            | Shared normalization/privacy contracts and M9 identity projection.                                                                                                                                                                      |
| **M13: Floors and Adjustments**                                | Price-floor rules and enforcement, bid adjustments, currency interactions, and targeting after adjustments across supported media types.                                                                                                                                                                                                                                                                                                                                                 | M9 currency handling and shared auction/targeting contracts.                                                                                                                                                                            |
| **M14: AMP and Native**                                        | `/openrtb2/amp`, AMP response/CORS and creative retrieval; full native request/response validation, assets, targeting, and client integration. Implement as separately reviewed phases.                                                                                                                                                                                                                                                                                                  | M8 stored requests, M10 caching where required, and shared privacy/auction contracts.                                                                                                                                                   |
| **M15: Additional Privacy and Transparency**                   | Deferred GPP sections 3 and 7-12, DSA request mutation/response validation, SChain merge/validation, and reference-data freshness improvements beyond existing tooling. Implement as separately reviewed policy/transparency phases.                                                                                                                                                                                                                                                     | Existing bounded parsers, activity controls, configuration, and reference-data validation.                                                                                                                                              |
| **M16: Compressed Ingress**                                    | Bounded inbound gzip for auction and sync bodies, extended consistently to new body routes; encoded/decoded limits, accounting, deadline handling, and errors.                                                                                                                                                                                                                                                                                                                           | EdgeZero ingress interfaces and current admission/resource contracts.                                                                                                                                                                   |
| **M17: Hooks and Modules**                                     | Extension lifecycle, ordering, configuration, resource budgets, privacy boundaries, and failure isolation.                                                                                                                                                                                                                                                                                                                                                                               | Stable processing contracts for the stages a hook can access.                                                                                                                                                                           |
| **M18: Metrics and Analytics**                                 | Full operational metrics first, then analytics integrations; bounded cardinality/delivery, privacy-safe payloads, and `reportAnalytics` enforcement. Implement as separate metrics and analytics phases.                                                                                                                                                                                                                                                                                 | Existing diagnostics; M17 where analytics uses hooks.                                                                                                                                                                                   |
| **M19: Browser Privacy Compatibility**                         | Deferred `account.privacy.privacysandbox` configuration, partitioned-cookie/cookie-deprecation behavior, and explicitly reviewed legacy Topics compatibility.                                                                                                                                                                                                                                                                                                                            | Existing sync/privacy contracts and a verified browser/reference contract.                                                                                                                                                              |
| **M20: Remaining Full-Catalog Migration**                      | Residual pinned source identities/branches after M7.7, including later-feature-dependent integrations, plus complete schema/binding/alias/sync/endpoint reconciliation and Tasks 20/21 terminal gates. | M7 foundations and cumulative M7/M7.7 evidence; applicable later shared features. Retain all-target resource/build gates and separate M7-R0 readiness. |

M8-M11 form the video-priority program; their shared foundations must be
reused for banner and native auctions rather than duplicated in video handlers.
M13's floors/adjustments also apply to video when they ship, but do not block
delivery of the M11 video-specific workflows. Completing M11 does not claim
that every shared parity feature in M12-M19 has already shipped.

### M7 Controlled-Native Completion

AG approved the [controlled-native amendment](superpowers/specs/2026-10-30-m7-controlled-native-completion-design.md)
and its [completion plan](superpowers/plans/2026-10-30-m7-controlled-native-completion.md).
It supersedes proof-before-activation for M7: deliver all 14 priority flows
on native with enforced input/work limits, predeclared C/I/V reservations,
allocation/stress checks, native accounting/headroom and real handler acceptance.
These are tested reservations, not proven worst-case bounds. New public
definitions are native-only and default inactive; existing runtime configuration
enables them without a rebuild. Guest readiness remains blocked.

The M7 implementation and local controlled-native acceptance are complete on
the M7 branch; see the [verification record](testing/evidence/m7-controlled-native-20261030.md).
This is a branch-delivery status, not a claim of merge, remote CI success or live
publisher readiness.

M7-R0 now also owns complete source/ABI worst-case schema residency,
initialization, work, scratch and stack certification. M7.6 retains actual
hosted/publisher execution. No priority protocol branch, privacy contract,
schema semantics, workload or deployment ceiling is waived. Do not widen a
reservation after observing a failure to make the acceptance pass.

### Prior Shipped-Schema Qualification Checkpoint

The following records the narrower 2026-10-29 checkpoint. Its static
proof-before-publication requirement is superseded by the native amendment
above; its source closure and correctness requirements remain.

AG approved the [2026-10-29 qualification correction](superpowers/specs/2026-10-29-m7-shipped-schema-qualification-design.md)
to focus work on completing M7. Qualify the actual shipped schema closure per
adapter/feature lane: planned priority bindings plus existing published rows,
their aliases, references, formats/helpers and runtime-disabled shipped state.
Do not emit or fully resource-qualify executable validators for imported
source-only rows merely because they are in the 261-schema inventory.

Use actual native/guest artifacts and bounded execution probes more directly,
retaining the source/compiler/library-owned bounds needed by shipped admission.
Distinguish proved, enforced and observed facts. Observations cannot become
upper-bound descriptors, provider readiness or permission to skip funding.
No schema/engine/feature/limit change, weaker privacy/security behavior or
omitted priority flow follows from this correction. Existing all-source
correctness, provenance and compatibility controls remain intact.

| Owner | Qualification work |
| --- | --- |
| M7 | Complete shipped closure, generated correctness, reachable owner/work/scratch facts, artifact/probe evidence, funded integration and full 14-flow onboarding |
| M7-R0 | Existing absolute target-fit/headroom, platform memory ownership and missing SDK/host resource facts |
| M7.5 | Selected embedded-host resource ownership and direct-engine acceptance |
| M7.6 | Actual deployed provider meters/limits, authorized failure/recovery and publisher rollout |
| M7.7 | New reachable schema/engine features before their expansion wave, including nested literal-owner introspection if first needed there |
| M20 | Remaining full-catalog dialect/format/regex/resource reconciliation and terminal closure |

A required owner in M7's shipped closure stays an M7 blocker. An unused engine
backend is not an open-ended prerequisite for every earlier flow. Preserve
full inventory and migration-state accounting; this changes the resource
qualification population, not source provenance or the complete-catalog target.
The written amendment requires review before serving/promotion changes and
does not itself claim acceptance or M7 completion.

### M9 Global Protocol Mode

M7's approved priority prerequisites include explicit expiring operator
floor-rate snapshots and bounded credential bundles. Full bid-response
currency conversion and external rate feeds remain M9; floor-only conversion
does not authorize relabeling a foreign-currency bid as USD.

Planned, not implemented: one host-wide runtime setting with two named values:

- `strict` (default): follow the supported OpenRTB contract and documented
  Auctioneer policies.
- `reference_compatible`: apply explicitly reviewed protocol/normalization
  differences against the pinned behavioral reference. This is not a promise
  of blanket bug-for-bug compatibility.

The M9 specification and plan must:

- Resolve the setting through the existing validated runtime-config path, not
  a Cargo feature. Capture one value in each operation's config snapshot and
  propagate it through the shared engine/context in standalone and embedded
  use, consistently across calls and aliases.
- Keep the choice global: no account, bidder or request override. Reject
  unknown setting values rather than silently selecting compatibility mode.
- Use one registry and implementation per bidder. Shared code carries the mode;
  reviewed bidder-specific differences remain bidder-owned, without a central
  provider-name switch.
- Keep privacy enforcement, authorization, cookie integrity, HTTPS/SSRF
  protections, admission, resource/deadline limits, exact sent coverage and
  authoritative identity attribution mandatory in both modes.
- Publish a version-pinned difference matrix for decoding, normalization and
  response/error behavior. Unsupported features stay explicitly unsupported;
  choosing a mode does not implement missing capabilities.
- Ship paired strict/compatible fixtures that actually exercise each difference,
  plus invalid-config, snapshot-update and shared-safety negative controls.

This requirement belongs to M9 and does not change current bidder behavior.

## 3. Full Bidder Catalog Completion

The [M7 catalog specification](superpowers/specs/2026-10-16-m7-bidder-catalog-design.md)
and [implementation plan](superpowers/plans/2026-10-16-m7-bidder-catalog.md)
are in progress: reference/tooling and accounting foundations have started;
the full catalog is not implemented. Under the scope correction below,
M7 owns foundations and all 14 priority flows; M7.7 owns reviewed post-pilot
expansion and M20 owns residual migration and full-catalog closure. APS (`aps`)
is an additional integration, absent
from the pinned source catalog; it needs its own reviewed protocol and
onboarding contract. The existing 13-name source-backed wave is therefore a subset of the priority
list, not the complete publisher checkpoint. The exact source inventory
remains 358 public definitions (261 bases and 97 aliases), not an approximate
implementation-directory count or a count including APS.

### 2026-10-26 Scope Correction

AG approved M7 = foundations plus full 14-priority selected-flow acceptance,
including Sharethrough and Rubicon, not merely protocol candidates or imported
metadata. All existing priority-local branch, schema/resource, privacy,
activation/settings/sync and composed onboarding requirements remain M7 work;
no required priority-local branch moves to M20. APS needs independent reviewed
contract/evidence. Partial wave checkpoints are not M7 closure.

This correction initially assigned remaining migration to M20. The
2026-10-27 correction below brings reviewed post-pilot waves into M7.7 while
retaining the implementation plan's Tasks 20/21 terminal gates under M20.
M7 closure cannot wait for those gates.
Keep the full 358-source inventory, 261 base bindings/parameter schemas,
259 implementations and 97 aliases; imported metadata is not implementation.
Normal/default `bidders check`, wave checks and full `--require-complete`
semantics are unchanged: full completion still refuses unfinished source
rows/branches and is required for M20, not M7. R0 and M7.5/M7.6 boundaries
and M8-M19 IDs/requirements are unchanged. This correction claims no delivery.

### 2026-10-27 Post-Pilot Bidder Expansion

AG approved additional bidder waves **after Trusted Server/publisher testing**,
not before schema qualification and not deferred until the parity program ends.
The intended sequence is M7 priority flows, M7.5 embedding, the selected M7.6
pilot/test acceptance, M7.7 expansion, then M8 onward. This adds no bidders to
M7's mandatory 14 and changes no validation, privacy, resource or promotion gate.

M7.7 reuses Task 20's per-family execution procedure, not its terminal
full-catalog gate. Before implementation, freeze a finite, nonempty initial
wave queue from demonstrated publisher demand and the pilot's findings: exact identities,
source/branch requirements, prerequisites, endpoint/settings/sync decisions,
evidence producers and acceptance criteria. No roster or popularity ranking is
invented by this roadmap. The queue must add accepted bidder flows beyond the
initial 14; relisting existing flows is not expansion. Further waves get their
own reviewed manifests.

Completion requires the frozen M7.7 queue's required branches and composed
standalone/embedded flows to pass on current code, with runtime activation,
invalid-parameter/privacy isolation, credentials and signed sync where applicable,
and the affected schema/resource and four-target build/package gates. Review
any rollout of the expanded set separately; the initial pilot cannot certify
new bidders. Report unfinished source rows outside the frozen queue honestly
and hand them to M20 with explicit owners/prerequisites. Required queued work
cannot be silently transferred to close M7.7; changing the frozen scope needs
review. The full `--require-complete` result may remain nonzero; it is not an
M7.7 exit gate and never becomes a partial-catalog success.

Use the selected pilot path's agreed M7.6 acceptance for this sequencing
checkpoint. Unrelated provider rows may remain explicit open work, but no row
may claim hosted readiness without its own evidence or bypass affected R0
requirements. M7.7 must not wait for every parity feature or silently bring
unsupported video/native/AMP delivery into current handlers. Bidders needing
those shared features remain assigned to M20 unless a bounded prerequisite is
separately reviewed and brought forward.

### Demand-Led Waves And Remaining Closure

These are planning categories, not unsupported popularity rankings or fixed
bidder rosters. Select exact membership and order from demonstrated publisher
requests/adoption, with reviewed provenance and prerequisites before each
smaller wave. M7.7 delivers the post-pilot queue supported by current shared
contracts; M20 owns the residual set. Reorder categories when actual adoption
warrants it; within a wave, group coherent protocol/shared dependencies without
weakening gates.

| Submilestone | Candidate demand category |
| ------------ | ------------------------- |
| M7.7 | Reviewed post-pilot display and other demand supported by current shared contracts |
| M20.1 | Residual display demand not completed in M7.7 |
| M20.2 | Remaining regional/mobile/native demand, including later-feature dependencies |
| M20.3 | Video/CTV-dependent flows |
| M20.4 | Remaining catalog closure |

Every remaining identity stays owned even without current demand evidence;
M20.4 reconciles the entire pinned catalog under Tasks 20/21, not only the
popular subset. M20 is not a prerequisite for M7 priority closure, pilots or
M7.7's reviewed expansion queue.

M7 includes the existing
[automatic registration design](superpowers/specs/2026-09-27-bidder-registry-design.md).
New implementations are discovered during the build and joined with explicit
baked metadata bindings at startup. There is no central hand-maintained
registry list, runtime plugin loading, or remote executable code.

Operators can enable or disable an already compiled, baked-eligible bidder
through the existing validated runtime config push without rebuilding.
Adding bidder code or changing baked metadata requires a reviewed rebuild.
Runtime configuration cannot override a baked safety disable switch.
The M7 draft also rejects host/request aliases that shadow compiled public
identities, including baked aliases; this deliberately removes legacy exact-name
base shadowing so it cannot bypass those identities' safety disables.

M7 owns catalog-scale accounting foundations, reused by M7.7 and M20: split
catalog-sized versus caller-sized work, fund resident catalog/schema memory
once, and enforce predeclared catalog-delta allowances against matched
per-target baselines.
Preserve native fit and at least 256 MiB of headroom. Current WASM accounted
demand already exceeds its declared ceilings by at least 76-454 times before
M7; these are conservative bounds, not measured resident memory. That existing
defect belongs to **M7-R0**, not an implicit 58-GB-to-128-MB catalog task.

The dedicated [M7-R0 design](superpowers/specs/2026-10-17-m7-r0-wasm-resource-envelope-design.md)
and [plan](superpowers/plans/2026-10-17-m7-r0-wasm-resource-envelope.md)
track remediation in the same M7 PR. Status corrected 2026-10-26: local R0
ownership/accounting improvements have landed, but complete remediation and
absolute WASM readiness remain blocked; this update grants no new qualification.
They still require normalized term/lifetime audits, a feasibility checkpoint,
justified bounded sharing, and explicit approval of any workload changes.
Its strict validator must use the same normalized inputs and additive derived
total as boot, with term-by-term equality controls, and prove fit with headroom at least
`ceil(effective_ceiling/16)` (6.25%); compiler/package passes and warn-mode boot
are not substitutes. See [M7 section 11.2](superpowers/specs/2026-10-16-m7-bidder-catalog-design.md#112-target-resource-acceptance)
for the baseline subtotals, ceilings and proof boundary. It may proceed in
parallel with catalog migration, but full WASM runtime readiness and its M7.6
deployments require it. Missing target meters remain pending observations,
not passing results. Credentialed deployed-handler execution and actual Q2-D
qualification remain M7.6 work. Neither track may silently omit bidders.

The M7 specification retains foundation/priority acceptance, M7.7's post-pilot
expansion rules and the full-catalog contract owned by M20, as corrected above.
Every inventoried bidder needs
an implementation binding, reviewed metadata and parameter validation,
transformation fixtures, supported sync behavior where applicable, and checks
across the existing target matrix.
Metadata import alone does not constitute an adapter migration.
The Rust checker derives migration state from current runnable protocol,
fixture, binding and target evidence; M20 full closure explicitly requires every
source identity to pass, not merely carry a hand-set `migrated` label.

Maintain a capability matrix that distinguishes adapter migration from shared
feature parity. A dependency on M8-M19 must name its owning milestone and have
an explicit unsupported/disabled behavior; it must not silently produce a
partial request or claim full support. A blocked adapter remains unfinished,
even if its metadata exists. If a shared dependency makes catalog closure
impossible, bring forward the required portion of its owning milestone rather
than relaxing the adapter contract or indefinitely blocking later work.

Deterministic reference fixtures demonstrate transformation compatibility;
credentialed live bidder certification is separate evidence. Both status and
remaining verification requirements must be visible rather than conflated.

## 4. Full Video Completion

Accepting `imp.video` or advertising a bidder's video capability is not full
video support. M11 must demonstrate the server-side path from validated request
through bidder auction, VAST caching, targeting, creative retrieval, and a
compatible player or SSAI integration.

The video specification must cover:

- Instream and outstream auction behavior, including mixed-media requests,
  supported video fields/protocols, bidder media-type validation, and invalid
  response handling.
- `/openrtb2/video` request/response behavior, stored pod configuration, multiple
  pods, duration ranges and exact-duration rules, category translation,
  competitive separation, and ad-server targeting.
- VAST XML and URL/wrapper handling, cache keys/URLs, bidder-precached creatives,
  expiration, and the reference's VAST tracking flow, including `/vtrack`.
- Cache outage, malformed cache responses, partial bidder failure, auction and
  cache deadlines, bounded XML handling, and SSRF-safe URL treatment. This does
  not authorize unbounded wrapper traversal or arbitrary URL fetching.
- Privacy-safe identity projection and diagnostics, deterministic reference
  fixtures, and creative retrieval/playback or SSAI integration tests.

The exact endpoint and protocol matrix must be tied to the chosen reference.
Auctioneer supplies server-side video behavior, not a new video player, ad server,
or SSAI product. Those systems are integration partners used to demonstrate
the complete delivery path. The upstream reference documents stored-request
expansion, pod requirements, category translation, cache storage, and duration/category
targeting in its [video endpoint contract](https://docs.prebid.org/prebid-server/endpoints/openrtb2/pbs-endpoint-video.html).

M10 should first integrate an external reference-compatible cache service, not
add a new cache backend product or a mandatory Auctioneer database. The cache may
be independently routed and deployed; see the
[reference caching architecture](https://docs.prebid.org/prebid-server/features/pbs-caching.html).

## 5. Cross-Cutting Requirements

Preserve the existing ownership boundaries in every milestone:

- Bidder code performs synchronous transformations, not provider I/O.
- The portable auction engine does not import EdgeZero or provider SDKs.
- Core and provider integration supply network/store services through existing
  interfaces. New handlers use EdgeZero manifest registration and the shared
  admission, route policy, and response ownership paths.
- Runtime settings use the existing config authoring, validation, push, and
  resolution path. Do not add another signing layer or per-feature config
  loader without a separately reviewed requirement.
- A milestone does not automatically need a new workspace crate.

Privacy controls, SSRF protections, resource limits, deterministic behavior,
basic operational diagnostics, and relevant tests apply from each feature's
first release. Additional jurisdiction support under M15 is not permission
to enable unsupported processing in those jurisdictions earlier. Full metrics
under M18 do not postpone necessary failure counters and logs.

Adversarial-input and performance verification belong to each milestone that
introduces or changes the surface, not to a later cleanup milestone. A
milestone that adds or modifies a decoder of untrusted input ships generated
hostile-input coverage with it, stating the invariants that must hold for every
generated input rather than only the cases an author enumerated. `auctioneer-privacy`
is the reference: `tests/tcf_property.rs` and `tests/gpp_hostile_input.rs`
exercise the consent parsers this way. The next milestone to change the OpenRTB
request decoder, user-sync cookie codec or bid-response decoder must identify
and run its corresponding generated hostile-input controls, adding missing
coverage. A crash, panic or bypass of the documented privacy/authorization
policy is a defect in the owning milestone, not a hardening backlog item.
A milestone that changes the auction
hot path, admission accounting, or a parser's cost model records a repeatable
measurement with its acceptance evidence, so later resource and latency claims
compare against a figure rather than an impression.

M15 does not replace the existing versioned GVL/GCL archive, freshness checks,
or scheduled refresh tooling. It must specify what additional freshness or
runtime distribution capability is needed, while preserving validation,
rollback behavior, and fail-closed evaluation. Initial config-store hot-reload
hardening remains owned by M6; M8 owns policies for its new stored objects.

M19 retains deferred scope but requires current compatibility evidence before
implementation. Google announced the retirement of Topics while retaining
CHIPS support in its
[2025-10-17 update](https://privacysandbox.google.com/blog/update-on-plans-for-privacy-sandbox-technologies?hl=en).
Do not equate all partitioned-cookie behavior with the legacy cookie-deprecation
flow, or silently drop a pinned-baseline compatibility requirement. The M19
specification must explicitly decide what remains required and document any
replacement or deliberate divergence.

## 6. Publisher Pilot Checkpoint

M6 is the local/staging hardening checkpoint, not a live publisher pilot.
Functional staging tests can begin earlier on the existing M3b/M4/M5
foundation. M7.5 establishes real embedding/host integration; M7.6 owns the
live limited-bidder banner/display pilot after the selected publisher's
migration wave and host setup are ready. Neither M20 full-catalog closure nor
the M8-M11 video program is a prerequisite. Native proof alone grants no
live-traffic readiness or waiver of the selected deployment's safety checks.

Before real traffic, validate the selected bidder credentials/parameters,
actual consent and identity flow, ad-server targeting and creative rendering,
reference-data freshness, provider behavior, latency/load expectations, and
an observable limited rollout with rollback. Consent collection by the CMP
must not be treated as proof that outbound enforcement is correct.

Q2-A/B/C and reusable Q2-E are explicitly owned by M6 under
[design section 10.1](superpowers/specs/2026-10-05-m6-parity-hardening-design.md#101-deployment-resource-qualification-q2).
M6 delivers validated profiles/reports, repeatable maximum-admitted load
fixtures, cold/warm memory and latency measurements, native overload/recovery
evidence, tuned limits and a rollout/rollback runbook. The selected actual
deployment must pass its accounted-budget, headroom and measured acceptance
criteria in M7.6 Q2-D before its publisher pilot; this is deferred execution,
not a waiver. Local runs or warn-mode boot are not actual-deployment
qualification. Other targets keep explicit pending/unsupported evidence rows.
This work does not wait for M18 metrics or block M3b's implementation merge.
Q1, Q3 and Q4 remain EdgeZero-owned connection/parser, config-allocation and
provider-lifecycle acceptance tasks; Auctioneer records their actual capabilities
instead of duplicating the SDK controls.

| Publisher requirement beyond the initial banner flow            | Additional owner |
| --------------------------------------------------------------- | ---------------- |
| Stored requests or independently stored accounts                | M8               |
| Automatic stored-UID ingestion, non-USD conversion, or multibid | M9               |
| Cached creative delivery                                        | M10              |
| Video or ad-pod workflows                                       | M11              |
| Request-level FPD or bidder-specific EID permissions            | M12              |
| Floors or bid adjustments                                       | M13              |
| AMP or full native workflows                                    | M14              |
| Currently unsupported GPP policy, DSA, or SChain behavior       | M15              |

"Non-video" alone does not guarantee the initial banner flow suffices: native
and AMP have their own requirements, and a banner publisher may rely on later shared features.
Bring forward the required portion of its owning milestone when justified
by the pilot rather than claiming an unsupported flow is ready.

Direct in-binary Trusted Server integration is owned by M7.5. Its specification
and host-bridge tests can begin using the existing reusable engine and selected
bidders without waiting for catalog/video parity. M6 local/staging evidence
does not prove embedding completion, and M7.5 integration evidence is not
live-pilot qualification. Standalone and embedded live limited-bidder pilots
belong to M7.6, subject to the same publisher-specific dependencies above.

## 7. Trusted Server Embedding Milestone

M7.5 makes embedding an explicit deliverable before the video/parity program,
not a capability to revisit after all other milestones. Its initial media
scope is banner/display with the selected publisher's supported bidder set.
It does not claim native, AMP, video, or features deferred to M8-M19.

Its reviewed specification and plan must cover:

- A pinned Trusted Server integration revision and the actual local-provider
  interface needed by its orchestrator. Preserve bidder ownership and prevent
  duplicate dispatch through local and remote providers.
- A direct call into the same `prepare`/`run_auction` path used by the standalone
  service, compiled into one host artifact without a Auctioneer sidecar or service
  hop. Do not register a second independent HTTP application inside the host.
- Explicit mapping of publisher/account configuration, consent and identity
  sources, aliases, deadlines/cancellation, admission/reservations, reference
  data, errors, and typed bid/targeting results.
- Host services supplied through the existing runtime interfaces, with no
  separate configuration/signing path and no privacy or resource bypass for
  an in-process caller.
- Equivalence tests using identical inputs, resolved policy, deterministic
  clock/RNG, and captured bidder results through standalone and embedded paths.
- A built integration artifact and host-flow tests on the selected publisher's
  target, with the existing target compilation checks. Document provider-specific
  limitations without making every hosted-provider deployment a prerequisite.
- A controlled publisher host-integration exercise demonstrating request admission,
  selected bidder dispatch, targeting, creative rendering, failure isolation,
  basic diagnostics, and rollback. This is not a live pilot: credentialed
  live bidder, browser-facing pilot and actual deployment Q2-D acceptance
  remain separate M7.6 evidence, not inferred from deterministic host-flow tests.

Completion requires a real Trusted Server host integration and its acceptance
evidence, not just an engine-library test or an HTTP client calling Auctioneer.
The existing library entry point is a foundation, not completion of M7.5.

## 8. Limited-Bidder Publisher Pilot and Hosted Acceptance

**M7.6 owner:** Auctioneer provider integration maintainer plus the selected
publisher/deployment owner. Approved by the user on 2026-10-15; no live run
or qualification is claimed by this scope correction. Reuse the
[provider contract](superpowers/specs/2026-10-05-m6-parity-hardening-design.md#10-provider-and-pilot-evidence),
[Task 14 preparation/execution checklist](superpowers/plans/2026-10-07-m6-parity-hardening.md#task-14-prepare-the-offline-hosted-handler-harness-and-runbook),
and the Q2/runbooks retained by M6, not a new serving or qualification path.

- Prerequisites: completed M6 offline harness/artifact handoff/refusal tests,
  native Q2-A/B/C and reusable Q2-E; the selected M7 bidder migration wave;
  M7.5 integration and applicable embedding/hosted setup; approved staging
  deployments/credentials, controlled HTTPS fixture, isolated config bindings,
  explicit mutation authorization, real resource meter and known-good
  artifact/config restoration inputs. Full M7 closure and video parity are
  not prerequisites. Resolve exact deploy commands from the pinned CLI;
  preserve Spin's digest-checked component and Fastly's verified release bundle.
- Hosted cases, independently on real Cloudflare/Fastly/Spin generated Apps:
  valid banner auction with proved fixture dispatch; failed/timed-out bidder;
  malformed request; denied and fully redacted debug; input/output caps; exact
  application response headers; observable cancellation/completion; valid
  config replacement and bounded backend visibility/restart observations.
  Preserve privacy enforcement and Cloudflare `Unknown` body omission.
- Corruption cases: only in an authorized dedicated test binding/key, inject
  bounded malformed JSON and valid-envelope/digest mismatch using the actual
  provider-native addressing. Prove the selected key changed and the App
  observed fail-closed refusal with no dispatch, then typed-push restoration
  and observed healthy recovery. Arm bounded cleanup and abrupt-termination
  recovery; stale reads, rejected pushes/writes, missing-chunk propagation,
  timeouts and failed restoration are not passes. Never mutate shared chunks
  or production/shared stores.
- Q2-D on the selected actual production-intended deployment/artifact: run
  all six retained workload families (cold/warm baseline, maximum admitted,
  response/decode overlap, debug states, auxiliary/config overlap, saturation
  and recovery) against predeclared windows, repetitions, subcases and numeric
  limits. Require accounted fit/headroom, real-meter peak/latency/outcome and
  recovery acceptance, and verified required SDK capabilities. Fixture builds,
  loopback overrides or native-only proof cannot qualify a hosted artifact.
- Before real traffic: verify the limited bidders' support/credentials and
  parameters, actual consent/identity enforcement, fresh reference data,
  targeting and creative rendering, host/provider behavior and resource
  acceptance; then execute limited rollout with monitoring/stop thresholds,
  coordinated known-good artifact/config rollback and observed recovery.
- Evidence: record per-target/case pass/fail/pending/unsupported with Auctioneer
  commit, artifact/features, SDK/runtime, serving config/effective SDK settings,
  bidder/host setup, profile/harness/fixture/reference-data digests, meter and
  bounded numeric/category observations. No raw payload/config/debug, secrets,
  identities or provider errors. Keep handler, resource, SDK Q1/Q3/Q4 and pilot
  evidence distinct; unobservable behavior remains pending/unsupported. A
  selected-target pilot needs its applicable passing rows, not qualification
  of unrelated providers; unexecuted promised provider rows stay open in M7.6.
- Invalidation: artifact/features, bidder set or host integration, effective
  config/SDK environment/profile, SDK/runtime, relevant reference data or
  platform budget changes invalidate affected rows. Re-evaluate/rerun against
  the new identity before continuing traffic; do not reuse stale passes.

## 9. Delivery and Next Step

Each milestone must record its baseline, interfaces, error ordering,
configuration changes, resource accounting, intentional differences, and
acceptance evidence. Unit tests stay alongside their implementation modules;
integration tests exercise cross-module, HTTP, provider, or client boundaries.
Name tests after behavior rather than milestone numbers. Target compilation,
host conformance, and live-provider/browser checks remain distinct evidence.

This roadmap approves priority and scope ownership only. It does not declare
future milestones implementation-ready or promote an artifact to production. The next
specification is M7: an exact bidder inventory, automatic-registration and
schema/tooling foundations, and measurable 14-priority closure under the
[2026-10-26 correction](#2026-10-26-scope-correction). The
[2026-10-27 correction](#2026-10-27-post-pilot-bidder-expansion) adds M7.7's
post-pilot bidder expansion before M8 onward. M20 retains residual migration
and full-catalog closure.
M7.5's embedding specification follows as the explicit integration track and
can be developed alongside bidder migration once its selected set is ready.
