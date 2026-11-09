# Shared Resource Ownership And Auxiliary Response Accounting

Date: 2026-11-09
Status: Draft for document review; not implementation authorization.
Issues: [#33](https://github.com/IABTechLab/auctioneer/issues/33),
[#50](https://github.com/IABTechLab/auctioneer/issues/50),
[#48](https://github.com/IABTechLab/auctioneer/issues/48).
Parent: [resource qualification](https://github.com/IABTechLab/auctioneer/issues/9).
Code baseline: `499f0e540e95e1c86b730c1030aabef28de01d0a`.
SDK baseline: `598f0a2a490eb83dd31c06c024a0cefa8e049628`.

## 1. Outcome And Scope

Deliver one bounded package through the existing ownership and accounting
paths. Reconcile what already works, assign the remaining proof and SDK
obligations, and close application-owned auxiliary response storage and
lifetime gaps. Independently justified accounting corrections are permitted;
projected savings, target fit and complete schema certification are not
promised outcomes.

AG approved retaining the current permits, completion callbacks and single
additive budget, bounding storage and proving any corrections. This decision
does not approve implementation before the written specification and plan
are reviewed and merged.

The package has three ordered stages with separate acceptance:

1. **#33:** reconcile the actual shipped closure and existing components.
2. **#50:** identify the exact adapter, external owner and next action for
   each remaining SDK or provider fact.
3. **#48:** implement and verify auxiliary output storage and lifetime
   bounds; change accounting only where the complete required proof exists.

Keep the existing dependency direction: #33 precedes #50 and #48. A joint
delivery does not make #33 depend on finishing those children, and a merged
planning PR closes none of the three issues.

### Non-Goals

- No new configuration representation, loader, startup settings, cache or
  operator knob. Preserve the single runtime policy and existing SDK flow.
- No new crate, resource framework, regex engine, bidder cohort or manually
  maintained executable bidder list.
- No changed workload limits, concurrency, platform ceilings, headroom floors,
  schema semantics, decoder caps, privacy or user-sync behavior.
- No broad schema certification from #46/#47, configuration extraction proof
  from #147, sealed DTO qualification from #150 or decoder audit from #148.
  Reconcile these obligations without doing their independent projects here.
- No claim that this package enables Fastly guest serving or proves the
  combined Trusted Server artifact. Those remain #73, #88 and #76 outcomes.
- No rewriting historical evidence, pinned captures or their measured bytes.

## 2. Current Implementation And Actual Gaps

The following are already implemented at the baseline and must be reused:

| Component | Existing owner | Boundary to preserve |
| --- | --- | --- |
| Shared checked arithmetic and all isolate terms | `auctioneer-runtime/src/budget.rs` | One additive total consumed by bootstrap and the CLI |
| Auxiliary materialization caps and reservation | `auctioneer-runtime/src/aux_work.rs` | Admission before configuration/input work; pinned GVL remains separate |
| Auxiliary route and work lifetime | `auctioneer-core/src/ingress.rs::admit_aux` | The route completion owns both the permit and work receipt through terminal egress |
| Egress ownership and deadlines | `host_primitives/ingress_lease.rs`, `handlers/aux_egress.rs` | No early release at grant drop, handler return or wrapper return |
| Exact-size info/health serialization | `auctioneer-core/src/serialize.rs::serialize_json_exact` | Count, reserve fallibly and serialize without buffer growth |
| Detached CORS storage | `handlers/sync_http.rs::SyncBrowserPolicy::allowed_origin` | Copy the matched configured origin, not an ingress-backed slice |
| Typed host bridge | `auctioneer-core` public bridge from #159 | Settings first; original funding and services; actual host acceptance remains separate |
| Current resource reports and strict/boot parity | Existing CLI `bidders resources`, runtime derivation and `resource_boot_parity` | Actual independently executed bootstrap and strict CLI, not two calls to one pure helper |

Paths above name crate-local source owners; unqualified paths are inside
`crates/auctioneer-core/src/`. These are source observations, not new
allocation measurements or guest proofs.

At this baseline, native secure/development builds have 17/18 executable
serving bindings and use 15/16 effective parameter schemas; guests have only
3/4 serving bindings because `native_only` remains enforced. Metadata
declarations and inspection membership are not thereby reduced to the guest
serving subset. The selected regex closure contains
four generated helpers and one shared hostname program, with hostname and
UUID format callbacks. Equal patterns in different generated owners are not
therefore shared storage. The controlled-native reservation and existing
schema component/constructor/cache models are implemented, but complete
worst-case certification is not. Record these as baseline observations, not
new hand-maintained membership gates.

Build-time bidder discovery already exists. Some schema staging and source
selection controls still use pinned priority/supplemental projections;
discovery-derived qualification selection is remaining #46/#47 work, not a
reason to rebuild the registry in this package. The current implementation
and the controlled-native amendment take precedence over older checkpoint
prose saying no serving validation exists.

The sync `BoundedBody` checks length before append but uses a growable `Vec`.
The HTML-escaping path can retain raw and escaped buffers simultaneously;
the final newline is another write. Application header checks occur after
construction and measure serialized count/bytes, not retained map capacity,
duplicate-value storage or an earlier larger allocation. Cookie formatting
and conversion into `HeaderValue` have their own overlapping allocations.
These distinctions must be resolved before granting handoff credit.

The current auxiliary isolate term multiplies the entire declared auxiliary
work peak by the target handoff factor for both auxiliary and independent
probe-configuration populations. It is conservative, not by itself a bug.
Multiplying less requires proof that the omitted work is not copied or live
in the handoff domain. A smaller measured receipt does not reduce an unchanged
declared isolate slot.

## 3. One Current Proof And Ownership Index

Add `docs/testing/resource-proof-ownership.md` as a current, source-linked
index, not another issue tracker or parallel accounting implementation. Use
the existing resource reports, schema selection/probe reports and GitHub
issues as its inputs. Do not introduce a custom discovery command or proof
serialization framework for this package.

Each applicable row records:

- Scope: application/feature lane, adapter and ABI, source and dependency
  identities, and the owning executable schema/helper/format/alias closure.
- Status: implemented component, demonstrated property, remaining proof,
  external fact or not applicable with a concrete reason. An observation is
  explicitly an observation, never an implicit bound.
- Resource: units, allocation owner, construction/admission point, lifetime
  end, simultaneous population and overlap with adjacent owners.
- Evidence: exact source/API references, existing test or artifact producer,
  missing experiment/proof, responsible role, GitHub issue and next action.

Separate retained schema program/storage **C**, initialization peak/work **I**
and per-validation cache/scratch/work/stack **V**. Also name configuration,
sealed response decoding, ordinary decoder audit, auxiliary handoff, host
bookkeeping, ingress/framing, codec and combined-host obligations. Reuse
implemented compiler/library layouts, caches, owned receipts and controls;
do not reset their status because complete target certification is pending.

Inventory executable definitions and public aliases from current generated
metadata/factory discovery, including runtime-disabled definitions present
in the build. Distinguish imported non-executable catalogue entries and
schema artifacts used only by full-catalogue controls. Preserve those
controls, but do not turn unused schemas or private engine alternatives into
new serving prerequisites. Record any existing source-selection limitation
honestly rather than relabeling a cohort report as complete discovery.

This index is authoritative only for its stated baseline. Updates bind the
new source and SDK identities and revisit changed owners; historical reports
are cited as history, not silently recertified. GitHub retains task status,
priority, blockers and final acceptance.

## 4. SDK And Adapter Fact Correspondence

Inspect the pinned SDK implementation and the actual adapter entrypoints,
rather than copying an earlier missing-fact list. For Axum, Cloudflare,
Fastly and Akamai Functions hosted Spin, record separately:

| Obligation | Required correspondence |
| --- | --- |
| Ceiling and scope | Published or operator-defined ceiling, what memory it covers, configured ceiling and applicable headroom |
| Population | Inbound/parser and body/outbound populations, actual enforcement, scope and independent application admission classes |
| Host ingress/framing | Parser, request-head/body backing and provider memory ownership before and after admission, including refusal and cancellation |
| Host bookkeeping | Prepared contexts, scopes, task registrations, queues, ledgers, boxed receipts, cleanup scratch and Fastly registration recorder/configuration-set storage |
| Codecs and output | Reachable decoder state/copies, response body/header conversion, host-added framing and the exact handoff contract |
| Execution identity | Actual adapter dispatch and artifact/ABI; a hosted profile name alone cannot establish deployment provenance |
| Separate stack | Published allowance versus application-required stack; the latter belongs to Auctioneer target qualification |

The baseline already supplies these facts; do not file them as missing APIs:

| Adapter | Provided or enforced today | Still distinct from proof of target fit |
| --- | --- | --- |
| Axum | A finite connection gate before parser construction; operator-configured SDK ceiling/population/ingress facts remain unknown | Actual operator quota/scope, runner settings and complete host/allocator correspondence; the gate is not absent |
| Fastly | 128,000,000-byte primary allowance, separate 1,000,000-byte stack and one execution-scoped inbound request | Application stack bound and host-ingress/allocator correspondence |
| Cloudflare | 128,000,000-byte instance-scoped allowance | Inbound population and host-ingress accounting; the declared outbound limit is not an inbound guarantee |
| Hosted Spin | 134,217,728-byte execution-scoped hosted default, inbound population one, and hosted metadata passed to actual dispatch | Authorized deployment provenance and host-ingress/allocator correspondence; no current generic/hosted dispatch mismatch is established |

Source facts are in the four pinned SDK adapter `src/lib.rs` declarations,
Axum's `dev_server.rs`/`ingress_config.rs`, and Auctioneer's adapter envelope
and dispatch owners. Current SDK codec reservations and scope/task/receipt
implementations also exist; their full allocator and simultaneous-owner
correspondence is the obligation, not implementing another decoder or task
system. #49 owns codec-certificate coordination; applicable certification
remains adapter-specific.

A published SDK subtotal is a usable component, not automatically a complete
mapping to Auctioneer's allocator, populations or charged memory scope.
A missing physical correspondence does not erase an implemented SDK feature.
Record source/API-contract facts separately from target-executed observations.

A configured-ceiling overflow is an Auctioneer demand failure even when the
platform ceiling is unknown. External absence and arithmetic overflow can
coexist; retain both. Never use an unknown provider population as zero, reuse
an outbound limit as an inbound limit, or credit a native substitute as a
guest layout. An unresolved fact remains nonpassing in the existing fit
verdict.

Application response-header limits exclude fields added later by a host or
provider. Prove the application storage domain here; assign the provider
domain explicitly. Fastly's application stack belongs to #73, not an SDK
implementer merely because the SDK publishes its separate-stack allowance.

#50 can close when this fact/owner/action mapping is complete. Provider fact
fulfillment and physical target certification stay in #72-#75; assigning an
owner cannot make an unknown value known or change `required_fit` to true.

## 5. Auxiliary Output Storage Contract

### 5.1 Route Coverage

Derive coverage from the existing registered routes and wrapper/egress
controls. Do not create a handwritten bidder list or silently limit the
proof to user-sync success. Cover:

- Cookie-sync, getuids and setuid success, preflight and each failure shape.
- Bidder list, single detail and all-details output, including runtime aliases,
  disabling, unknown names and invalid query values.
- Authorized and unauthorized GVL health output.
- Unknown route, wrong method, bounded-drain refusal and rendered config,
  input, allocation and handler failures.
- Probe configuration and its distinct status output/cache; preboot and
  stale-App funding refusals; independent static and auction responses as
  adjacent, disjoint owners.

Keep output bytes, status, header multiplicity/order where significant,
content type, newline/HTML escaping, cookie signing, CORS policy, callback
encoding, error isolation and write-deadline anchoring unchanged. No generic
new response ceiling may narrow an existing info or health endpoint.

### 5.2 Body And Cookie Construction

Use the existing capped-writer/size-counter ownership rather than a second
serializer. For a cap-bearing sync response, stop counting at the existing
payload cap before reserving output; an uncapped size counter must not replace
early over-cap refusal. Check raw and escaped sizes, including the final
newline, against that cap before their respective fallible reservations.
Prevent buffer growth while writing. For HTML escaping, account raw and
escaped backing and scratch simultaneously or eliminate one only with
equivalent bytes and an independent lifetime proof. Require the second pass
to emit exactly the counted size: both growth and shrinkage are terminal
count/serialize disagreement. Allocation failure is also terminal: no retry,
partial JSON or unbounded fallback. This does not add a cap to info/health
serialization, whose existing contract is counted-size reservation only.

A logical cap and `try_reserve_exact` alone are not a universal physical
allocation proof. Record actual retained capacity and the applicable
container/allocator contract, source-bound ABI layout and requested versus
physical allocation distinction. Do not claim provider memory or RSS from a
native counting allocator.

Include the complete cookie formatter, its input, formatted field, header
conversion and retained header backing. Preserve the existing 4-KiB complete
cookie ceiling and preflight/refusal behavior. Temporary formatter storage
belongs to construction work; response-retained backing belongs to handoff.
Never count one shared backing twice or assume borrowed bytes are detached.

### 5.3 Application Header Storage

Bound both materialization and retained storage: map buckets/indices,
duplicate-value storage, header name/value backing, capacity retained after
removal, CORS copying, `Vary`, the complete cookie field and formatter overlap.
Use the current `http`/SDK APIs and source contracts; do not duplicate private
Rust layouts in Auctioneer or infer capacity from serialized length alone.

Prefer constructing only the final reviewed application header set through
the owning helper. Preserve valid duplicate values. Rejecting an oversized
input after copying it is not a preallocation bound. Prove the error/fallback
header set as well, including when normal response construction has failed.

Where the locked library or provider contract cannot support a physical
bound, retain the conservative charge and record the exact missing fact and
owner. Do not fabricate a per-entry constant or widen a reservation from an
observed peak until tests pass. A new dependency, vendor patch, product limit
or SDK extension requires separate review, not an implicit plan fallback.

## 6. Funding, Populations And Accounting Changes

Retain `admit_aux`'s single acquisition and terminal completion. No handler
acquires a second auxiliary permit. No work receipt is released at handler
return while funded response backing remains live. Receipt/permit destruction
must still be exactly once on send success, failure, timeout, cancellation,
abandonment, fallback and partially constructed responses.

Preserve these distinct populations:

- Auxiliary requests `A`, holding their permit and work receipt to terminal.
- Independent probe-configuration work `P = max(A, PROBE_CONFIG_MIN_PERMITS)`,
  ending after configuration/render/publication, not after probe transmission.
- Independently gated probe outputs and the cached probe response.
- Auction work, auction egress and static terminal responses.
- Separately pinned GVL versions, including versions held past cache eviction.

Do not replace their additive accounting with a startup/steady maximum or
claim they cannot overlap merely because one request uses only one route.
For `A = 0`, probe configuration/output/cache still require their funding.

An accounting correction requires an explicit old/new derivation before its
production edit. For every affected term, record units, concrete storage
owners, enforced populations, overlapping capacities, target scope, exact
charged delta and residual demand. Keep declared work slack funded; do not
subtract output twice because it already appears in `AuxWorkCharges.output`.

In particular, replacing `handoff_factor * declared_work` with a smaller
expression requires a source-bound upper bound on the retained response
domain for **every contributing route and error path**, plus the applicable
host-copy contract. Removing probe-config transmit/copy charges requires a
separate proof that independently charged probe output covers that domain
without dropping probe-config construction/storage overlap. A route-only
bound cannot justify changing a combined auxiliary/probe term.

Reuse the runtime checked derivation, existing adapter-owned profiles and
existing core/CLI report consumers. No CLI-local replacement equation or
second numeric descriptor assembled from copied private layouts is allowed.
If the complete proof is unavailable, ship the bounded ownership improvement
with unchanged charges; #48 stays open for any unfulfilled acceptance.

Keep current accounting distinct from validation of original pinned resource
records. A cutover must preserve both catalogue baselines and their historical
arithmetic checks without rewriting their bytes or applying new coefficients
to old receipts. Current records must still require all current bound data.
Retain the catalogue comparison's matched-input, growth-only contract;
accounting savings are not catalogue growth. Reject unsupported mixed-model
comparisons rather than relaxing that gate or recertifying old evidence.

## 7. Independent Verification

The implementation plan must name existing owner targets and full commands.
Expected results in a plan are not executed evidence. Preserve the full CI
matrix, hermetic compiler/producer owners, feature lanes and guest checks.
Do not duplicate expensive suites in new binaries merely for this package.

Required new or extended controls:

1. **Body backing:** empty, small, intermediate, at-limit and limit-plus-one
   inputs; independently varied input size/output cap; HTML expansion and
   newline boundaries; no growth, partial output or hidden retry. Exact wire
   expectations must not be recomputed by the writer under test. Over-cap
   counting must stop before output allocation and before walking the remaining
   serializable entries; cover raw and escaped overflow separately. Exercise
   second-pass growth and shrinkage, and execute the production handler wire
   oracle rather than only fixture metadata checks.
2. **Headers/cookie:** vary count, name bytes, value bytes, duplicate values,
   CORS input backing and cookie bytes independently. Cover insertion/removal,
   construction peak, final retained storage and refusal. Include positive
   allocator controls and at least two unequal sizes with a differential
   bound, not a generous aggregate slack multiplier.
3. **Lifetime:** suspend real egress after handler return and observe original
   route/work counters; exhaust and recover; cancel during construction and
   after handoff; exercise errors and abandonment; prove no double acquisition
   or release. Test completion execution and completion drop separately.
4. **Population/accounting:** vary auxiliary, probe-config minimum, probe
   output, static and auction counts independently. Test zero, one and unequal
   nonzero populations, checked overflow and one-byte boundaries. Expected
   deltas come from independent literals/source layouts, not the constant or
   helper being asserted.
5. **Boot/strict parity:** execute the real bootstrap and real CLI subprocesses
   in the existing isolated harness. Preserve independently fitting and
   overflowing rows, configured/platform ceiling order and missing facts.
6. **Adapter correspondence:** execute applicable native/guest controls and
   record artifact, ABI, features, source and SDK identities. Compilation is
   not execution; missing provider facts remain explicit. No native result
   substitutes for guest/host physical proof.
7. **Negative sensitivity:** show each new consequential guard rejects its
   independent failing case. Source inventory, filtered dispatch and route
   coverage checks must reject omitted rows, not pass with zero iterations.

Unit cases stay inline. Use `serialize_allocation_bound`,
`usersync_handler_alloc`, the existing auxiliary/egress integration owners,
runtime units, `bidders_resources` and `resource_boot_parity` where applicable.
Process-global gates, cold state and allocation counters retain isolation.
Fixtures use capable `fake_` bidders outside real bidder-owned protocols.

## 8. Acceptance And Handoff

**#33 complete:** a current source-bound index accounts for the actual shipped
closure and completed components, separating every remaining proof/fact and
assigning its issue, responsible role and concrete next action. No duplicate
resource framework or reopened delivered capability.

**#50 complete:** the four adapter fact correspondences are reconciled against
the pinned SDK and current dispatch; unknowns, local demand failures, separate
stack and provider-added output domains remain distinct. Every missing fact
has its external/internal owner and verification action. Completion records
coordination, not fulfillment or qualification.

**#48 complete:** all applicable application-owned auxiliary output backing,
headers, cookie construction and terminal overlap have justified bounds and
passing independent controls. Any changed charge has its complete source,
lifetime, ABI/scope and population proof with real boot/strict parity. Retain
the issue open if a required bound or control remains blocked; assigning that
gap to another owner does not itself satisfy the criterion.

The implementation handoff states each issue's actual outcome separately,
accounted versus observed changes, residual demand, unavailable checks and
remaining external prerequisites. Keep #46/#47, adapter certification and the
actual Trusted Server host handoff open until their own acceptance is met.
No unchecked gap is converted to `qualified`, and no merge closes an epic
merely because this package has landed.

## 9. Review Sequence

1. Review this written specification, including its independent issue closure
   rules and unchanged-workload boundaries.
2. Write and independently review an ordered implementation plan with exact
   files, owner tests, full-suite commands and any proposed charged derivation.
3. Publish both documents in one non-draft planning PR when authorized. Use
   `Refs #33`, `Refs #50` and `Refs #48`, not closing keywords.
4. Merge the approved planning PR only with the user's merge authorization.
   Then implement in a separate worktree and PR from the updated default
   branch. No implementation is authorized by starting this planning cycle.
