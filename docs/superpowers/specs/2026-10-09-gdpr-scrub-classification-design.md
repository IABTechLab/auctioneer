# GDPR Scrub Classification Design

**Status (2026-10-11):** sourced membership and compiled rules, the live
per-object mutator, prepaid construction costs and the bounded core audit
consumer are implemented in M4. Full native and alternate-feature checks,
strict native/WASM lint, executed WASI depth/cleanup proofs, vendor packages,
compiler mutations and charged-handler failure tests pass. Independent code
reviews found no confirmed blocker. The implementation is committed and pushed
as `c2b00a2`, inherited by verified M5 `9ff017f` and M6 `6177742` source
integrations. This status is not human
merge approval or hosted certification.
Current results and limitations are in the
[execution record](../sdd/2026-10-11-gdpr-scrub-classification.md).
Execution is governed by the
[implementation plan](../plans/2026-10-09-gdpr-scrub-classification.md).
**Implementation base:** M4, in `.claude/worktrees/m4-privacy`; M5 and M6 inherit
the verified change. M3b remains the conservative foundation, not a second
implementation target. Copies of these documents on M3b describe follow-on work.
**Scope:** preserve reviewed user/device context without weakening ID, geo,
FPD, activity, CCPA/LMT, COPPA, transaction-ID, or shadow protections.

## Goal And Non-Goals

The pre-cutover scrub closed unknown channels by clearing catch-all maps. Device
context and interstitial constraints have already been repaired by typing known
fields; most Geo context remains flattened. The remaining problem is to preserve
reviewed fields without depending on whether the Rust model has typed them.

Use one exchange-owned, per-object mutation path over the validated bidder-owned
typed request and its flattened maps. Keep the existing decode/validate boundary.
Do not add a full-request Value privacy walker, a second decode, a GDPR derive,
or a duplicate typed implementation retained solely for old tests.

This is field/schema enforcement, not arbitrary-string data-loss prevention.
Unknown fields inside protected boundaries remain closed when ID or geo
disclosure is restricted. A documented context string can still be abused by a
sender; unrelated bidder parameters and policy-owned FPD payloads are not
globally scanned for concealed personal data. No legal certification is claimed.

## Evidence And Execution Base

The reviewed M4 implementation exists at `78f57b7`:

- `crates/auctioneer-exchange/src/split/mod.rs` materializes survivor projections
  and consumes each into a typed request before validating it.
- `src/run/mod.rs` resolves bidder decisions and invokes composed scrubs.
- `src/scrub/composition_tests.rs` contains independent 256-decision and shadow oracles.
- Core prepays resource reservations before its asynchronous config/body work.

These are chronological M4 baseline facts, not claims that the same code was
present on M3b. The classified rules, audit consumer and resource-cost contract
below are now implemented in the M4 worktree. Recheck branch heads and the
plan's execution evidence rather than treating design approval as qualification.

## Standards And Policy

The reference is [OpenRTB 2.6-202606](https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/1debba8001f40eb14597102dec3205afddf811d4/2.6.md),
commit `1debba8001f40eb14597102dec3205afddf811d4`. Sections 2.6, 2.7 and 2.9
do not require forwarding unknown data after a privacy denial.

[TCF Policies dated 2026-05-29](https://iabeurope.eu/wp-content/uploads/TCF-Policies-TransparencyConsentFramework_Policies_Version-2026-05-29.5.0.b.docx.pdf)
inform the existing permission resolver. This change neither reparses consent
nor changes that resolver's purpose/vendor/SF1 rules. `pass_id` currently checks
purposes 2 through 10 under forced enforcement; it is not a purpose-3/4-only gate.

Coordinate rounding, CIDR masking, postcode/demographic removal, accuracy
omission, and conservative unknown removal are reviewed Auctioneer policies, not
rounding or masking algorithms prescribed by OpenRTB. Operators remain
responsible for their contractual and jurisdiction-specific obligations.

## Resolved Disclosure

Resolve privacy once per observable bidder/alias before its construction.
Convert the existing eight decisions into an immutable input:

```rust
enum GeoMode { Pass, Coarsen, RemoveAll }

struct Disclosure {
    deny_ids: bool,
    geo: GeoMode,
    deny_user_fpd: bool,
    deny_tid: bool,
    ip_keep_bits: IpKeepBits,
}
```

Preserve M4's union:

```text
deny_ids = !pass_id || !transmit_ufpd || ccpa || lmt || coppa
geo = RemoveAll if coppa
      Coarsen if !pass_geo || !transmit_precise_geo || ccpa || lmt
      Pass otherwise
deny_user_fpd = !transmit_ufpd || coppa
deny_tid = !transmit_tid
```

The mutation code never reads account policy, reparses signals, or loosens this
decision. GDPR AllowAll is a global no-op only if all composed restrictions are
absent. Bidders share neither mutable requests nor mutable decisions.

## One Rule Source

### Membership, Review Metadata And Rules

Keep these build-time inputs under `crates/auctioneer-config/data`:

- `openrtb-2.6-source.md`: exact vendored source, including attribution/license.
- `openrtb-2.6-source.toml`: explicit source URL, release, commit, digest, reviewer
  and review date. Review metadata is human-authored, not generated from today.
- `openrtb-2.6-catalogue.toml`: generated standard object/field/type membership;
  no privacy classes or independently copied review metadata.
- `supported-extension-catalogue.toml` and `disclosure-sources/`: independently
  reviewed extension schemas with immutable references and verifiable artifacts.
- `request-disclosure-policy.toml`: one disposition per scoped field, shape,
  transform/child/delegate where needed, and review rationale.

An offline host-only importer uses structured Markdown tables for User, Device,
Geo, UserAgent, BrandVersion, EID and UID. Missing/empty sections, duplicate rows
and unrecognized type notation fail extraction. It has no config dependency,
so a failed config build cannot prevent regeneration.

**Operator-command supersession:** CLI consolidation replaces the standalone
catalogue executable with `auctioneer openrtb catalogue`. The build-time
`auctioneer-openrtb-catalogue` library remains independent of the CLI and config.
Use maintenance-only CLI mode so config build failures cannot block these
commands; historical execution records retain their original commands.

```sh
cargo run --locked -p auctioneer-cli --no-default-features -- openrtb catalogue generate \
  --manifest crates/auctioneer-config/data/openrtb-2.6-source.toml \
  --source crates/auctioneer-config/data/openrtb-2.6-source.md \
  --output crates/auctioneer-config/data/openrtb-2.6-catalogue.toml
cargo run --locked -p auctioneer-cli --no-default-features -- openrtb catalogue check \
  --manifest crates/auctioneer-config/data/openrtb-2.6-source.toml \
  --source crates/auctioneer-config/data/openrtb-2.6-source.md \
  --catalogue crates/auctioneer-config/data/openrtb-2.6-catalogue.toml
```

The same explicit inputs reproduce the same rows on different dates and hosts.
Check validates the manifest and source digest without rewriting review metadata.
Builds and requests never fetch source artifacts. Warn when the last review is
over 180 days old; do not turn standards age into an outage or conflate it with
GVL freshness.

The read-only `check-openrtb-drift.yml` workflow checks the upstream document
weekly and on manual dispatch against the pinned digest and committed catalogue.
A mismatch fails the check and calls for review; the workflow cannot update pins,
catalogue membership, policy, or approval metadata. The schedule becomes active
only after the workflow reaches the default branch. This detects source drift;
it does not replace the independent policy review or the age warning.

Config's existing build path validates and emits small static descriptors.
For the reviewed extension subset, compilation also checks the exact declaration
bindings, source origins and bytes, excerpt ranges, and license pin independently
of editable catalogue/policy rows. These are membership/provenance checks, not
a second set of privacy dispositions or a general source-language parser.
Runtime lookup uses bounded schema/field IDs and sorted tables, with no
request-local registry initialization. Host parsing dependencies stay build/dev
only; no new production crate or policy-aware wire-model attributes are needed.

### Derived Domains And Shape Checks

Do not author `domains` masks in policy TOML. Derive direct domains from the
rule/transform/delegate, then propagate through child schemas. Protected open
objects include ID and geo for their unknown-field rule. Reject cycles, missing
children/transforms/delegates, unknown TOML keys, duplicate dispositions, shape
inconsistencies and extra invented standard fields.

A reviewed context leaf has no disclosure domain of its own, but its declared
shape must still be checked whenever its containing protected boundary is
restricted. A domain fast path cannot preserve `geo.city = {"uid2":"secret"}`
because city is context. Primitive arrays have reviewed element shapes;
object-valued context always names a child schema.

Unknown runtime input is not a catalogue error. Remove its whole subtree under
either protected-domain restriction; otherwise leave it alone. No missing-row
transmission fallback, position-only exemption, or runtime passthrough switch.

### Required Dispositions

Protected boundaries are User and Device, their direct extensions, both Geo
objects, structured UA/brand entries, EID/UID objects and supported descendant
extension schemas. Explicit FPD delegation is the only opaque-branch exception
here. Site/app/impression bidder parameters and unrelated request/Source
extensions are outside this classifier; exact TID ownership still applies.

The complete generated inventory and policy must agree exactly. The following
decisions are mandatory, not loose prose categories:

| Object                 | Fields                                                                                          | Disposition                                                       |
| ---------------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| User                   | `id`, `buyeruid`, `customdata`, `yob`, `gender`, `eids`                                         | ID-gated; EIDs removed as one collection                          |
| User                   | `keywords`, `kwarray`, `data`                                                                   | Named FPD ownership                                               |
| User                   | `consent`                                                                                       | Preserve admitted control value                                   |
| User                   | `geo`, `ext`                                                                                    | Declared child schemas                                            |
| Device                 | `ifa`, `didmd5`, `didsha1`, `dpidmd5`, `dpidsha1`, `macmd5`, `macsha1`                          | ID-gated                                                          |
| Device                 | `dnt`, `lmt`                                                                                    | Preserve admitted opt-out/control values; no reinterpretation     |
| Device                 | `ua`, `os`, `osv`, `make`, `model`, `hwv`, `language`, `langb`, `flashver`, `carrier`, `mccmnc` | Reviewed string context                                           |
| Device                 | `devicetype`, `h`, `w`, `ppi`, `js`, `geofetch`, `connectiontype`                               | Reviewed integer context                                          |
| Device                 | `pxratio`                                                                                       | Reviewed numeric context; retain integer/float representation     |
| Device                 | `ip`, `ipv6`                                                                                    | Mask the explicit address family under geo restriction            |
| Device                 | `geo`, `sua`, `ext`                                                                             | Declared child schemas                                            |
| Geo                    | `lat`, `lon`                                                                                    | Coarsen coordinates; remove on invalid transform                  |
| Geo                    | `country`, `region`, `regionfips104`, `metro`, `city`                                           | Reviewed string context                                           |
| Geo                    | `utcoffset`, `type`, `ipservice`, `lastfix`                                                     | Reviewed integer context                                          |
| Geo                    | `zip`, `accuracy`                                                                               | Remove on coarsening; COPPA removes all Geo contents              |
| Geo                    | `ext`                                                                                           | Unsupported opaque extension; closed under ID or geo restriction  |
| UserAgent              | `browsers`, `platform`                                                                          | BrandVersion element/child schemas                                |
| UserAgent              | `mobile`, `source`                                                                              | Reviewed integer context                                          |
| UserAgent              | `architecture`, `bitness`, `model`                                                              | Reviewed string context                                           |
| BrandVersion           | `brand`, `version`                                                                              | String and string-array context                                   |
| UserAgent/BrandVersion | `ext`                                                                                           | Unsupported opaque extensions                                     |
| EID                    | `inserter`, `source`, `matcher`                                                                 | Reviewed string provenance when IDs are allowed                   |
| EID                    | `mm`                                                                                            | Reviewed integer context when IDs are allowed                     |
| EID                    | `uids`                                                                                          | UID element schema; ID-gated collection                           |
| UID                    | `id`                                                                                            | ID-gated string                                                   |
| UID                    | `atype`                                                                                         | Reviewed integer context when IDs are allowed                     |
| EID/UID                | `ext`                                                                                           | Unsupported opaque extensions; closed under ID or geo restriction |
| User extension         | `eids`, `data`                                                                                  | Sourced extension EIDs and FPD ownership, not standard User rows  |
| Device extension       | `atts`                                                                                          | Preserve admitted control value                                   |
| Device extension       | `prebid.interstitial.minwidthperc`, `minheightperc`                                             | Sourced nested extension context with child schemas               |

Standard `User.eids` and extension `user.ext.eids` have distinct provenance.
Deprecated device hashes are real standard fields; `device.ext.ifv` and `ppid`
must not be invented as standard Device fields. Unsupported extension spellings
are covered by unknown removal, not a universal spelling denylist.

FPD delegation explicitly owns `user.data`, `keywords`, `kwarray` and
`user.ext.data`; it is not a generic opaque-context exemption. These branches
survive ID-only denial if their owning permission allows them. `customdata`
is ID-gated. TID ownership is exactly native `source.tid`, its direct shadow,
`imp.ext.tid`, `ImpExt.other["tid"]`, and `Imp.other["ext"]["tid"]`.
Unrelated Source extensions and ordinary request/impression IDs are not TIDs.

## Mutation And Representations

### Existing Validation Boundary

Use the existing lifetime:

```text
source admission and grouping
  -> resolved survivor decisions and scoped resource preflight
  -> existing owned projection with FPD/supplements
  -> consume/decode and validate once
  -> mutate bidder-owned typed slots and flattened maps in place
  -> adapter construction and bounded serialization
```

Invalid survivor FPD still fails before a denying scrub can erase it. Keep code
15 where applicable. Do not scrub the shared request, construct rejected bidders,
clone the whole request for privacy, serialize/decode it again for traversal, or
retry failures with unsanitized data. No additional full-request Value or typed
tree is introduced.

Wire models remain policy-free in `auctioneer-openrtb`; resolution remains in
`auctioneer-privacy`; descriptors remain in `auctioneer-config`; mutation, limits,
transforms and audit remain in `auctioneer-exchange`. Existing typed context repairs
remain. Do not type more Geo fields merely to obtain a privacy allowlist: reviewed
untyped fields can be retained safely by their schema and shape.

### Typed Slots And Flattened Maps

One production mutator visits typed slots and calls the same per-schema map
helper for flattened values and supported shadow containers. The shared pure
action function uses the compiled rule and immutable Disclosure; typed adapters
only provide access to slots, not a second list of privacy classes.

Exhaustively destructure scoped structs/backing fields without `..`.
Use generated field IDs for known slots and explicit unknown handling for a new
typed field absent from the pinned inventory. New model fields therefore require
an adapter update, but do not silently gain transmission permission.

Denied slots and all their shadows are removed. Visit declared containers and
every object-array element; check leaf/element shapes before preserving them in
restricted boundaries. Unknown subtrees are removed whole and counted once.
`Map::retain` may mutate its values, but must not preserve a container wholesale
merely because its name is allowed.

Keep M4's owning-restriction shadow semantics: typed Geo wins over a conflicting
shadow only when geo is restricted; shadow-only Geo uses the same rules. TID-only
denial touches only the exact TID locations; FPD touches only its owned branches.
No ID-only general typed-container precedence or normalization of unrelated
shadows. Existing projection serialization can collapse programmatic duplicate
keys before decoding; that is a representation boundary, not a privacy policy.
There is no separate full-Value path promising equivalent outcomes for ambiguous
typed-plus-shadow input.

Scoped programmatic tests exercise this same production mutator, not a second
compatibility implementation. Inspect actual serialized bytes with a duplicate-
aware visitor so a last-key-wins parse cannot hide a denied occurrence.

### Raw, Sparse And Null Values

An unsupported typed RawValue extension is removed under the applicable ID/geo
restriction without parsing it for disclosure. Do not introduce raw context
exceptions. The compiler derives storage constraints from the Rust model's
structured declarations throughout the source tree, separately from reviewed
wire membership. A missing or unclassified model slot is a compilation error,
not authorization to traverse it. Reviewed fields stored in a known object's
explicit flattened `serde_json::Map<String, serde_json::Value>` catch-all have
JSON storage; an absent object cannot claim that capability. Every
admitted slot whose type embeds `RawValue` requires `opaque`; adding another
object to the inventory does not bypass this check. This includes the currently
scoped `UserAgent.ext`, `BrandVersion.ext`, `Geo.ext` and `UID.ext`, as well as
raw slots on an expanded inventory. Child, object-context and object-ID rules
are rejected because these slots cannot be traversed by their typed adapters.
JSON-tree-backed extension fields are not subject to that raw storage
restriction, but their declared child schemas must also have known model storage.
Storage discovery covers direct and wrapped named types, renamed
imports and nongeneric local aliases, including `self::` aliases. Unsupported
local alias syntax and type projections reject. Cross-file aliases, inline
module models, cfg alternatives and macro-generated definitions require an
explicit compiler/adapter review before introducing them; the scanner is not
a general Rust type checker. Supporting a future raw child schema requires a
separately reviewed bounded parse/rewrite lifetime before changing that contract.

Option/JsonField/Box handling preserves admitted missing/null/empty states where
the existing model distinguishes them. Use `BrandVersion::fields_mut()`; do not
allocate backing fields for empty brands. Unrestricted composed decisions do not
run the privacy mutator and retain existing model serialization semantics.

### Geo Arithmetic And Intentional Changes

For successful finite results, retain `floor(v * 100 + 0.5) / 100`, including
negative/tie behavior, and existing IPv4/IPv6 masking. ID denial alone neither
masks permitted IPs nor rounds permitted coordinates.

**Intentional change:** the pre-cutover code returned non-finite input unchanged.
The live
transform returns an optional finite result and removes a coordinate on non-finite
input, intermediate overflow, or non-finite output. A finite `f64::MAX` input
is not a successful transform. Apply identical behavior to typed and flattened
coordinates; neither restore the original nor emit null/infinity as a repair.
Malformed masked addresses are removed.

**Intentional changes:** preserve reviewed untyped Geo context and kwarray under
ID-only denial; remove unknowns in Geo and every protected descendant under
either ID or geo denial; omit zip and original fine accuracy on coarsening.
COPPA empties every existing Geo object to `{}`, preserving absence, and dominates
coarsening. Record these changes and the replacement of old assertions explicitly
in the offline diff; no claim that old non-finite/catch-all behavior is preserved.
The 2026-10-12 approved policy also removes all four named user-FPD branches on
COPPA, even when `transmitUfpd` allows them. CCPA/LMT alone still do not remove
user FPD. This composition amendment is Auctioneer policy, not a field-classification
or IAB-standard change.

## Resource And Deadline Contracts

### Admission And Bounded Work

Use one resource-only source preflight before the engine's first clone or
serialization, regardless of disclosure mode. Bound typed/flattened structure
and embedded RawValue before recursive serialization or deletion. Count each
JSON value and each object key as a syntactic node; raw JSON is not one string
node. Reuse bounded JSON parsing utilities without allocating a temporary tree.
A counting serde traversal is acceptable only with pinned/tested RawValue
handling and a proven abort before over-depth descent.

Distinguish three allowances: admitted input, normalized input including bounded
implicit/header additions, and survivor projection including FPD/supplement
expansion. Derive those additions with checked arithmetic and preserve M4's
aggregate projection receipt. Do not reset a full budget per extension/bidder
or reject a valid normalized request by reusing only its original parser cap.

The normalization allowance includes escaped copied headers, two source-sized
derived domain strings and bounded implicit structure. Advisory Referer values
larger than the configured header-source cap are not parsed for domain
enrichment; a truncated URL must not invent a different host. The bounded page
copy remains independent. IDNA-expanded hosts larger than their admitted source
URL are likewise omitted, rather than creating unfunded derived copies. These
are intentional enrichment refusals, not privacy permissions or new parser caps.

Preflight merged depth/work before cloning its destination. The candidate ceiling
is 128 containers, root depth 1; prove 127/128/129 and decoder/merge boundaries on
all targets before accepting it. A different safe ceiling requires an explicit
admission-contract decision. Bound frames, visits, lookups and transform buffers;
no width-sized stack, path strings, per-deletion vectors or quadratic key matching.

Borrowed engine/mutator rejection leaves input ownership with the caller.
Arbitrarily deep programmatic objects require caller-owned bounded admission
before passing them to owned helpers such as `prepare`; this change does not
claim that those owned helpers are safe for unadmitted arbitrary-depth inputs.
Caller-created rejected inputs also require safe cleanup. Allocate engine-owned
projections only after proving safe clone/destruction depth. Include subtree
removal, unwinding and cancellation, not only a successful iterator loop.

### Cost Ownership And Reservation

Runtime owns a neutral `RequestConstructionCosts` value containing fixed
per-auction ownership, retained per-scope growth and reusable scratch charges.
Exchange computes its actual
frame/transform/audit layout costs; core adds its guard, observation and bounded
formatting costs. Runtime accepts the composed scalar descriptor alongside the
existing outbound-metadata charge. It never depends on exchange/core or copies
their layout constants.

Thread the same descriptor through BootEnvelope, installed boot derivation,
diagnostic re-derivation, reservations, all four adapters and their proof tests.
Prepay the new terms before allocating audit/scratch state. Include them exactly
once in disjoint phase reservations and total/per-auction derivation. Retained
per-bidder vectors remain part of M4's full lifetime proof; measure 1/2/max
survivors and independent input sizes. Sequential construction is a focused
alternative if the retained lifetime cannot fit, not permission to weaken caps.

Projection admission checks estimated copies, bounded retained growth for all
surviving scopes and the installed reusable scratch charge against the receipt,
rather than allowing copies to consume scratch headroom. Engine-owned minimum
costs cannot be lowered by a caller-supplied descriptor.

Canonical IPv6 formatting can expand a short valid address, including mapped
addresses and singleton zero compression. Retain standard formatting independent
of input capacity: fund at most 39 bytes per destination for both the typed and
flattened Device IPv6 slots. Reserve exact additional capacity fallibly before
clearing the destination; allocation failure aborts construction with a static
error, not a capacity-dependent address removal. That destination remains owned
across sibling scrubs and must not be charged as reusable scratch. Mutation is
allocation-free when the existing string capacity suffices; the contract does
not assert universal zero heap allocation.

Normalized wire allowance also covers bounded numeric reserialization. Short
scientific notation may become a longer decimal representation under the pinned
JSON serializer. Fund that lexical expansion from the admitted node limit,
separately from header/domain enrichment and without raising generic factors.
The pinned `serde_json` 1.0.150 formatter uses `zmij` 1.0.21's 24-byte buffer for
finite floating-point numbers; its admitted integer widths are smaller. Thus
the pinned serializer emits at most 24 bytes per finite JSON number, while
every admitted value already occupies at least one source byte. With body cap
`B`, copied-header cap `H` and admitted input-node cap `N`, the checked normalized
wire allowance is `3 * B + 8 * H + 189 + 23 * N`. The numeric term applies to
input nodes, not the extra implicit nodes; the implicit fill has its own fixed
wire charge. Tests independently exercise the pinned number-width bound and a
real HTTP request whose compact exponent tokens expand before denial removes
their containing subtree.

Let `P` be the estimated retained projection copies, `K` the surviving scope
count, `G` the retained growth charge and `S` the reusable scratch charge.
Live admission requires `P + K * max(installed_G, engine_G) +
max(installed_S, engine_S)` before materialization. Boot funds its configured
scope ceiling using the same descriptor. The exchange-owned current `G` is
`2 * size_of::<AddressBuffer>()`, or 78 bytes, for the typed and flattened IPv6
destinations; it is not multiplied into a generic clone factor.

Completion both observes and destroys the audit owner before
releasing its funding or auction admission slot.

An explicit fixed term is allowed; generic allocation factors, node limits,
target ceilings and artifact budgets must not be raised to hide overhead.
Admission arithmetic is checked; only diagnostic counters saturate.

### Error And Deadline Precedence

**Decision:** retain M4's validation-first precedence. Resolve the original tmax
early if needed to obtain a deadline, but do not skip survivor projection/
validation because it has already expired. An expired allowed survivor with
malformed FPD still yields code 15. Privacy-blocked/sampled/capped candidates
still never materialize.

After valid construction, check the original deadline before mutation/adapter
work, after restricted construction, between survivors, and before dispatch.
Drop expired built calls and preserve existing warning/seatnonbid bookkeeping.
Dropping refunds owned buffers and uncommitted resources; successful adapter
outcomes have already committed their call-count slots, which are not refunded.
These checkpoints neither start a fresh allowance nor preempt a
synchronous serde call; bounded work remains necessary.

On scrub work/scratch failure discard the scope, merge safe audit counters and
return a static `AuctionError::Internal`; construction failure dispatches no
calls. Existing input/supplement/projection errors retain their classes. No
partial outbound request, silent truncation or unsanitized retry.

## Audit Owner And Production Consumer

Exchange exposes plain fixed enum-indexed counters by schema/action/reason, plus
attempted/failed scope counts and a saturation flag. There is one auction
aggregate, not a retained per-bidder map. No values, raw keys, dynamic paths,
consent, credentials or copied bidder names are stored.

Remove a subtree/EID collection once. Absent fields, unchanged transforms,
already-empty COPPA Geo and repeated scrubs add no change counts. Distinguish
identifier removal, unknown removal, invalid shape/transform and COPPA removal.
Merge failed/zero-call construction scopes before dropping them.

Core owns an `AuctionDisclosureScope` with plain counters after prepayment and
before asynchronous config/body work. Pass its mutable audit through
`auction_inner` and the required `run_auction` parameter. No compatibility
wrapper discards it; no Arc/Mutex or audit handle in consumed request extensions
is required.

The concrete production consumer is core's `observe_request_disclosure`: it
derives a fixed summary and emits one bounded debug-level operational record via
the existing installed log sink, using static labels and aggregate numeric
counts/status only. The observer executes outside `cfg(test)`; disabling debug
logging is not an accumulation/publication mechanism. No endpoint, metric backend
or operator-visible dashboard is promised.

Normal success/error explicitly finalizes this scope before dropping lease/
reservations. Drop finalizes an unfinished scope as cancelled, exactly once,
while charged ownership remains live; declaration/nesting order enforces this on
every early return and suspended cancellation. Prepayment refusal allocates no
audit and has no scrub work to report. A post-prepayment parse/privacy refusal
reports zero scrub changes. A later failure retains earlier changes.

Formatting uses a fixed bounded buffer whose size is included in core's cost
descriptor. Do not dump the schema-counter array or form input-derived messages.
Test the real production observer, status/counts and reservation/drop ordering,
not only an externally held test handle.

## Verification And Cutover

All unit tests are colocated; separate binaries cover allocator, wire, workflow,
compile-failure and cross-crate integration. Names describe behavior, not phases.
Required evidence:

1. Every rule/shape and all four ID/geo combinations; all 256 composed tuples
   against M4's independent expected-wire oracle.
2. Presence and absence for every scoped standard field, especially dnt/lmt,
   all ten previously prose-only Device fields, Geo context and interstitial
   constraints; malformed context objects/array elements are removed.
3. Hostile unknowns in supported descendants, both raw and flattened extensions,
   typed/shadow conflicts, and actual duplicate-aware outbound-byte inspection.
4. Sparse/missing/null/empty and integer/float model round-trip controls; no
   additional whole-request parse, clone or empty-brand allocation.
5. Validation-before-denial and expired-plus-invalid FPD precedence; zero
   unsanitized dispatch on every construction failure.
6. Catalogue membership/digest/shape/child/delegate mutations and exhaustive
   typed-field compile coverage. Rules determine domains, not supplied masks.
7. Idempotence, stricter-decision monotonicity, composition/union equivalence
   and original/sibling isolation.
8. Exact audit semantics and real observer delivery on success, error, zero
   calls and cancellation before reservation release, with no secret fragments.
9. Full retained lifetimes, independent byte/node caps, cross-size allocation
   deltas, depth/work/destruction boundaries and checked-overflow refusal.
10. Native tests/lint, strict target lint, actual vendor packages/unchanged size
    gates, and active push/PR workflow mutation protection.

Run old/new offline on the same decisions, scoped inputs and adapter path before
switching the live engine. Use committed corpus, adversarial/shadow fixtures and
the 256 oracle. Every retained path and additional removal needs an independent
reviewed golden; the new classifier cannot generate its own expectations.
Reports have input/work/output bounds, static fixture/path labels for curated
cases, and aggregate-only external-input output.

Update old country-only/clear-all/non-finite assertions and the master spec's
current catch-all text in the same verified cutover. No production dual run,
passthrough toggle or partial catch-all relaxation. Compare serialized baseline
bytes for unaffected controls separately from the corpus's JSON-value equality
contract. Borrowed-validation probes from the previous design are historical
evidence, not qualification of this architecture.

Acceptance requires reviewed offline differences, the live-path tests, funded
costs and real audit consumption on M4, then verified propagation to M5/M6.
M3b production code is not expanded to duplicate this implementation.

## Alternatives And Residual Risks

The selected per-object design retains one decode/validation and one production
mutation path. The earlier full-request Value design required another typed
decode plus separate synthetic-shadow adapters without a production need; it
is not the default or an automatic fallback if this proof fails.

Exhaustively typing Geo is optional wire-model work, not necessary for reviewed
map retention. A shallow name allowlist is insufficient for nested objects or
malformed context shapes. A privacy-aware derive couples wire modelling to
policy and is not introduced.

Unknown context can still be removed; classifications and upstream pin updates
need human review; raw context extensions remain unsupported; unrelated
policy-owned payloads are not globally sanitized. None permits forwarding
unknown IDs or precise location through a restricted protected boundary.
