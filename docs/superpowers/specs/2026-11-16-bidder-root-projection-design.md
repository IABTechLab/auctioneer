# Recipient-Scoped Bidder Roots And Materialization Measurements

Date: 2026-11-16
Status: Written specification approved by the user on 2026-11-16; implementation, publication and merge remain separately gated
Issues: [#176](https://github.com/IABTechLab/auctioneer/issues/176), [#182](https://github.com/IABTechLab/auctioneer/issues/182)
Inspected base: `86fe367b9fb91ed2123fdaedb512cd19a5ca65b6`

## 1. Scope And Approval

The user approved a shared recipient-specific root builder that selects fields before copying, preserves parameter precedence, aliases, privacy ordering and funding, and stops forwarding unresolved request-only FPD. The associated benchmark reuses production stages through the existing test-support boundary. The user approved this written contract on 2026-11-16 and authorized writing the implementation plan; implementation, publication and merge remain separately gated.

#176 is the P1 disclosure correction. #182 is P3 measurement and a recommendation, not authorization to redesign the request model. They share the materialization owner and one planning package. A missing benchmark result must not delay an otherwise accepted privacy correction: implementation may deliver the correction first and measurement in a separate follow-up under this same reviewed plan, without closing #182 prematurely.

The change applies to the common exchange path used by all four adapters and the portable embedded API. It introduces no operator knob, second configuration, bidder roster, crate, dependency, schema change, engine substitution or new resource allowance. It does not enable guest routing, certify target fit or complete request-level FPD.

## 2. Current Behavior And Reference

At the inspected base, [`group_by_bidder`](../../../crates/auctioneer-exchange/src/split/mod.rs) serializes one canonical typed request into a shared `Value`, then records participation. `run_auction` decides privacy, sampling and bidder caps before materialization. `materialize_per_bidder` builds scoped impressions but clones every root field except `imp`; the complete root `ext.prebid` consequently survives for each recipient. Typed decoding and request validation follow, then privacy scrubbing and legacy-signal projection.

The disclosure walk covers user/device fields and transaction IDs, not arbitrary request-level `bidderconfig` data. A recipient can therefore receive another bidder's parameters, request aliases and unresolved FPD containing identifiers. The existing grouped snapshot records this behavior; it is not authority to preserve the disclosure.

The pinned Prebid Server reference [`buildRequestExtForBidder`](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/exchange/utils.go#L493-L536) rebuilds the root extension, selects the recipient's parameter value and request alias, and copies selected controls. Its sibling helpers also filter alternate bidder codes, multibid and targeting. This proposal adopts recipient scoping, not wholesale parity with those sibling transforms. These are application-extension decisions, not changes to standard OpenRTB field parsing.

Two alternatives were rejected: cloning and then deleting retains unnecessary competitor copies; decoding the root once and cloning typed fields is a separate optimization requiring measurements and a reviewed design.

## 3. Recipient Projection Contract

### 3.1 Selection Boundary

Add a private root-projection owner within the existing exchange split module tree. The production materializer calls it after survivor selection and existing projection admission, before typed decoding. It borrows the shared grouped request and the selected requested identity. It does not mutate the caller, group, alias resolver or selected-impression membership.

Copy retained root fields except `imp` using their current canonical representation. Insert only the already-scoped impressions. Treat root `ext.prebid` specially: build its destination directly from the dispositions below. Do not clone the old root extension and subsequently strip it. Ordinary root `ext` siblings outside `prebid` retain their current values and serialization behavior.

Lookup bidder and alias names as exact object keys. Names containing `/`, `~` or escape-like sequences are not dynamic JSON Pointers. Never fall back from a requested alias to a base bidder's root parameter entry. The accepted alias resolver and original inbound request remain the authorities for participation, privacy vendor overrides and account/host policy.

### 3.2 Exhaustive Root `prebid` Dispositions

| Source field | Outbound disposition |
| --- | --- |
| `bidderparams` | Select only the exact recipient's entry. Forward its object members as the value of outbound `bidderparams`, without the outer bidder-name map. Do not substitute the impression-merged parameters. |
| `aliases` | If the inbound request declared the selected alias and the accepted resolver retains it, keep only that alias-to-base entry. A base request gets no competitor alias entries. Do not serialize the host alias roster or synthesize a request alias entry that was absent on input. |
| `alternatebiddercodes` | Preserve its current canonical value when present; entry filtering is not implemented here. |
| `channel` | Preserve its current canonical value when present. |
| `currency` | Preserve its current canonical value when present. This does not implement currency conversion. |
| `debug` | Preserve its current canonical value and integer/bool form when present. |
| `integration` | Preserve its current canonical value when present. |
| `multibid` | Preserve its current canonical value when present; validation, entry filtering and bid limits remain separate work. |
| `sdk` | Preserve its current canonical value when present. |
| `server` | Preserve its current canonical value when present. |
| `targeting` | Preserve existing typed targeting and its extension values. Do not adopt the reference helper's narrower targeting object in this correction. |
| `aliasgvlids`, `nosale`, `returnallbidstatus` | Request/exchange controls only; omit from every bidder request. Their original inbound consumers and response effects stay unchanged. |
| `data`, `bidderconfig` | Request-only inputs until the reviewed FPD resolver exists. Omit regardless of whether the recipient's activity decisions allow or deny disclosure. Do not attempt partial evaluation or move them into `user`. |
| Every other field, including raw `schains` | Omit from outbound `prebid`. This neither removes standard `source.schain` nor implements the deferred supply-chain merge. |

The forwarding set is deliberately finite. A newly typed or flattened root-Prebid field does not become forwarded automatically. Test expectations spell the set independently; tests must not compute expected output from the production disposition table.

Preserving `alternatebiddercodes`, `multibid` and targeting is an explicit scope boundary, not a claim that every occurrence of another bidder's name is removed from every control payload. In particular, this package does not implement the separately tracked multibid/alternate-code semantics or fully resolved FPD.

### 3.3 Presence And Non-Object Parameters

Apply projection to the current canonical typed representation, not raw source bytes. Preserve an absent root `ext` or absent `prebid` as absent; do not create either merely because the operator configured an alias. A present `ext` remains present. A present `prebid` whose retained fields are empty remains an empty object, subject to the existing field-level Serde omission rules.

The inbound `bidderparams` map permits arbitrary `Value` entries, but the current typed outbound slot is object-backed. A missing, null, scalar or array recipient entry produces no outbound `bidderparams`. A nonempty object is unwrapped as above; an empty object is omitted by the existing empty-map serialization rule. Do not widen the model, introduce a second raw slot or turn an ignored non-object supplement into a new auction-wide error. This deliberate outbound-shape rule is tested independently from impression supplementation.

The existing supplement merge remains object-to-object, fills missing keys recursively and never creates participation. Explicit impression values, including nulls, continue to win. A base bidder's supplement does not fill a requested alias's parameters. Request-level parameters remain unmodified for later recipients.

### 3.4 Ordering And Errors

Preserve the current sequence:

```text
normalize and resolve aliases
-> group participation once
-> decide privacy, sampling and caps
-> check existing supplement and projection funding
-> construct recipient impressions and root
-> typed decode and request validation
-> apply recorded disclosure restrictions and legacy projection
-> invoke bidder construction
```

Do not resolve aliases, consume another RNG draw, rerun privacy, add a permit or reconsider dropped bidders in the new builder. Disabled, privacy-denied, sampled-out and capped-out identities never materialize. Their original warning/status bookkeeping remains on the exchange response, not in another bidder's request.

Existing scoped-impression identity checks, merge-patch behavior, code-15 typed-roundtrip/validation failures and deadline precedence remain intact. An unrelated recipient's omitted request-only data is not newly parsed or validated. Corrupt private grouping invariants retain the current fixed internal-error path; diagnostics must not echo omitted content.

## 4. Resource Boundary

[`GroupedBidders::projection_charge`](../../../crates/auctioneer-exchange/src/split/resource.rs) continues to charge the original serialized grouped request per survivor, amplified supplements, selected parameter scratch and FPD copies with the existing checked arithmetic and factor. Do not lower this conservative allowance because fewer fields are retained. Preserve zero-survivor behavior, overflow refusal and refusal before destination allocations.

Direct selection avoids allocating the omitted root subtrees in each destination; it does not remove the one shared grouped input tree or claim that input never occupied memory. Selected parameter members and alias entries are copied from already-funded source content. Existing native allocation-bound owners must still demonstrate that construction fits the unchanged allowance; if a new temporary owner does not fit, return to review instead of raising a cap or changing the equation.

Keep completed projections alive in allocation measurements when production retains them. No maximum observed allocation becomes an allocator/guest bound, resource descriptor, smaller production charge or deployment-readiness certificate. The remaining qualifications assigned by #162 stay open.

## 5. Production And Test Ownership

The extension policy belongs to exchange splitting, not a bidder-specific switch, each adapter, the privacy classifier, a generic JSON sanitizer or a new framework. A small ordinary submodule is appropriate for the root builder; do not split unrelated auction code merely to support measurement.

Unit cases stay inline beside their production owner. Cross-module wire cases extend existing exchange privacy targets and reuse the capable shared fake bidder. Use `fake_` identities and an independent expected root object; first prove sensitive/competitor data is present in the inbound fixture. Preserve the grouped input and inbound request bytes across projection.

#176's request-level parameter-consumer concern is covered in two owned places: a shared fake observer checks the shape delivered by the exchange, and PubMatic-local conformance checks its real `wrapper`/`acat` consumer. Do not add a PubMatic factory or fixture to exchange/core tests or a handwritten cross-crate source import. A contract mismatch discovered between those owners is repaired at the shared shape boundary, not hidden by changing the capture.

This deliberately replaces #176's original exchange-level real-PubMatic acceptance criterion, not the consumer behavior it requires. During PR #202 review the user authorized amending only that criterion to the two-owner check. Record/read back the issue amendment before claiming acceptance, keep its other criteria unchanged and leave the issue open until implementation delivers both owners. Reuse the existing wrapper capture, valid root `acat` outgoing-wire unit and malformed-root rejection units; do not add a duplicate consumer test.

Change the existing grouped snapshot only at the reviewed outbound root paths. Keep response bookkeeping and all unrelated fields identical. Update the clone-first comparison oracle to construct the new expected root independently rather than cloning the original `ext`. Existing impression-scoping oracles remain independent.

The implementation also updates all five master passages: the root extension field row, step-7 competitor contract, step-9 materialization contract, deferred-FPD prose and deferred-FPD divergence row, plus the model comments that currently call `data`/`bidderconfig` pass-through. Inbound round-trip representation remains supported; bidder forwarding is no longer implied by a typed field.

## 6. Measurement Contract For #182

### 6.1 Purpose And Access

Measure the corrected production path before proposing typed-root reuse. Retain one benchmark owner in `auctioneer-exchange`, following the workspace's harness-less release benchmark convention. Its read-only/setup/reporting access is behind the existing `test-support` feature; bidder serialization uses the existing fake. No new feature flag, public serving API, handwritten source import, duplicated projection algorithm or benchmark framework is authorized.

The benchmark and serving orchestrator must call the same root, impression, decode and validation stages. Any extraction needed to observe those stages is behavior-preserving and limited to their owner. The benchmark may not reconstruct an easier request independently, bypass validation or serialize an unrelated surrogate. A smoke comparison must prove identical outputs and errors between measured and ordinary materialization.

Full repeated timing is an explicit measurement mode, not extra long work in every `cargo test --all-targets` feature lane. Normal test-mode execution performs bounded nonempty correctness/dispatch controls. Specify the exact Cargo/argument handling and a single existing hosted owner in the implementation plan; reject unknown modes and zero-case runs. Keep every existing CI test and feature lane; do not add macOS runners, another compiler lane or test-result caching.

### 6.2 Inputs And Phases

Freeze the input matrix and iteration policy before the first timing run. Numeric/size axes each have at least three admitted values; presence axes cover both states. Vary separately:

- Survivor count, with nonparticipating competitor data independent of it.
- Impressions per recipient.
- Ordinary root `ext` payload size, excluding request-only data removed by #176.
- User context size.
- Device context size.
- Object supplements: absence/presence and content size when present.
- Existing impression-specific FPD: absence/presence and patch size when present.

Use in-limit fictional fixtures. Record actual serialized input bytes, nodes, depth, selected names and impressions for each row; reject an unadmitted row rather than increasing a production limit. Hold other declared axes fixed within a sweep and record unavoidable structural overhead. Large omitted `bidderparams`/`bidderconfig` rows are separate isolation controls, not silently substituted for a retained-root size axis.

Report grouping, impression construction, root projection, typed decoding, validation and real fake-bidder bounded serialization separately, plus the complete operation. A phase that is small is still measured, not silently combined into another phase. Check actual projection and serialization counts against declared survivors/impressions; zero work, partial work and early errors are not successful samples. Verify expected output size/content outside timed windows and use `black_box` on inputs/results.

### 6.3 Allocation And Timing Integrity

Use fresh native processes and the existing thread-local counting-allocator support for allocation observations, not another hand-written allocator. Report cumulative requested bytes, allocation/reallocation call counts, retained requested bytes and peak growth with clear lifetimes. If call counts require extending the existing helper, retain its current default semantics and add independent positive/deallocation/reallocation controls; do not infer call counts from byte totals.

Fixture construction, registry setup, input hashing, expected-value construction, reporting and result-vector reservation occur outside measurement windows. Keep the same retained outputs and drop points as production. Use separate timing and allocation runs and document their instrumentation. Any test-support-only counter pause needed for timing must preserve existing allocator tests and be proven with a positive control. Include an unphased end-to-end control to expose observer overhead. Do not subtract overhead into negative or invented phase costs.

Record source revision/tree, dirty state, lock hash, toolchain, release profile, features, runner/CPU identity, mode, matrix and iteration/warmup policy alongside raw samples and summaries. A run containing uncommitted instrumentation must identify it and is not final source-bound evidence. Timing is observational, with no newly invented pass threshold or production-latency promise. Native timing is not a guest CPU/stack certificate.

Do not compare the old leaking output with the corrected output and call the difference a typed-root optimization. #182 evaluates the remaining materialization cost after the reviewed privacy delta. A future optimization needs the same corrected semantics on both sides, matched runs and a separate reviewed design. The issue closes only after committed measurements and an explicit recommendation: keep the implementation, or link that follow-up design. Writing the benchmark alone is not completion.

## 7. Required Controls And Acceptance

The implementation plan assigns each control to an existing owner and verifies that filtered tests actually run. These requirements are prospective, not claims of executed tests.

| Surface | Required controls |
| --- | --- |
| Recipient shape | Two allowed recipients with distinct params; base and request alias cases; exact own-object shape; no base fallback; no competitor alias; `/` and `~` names; request and grouped input unchanged. |
| Field dispositions | Independently enumerate every retained field and forbidden family; verify before-state presence. Non-`prebid` root extension survives. New unknown root-Prebid fields do not forward. |
| Presence | Absent/present-empty root ext/prebid; only-removed fields; missing/null/scalar/array/empty/nonempty object recipient parameters; no new auction-wide failure or manufactured host alias. |
| Privacy | Vary recipient, the existing GDPR `PassID` decision and `transmitUfpd` independently, with other activity/regulatory decisions allowing. Assert inbound presence and expected outbound preservation/removal of both `user.eids` and `user.ext.eids`, including `PassID=false` with `transmitUfpd=true`. Raw request-only FPD is absent in allow and deny rows. No new activity or privacy-schema field is introduced. |
| Selection | Independently privacy-denied, disabled, sampled-out and capped-out aliases; no materialization or leaked alias/parameter entries; unchanged response statuses, warnings and RNG behavior. |
| Precedence | Existing hoisted/typed/legacy impression slots, explicit nulls, nested object supplementation, aliases and nonparticipating supplements. Preserve scoped FPD errors and impression identity. |
| Allocation/funding | Existing projection-charge and sparse-copy owners pass unchanged except explicitly reviewed root expectations; retain independently varied impression/survivor/root sizes and underfunded refusal controls. Demonstrate that a large omitted subtree is not cloned per survivor, with a positive retained-data control. |
| Real consumer | Shared fake verifies exchange-delivered parameter shape; bidder-owned PubMatic conformance verifies the real consumer. Immutable captures and real bidder ownership remain in place. |
| Measurement | Exact nonzero case/phase/dispatch counts, all matrix axes, allocator positive controls, ordinary/measured-path equivalence, input/output validation and reproducible source-bound reports. Measurements cannot relax any acceptance gate. |

At least the following deliberately bad changes must be detected by their named owners: restore the old root copy; emit the full alias or parameter map; forward `data` or `bidderconfig`; select base parameters for an alias; replace the supplement merge with overwrite; skip typed validation; report a measurement row with no projection/serialization; and disable allocation counting. Run such experiments only in the implementation cycle's disposable owned environment, never another reviewer's worktree.

The complete authoritative GitHub matrix remains mandatory, including secure defaults, workspace feature lanes, once-owned compiler/producer controls and all adapter builds. Per the user's instruction, CREATOR does not execute CI checks locally. The planning baseline's hosted checks and eventual planning/implementation checks must be identified separately by commit, command, result and limitations; pending or unexecuted work is not green.

## 8. Boundaries And Delivery

Out of scope: request-level/bidder-configured FPD resolution (#59/#81/#94), multibid limits/filtering (#55), alternate-code policy redesign, currency conversion, supply-chain merging (#40), privacy-default/jurisdiction decisions, stored-object/account lookup, inbound body deadlines, typed-root reuse and resource-accounting reductions. None is silently completed by retaining or removing an extension field.

Before implementation, the user reviews this written contract, then an ordered implementation plan is written and reviewed in a separate planning PR containing both documents. Publication requires authorization; planning merge and implementation approval remain separate gates. The security implementation can proceed to acceptance without waiting for an optimization proposal or unfinished benchmark report. Keep #182 open until its measured outcome is delivered and keep #176 open until the actual projection correction and full acceptance are merged into the default branch.
