# API-backed stored objects and account policies

Date: 2026-11-09
Status: User-approved for planning and verified planning-PR publication; implementation and merge remain separately gated
Refreshed code baseline: `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`
Reviewed SDK: official EdgeZero PR #275, `e9ffaa8f8057144155ffe164280994445fa0f6d8`
Design issue: [#35](https://github.com/IABTechLab/auctioneer/issues/35)
Implementation owners: [#53](https://github.com/IABTechLab/auctioneer/issues/53),
[#77](https://github.com/IABTechLab/auctioneer/issues/77), and
[#78](https://github.com/IABTechLab/auctioneer/issues/78)

## 1. Purpose and scope

Allow an operator to change account policies and stored OpenRTB request and
impression templates without rebuilding Auctioneer. Fetch these records through
an operator-configured HTTP API, not a database connection. Standalone handlers
and an embedding host use the same preparation, policy and execution path.

There remains one operator configuration, authored as `auctioneer.toml` and
validated/pushed/read through the existing EdgeZero flow. The external API owns
record persistence, authorization and snapshot retention. Auctioneer supplies a
bounded client and resolver, not an API server, database driver, file fallback,
second configuration loader or process-wide data cache.

This design defines the shared contract for the three implementation issues.
It does not implement them, complete Trusted Server integration, qualify a
provider deployment, change workload limits or approve new resource reserves.

### 1.1 Current behavior and intentional changes

At the refreshed base, shared preparation is synchronous. Root stored-request
extensions survive inbound catch-all parsing, but #232's recipient-root
projection does not forward them to bidders. Neither root nor impression
stored-request controls have a resolver. `BidRequest` and `Imp` require IDs during
typed deserialization, so partial requests must be expanded before that decode.

`OperatorPolicy` currently contains `account_defaults` and an inline `accounts`
map. Account discovery precedes final OpenRTB preparation. Missing named accounts
fall back to defaults. None of those facts implies an existing API client.

The proposed cutover deliberately:

- Makes shared preparation asynchronous, retaining settings as its first argument.
- Expands partial JSON before full OpenRTB deserialization/validation.
- Replaces the inline named `accounts` map with API records, not a second lookup
  alternative. Product defaults and `account_defaults` remain local policy.
- Refuses a selected account that is absent, denied, malformed or unavailable;
  it never silently substitutes default policy for that account.
- Rejects duplicate object keys before merged documents can erase their meaning.

OpenRTB defines the auction wire objects, not this record-storage protocol.
Publisher IDs are seller/account selection claims, not proof of caller identity.
See the [OpenRTB 2.6 Publisher definition](https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/main/2.6.md#3215---object-publisher)
and [extension rules](https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/main/2.6.md#29---customization-and-extensions).

## 2. One runtime configuration

Add one optional, strictly typed top-level `auction_data` block to the canonical
operator model in `auctioneer-config`, alongside `host` and `account_defaults`.
It configures the data source, not host-policy inheritance; do not nest it under
`host` or add a second accepted spelling. The API URL is fully customizable in
`auctioneer.toml`:
host, port and path are operator choices within the HTTPS requirement below.
Auctioneer must not append a hardcoded version or lookup path, derive separate
URLs by record kind, or allow
an auction/template/API response to supply or override that URL.

Proposed example, not a configuration accepted by today's binary:

```toml
schema_version = 2

[auction_data]
endpoint = "https://data.example.test/custom/auction-lookup"
namespace = "fake_operator"
credential_key = "fake_auction_data_token"
require_account = true
# Optional replacement for the default ordered account-ID selectors.
account_id_paths = ["/site/publisher/id", "/app/publisher/id"]
# Relative to a found account row's document, not the response envelope.
# Omit or use "" when document is already the AccountPolicy object.
account_policy_path = "/auctioneer"
```

`endpoint` is the complete batch API URL. It must be absolute HTTPS, with a
nonempty host and without user information, query, fragment or template macros.
An operator may choose an internal service hostname; this is not a requirement
to make tenant data public. Requests do not influence host/backend selection.
No redirects are followed. Existing adapter egress/backend bindings still apply.

`namespace` is a nonempty, operator-selected record namespace, not a field read
from OpenRTB. `credential_key` is a nonempty key in the existing bound secret
store, not a credential value or another store binding. The client sends its
bounded secret value as `Authorization: Bearer ...`; reject invalid header bytes
without retaining them. Do not place credentials in URLs, diagnostics or config
examples. No unauthenticated mode or new SDK secret-injection mechanism is added.

`require_account` defaults to false and controls only an absent account selection.
It applies to auctions and the existing account-selecting `/cookie_sync` and
`/setuid` routes; it does not invent account selection for `/getuids` or `/status`.
With no selection, false uses product defaults plus `account_defaults`; true
refuses. With a nonempty selection, the API must return a valid account record
regardless of this flag. A disabled valid record still refuses the auction.

With `auction_data` absent, requests without stored references or a named
account use defaults and perform no lookup/secret read. Requests requiring a
record fail as unconfigured; they do not treat records as missing/defaulted.
An invalid supplied block is a configuration error, not a disabled source.

Remove `accounts` from the new operator schema and use application format 2 as
an explicit hard cut. Do not accept format 1, an empty legacy `accounts` map,
dual-source modes or legacy environment aliases after this cutover. Update
current examples and CLI validation together; preserve historical evidence.
The API protocol version below is independent of this operator-schema version.

Push-time validation checks source structure, URL safety, secret-reference shape
and effective default policy without fetching records or secrets. Selected API
records still require bounded read-time validation: they can change after a
config push. This is validation of external input, not a second config system.
An authoring environment override, when the key exists in the file, uses the
normal `AUCTIONEER__AUCTION_DATA__ENDPOINT` naming convention, not a legacy
`HOST` namespace or an independent serving-time environment reader.

### 2.1 JSON Pointer field selection

Use [RFC 6901 JSON Pointer](https://www.rfc-editor.org/rfc/rfc6901) string syntax
for the two scoped selections below. There is no jq interpreter, expression
language, transformation, wildcard evaluation, recursion or per-field policy
rewrite. Literal member names and positional array indices follow JSON Pointer;
`~0` and `~1` escape `~` and `/`. URI-fragment syntax is not accepted.

`account_id_paths` is an optional, ordered, nonempty list of nonempty JSON
Pointers evaluated against the post-root auction document. Supplying it replaces
the default list; it does not append hidden fallback paths. Missing results,
null and empty strings advance to the next path. A present non-string result
is invalid input, not an instruction to try a more permissive account. The
first nonempty string is the selected account ID, without trimming or coercion.
If none is found, apply `require_account`. Never infer caller authorization
from a successful selector. Stored impressions cannot reselect the account.

Account-ID selectors have a field-origin restriction, not merely valid syntax.
After JSON Pointer decoding, the first token must be `app`, `site` or `dooh`,
followed by `publisher`. Accept either `id` as the third and final token, or
`ext` followed by at least one further token.
These are JSON member tokens, not string-prefix matches or filesystem paths.
This preserves the default parent-account paths and operator-specific publisher
metadata such as `/site/publisher/ext/network/account_id`. Reject all other
locations during config validation, including `/user/consent`, `/regs/gpp`,
user/device identifiers, impression data, headers and bidder-specific FPD.
A bare publisher or extension object is not an account-ID location.

Only the selected account ID is copied into the lookup; never copy its parent
object or evaluate a fallback outside the configured list. The operator must
designate a seller/account identifier, not repurpose a publisher extension for
consent or browser identity. This enforces data origin, not semantic recognition
of arbitrary strings: a caller can put unrelated data in a publisher ID too.
Neither this restriction nor a matching record proves caller authorization.

Omission, including an absent `auction_data` block, uses the current precedence:

```text
/app/publisher/ext/prebid/parentAccount
/app/publisher/id
/site/publisher/ext/prebid/parentAccount
/site/publisher/id
/dooh/publisher/ext/prebid/parentAccount
/dooh/publisher/id
```

`account_policy_path` is one optional JSON Pointer applied to each found account
row's `document`. Its default is the empty pointer, which selects that complete
document. For example, `/auctioneer` selects the policy inside
`{"auctioneer": {"disabled": false}}`. It never selects from the batch envelope,
changes row identity/status/snapshot, or applies to request/impression templates.
The selected value must be an object decoded and validated as `AccountPolicy`.
Missing, null, non-object or malformed selected policy is a malformed account;
never replace it with defaults or fall back to the containing object. Unselected
wrapper fields have no policy effect but remain subject to the same full-document
syntax, duplicate-key, byte, node and depth admission bounds.

Both settings are operator-controlled and captured with the one accepted policy.
Validate syntax, escape sequences and bounded selector storage during CLI/policy
validation; reject invalid configuration before serving adoption. All selector
traversal and temporary storage also enter the existing request work/resource
admission; a simple path is not an allocation or CPU exemption. Use existing
structured JSON facilities, not a handwritten query engine or another parser.
These selectors locate one account ID and one typed policy object; arbitrary
field-by-field remapping or disabling policy validation is outside this design.

## 3. Versioned batch API

### 3.1 Transport and request

POST JSON to exactly the configured endpoint. Protocol version is in the JSON
body, not inferred from a URL path. Use `Content-Type: application/json` and
`Accept: application/json`. Construct only the declared control envelope and
the admitted record IDs. Do not forward cookies, incoming request headers,
known consent fields, bidder credentials, publisher objects or the whole
auction. Account-ID field origins are restricted by section 2.1; a syntactically
valid pointer alone must not authorize reading a privacy signal as an ID.

Protocol version 1 requires the service to return uncompressed JSON. Send
`Accept-Encoding: identity`, without request `Content-Encoding`. Client coding
admission uses the response headers exposed AFTER SDK normalization: accept an
absent `Content-Encoding` or a single identity token; visible compressed,
stacked, unknown or ambiguous codings are protocol failures, even if decoding
would produce a valid API document. No automatic decompression, decompression
fallback or coding negotiation is part of this API version. This restriction
concerns the new lookup API, not bidder response compression or existing workload.

Use the existing SDK `ResponseDecoding::Original` and streamed response mode,
apply the SDK-error precedence in section 7.1, admit returned status/encoding
before application body collection, then collect with
`into_bytes_bounded_until` under the original deadline and remaining allowance
defined in section 6. Do not switch back to SDK buffered-response mode before
this admission; its request builder mode setters are last-write-wins. Original
mode is load-bearing: the ordinary decoding path can remove coding/length
headers and exposes no original encoded-byte count for cumulative accounting.
Original mode preserves entity bytes, NOT an unmodified origin-header block.
The SDK removes Connection-nominated and other ineligible header fields before
exposing them, including a nominated `Content-Encoding`. Do not infer that an
absent normalized header was absent at the origin, bypass normalization, or claim
that Auctioneer rejects origin labels the SDK no longer exposes. Under this
explicit normalized-header contract, a valid JSON body whose coding label was
removed is evaluated as an unlabeled response under every body/JSON/API bound.
The same removal never authorizes decoding: gzip bytes with no exposed coding
header remain original bytes and fail JSON admission. A requirement to attest
all original coding labels would need a separately reviewed SDK capability; it
is not a hidden prerequisite or a guarantee of this client contract.

```json
{
  "protocol_version": 1,
  "namespace": "fake_operator",
  "snapshot": null,
  "lookups": [
    {"kind": "account", "id": "fake_account"},
    {"kind": "imp", "id": "fake_banner"}
  ]
}
```

Every control field shown is required. `snapshot: null` requests the latest
committed coherent snapshot at service acceptance. A subsequent batch supplies
the returned nonempty opaque snapshot string instead. `lookups` is nonempty;
deduplicate and sort exact `(kind, id)` pairs before serialization. Kinds are
exactly `account`, `request`, and `imp`. IDs are nonempty strings; do not trim,
case-fold, decode them as URLs, or interpolate them into a path/query. Their
length and cumulative storage are bounded by the admitted document/batch caps.

The service credential authorizes the configured namespace. Each row must also
be authorized under that service contract. A selected account or template ID
does not grant that authority. This protocol adds no end-user authentication;
a deployment needing per-caller tenant authorization must enforce it through
its host/service policy rather than assuming a publisher ID authenticates it.

### 3.2 Response and record shapes

A successful batch is HTTP 200 with one JSON object:

```json
{
  "protocol_version": 1,
  "snapshot": "fake_snapshot",
  "results": [
    {
      "kind": "account",
      "id": "fake_account",
      "status": "found",
      "document": {"auctioneer": {"disabled": false, "debug_allow": false}}
    },
    {"kind": "imp", "id": "fake_banner", "status": "missing"}
  ]
}
```

Response order is irrelevant. Require exactly one result for each requested
pair and no others. Reject duplicate rows, missing rows, extra rows, mismatched
IDs/kinds, unknown versions/statuses/control fields, duplicate JSON keys,
trailing data and non-object control envelopes. Success requires a nonempty
snapshot, even when all rows are missing. Never partially accept a bad batch.

Result statuses are `found`, `missing`, and `denied`. `found` requires a non-null
object `document`; the other statuses forbid `document`. Authentication failure
for the whole service is not a row marked missing. An invalid stored record is
not missing: the service must fail the batch, or the client rejects its document.

A found `account` document supplies the policy object selected by
`account_policy_path` (the whole document by default). That object is the
existing strict sparse `AccountPolicy` model, not an operator-policy root or an
effective `AccountConfig`. Reuse nested validators, explicit-clear/inheritance
rules and effective-policy validation. Unknown fields, operator-level
`host`/`resource_limits`/`bidders` blocks and arrays in place of objects are
invalid. The existing account fields `max_transport_bytes` and
`max_decoded_bytes` ARE permitted requested caps; they do not alter installed
capacity. Apply product defaults, then `account_defaults`, then this one overlay.
A record cannot change the installed resource capacity or the API source.

Keep the same admitted overlay available until BOTH effective `AccountConfig`
and scoped response budgets have been projected. `AccountConfig` alone is not
a lossless representation of `AccountPolicy`: today it carries no response-cap
fields, while `config::budgets::resolve_policy_budgets` independently reads the
inline map. Replace that lookup with this selected overlay, not a synthesized
operator policy, a second API call or a default-only budget projection.
Resolve each response-cap field independently in the existing order: requested
bidder override, alias-base override, selected remote account, `account_defaults`,
runtime host, then product-host/legacy fallback. Preserve the existing final
installed-envelope clamp and warning ownership. Freeze both projections from
the same snapshot before dropping the overlay; downstream execution must not
re-resolve the account by name or consult a different source.

A found `request` or `imp` document is a partial OpenRTB object of that kind.
Do not require it to be a complete valid auction on its own. Reject duplicate
keys, malformed shapes, forbidden nested references and structural/cap failures
before merge; validate the final expanded request using canonical ingress.
Unknown OpenRTB extension members retain existing canonical treatment. Do not
confuse their extensibility with the closed API control envelope.

### 3.3 Snapshots, failure and freshness

The service pins every row in a batch to the returned immutable snapshot. A
later batch with that token must read exactly that snapshot, including missing
records and authorization results. It must not silently substitute newer data.
Reject a changed token. HTTP 409 means the requested snapshot cannot be served;
refuse this preparation without retrying at a new snapshot.

For a response head successfully returned by the SDK, all other non-200 statuses,
including endpoint 404, are service failures, not record absence. SDK refusals
before a head is returned follow section 7.1 instead. Do not deserialize response
bodies from failed HTTP responses
or echo them. No automatic redirects, retries, fallback source, stale account
reuse or negative cross-request cache is allowed in the initial implementation.
The source must bypass SDK/provider response caching for these requests and
return `Cache-Control: no-store`. A service may cache internally only while
preserving its latest-committed and pinned-snapshot contract. This does not
promise instantaneous visibility of a write that the service has not committed.

Each new auction starts with `snapshot: null`. Therefore a completed external
record update can be observed on the next lookup without redeploying Auctioneer.
An in-flight auction remains on its old snapshot. A secret or operator-config
update affects later captures, not a half-completed auction.

### 3.4 Other existing account consumers

`/cookie_sync` and `/setuid` currently select account policy from the same inline
map using their existing account input. Under the hard cut they must call the
same account-only resolver and validators, using a single initial-snapshot batch
with one account key. Do not keep an inline map, silently omit account privacy,
or duplicate account merge semantics in these handlers. Their existing input,
opt-out and bidder eligibility checks remain in place before account-dependent
side effects. An absent optional account uses defaults without a lookup.

These routes retain their original auxiliary admission/deadline/funding, not an
extra auction permit. Their API/secret/body/record storage must be included in
the auxiliary resource derivation before activation, with fixed errors mapped
through their existing route response shapes. A failed account lookup cannot
emit a sync URL, set a UID or mutate a cookie. `/getuids` remains unchanged; no
new account lookup is introduced there. Account-only requests do not fetch
templates or send browser identifiers/consent to the API.
These routes keep their explicit account parameter, rather than evaluating
auction `account_id_paths` against a query/cookie document. Their returned
account record uses the same configured `account_policy_path` and validators.

## 4. Finite expansion and precedence

### 4.1 References and stages

Support root `ext.prebid.storedrequest` and impression
`ext.prebid.storedrequest` as exact objects `{"id": "fake_template"}`.
An absent member means no reference; null, empty ID, wrong type or additional
reference-control fields is invalid. These technical identifiers are extension
conventions, not attributes mandated by OpenRTB itself.

Preparation performs at most two logical API calls, sequentially:

1. Bound and validate live JSON syntax, duplicate keys, reference shapes and
   original deadline inputs. If there is a root reference, look up that request
   alone, consume its reference, merge root template with live input, and keep
   the returned snapshot. Otherwise use the live document directly.
2. Discover account selection and all impression references from that post-root
   document. Batch the selected account and distinct impression IDs at the
   pinned snapshot, or request the initial snapshot if step 1 made no call.
   If no records are needed, make no empty API call. Validate the whole batch,
   expand impressions, resolve the selected account and validate final ingress.

Stored root templates may contain impression references, but may not contain
their own root stored-request reference. Stored impression templates may not
contain an impression stored-request reference. Reject these even if a live
overlay would hide them. There is no recursive fetch, reference graph, cycle
retry or implicit default template. Cycles fail through this explicit nesting
rule. Two stages suffice; an implementation must not grow a third to recover.

The nesting prohibition also covers recognized bidder-FPD patches. In particular,
an entry in `imp.ext.prebid.imp` may not contain `ext.prebid.storedrequest`, even
with null or an invalid value, and even when it would later be overwritten.
Check these semantic patch locations in live and stored documents before merge;
do not recursively treat every similarly named bidder-protocol field as a
reference. A live violation is invalid input; a stored-document violation is
invalid source data. Neither starts another lookup.

Deduplicate per auction by `(kind, id)`, never by ID alone. Repeated impression
references fetch one document but produce independently merged impressions;
fetch deduplication does not exempt repeated expansion from byte/node/work caps.

### 4.2 Merge rules

Merge a stored object as the base and the live object as the overlay, using
[RFC 7396](https://www.rfc-editor.org/rfc/rfc7396) object semantics: recursively merge objects, replace arrays and
scalars, and let explicit null remove a member. Missing overlay fields inherit.
A live root `imp` array replaces a stored root array; do not concatenate or
match elements by ID. Impression templates are then merged with each selected
impression independently. Remove the consumed stored-reference member while
preserving its extension siblings; no lookup token becomes bidder payload.
After all shared bidder-specific FPD projection, check the reserved root and
impression stored-reference paths again before bidder request construction or
dispatch. A reintroduced control is the existing typed
`InvalidImpFirstPartyData` failure with a fixed diagnostic, not a new reference
to resolve or a field to silently forward. Preserve that failure's existing
auction scope and HTTP mapping. Admission checks and this final guard must both
exist: early cleanup alone does not constrain a later mutation.

Current materialization projects the grouped root through a finite forwarding
set before typed decoding; it already omits unrecognized root controls. Check
the grouped root's reserved path before that projection can erase it. Check
scoped impressions after their shared FPD mutation. Silent removal by root
projection is not evidence that the required refusal guard executed, and this
feature must not widen the finite forwarding set to make a test reach it.

A nonempty explicit live request/impression ID wins; a template may supply an
omitted ID. Null/empty final IDs are invalid. A final impression array must be
nonempty and contain unique, nonempty IDs. Do not mint IDs, coerce types or
repair malformed creative/media objects as a compatibility shortcut.
Canonical final preparation still applies its existing inventory, media,
privacy and supported-wire validation; this design does not claim that today's
validator checks every normative OpenRTB constraint.

Validate participating JSON before materializing a last-key-wins map or mutating
any source. Use existing Serde/structured APIs and bounded admission helpers;
do not build another lexical JSON parser. Preserve absent/null distinctions
through expansion, then let canonical typed decoding decide final field shape.
Operator `AccountPolicy` overlays are NOT JSON merge patches: their explicit
clear and nested inheritance semantics remain unchanged.

Account selection uses the configured `account_id_paths` on the post-root
document, or the ordered defaults specified in section 2.1 when omitted. An
impression template cannot change root inventory or reselect the account.
Neither a template nor a remote account supplies host policy, API credentials,
registry bindings or permission to use another namespace.

The typed embedding input uses its canonical serialized JSON representation as
the overlay, not a claim to recover raw fields it never represented. Shared
behavior is required for representationally equivalent JSON/typed inputs;
lossless byte parity across missing/null/number spellings is not promised.
Retain the original JSON extension-shape observation for existing diagnostics;
the expanded request is the input to account/privacy/auction decisions. Typed
input keeps its existing unavailable-original-shape semantics.

## 5. Shared asynchronous boundary and lifetime

Extend `AuctionServices` with a borrowed, host-supplied stored-object source and
make `prepare_auction(settings, input, services, funding)` asynchronous. Keep
settings first. Both the standalone HTTP wrapper and Trusted Server call this
same preparation API; neither performs its own merge/account lookup first.
Prepared execution remains the existing operation/result path.

The portable resolver owns batch planning, response admission, expansion,
account policy and fixed errors. Host integration supplies existing EdgeZero
HTTP and secret-store services. Capture the accepted source configuration,
service handles and original scope once; do not fetch operator config again or
recapture rotating secrets between stages. An unavailable source is allowed
only when this particular input needs no lookup.

All lookup and merge work retains the original auction permit and funded
receipts. Do not acquire another auction permit, detach work into a background
task, keep a host callback alive by retaining a weak client handle, or return a
completed result with unfunded storage. On cancellation, deadline, parse error
or drop, release request-owned data and receipts exactly once. Preserve the
existing borrowed-input/owned-prepared-state separation; no self-referential
result, whole-config clone or process cache is needed.

Time starts at original admission, not the first bidder call. The lookup
deadline is the earliest of the host outer deadline, admitted live `tmax` and
host default/max policy anchored to that timestamp. Absent live `tmax` uses
the host default; malformed live `tmax` is invalid before I/O. A template may
shorten the remaining auction deadline, never extend it. Check expiration before
each fetch, after awaited I/O and during bounded expansion. A per-fetch SDK
timeout cannot reset or extend this absolute deadline. No bidder dispatch occurs
after a preparation failure or exhausted deadline.
Store the captured, possibly shortened absolute deadline in the prepared
operation and forward it to execution; execution must not re-anchor it at the
end of lookup or reconstruct a longer budget from the merged `tmax`.
Reuse the existing `resolve_tmax` default/max semantics at the original time
anchor, including `tmax = 0` selecting the host default; do not treat zero as an
immediate-expiry budget or cap an explicit positive `tmax` by the host default.

## 6. Bounds and accounting acceptance

Existing limits remain authoritative. Below, B is the installed/effective ingress
byte cap, N the ingress JSON-node cap, I the impression cap, T/D the installed
encoded/decoded response caps and H the common bounded outbound facade's
response-header caps. Resolve caps
from the accepted profile and operator requests through existing clamping; do
not introduce an unfunded independent source profile or silently alter a cap.

Lookup T/D come from the accepted host resource row after existing
`limits_for_request` narrowing against the installed profile. They are frozen
before record I/O and are not the per-bidder budgets from
`resolve_policy_budgets`. Named remote accounts, `account_defaults` response-cap
overlays and bidder/alias overrides do not change the lookup allowance; those
caps govern the later bidder-response projection. A record cannot retroactively
change how its own lookup was admitted or reset the cumulative counter.

| Work or storage | Required bound |
| --- | --- |
| Live, post-root intermediate and final request | Each <= B, N and existing construction-depth cap; final impressions <= I |
| API batch request | Existing outbound request-body/header caps, checked before dispatch |
| API calls and requested keys | At most two calls; at most I + 2 distinct pairs across both stages |
| API encoded/decoded bodies | No decoding; one observed original entity/JSON-body total obeys BOTH T and D cumulatively under normalized-header admission |
| API headers | H and existing header-count cap per response, enforced by transport |
| All template documents | Cumulative serialized template bytes <= B; all response JSON nodes share an aggregate N cap |
| Account document | Also obey existing config-model byte/node/depth admission caps and nested validators |
| Dedup, IDs, snapshot, errors, credentials, selectors | Included in admitted config/batch/secret and aggregate retained-storage/work bounds, never unbounded side tables |
| Repeated expansions and merge traversal | Charge every visit/output against the admitted construction/work budget, including overwritten/deleted values |

Do not multiply a per-template allowance by I, apply the bidder raw-body decoder
as an unbounded convenience, or check size only after constructing a full tree.
The SDK's bounded body collection is necessary, not a JSON or allocation proof.
Exact JSON end-of-input, duplicate-key, node and depth checks precede mutation.

### 6.1 Observable cumulative body accounting

For a preparation, let `C = min(T, D)` and initialize `used = 0`. Before each
lookup compute `remaining = C - used` with checked arithmetic. If another batch
is needed and no allowance remains, refuse before dispatch. Pass `remaining`
to both SDK encoded and decoded response limits, and to the final bounded body
collector. After a complete admitted identity body, add its observed byte length
to `used` with checked arithmetic and retain that counter for the next stage.
Count the entire JSON envelope, including IDs, snapshot and wrapper fields,
not just the selected template/account objects. Auxiliary account resolution
uses the same rule for its one batch.

The counted original entity body and the JSON parser input are the SAME bytes:
SDK decoding is disabled and Auctioneer performs no decompression. Apply both
T and D to that observed length, not to a hypothetical decoded representation
or an assertion about removed origin headers. The normalized-header admission
rule is defined in section 3.1. Do not use `Content-Length` as the consumed count,
reset either allowance between calls, divide C into arbitrary per-stage quotas,
or debit the first batch's configured maximum instead of its observed usage.
Once earlier SDK metadata/framing checks admit a response, an exact-boundary
observed total is admitted; a larger total is a typed resource refusal.
JSON/node/structure admission can independently refuse an otherwise fitting body.
Visible unsupported response coding is a protocol failure, never a reason to
decode and approximate the missing encoded count. A declared length can cause
an earlier SDK refusal without becoming the observed `used` count. No SDK
extension or second codec stack is required by this design.

These are entity-body caps, not TLS/framing or platform-private-buffer measures.
Qualification must verify original/identity delivery and limit behavior for each
adapter. If a provider transparently recodes a response and its original body
representation cannot be established, record that target as unqualified for
this lookup path rather than treating decoded bytes as an encoded-byte proof.
This does not waive the existing platform/readiness gates or certify memory fit.

### 6.2 Storage and work admission

These logical caps do not prove simultaneous allocations fit. Before activation,
#53 must freeze and review a checked, target-aware storage/work derivation using
the existing additive budget and original receipt owners. It must include live
input, HTTP/secret buffers, API envelope, root result, merged intermediate,
account overlay/effective policy, dedup keys, snapshot, repeated impression
output, traversal stack and final typed request at their actual overlap peaks.
The original input cannot be counted as dropped while a caller still borrows it.

Reuse the existing RuntimeConfig/Ingress/Projection/Outbound owners only with
explicit non-overlap or additional independently justified charges; sharing a
phase name is not evidence of free capacity. No reservation may be inferred from
a passing fixture, arbitrary slack, zero-cost placeholder or unrelated bidder
quota. Failure to fit blocks activation, not permission to reduce supported
traffic, raise a provider ceiling or hide platform transport facts. Complete
WASM/schema qualification and the separate shared-resource work retain their
own acceptance gates; this design does not certify them.

The two lookup calls are not bidder calls or fake bidder slots. They do not
consume or enlarge the configured bidder fanout population. Their additional
transport/work storage must be funded explicitly, so neither subtracting them
from the supported bidder workload nor treating the existing bidder quota as
free source funding is acceptable. Auxiliary account lookups require their own
corresponding checked charge within the original auxiliary owner.

## 7. Failure contract and diagnostics

Fail before any bidder call on a source/merge/account failure. Preserve typed
internal categories without returning API bodies, tokens or record IDs in
errors, Debug, logs, auction debug output or metrics labels.

| Condition | Outcome |
| --- | --- |
| Bad live JSON, duplicate key, reference, selected account-ID type, merge result or final OpenRTB | Invalid request; existing HTTP 400 family |
| Referenced template explicitly missing | Invalid stored reference; HTTP 400 |
| No account selection with `require_account = true` | Account-required refusal; HTTP 400 |
| Selected account missing or any row denied | Account/access refusal; HTTP 403 |
| Valid account disabled | Preserve existing auction account-disabled HTTP 503; sync handlers retain their existing disabled-account refusal shape |
| Found account malformed or effective account invalid | Fixed malformed-account category; HTTP 500, no default-policy fallback |
| Found template structurally invalid | Source-data failure; HTTP 500, not caller-missing |
| Source required but unconfigured, secret missing/invalid, returned HTTP 401/403, invalid protocol or visible unsupported response coding | Fixed source configuration/protocol failure; HTTP 500 |
| Source I/O/unavailable/returned 429/5xx or snapshot expired/mismatched | Source unavailable; HTTP 503 with bounded retry diagnostic |
| Original auction deadline spent | Existing typed auction-deadline outcome; not a refreshed API timeout |
| Funded storage/construction exhausted | Existing typed resource refusal; never truncate, skip a required row or substitute policy |

A returned service non-200 not otherwise listed is a protocol/service failure
(HTTP 500), never a missing record. The status rows require an exposed head;
section 7.1 governs SDK refusals that contain no upstream status. Map new fixed
preparation reasons through the common
result projector, not independent standalone/embedding branches. Keep liveness
routes independent of the external service. Do not query the service at boot or
make `/status` depend on it.

Precedence is: installed funding/bindings and already-spent deadline; live input
admission; root lookup/merge; post-root reference/selection admission; second
batch transport/envelope validation; row availability; document validation
(including effective account policy) and impression expansion; account-disabled
check; final canonical request
and privacy validation. Within each stage, recheck the original deadline before
accepting success. Finish batch envelope/coverage/syntax admission first; an
invalid envelope is a protocol failure even if another row says denied.
For an admitted batch, use this exact order, independent of response order:

1. Any denied row: fixed access refusal, HTTP 403.
2. Missing selected account: fixed account-missing refusal, HTTP 403.
3. Any missing request/impression template: fixed template-missing refusal,
   HTTP 400; do not emit a backend-selected ID or per-template diagnostic.
4. Validate the found account overlay and its effective policy, then found
   template documents and their expansion. Malformed account precedes malformed
   template; use the fixed HTTP 500 categories above. A valid account's disabled
   flag is evaluated at the existing account/disabled checkpoint, not as a
   substitute for this document-validation order.

Thus account-missing plus impression-missing is always 403, while a malformed
found account plus a missing impression is always 400. A denied row plus a
malformed found document is 403 unless full-document syntax/envelope admission
already failed. A later stage never overturns an earlier failure. Never use
another row's success to excuse failure. No side effects or bidder work precede
final success.

### 7.1 SDK boundary and transport precedence

`HttpClient::send` performs header normalization, framing and declared-length
checks before returning even a streamed/original response. The client cannot
inspect an upstream status or coding header when that operation returns only
an SDK error. Apply this order independently at each lookup:

1. Recheck the original absolute deadline after the await; if spent, return the
   existing typed deadline outcome. Do not extend the deadline or reinterpret a
   shorter connection/idle timeout as more auction time.
2. If `send` returned an error, classify its typed variant, not its message or a
   guessed upstream status. `ResponseTooLarge` (body/header limits) and an
   outbound-upload `PayloadTooLarge` produce the existing typed resource refusal.
   A protocol/framing `BadGateway` produces the fixed source-protocol failure.
   Transport/unreachable failures and shorter transport timeouts produce source
   unavailable; other local/preparation errors produce a fixed source
   configuration/protocol failure. Never retain or parse SDK diagnostic text.
3. Only a successful returned head reaches the HTTP status rules above, followed
   by the normalized coding gate. A rejected status/coding does not trigger
   application body collection. Collection errors use the same typed mapping
   and original-deadline check before JSON/envelope/row admission proceeds.

For example, with 1,024 bytes remaining, a 401, 503 or gzip-labeled 200 declaring
2,048 bytes can be refused by the SDK as `ResponseTooLarge` before any head is
visible. Each is a resource refusal, not an inferred authentication, service or
coding outcome. A 401 whose head is returned instead follows the fixed HTTP 500
source rule without collecting its body. No cap is widened to recover metadata;
the original status cannot be recovered from `ResponseTooLarge`, and no head-first
SDK extension is required. This ordering applies to account-only sync lookups too.

## 8. Implementation ownership and verification

#53 owns the API models/client, shared async preparation seam, snapshot behavior
and reviewed funding/work bounds. #77 owns root/impression expansion and the
canonical ingress integration. #78 owns remote account overlays, required-account
behavior and removal of inline named accounts. Changes must land in a reviewable
order, including the existing sync-route account consumers, that never exposes
a half-wired alternate account authority. Before the hard cut activates, all
three requirements must be present; don't introduce a
temporary operator switch or an indefinitely supported second lookup mode.

Shared config models stay in `auctioneer-config`, selected policy projection and
the preparation/source orchestration in `auctioneer-core`, admission/funding in
existing `auctioneer-runtime` owners, and concrete SDK attachment in the existing
host integration. No new crate, HTTP framework or provider-specific resolver.
Reuse the current SDK typed CLI flow. Static validation does not fetch backend
records; API responses reuse the same account validators at read time.

Keep pure unit tests inline. Reuse existing core account/handler/preparation and
allocation owners, with one capable synthetic source; do not copy real bidders
or create a test target per record kind/provider. End-to-end tests must exercise
the real SDK HTTP boundary with controlled fixtures as well as pure resolver
tests. Adapter contract checks cover Axum, Cloudflare, Fastly and Spin without
claiming unexecuted guest/hosted checks passed.

Required controls include:

- Two arbitrary configured endpoint paths and hosts; verify the actual destination
  and method, no appended route, no query/redirect/credential injection, secret
  rotation across auctions but not stages, and no-lookup/no-secret-read inputs.
- Literal batch-shape expectations, all record kinds with identical IDs, reversed
  response order, absent/duplicate/extra rows, every status, malformed control
  objects, unsupported version, trailing JSON and duplicate keys before merge.
  Permute mixed failures and assert the exact category/HTTP outcome, including
  account-missing plus template-missing, missing plus malformed, denied plus
  malformed, and invalid envelopes containing denied rows.
- Two-stage coherence across a concurrent update, matching/mismatched/expired
  snapshot tokens, a fresh snapshot on a later auction, and no retry or stale
  fallback on service/auth/secret failures. Count actual calls and rows.
- Pin outgoing `Accept-Encoding: identity`, SDK original/streamed response modes
  and rejection of visible non-identity codings before application collection/parsing.
  Use a valid gzipped API document as a negative control: default SDK decoding
  would otherwise remove its coding header and make it look admissible. Include
  valid-looking JSON with an exposed compressed label, stacked/ambiguous codings,
  absent/identity headers and identity bodies without `Content-Length`. Through
  the real SDK normalizer, also pair `Connection: content-encoding` with a gzip
  label: fitting valid JSON is admitted under the normalized-header contract,
  while actual gzip bytes are refused by JSON admission without decompression.
  Assert unchanged collected bytes and caps, not invented original-header evidence.
  Across two batches test below/exact/above C with independent T < D and D < T
  rows, a large first/small second body and the reverse, exact envelope overhead,
  and refusal before a second dispatch when no bytes remain. These controls must
  reject counter resets, automatic decoding that hides a coded body,
  maximum-debit and arbitrary half-budget implementations, not merely
  per-response overflow. Do not pretend two separate counters can be varied
  independently on admitted bodies: original entity bytes and parser input are
  identical because no decoding occurs. Execute adapter-specific delivery checks
  separately from any whole-target fit claim.
- Pin SDK preflight precedence with identity 200/401/503 and gzip-labeled 200
  heads declaring a length above the remaining allowance. Assert exact typed
  resource outcomes and no application body polling, not an inaccessible status.
  Pair returned, fitting heads with their exact status/coding outcomes. Cover
  header-limit and malformed-framing errors, collection-time limits and original
  deadline expiry independently, and verify no retry or diagnostic-text leak.
- Root-only, imp-only and combined references; dedup versus repeated expansion;
  nested/cyclic rejection; arrays/objects/scalars/null and omitted fields varied
  independently; live/template/final ID and publisher selection precedence.
  Cover references reintroduced by live-supplied and template-supplied bidder
  FPD, including null, malformed and valid-looking controls; prove both early
  admission and post-FPD guards are load-bearing. Unit-test the final guard on
  an already-projected synthetic request independently of admission, so the
  earlier check cannot mask its removal; no production-only failure hook is
  needed. Wire tests must show no reintroduced reserved controls and no third
  lookup, using fake bidders.
- Account absent, named missing, denied, disabled, corrupt and available; vary
  selector order, replacement versus hidden fallback, missing/null/empty/string
  versus non-string results, escaped keys, positional indices, malformed pointer
  syntax and required-account refusal independently. Test identity and nested
  account-policy selectors with exact outcomes; malformed/missing selected policy
  must not fall back to its wrapper or permissive defaults. Also vary
  `require_account` and default/overlay privacy independently, including a strict
  account behind permissive defaults and updates without an application rebuild.
  Reject forbidden selector origins at CLI and serving config validation before
  HTTP/secret reads; permit custom publisher-extension paths and escaped keys.
  Check decoded tokens rather than a lookalike prefix, including rejection of
  `/site/publisher/extensional/account_id`. Prove only the chosen string, not
  the parent object or a consent/browser-identity sibling, enters the API body.
  Independently vary both remote response-cap fields, account defaults, host
  requests, bidder and alias overrides, and installed clamps. Pin literal
  expected budgets and prove the admitted overlay is fetched only once.
  Repeat selected-account privacy/refusal controls on `/cookie_sync` and `/setuid`
  through their existing test owners, including no cookie/URL output on failure.
- Equivalent JSON/typed HTTP/embedding inputs through the same async preparation;
  settings first, caller data unchanged, original extension diagnostics preserved,
  original permits/receipts retained and exactly-once release on every exit.
- Deadline boundaries before/between fetches, during merge and before dispatch;
  cancellation while a body/secret read is pending; no clock reset or post-failure
  bidder call. A slow API consumes auction time rather than granting extra time.
- Independent byte/node/depth/key/visit axes at below/exact/above bounds, a positive
  allocation control, repeated references under maximum impressions, overlapping
  owner lifetimes and unchanged limits. Guard each consequential failure reason,
  not only a generic `is_err()` or a neighboring refusal.
- Format-1 and inline-account rejection, format-2 CLI validate/diff/push and
  serving-reader agreement, unchanged current behavior for default-only inputs,
  and fixed/redacted errors with attacker-controlled partial-secret canaries.

The ordered plan must name existing owners, commands and actual dispatch counts,
then run the complete prescribed CI matrix in addition to targeted controls.
No ignored failure, empty filtered loop or conditional test may stand in for a
required acceptance surface. External service and real host tests must identify
their prerequisites; missing access is blocked verification, not a passing test.

## 9. Review and publication state

The user selected API-based storage, defining the contract here, configuring
the complete endpoint through `auctioneer.toml`, and using JSON Pointer for
field access. The source is proposed as a top-level `auction_data` section.
The user approved the revised written spec for planning on 2026-11-09 with
"ok plan it". The implementation plan requires review before production changes.
No new dependency, SDK extension, numeric reserve, workload or ceiling change is
authorized by this proposed contract. A later planning PR contains the spec and
plan together. On resumption, the user authorized publishing the verified
documents in one non-draft planning PR; that authorization includes no
implementation, numeric reserve, deployment or merge.

The requested self-review corrections make header admission explicitly relative
to SDK-normalized metadata and define typed SDK refusals before returned-status
rules. They retain original-body collection, both byte caps, no decompression and
all resource/provider gates; they do not attest removed origin headers or add an
SDK extension. The accompanying plan corrects the CLI test owner and requires
nonzero enumeration. These are document corrections, not implementation approval.

The initial default-workspace run at base `499f0e540e95e1c86b730c1030aabef28de01d0a`
reported 6,521 passed, one failed and 20 ignored across 245 result summaries.
The failure was the CLI private-frame listener-shutdown test; its isolated rerun
passed. The cause remains unconfirmed. That rerun does not turn the full baseline
green, waive any check or demonstrate readiness to publish or implement.

That observation belongs to the initial draft's old base, not the current
resumption. The refreshed main baseline
`d26fcf7f566db6d1dce2e1847d79c9f44581edb8` completed all 25 hosted checks
successfully in [Run Tests 44246265249](https://github.com/IABTechLab/auctioneer/actions/runs/44246265249)
and its companion format/provider workflows. The [secure owner](https://github.com/IABTechLab/auctioneer/actions/runs/44246265249/job/134605855644)
executed the existing full bidder/exchange/core and privacy-composition commands,
exit 0, including:

```text
test result: ok. 309 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.66s
test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

These are inspected GitHub Actions results, not local execution, new lookup
tests or proof that the original shutdown failure's cause was established.
The user prohibits local Cargo/tests/fmt/Clippy/CI scripts. The eventual
planning-PR head still needs its own complete hosted verification.
