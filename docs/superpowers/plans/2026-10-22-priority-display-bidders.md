# Priority Display Bidder Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development and test-driven-development. Keep the operation contract separate from execution evidence.

**Goal:** Implement the 13 source-backed display flows and the additional priority APS integration (14 selected flows total), using reviewed source-specific or independent integration requirements rather than treating metadata, a shared endpoint or generic wire handling as migration.

**Architecture:** The data-only [priority contract](../../../crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml) records public identity, canonical schema, implementation key, exact source-owner blobs and required operations. Implementation discovery remains adjacent `bidder.toml` plus generated registration. No registry name list or runtime plugin loading is added.

**Tech Stack:** Existing Rust workspace, `serde`/TOML, the maintained schema compiler after qualification, existing capped sinks and sealed response decoding, shared EdgeZero configuration/secret handles.

## Status And Boundaries

This is Task 18A preparation, **pending design review**, not approval of its
remaining protocol decisions. The source audit read immutable revision
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`; no foreign source program ran.
The wave's `review_status = "pending"` must not be replaced by a fabricated
human approval. Source-owner files include adjacent tests and parameter tests,
not only the main implementation. The source revision binds fixtures and shared
helpers too; listed files are not an exhaustive dependency/behavior proof.

APS (`aps`) is the fourteenth requested priority identity. It is absent from
the pinned source catalog and therefore not represented in `priority.toml`'s
13 source-backed rows, operation counts or owner hashes. Obtain and review its
own protocol/schema, endpoint/authentication/settings, privacy and sync
contract and independent request/response fixtures before implementation.
Then use adjacent discovery in the existing `auctioneer-bidder` crate, not a
central registration edit. The publisher checkpoint requires both the 13
source-backed flows and APS; source-wave success alone cannot complete it.

On 2026-10-23 AG supplied a newer local reference revision containing APS.
The [supplemental APS contract](../specs/2026-10-23-aps-priority-integration-design.md)
pins that revision separately; it does not repin or expand this wave's original
358-identity inventory. AG approved the SDK/null-extension/region decisions,
and source review checked the 16 protocol/14 parameter expectations and
explicit differences. APS now has a discovered Rust protocol candidate and
adjacent immutable captures. Shared bounded ingress supplies original root
extension-shape provenance to its request context, and local candidate tests
exercise that guard. Operation/producer acceptance, metadata promotion, shared
schema/resource qualification and composed publisher delivery remain open.

`bidders check --wave ...` checks design shape and source identities only. A
successful report has `scope = "wave_design_only"`, pending review, zero
promoted definitions and zero migrated identities. `--require-complete` still
fails. Its digest identifies this design revision, **not** a successful test
run or an evidence receipt. Future Task 16 execution checking must consume
independently identified producer receipts before any selected-flow success.

The pending factory witness is a bounded external input, not Rust source
included from a test directory. Wave checks default to the existing shared
`crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml`; use
`--factory-bindings PATH` outside that repository-root layout. Its exact raw
SHA-256 is pinned before parsing, so a consistently remapped witness plus wave
cannot authorize a different source factory. The witness remains pending
review and never becomes serving registration or protocol qualification.

The unmerged wave format accepts single-line basic/literal TOML strings,
ordinary assignments, arrays and inline tables, with at most eight combined
container levels. Array-table headers are exactly `[[bidders]]`,
`[[bidders.operations]]` and `[[bidders.source_files]]`; comments are allowed.
Dotted assignment keys, other/quoted headers, a leading byte-order mark and
multiline strings are rejected before constructing the TOML tree. Dots and
delimiters inside strings/comments are not structural syntax. This admission
subset bounds structure; the maintained TOML parser and strict typed DTO still
validate the actual document.

The operation labels name **planned** Rust test owners, not executable commands
or existing passing tests. A label cannot select a shell command. Protocol unit
tests belong adjacent to their implementation. Cross-package behavior belongs
in the named integration targets below. Do not rename tests after a milestone.

The catalog's independent schema-work/cache/target-resource qualification,
funded strict serving join, host context, correlation and proprietary response
decoding gates remain open. This plan neither bypasses them nor makes R0 ready.
Source-local future media/browser branches remain required for full migration;
an unsupported-feature refusal cannot count as implementing that branch.

## Endpoint Policy Checkpoint (2026-10-28)

AG approved HTTPS-by-default selection, superseding this checkpoint's earlier
separate vendor TLS-mapping evidence requirement. Apply [spec section 6.1](../specs/2026-10-16-m7-bidder-catalog-design.md#61-baked-versus-runtime):
change only the scheme, preserving authority/port, path/query and bidding
protocol. Keep exact source URLs/blobs and effective transport-difference
records. HTTP requires an explicit scoped exception; preserve only the existing
`insecure-bidders` Mocktioneer loopback exception here. No global HTTP fallback
or automatic downgrade on HTTPS error. Absent/placeholder/unusable destinations
stay unavailable; source-disabled stays disabled unless separately approved.

| Identity | Approved Transport Selection | Remaining M7 Work |
| --- | --- | --- |
| `msft` | Select `https://ib.adnxs.com/openrtb2` from the pinned HTTP destination under AG policy. The same transport policy covers the existing AppNexus override, not shared protocol qualification. | Bind effective metadata, source URL/blob and transport difference; retain independent member/HB-source/chunking and extension/header behavior review and deterministic route tests. No separate vendor TLS contract is required. |
| `openx` | Select HTTPS on the unchanged `rtb.openx.net/prebid` destination from the [pinned source metadata](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/static/bidder-info/openx.yaml). | Bind effective metadata/provenance and run deterministic HTTPS/no-unapproved-HTTP route tests, not a separate vendor TLS-mapping review. Do not substitute browser or sync destinations. |
| `sovrn` | Select HTTPS on the unchanged `pbs.lijit.com/rtb/bid?src=prebid_server` source destination. | Bind effective metadata/provenance and test HTTPS routing plus source header/encoding behavior. No separate vendor TLS contract or browser API substitution. |
| `rubicon` | Retain the [pinned regional HTTPS configuration](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/static/bidder-info/rubicon.yaml): `exapi-us-east`, `exapi-us-west`, `exapi-apac`, `exapi-eu`. No invented endpoints or failover. | Freeze/review operator datacenter-region selection/failure policy, the existing scoped source-disabled reclassification and coupled credential/settings contract. No user-geo routing. |

The transport-selection implementation changes the authored pending fixture's
three source HTTP rows to `routing=https_default_pending`, with a new checked
design digest. Its 13 identities, 113 requirements and original source witnesses
are unchanged; no historical capture is rewritten. Native import/check now
binds typed scheme-only selections to source artifacts, independently of serving
metadata binding and deterministic route acceptance. The wave stays `pending`.
No runtime endpoint changes or probes occur here. HTTPS selection proves neither
connectivity nor live authorization. Live authorization, allowlisting, provider
credentials and deployed traffic tests remain M7.6; Rubicon's technical
regional/credential work and all deterministic selected-flow acceptance stay M7.

## Delivery And Files

| Public Identity | Rust Directory / Unit Filter                                               | Special Dependency                                                                                                                                                      |
| --------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| msft            | `src/bidders/msft/` / `msft::tests`                                        | 10-impression chunks, member consistency, HB-source settings; HTTPS-default binding and route tests                                                                     |
| ttd             | `src/bidders/thetradedesk/` / `thetradedesk::tests`                        | Baked alias, canonical base activation, SupplyId path/settings                                                                                                          |
| rise            | `src/bidders/rise/` / `rise::tests`                                        | First identifier selection and safe query construction                                                                                                                  |
| sovrn           | `src/bidders/sovrn/` / `sovrn::tests`                                      | Scoped identifying headers, ADM query decoding; HTTPS-default binding and route tests                                                                                   |
| yahooAds        | `src/bidders/yahoo_ads/` / `yahoo_ads::tests`                              | Per-impression calls, GPP relocation, exact sent-call coverage                                                                                                          |
| unruly          | `src/bidders/unruly/` / `unruly::tests`                                    | Typed extension rewrite and bidder-fatal input failures                                                                                                                 |
| sharethrough    | `src/bidders/sharethrough/` / `sharethrough::tests`                        | Media splits, cumulative block lists, currency conversion, version decision                                                                                             |
| openx           | `src/bidders/openx/` / `openx::tests`                                      | Media grouping, response/config metadata; HTTPS-default binding and route tests                                                                                         |
| kargo           | Rust protocol candidate at `src/bidders/kargo/` / `kargo::tests`           | Bounded passthrough and source status/media rules; site eligibility, activation and complete schema/resource/delivery gates remain open                                 |
| rubicon         | `src/bidders/rubicon/` / `rubicon::tests`                                  | Regional routing, coupled credentials, currency conversion, proprietary envelope                                                                                        |
| pubmatic        | Existing `src/bidders/pubmatic/` / `pubmatic::tests`                       | Complete transform/metadata audit, not automatic acceptance                                                                                                             |
| triplelift      | Rust protocol candidate at `src/bidders/triplelift/` / `triplelift::tests` | Bounded impression filtering, exact sent coverage, nested format/status/USD rules; approved case/ambiguity policy; delivery and wire parity open; not triplelift_native |
| criteo          | `src/bidders/criteo/` / `criteo::tests`                                    | Extension media/network metadata and IGI response branch                                                                                                                |
| aps (APS)       | Rust protocol candidate at `src/bidders/aps/` / `aps::tests`               | Supplemental pinned-source port; metadata promotion, full schema/resource qualification and composed delivery acceptance remain open                                    |

All directories above are relative to `crates/auctioneer-bidder/`. Add only the
implementation files/helpers each family needs and adjacent declarations;
do not create one crate per bidder. Public `ttd` metadata depends on
`thetradedesk`; the canonical base is not a fourteenth selected display flow.
Metadata, vendor, settings, schema and activation remain public-identity scoped
when a factory is shared.

Kargo protocol preparation (2026-10-23), not wave acceptance: the
[candidate plan](2026-10-23-kargo-protocol-candidate.md) covers the discovered
factory, inline units and five immutable source captures. AG approved only the
single case-insensitive media-key and bid-local ambiguity policy. The two
captures with `video.mimes: null` now compare unchanged after AG approved
presence-aware typed fields; non-object response extension behavior and complete
wire parity remain open. Its empty public definitions leave the effective
catalog unchanged. The whole-wave review/freeze and all delivery gates below
remain pending.

Triplelift preparation (2026-10-23) is scoped by its
[candidate plan](2026-10-23-triplelift-protocol-candidate.md). Its 12 immutable
captures include ten end-to-end cases and two explicitly separate shared typed
boundary controls. AG separately approved single case-insensitive parameter/
media keys and affected-object rejection of competing spellings on 2026-10-23;
this does not recover exact duplicate-key order. Presence-aware typed parameters
now distinguish absent from explicit null and preserve source defaults on wire.
The public definitions remain empty, and this does not complete the pending wave
freeze or response-shape/resource/delivery
requirements.

## 1. Review And Freeze The Design

- [ ] Compare every operation in `priority.toml` with its pinned implementation, adjacent tests, complete JSON fixtures and shared helpers. The manifest's owner-file set is machine-checked; behavioral review is still necessary.
- [ ] Review APS's independent integration contract, exact operation/fixture counts and fixed producers before implementation. Keep it prioritized alongside the source-backed wave; do not fabricate source hashes or count the current 13-row artifact as coverage of APS.
- [ ] Apply AG's 2026-10-28 HTTPS-default decision to the three original HTTP destinations without separate vendor TLS-mapping evidence. The native transport records and pending labels are prepared; complete serving metadata binding and deterministic HTTPS/no-unapproved-HTTP route tests. Preserve authority/port/path/query and bidding protocol and keep msft's protocol qualification distinct from AppNexus; scheme selection, factory equality, source activation or an unavailable row alone cannot pass display acceptance.
- [ ] Review Rubicon's four named regions, explicit source-disabled reclassification and versioned `{username,password,tracker}` bundle. No placeholder or secret value belongs in the contract, metadata output or diagnostics.
- [ ] Freeze Sharethrough's Auctioneer version value, TTD SupplyId path encoding/empty-default disposition, safe malformed/null handling, candidate-local error differences and source-versus-shared media classification. These are explicit reviewed decisions, not accidental differences hidden by broad assertions.
- [ ] Record source-specific status/error-code and currency tables. Pin 200/204/400/other separately; Kargo and Yahoo cannot inherit another bidder's 400 rule, and source currency behavior differs across families.
- [ ] Assign required positive/negative fixture counts to each operation and its fixed producer. The current design's operation list is not a claim that those fixtures or producer counts already exist.
- [ ] Record actual review and freeze the resulting contract digest in Task 16's requirements before 18B implementation. A changed source/schema/shared helper, operation or intentional difference requires renewed review; an implementation cannot trim its own requirements.

## 2. Qualify The Shared Paths

**Files:** the existing main plan Tasks 3, 4, 7 and 12-16 own these changes.

- [ ] Finish qualified schema cost/cache/target descriptors and original-deadline precharging. Invalid final scoped parameters remove only that bidder's impression before body allocation; expensive refusal is code 5, not schema-invalid code 2.
- [ ] Install the complete funded/cached metadata/factory join with no generic fallback. Preserve current eligibility and endpoint behavior until reviewed cutover.
- [ ] Add typed settings plus lazy bounded coupled secret reads; no new provider clients, eager credentials or secret caches in bidder singletons.
- [ ] Implement immutable sent-call correlation, bounded nonfatal diagnostics and the shared monotonic response decode receipt, including malformed envelope versus candidate-local failure. Do not retain/reparse outbound bodies.
- [ ] Bring forward a separately reviewed bounded currency-conversion prerequisite if required source branches cannot be implemented under the existing USD-only shared contract. Refusal-only display tests do not complete those branches.

Do not implement a permissive schema fallback or claim the selected flows usable
because their protocol bodies compile. M7 may progress while R0 blockers remain
explicit, but this does not waive the catalog's own qualification gates.

## 3. Port One Coherent Family At A Time

**Files:** each listed directory's `mod.rs`, adjacent `bidder.toml`, colocated
inline unit tests and private borrowed transforms, bidder-specific `fixtures/`,
and reviewed metadata/schema promotion in the same bidder-owned folder, using
the [asset migration contract](2026-10-23-bidder-owned-assets.md). Follow the
[common rules](../../../crates/auctioneer-bidder/src/bidders/README.md): only
conformance tests use separate files; process allocation and real cross-crate
controls stay with their boundary owners.

- [ ] Capture independent external positive/negative request and response expectations before writing the family. Preserve source revision, literal bytes and provenance; don't compute expected results with Auctioneer.
- [ ] Observe each missing behavior fail, then implement borrowing serializers through `OutboundRequestSink`. No request clone, arbitrary raw response access, prebuilt body vector or unbounded diagnostic channel.
- [ ] Exercise every operation and exact fixture count from the frozen contract. Vary all decision parameters independently; include mixed good/bad impressions, bids across seats and failed calls alongside valid siblings.
- [ ] Test local typed settings and parameter views with `validator::Validate` where applicable; generated schema validation remains the constraint source, not a second hand-written translation.
- [ ] Assert bodies, metadata, headers, coverage, source currency and typed error/provenance, not only `Result::is_ok()` or constructor existence.
- [ ] For msft, run 10/11/21-impression boundaries and conflicting members before adding any shared-code reuse. For PubMatic, audit all listed source branches; its unused source impression constant is not permission to invent chunking.
- [ ] Keep aliases requested-name attributed, with canonical schema and independent effective vendor/privacy identity. Confirm discoveries occur through adjacent declarations, without any central name edit.
- [ ] Run secure-default and feature package tests separately; commit only the verified family and its current evidence. An unfinished source-local branch leaves the identity in progress.

Unit command shape, once that family exists:

```sh
cargo test --locked -p auctioneer-bidder --lib msft::tests
cargo test --locked -p auctioneer-bidder --lib --features insecure-bidders msft::tests
```

An absent filter matching zero tests is a failure: the fixed producer must pin
its expected test/operation set and positive/negative dispatch counts.

## 4. Cross-Package And Publisher Checkpoint

**Files:** planned `auctioneer-exchange/tests/bidder_protocol_limits.rs`, existing
core `tests/usersync_catalog.rs`, planned core `tests/bidder_onboarding.rs`,
Task 16's evidence driver/checker and Task 17's CI guards.

- [ ] Drive actual generated-App dispatch for all 14 selected flows with source-derived or independently captured integration parameters/responses, real implementations and bounded test handles. A test-only fanout is not provider-network evidence.
- [ ] Require source-consistent serialized privacy behavior and zero calls for denied/schema-invalid/missing-credential work while retaining a valid sibling. Independently exercise each budget owner and original deadline.
- [ ] Run fresh snapshot enable/disable/re-enable without registry reconstruction, base veto and alias-specific disable. No public/implementation-name conflation.
- [ ] Reuse signed-cookie handlers and existing caps for declared sync endpoints. TTD's selected alias, Rubicon support-only and other absent declarations stay explicitly unavailable unless separately reviewed configuration supplies a legitimate contract.
- [ ] Bind the exact selected 14-name set (13 source-backed names plus `aps`), dependency closure, operation counts and required producers to a fresh run/attempt/commit. Require both the source-backed wave and APS contract/evidence; missing/stale/failed evidence cannot report selected success or migrated.
- [ ] Build all four shipped applications and run matched catalog-relative checks without claiming physical WASM fit from successful compilation. Keep credentialed/live Q2-D and publisher/provider execution in M7.6, after M7.5 and affected R0 prerequisites.

No threshold, workload limit, dependency, feature, funding equation or existing
external fixture changes merely to make this design pass.

## Preparation Verification (2026-10-22)

The checked design SHA-256 is
`b6a77bc7cfe7f78b95881f082ad3963c73b1f5a133c5e01748e2d223989fa2ac`.
This is pending-review design identity, not a successful migration receipt.

- Final maintenance-mode CLI unit run: 147 passed, none failed or ignored.
- Final `bidders_wave` integration run: nine passed, none failed or ignored, against the separately pinned local source checkout. Includes root-layout defaults, independent source drifts, coherent factory/wave remapping refusal and full-completion refusal.
- Earlier in this checkpoint, source and import integration targets passed 11 and five tests respectively; these preceded the final I/O-only dispatcher helper extraction. The latest wave run exercises the extracted checking path.
- `auctioneer-core --test ci_workflows`: 104 passed, none failed or ignored, including independent removal of the required wave target from CI.
- Full workspace/all-target Clippy with `insecure-bidders,conformance-suite` and `-D warnings`: passed. This compiles/lints the application-enabled CLI too; it does not execute its tests. Existing GVL deletion notices and dependency future-incompatibility warnings remain.
- Formatting, `git diff --check`, the raw-JSON gate (168 fixtures across 50 files) and that gate's fixture controls passed.
- Admission controls were observed failing before their repairs: 23 passed/two failed with no syntax admission, and zero passed/one failed with the BOM check absent. Final controls cover array-only, inline-table-only and mixed depths seven/eight/nine plus valid alternate TOML DTOs that only admission rejects. No stack-exhaustion exploit was executed or claimed reproduced.

An earlier application/default-feature CLI test build was stopped while
compiling and produced no test result; it is not counted as passing. No live
provider tests, new bidder protocols, schema-engine/resource qualification,
guest execution or physical memory-fit verification ran for this preparation.
