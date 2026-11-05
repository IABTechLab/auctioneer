# diagnostic qualification Parity Hardening Design

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-05
Updated: 2026-10-15 (implementation and native local acceptance complete; live execution assigned to publisher pilot)
Status: diagnostic qualification implemented and locally verified; PR review and publisher pilot deployment acceptance remain separate
Worktree: `.claude/worktrees/diagnostic-hardening` (`diagnostic-hardening`)
Historical design baseline: user sync commit `f616de7762be31472d22bc05a216ca49fbd14f56`
Historical hardening inheritance: user sync `5367ab0`, merged as `74c8bfa`.
Historical classified disclosure integration inherited privacy enforcement `c2b00a2` through
user sync `9ff017f`, including actual construction costs, auxiliary work accounting
and exhaustive model coverage. Current source integration inherits merged
`main` at `b01c309` (user sync PR #18, final source `df351ee`), including signed
user-sync cookies, privacy safeguards and Rust-only reference tooling, with
EdgeZero originally inherited at `28038cce26a499499fd4e050387d4d02e35fa7c7`.
The current pin is `596f9f4e23e43c8952a51ce6d92961d461032004` from PR #275,
using Rust 1.98.1 and the SDK's updated public dependency requirements.
Original Content-Type provenance was introduced at `65312643`, with explicit
response construction/decomposition updates. The current SDK retains that
contract; changing the SDK pin does not independently authorize debug capture
or qualify a deployment. Historical measured captures keep their original
compiler and SDK identities.
Prepaid debug funding, the fixed body projector, immutable SDK provenance
integration, pure debug permission/wire/metadata policies and bounded sanitized
capture storage/assembly are implemented. Engine and HTTP integration now enable
sanitized debug under the account/base-bidder gates, independently bounded value
and complete-response caps, and recognized-request no-store policy. Config and
snapshot acceptance, native debug allocation/redaction/lifetime guards, Q2
profile/evaluator/export tooling, the external release harness, offline provider
preparation and the reusable rollout/rollback runbook are implemented, with
verification recorded in the plan. Native Q2-C passed all 45 local rows with
independent evidence review, and the integrated local verification matrix is
complete. See the [verification record](../../testing/evidence/diagnostics-qualification-verification-20261015.md).
Overall target qualification remains unsupported, SDK attestations pending and
publisher readiness false. publisher pilot owns credentialed hosted execution and actual
deployment acceptance; these are not inferred from local completion.
Implementation plan: [diagnostic qualification parity hardening](../plans/2026-10-07-diagnostics-and-qualification.md).
The final bounded fanout `8a48982` repairs were ancestors of the historical integrated user sync
source. user sync was squash-merged; current source inheritance is verified by tree
equality, not feature-branch ancestry. The plan records baseline checks
separately from new diagnostic qualification implementation.

## 1. Scope and Authority

Finish the initial auction/privacy hardening capability: enable safe auction
debugging, reconcile config and error-precedence acceptance with bounded fanout-user sync,
exercise config updates, prepare the offline shared-handler harness/artifact
handoff/refusal tests and provider runbooks, and deliver complete native
Q2-A/B/C plus reusable Q2-E under section 10.1. diagnostic qualification does not require bounded fanout to reach
an ideal production state before implementation planning or native behavioral
tests can start. Q2 is an explicit diagnostic qualification requirement, not work deferred to metrics and analytics.

**User-approved scope correction (2026-10-15):** skip credentialed live tests
for diagnostic qualification and execute them under [publisher pilot: Limited-Bidder Publisher Pilot and Hosted
Acceptance](../../roadmap.md#8-limited-bidder-publisher-pilot-and-hosted-acceptance),
after Trusted Server embedding and the selected publisher bidder migration wave/host setup, not
full bidder catalog catalog closure or video parity. publisher pilot owns real Cloudflare/Fastly/Spin
deployed-handler execution, authorized corrupt-config injection/recovery,
selected actual deployment Q2-D and limited rollout evidence. diagnostic qualification is a
local/staging checkpoint, not live-pilot readiness. These are deferred,
unexecuted requirements, not waived or checked evidence; all other diagnostic qualification scope
and selected-target safety/privacy requirements remain unchanged.

This document supersedes the master's incomplete debug contract in sections
5.6 and 11, historical config merge/storage examples in section 9.1, and the
residual-work wording in section 14. It does not supersede existing privacy,
outbound, admission, timeout, or egress guarantees. The [master design](2026-05-21-auctioneer-openrtb-server-design.md), [user sync design](2026-09-30-usersync-design.md), [bounded fanout plan](../plans/2026-07-10-multi-adapter-production-fanout.md), and [roadmap](../../roadmap.md) retain their other requirements.

No implementation code, new endpoint, workspace crate, plugin mechanism,
second config loader, signing layer, or backward-compatibility alias is
introduced by this documentation change. Implement focused modules inside
the existing owners. Full bidder catalog coverage is bidder catalog, direct embedding
is Trusted Server embedding, and stored objects plus video foundations remain stored-request, auction, cache and video delivery work. Full
analytics/metrics remain metrics and analytics; necessary safe diagnostics are not deferred.

## 2. Ownership

| Owner                                    | Responsibility                                                                                                                                                                                        |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `auctioneer-exchange::debug`                 | Signal combination, account/base-bidder gates, sanitized DTOs, capture limits, deterministic assembly                                                                                                 |
| `auctioneer-bidder::debug_projection`        | One fixed schema-allowlist body projector, fixed media-eligibility helper and bounded response entry point preserving the sealed-body contract                                                        |
| EdgeZero outbound response boundary      | Supplies immutable pre-normalization `ContentTypeProvenance` at `65312643`; provider-hidden original field boundaries remain `Unknown`                                                                |
| `auctioneer-bidder` response/error contracts | Consume immutable original-header eligibility and non-wire response-parser provenance; existing public error codes/messages remain unchanged                                                          |
| `auctioneer-core` outbound/proxy conversions | Carry trustworthy SDK header evidence into the sealed response; normalized headers alone cannot establish original eligibility                                                                        |
| `auctioneer-runtime`                         | Checked debug peak derivation and existing reservation/JSON-size primitives                                                                                                                           |
| `auctioneer-core::config`                    | Existing typed overlays, one validated request snapshot, update/failure tests                                                                                                                         |
| `auctioneer-core::handlers::auction`         | Bounded HTTP debug signals, context construction, final-output fit and response headers                                                                                                               |
| Existing provider adapters               | Existing generated App and ingress/egress integration; diagnostic qualification offline harness/handoff/refusal preparation, publisher pilot credentialed live execution; no adapter-local auction router                              |
| Auctioneer qualification tooling and tests   | Reproducible release-artifact load scenarios, validated profile/report inputs, cold/warm and overload evidence under section 10.1                                                                     |
| Auctioneer deployment owner                  | diagnostic qualification native measurements, funded profile tuning, thresholds and reusable rollout/rollback runbook; publisher pilot selected actual deployment Q2-D/pilot evidence; no provider guarantees inferred from local runs |

`run_auction` remains responsible for permission checks, redaction, and
populating `BidResponse.ext.debug`. Direct engine callers cannot bypass an
account or base-bidder gate by setting a context flag. HTTP callers may
discard diagnostics to fit an output limit, but never inject raw traces.
Use typed serde DTOs and the existing structured JSON parser/size/writer
APIs. Valid test fixtures use `serde_json::json!`; literal byte fixtures are
reserved for malformed syntax, exact-wire, UTF-8, and escaping cases.

## 3. Debug Requests and Permission Gates

### 3.1 Signals

Debug is requested when any of these independently normalized signals is true:

| Source                       | True value                                                           | Other values                                                                                                      |
| ---------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Normalized `BidRequest.test` | Integer `1`                                                          | False                                                                                                             |
| `ext.prebid.debug`           | Integer `1` or boolean `true`                                        | Other successfully parsed integer/boolean values are false; invalid JSON types keep existing request parse errors |
| Query `debug`                | Decoded value exactly `1`; first decoded occurrence wins             | False                                                                                                             |
| `X-Auctioneer-Debug` header      | One field value, exactly `1` after HTTP optional-whitespace trimming | Missing, repeated, comma-combined, or other values are false                                                      |

Combine with OR; a false signal does not cancel another true signal. Header
names are case-insensitive. No signal overrides permission, privacy, sampling,
resource, or timeout policy. There is no separate force-debug mode.

The HTTP query scan is bounded to 4096 raw query bytes and uses the existing
structured form-decoding dependency and strict percent/UTF-8 validation.
An oversized or malformed query contributes no query signal; it does not
turn an otherwise valid auction into a debug-specific HTTP error, and does
not cancel a valid body/header signal. Only the first decoded `debug` value
is considered, without trimming its decoded value. Thus `debug=+1`, `debug=1+`,
`debug=%2B1`, and `debug=1%20` are false. Reuse `url::form_urlencoded`, not
the sync parser's different limits/error semantics. Other query fields are
not captured or reflected. Header whitespace trimming is independent; this
parser must not change the sync endpoints' separate query contract.

The handler combines HTTP signals with the normalized body signals and
passes the result to the engine. The engine independently ORs normalized
body signals with the caller flag, so an embedding caller cannot accidentally
lose `test=1` or `ext.prebid.debug=true` by passing false.

### 3.2 Gates and Warnings

Unrequested debug emits neither `ext.debug` nor debug-denial warnings and
creates no capture buffers, redaction trees, or per-call debug records.

Task 4 provides the pure permission decisions and fixed warning constructors;
it does not yet emit them from the engine. Account inspection and alias/base
registry lookup allocate nothing. Borrowed bidder-name lookup preserves exact
case-sensitive identity, including when an alias shadows a compiled name.
Tasks 5-7 still own capture, warning placement/deduplication and HTTP integration.

Requested debug with account `debug_allow == false` produces exactly one
10002 warning under `ext.warnings.prebid[]` and omits `ext.debug`. Add this
auction-wide warning before engine early returns, including an expired tmax
or no eligible bidders. A fatal caller/engine error remains its existing
HTTP error rather than manufacturing a `BidResponse` just to carry debug.

Only finalized calls entering the fanout call list can produce bidder-level
debug-denial warnings. Evaluate permission without allocation before any
tentative pre-compression capture; commit warning eligibility only after
successful finalization. Resolve `BidderInfo.debug_allow` through the compiled base
bidder, while output keys and warning keys use the observable alias. A false
base gate cannot be relaxed by an alias, account setting, or HTTP signal.
Unavailable base metadata fails closed for capture. Denial yields one 10003
warning per observable bidder name, not per call, under
`ext.warnings[<bidder>]`, with no HTTP-call entries for that name. Account
denial never also produces 10003. Privacy-blocked, disabled, sampled-out,
cap-dropped, finalization-failed, and pre-fanout-expired bidders do not
produce 10003 because they did not reach this stage.

For requested and account-permitted debug, an auction with no captured calls
still emits the empty debug shape unless bounded assembly or final-output
fitting omits it under sections 4/6.
Permission-denied calls do not contribute to debug dropped-call counts.

## 4. Wire Contract

`ext.debug` contains only `httpcalls`, `truncated`, and `dropped_calls`.
`httpcalls` is an object keyed by observable bidder/alias, with arrays in
ascending fanout `call_index` order. Object keys are lexical. Indices are
zero-based positions in the finalized fanout call list, not completion order
or per-impression indices. Permission suppression may leave gaps.

Every captured call has these fields:

| Field        | Shape and meaning                                                                                                                                                                                                                                  |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `call_index` | Nonnegative integer, stable positional correlation                                                                                                                                                                                                 |
| `request`    | Object containing a fixed method token (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`, `TRACE`, `CONNECT`, or `OTHER` for extension or malformed methods), `endpoint_origin`, sanitized `headers`, `body_status`, and optional `body` |
| `response`   | Object containing HTTP `status`, sanitized `headers`, `body_status`, and optional `body`; omitted when transport produced no response                                                                                                              |
| `elapsed_ms` | That positional `BidderCallResult.elapsed`, floored to whole milliseconds and saturated to `u32::MAX`; forward it, never recompute it from bidder-aggregate or batch wall time                                                                     |
| `error`      | Optional fixed category: `timeout`, `preflight`, `oversize_request`, `oversize_response`, `decode`, `transport`, or `response_parse`; never free-form error detail                                                                                 |

The engine records a safe failure category even if body projection is
unavailable. HTTP error responses may still have a `response` object. An
adapter parse failure does not require exposing its error message or raw
body. Do not report a pre-dispatch finalization failure as an HTTP call;
existing `ext.errors` already describes it.
Method categorization changes only the debug projection, not transport
preflight behavior; never reflect an original extension/malformed token.

Debug error classification is exhaustive over the existing neutral transport
taxonomy, including nested origins; it does not change public error codes,
messages or seatnonbid mappings:

| `FanoutError`                                                      | Debug `error`       |
| ------------------------------------------------------------------ | ------------------- |
| `Timeout(_)`                                                       | `timeout`           |
| `Preflight`                                                        | `preflight`         |
| `OversizeRequest { .. }`                                           | `oversize_request`  |
| `OversizeResponse(RequestHeaders)`                                 | `oversize_request`  |
| `OversizeResponse` with any other origin, including `Unclassified` | `oversize_response` |
| `UndecodableResponse(_)`                                           | `decode`            |
| `BadServerResponse`                                                | `transport`         |
| `Transport(_)`, `Unreachable`, `Other(_)`                          | `transport`         |

`RequestHeaders` describes a request refusal despite its existing outer
variant/public code. `BadServerResponse` has already collapsed non-biddable
HTTP status and protocol/decode failures; debug cannot recover a precise
cause and must not guess `decode` from that variant or free-form text.
Payloads such as unsupported coding tokens and `Other` strings are not
rendered. A new taxonomy variant needs an explicit reviewed mapping.

`response_parse` instead comes from a minimal, non-wire
`BidderErrorKind::{Other, ResponseParse}` discriminator. Existing ordinary
error constructors default to `Other`; the sealed bounded response parser
marks its syntax/schema, node-budget and refused raw-value errors
`ResponseParse`, preserving their existing codes and safe bounded messages.
Inspect both error channels from `make_bids`: `Err(errors)` and the bid-local
`errors` in an `Ok(BidderResponse)`. Emit `response_parse` iff at least one
error in the applicable channel has that provenance, including mixed error
kinds and responses retaining valid sibling bids. Debug classification must
not discard those bids or duplicate their error entries. Numeric codes 4/14
and message text are not provenance. Other semantic adapter failures,
including media classification and unknown-impression failures, retain existing
`ext.errors` behavior without inventing a debug parse error. Projection omission
itself is not an adapter parse failure; there is no second parse to determine
the category.

Elapsed values retain the existing target capability profile's completion
or observation semantics and queue-accounting definition. At the inspected
SDK pin, Fastly reports `ObservationTime`: serial input-order harvesting may
include waiting for a preceding slow call. Forward that call's value
unchanged; do not label it an independently measured bidder latency or add
a completion-time guarantee. The capability/evidence matrix supplies this
interpretation without a new debug wire field. Qualification latency uses
its separately declared measurement boundaries and sample populations, not
an assumption that debug elapsed is universal end-to-end latency.
The October 30 SDK adoption additionally makes earlier-own-deadline timeouts
report logical expiry, not delayed observation, and orders ready receipts by
that terminal instant. This exception applies to Fastly too. Forward the SDK
value unchanged; do not reconstruct an observation timestamp that its receipt
does not expose. See
[`2026-10-30-edgezero-outbound-adoption.md`](../plans/2026-10-30-edgezero-outbound-adoption.md)
for the source-audited change and required regression checks.
Queued calls expiring before dispatch may already carry a shared elapsed
value measured from fanout entry; in-flight cutoff values may be anchored
to batch entry. In particular, wave `drain_expired` applies one observation
to every due slot in that drain: two such calls must retain equal `elapsed_ms`,
not a synthetic spread from reading the clock once per row. Preserve the
supplied value with the same flooring/saturation as other results. Neither an elapsed
value nor an HTTP-call row proves that network dispatch occurred.

`headers` maps canonical lower-case allowed names to arrays of safe values;
duplicate allowed values retain receipt order. `body` is a JSON string
containing a sanitized structural projection, not original bytes and not an
embedded unrestricted JSON value. Resolve `body_status` in this order, stopping
at the first applicable row; do not let multiple failure causes pick a status
according to implementation traversal order:

| Order | Condition                                                                                                | Status              |
| ----- | -------------------------------------------------------------------------------------------------------- | ------------------- |
| 1     | Body is known to have zero bytes                                                                         | `empty`             |
| 2     | Body cannot be accessed, or capture cannot start/finish within its allocation/deadline allowance         | `unavailable`       |
| 3     | Adapter schema is unsupported                                                                            | `omitted_type`      |
| 4     | Body or candidate MIME/header input exceeds its byte cap                                                 | `omitted_size`      |
| 5     | Original Content-Type evidence is ineligible/unknown, MIME is invalid/non-JSON, or body is invalid UTF-8 | `omitted_type`      |
| 6     | JSON syntax, trailing input, syntactic node or depth limit fails                                         | `omitted_structure` |
| 7     | A traversed container or allowlisted numeric leaf violates its declared schema/type/domain               | `omitted_type`      |
| 8     | Valid body has no allowlisted leaf or fixed masking path                                                 | `omitted_policy`    |
| 9     | Complete sanitized projection cannot fit its output cap                                                  | `omitted_size`      |
| 10    | Complete sanitized projection fits                                                                       | `captured`          |

For example, a known zero-length body with duplicate Content-Type is `empty`.
With available capture budget and the `OpenRtb` schema, a nonempty body with
Content-Type `text/html` and an exceeded body/header input byte cap is
`omitted_size`, not `omitted_type`: the byte-cap decision precedes MIME validation.

Known emptiness requires no MIME/schema parse or allocation. Transport absence
omits `response`, rather than inventing an empty body. Only `captured` carries
a body string; unsupported or malformed formats never fall back to raw bytes.
If a deadline/allocation failure interrupts a later stage, use `unavailable`
unless an earlier terminal decision was already made. Final assembly may
replace `captured` with `omitted_size` when removing that body's string.

`truncated` is true if any permitted body is omitted (including `unavailable`
for capture failure), or any permitted finalized call row is omitted.
Known empty bodies and transport absence alone do not set it. `dropped_calls`
counts each debug-permitted finalized call with no retained row exactly once,
whether due to call/storage/wire caps, allocation refusal or deadline. Body-only
omission does not increment it; permission denial and finalization failure
contribute neither counter. A genuinely empty permitted finalized call set has
`httpcalls: {}`, `truncated: false`, `dropped_calls: 0`; capture failure must not
masquerade as that successful empty result. If bounded assembly or HTTP fitting
removes the entire debug object, no replacement marker or warning is required.
No randomness, trace ID, or completion ordering is introduced.

## 5. Redaction and Capture Lifetime

### 5.1 Safe Projection, Not a Blocklist

Debug redaction is unconditional for permitted capture, even when privacy
policy permits transmitting identifiers to a bidder. It never mutates the
auction request, outbound bytes, returned bids, or privacy decisions.

Request and response projection use separate wire-path allowlists in a
debug-specific structured serde visitor over the original body bytes. Do not
deserialize into the full OpenRTB models and serialize their flattened maps.
Retain only listed leaves and the ancestor structure needed to represent them;
an allowed container never authorizes copying any sibling. `[*]` means bounded
array traversal, not permission to copy arbitrary members.
All numbers must be finite and satisfy the corresponding field's schema.
Nonnegative prices are checked before floating-point underflow can erase a
negative nonzero significand; signed zero remains zero.

| Schema   | Allowed value paths                                                                                                                                                                                                      |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Request  | `at`, `test`, `tmax`, `allimps`; `imp[*].banner.{w,h,pos}` and `imp[*].banner.format[*].{w,h}`; `device.{dnt,lmt}`; `regs.{gdpr,coppa}` and legacy `regs.ext.gdpr`; `cur[*]` only when equal to supported currency `USD` |
| Response | `cur` only when equal to `USD`; `seatbid[*].group`; `seatbid[*].bid[*].{price,w,h}`                                                                                                                                      |

For these fixed masking paths, preserve the fixed field name when present
but replace the entire value with `[REDACTED]`, irrespective of its original
type or length. A whole-value mask takes precedence over any descendants.

| Schema   | Masked paths                                                                                                                                                                                                                              |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Request  | Root `id`, `site`, `app`, `dooh`, `source`, `user`, `ext`; `imp[*].{id,tagid,ext}`; `device.{ua,sua,ip,ipv6,ifa,geo,dpidsha1,dpidmd5,didsha1,didmd5,macsha1,macmd5,ext}`; `regs.{gpp,us_privacy}` and `regs.ext.{gpp,us_privacy,consent}` |
| Response | Root `id`, `ext`; `seatbid[*].seat`; `seatbid[*].bid[*].{id,impid,crid,adid,dealid,adm,nurl,burl,lurl,ext}`                                                                                                                               |

Unknown fields and arbitrary-key maps are omitted, including their original
keys. Do not retain unrelated fields under an allowed container. Fixed mask
values are safe projectable content; a body with neither allowed leaves nor
listed mask paths uses `omitted_policy`. Typed schema does not make an
arbitrary string or opaque extension safe. Additional paths require a
code/spec change and adversarial tests, not a permissive catch-all.
Projection preserves repeated permitted wire-field occurrences in receipt order;
it is not a canonical or deduplicated JSON representation. Every occurrence is
independently validated or masked. A malformed allowed occurrence omits the body
even if another occurrence is valid; neither first-key nor last-key selection
may bypass that decision. Every occurrence of a masked field stays redacted.
The syntactic preflight counts/validates input; it does not erase its keys or
replace the wire-path policy. A wire leaf such as `allimps` can be explicitly
allowed even if the ordinary model stores it in a catch-all. Conversely,
`imp[*].banner.ext`, `regs.ext.gpc`, and `regs.ext.dsa` are default-omitted,
including their keys; typing a field does not permit its debug disclosure.
Legacy `regs.ext.gdpr`/`us_privacy` projector cases are defensive adapter-wire
coverage, not a change to ingress's rejection of legacy privacy placement.
Whole-root `user` masking also covers legacy consent below it. Project response
`cur` from its original presence/value, distinguishing absent from empty;
ordinary `BidResponse` defaults must not manufacture debug leaves.

At minimum mask or omit all native and legacy consent strings, GPP/USP,
IP addresses, advertising/device IDs and hashes, user/buyer/EID identifiers,
precise coordinates, demographics/user data, transaction IDs, cookies,
bidder/account credentials, and free-form publisher/bid/creative IDs. This
includes flattened field shadows and `user.ext`/`regs.ext` legacy projections
created for bidder delivery. Creative markup, tracking/win/loss URLs,
arbitrary vendor extensions, user-supplied page URLs, and non-JSON formats
are never copied without an explicitly reviewed safe projection.

Display only an endpoint's validated scheme/host/port origin. Omit its
userinfo, path, query, and fragment: path macros may contain credentials
or identities too. Header output has this fixed value policy:

| Header             | Allowed output                                                                                                                                  |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `content-type`     | `application/json` only, after valid MIME parsing and case normalization; discard all parameters                                                |
| `content-encoding` | A single bare `identity`, `gzip`, `br`, or `deflate` token, normalized to lower case; omit stacked, parameterized, malformed, or unknown values |
| `sec-gpc`          | Exactly `1` after HTTP optional-whitespace trimming                                                                                             |

Use a debug-specific fixed representation; the existing inventory
`MediaType` enum is not an HTTP MIME enum. Omit all other header names and values,
including Authorization, Proxy-Authorization, Cookie, Set-Cookie, API-key,
forwarded-IP, and bidder-specific credentials. Unsupported values of an
allowed header are omitted, not echoed. No original secret length or
secret-bearing key is retained as a replacement marker.

For a nonempty body, `OpenRtb` capture also requires exactly one original
Content-Type field value that parses as a valid MIME value with base type
`application/json`, case-insensitively. Determine original-header eligibility before filtering
or sanitizing headers: missing, repeated (even identical), comma-combined,
malformed or non-JSON Content-Type yields `omitted_type` without a body string.
Syntactically valid MIME parameters may be present but are never copied;
they do not relax the independent UTF-8 requirement or select another decoder.
The MIME essence must be exactly `application/json`: suffix-bearing types such
as `application/json+json` and `application/problem+json` are ineligible. A `+`
inside a syntactically valid parameter value is not a MIME suffix.
A zero-length body uses `empty` without requiring Content-Type. Add request
and response cases for every combination; a conflicting value must not
disappear during header filtering and accidentally authorize capture.

For responses, provenance must originate before the SDK's lossy normalization,
not just before Auctioneer's `to_str()` conversion. At the inherited `28038cc` pin,
EdgeZero's `normalize_response_headers` drops unrepresentable header values
before either Auctioneer conversion receives a HeaderMap. A valid JSON Content-Type
plus a second non-UTF-8 value can therefore arrive as one valid value. Counting
that normalized map does not prove eligibility.

**Current EdgeZero contract:** `596f9f4e` retains `ContentTypeProvenance`, introduced at `65312643`, with
`SingleTextUnchanged`, `Ineligible(reason)`, and `Unknown`. The normalizer records
it before filtering; `OutboundResponse::content_type_provenance()` and the fifth
element of `into_parts()` preserve it. Mutable header access invalidates a
previous singleton; conversion to an HTTP response carries evidence as an
extension and cannot upgrade original ineligible/unknown evidence during that
conversion. Decomposed HeaderMaps and ordinary HTTP response headers are still
mutable; an enum/extension alone is not a sealed certificate after such edits.
Auctioneer must immediately seal the paired candidate/evidence, or invalidate it
before any later header mutation. Normalization returns
`ResponseHeadNormalization`, not just the body disposition. No raw header
value is copied into the metadata. This certifies original text/count retention,
not MIME validity; comma-combined text still requires application MIME refusal.
Axum, Spin and Fastly carry native pre-filter evidence. Cloudflare explicitly
returns `Unknown` because Fetch hides original bytes/field boundaries; normalized
appearance cannot restore that evidence. SDK tests cover normalization/ownership,
native raw TCP duplicates and hosted conversion paths, but test presence is not
a claim of executed hosted deployment evidence.

**Auctioneer requirement:** Task 3 retains the SDK evidence through
`outbound::lift_response`/`BoundedProxyResponse` into `outbound::wave`, and through
`host_primitives::edgezero_proxy`. Both paths seal the original candidate and
evidence before further conversion; bounded response headers/provenance and
neutral response headers/eligibility are private and read-only. They carry fixed
`DebugBodyEligibility::{SingleTextValue, Ineligible}` evidence
immutably into the sealed response. Only SDK `SingleTextUnchanged` can become
SingleTextValue, and only with the unchanged candidate in the read-only header
list. All other/unknown future provenance variants remain Ineligible.
It is not proof of JSON MIME. Unknown/already-lost evidence is Ineligible,
even when normalized headers look valid. Only after permission passes does
bounded `mime` 0.3.17 validation authorize application/json projection. Keep
headers/eligibility read-only, retain no additional raw-header copy, and add no
raw-body getter, clone or arbitrary callback. A helper over a complete supplied
list may derive count/representability for requests and synthetic fixtures; it
cannot manufacture original evidence from the SDK's normalized map.

The Task 3 mapping and pure projection tests pass, including real native TCP
singleton, duplicate and non-text-header cases through both wave and legacy
bridge paths. Compiler probes reject header/evidence mutation, raw access,
arbitrary callbacks and wider projection budgets. This is not engine/HTTP or
hosted-handler acceptance. Unknown/ineligible nonempty responses retain safe metadata and
`omitted_type`, including all Cloudflare responses at this pin. Positive response
capture is exercised only where original evidence is available; Cloudflare's
fail-closed omission is the expected row, not a missing grant to synthesize.
Request eligibility uses complete bounded metadata before compression, and known
empty responses retain `empty` independently.

The existing reference bidders must all exercise safe request/response
projection in diagnostic qualification, with real response eligibility subject to the SDK/provider
contract above. Synthetic positive tests are not real transport acceptance.
Each compiled bidder declares its debug body schema via the bidder contract:
`OpenRtb` selects the fixed request/response policies;
the default `Unsupported` omits bodies. Do not maintain another central
bidder-name list. Future formats require code-reviewed enum/projector
extensions, not arbitrary byte callbacks, operator regexes, or executable
runtime configuration. Unsupported bodies retain HTTP-call metadata and an
omission reason; they do not weaken the default policy. The body projector
is defined once in `auctioneer-bidder::debug_projection`; the exchange delegates
to it and owns permission gates, URL/header sanitization, and final DTOs.

### 5.2 Capture Stages

Only after account/base-bidder permission passes, capture a bounded sanitized
request-body projection after per-bidder privacy
scrubbing, legacy projection, and adapter request construction, but before
gzip replaces the body. Do not capture the original incoming auction body
or a pre-privacy clone. Hold a tentative sanitized record until finalization
succeeds; discard it for failed finalization. Capture sanitized headers
after the exchange's final appends, so actual Content-Encoding/Sec-GPC
metadata is represented. Do not clone/share the request's metadata Arc
before its required unique-mutation finalization step.

For responses, capture only after the existing transport decode and byte
caps and after attaching the exchange's node budget, but before adapter
processing consumes the result. Add a response-owned bounded projection
method in `auctioneer-bidder`, returning only a sanitized DTO/string. This method
must enforce the smaller of the response's installed node budget and the
debug node budget and consume the immutable original-header eligibility
recorded at transport conversion, not inspect a lossy header list for proof.
It must not expose an unrestricted byte borrow, add
`Clone`, or permit a bidder to widen its node budget. The method delegates
to a concrete trusted redactor, not an arbitrary callback receiving bytes.

Correlate captures through the same positional slots as fanout results.
Projection is of decoded content, never the compressed wire representation;
header encoding metadata does not change that definition. Transport errors
without a response body produce fixed metadata only. Temporary raw buffers
remain their existing owners' buffers; debug owns only sanitized results.
Drop redaction scratch after each body, and retain captures under the auction
reservations through response assembly. Preserve existing completion-owned
admission/egress lifetimes when the HTTP response is handed off.

## 6. Resource and Output Limits

Use compiled diagnostic qualification limits, not new runtime operator knobs. `max_outbound_requests`
below is the existing resolved/clamped call ceiling, not an unmaterialized
struct default; the fixed debug ceiling can only narrow it:

| Limit                                                   | Value                                                             |
| ------------------------------------------------------- | ----------------------------------------------------------------- |
| Debug-permitted call records per auction                | `min(max_outbound_requests, 64)`                                  |
| Input considered for one body or MIME/header projection | 64 KiB of already bounded bytes; apply before MIME parsing too    |
| Nodes considered for one body projection                | `min(existing installed node budget, 4096)`                       |
| Body projection JSON nesting depth                      | `min(existing JSON depth ceiling, 64)`                            |
| One sanitized body string                               | 16 KiB UTF-8, before its outer JSON-string escaping               |
| Aggregate retained capture storage                      | 64 KiB, including record/container/key/string allocation capacity |
| Serialized `ext.debug` value                            | 64 KiB, including JSON escaping and metadata                      |

Existing smaller byte/node/depth limits still win. Unrequested or denied
capture must not allocate against these limits. Apply the header-input byte
ceiling before trimming any header value, not only before MIME parsing.
Process bodies sequentially; never create a second complete batch of copied
raw bodies. Use the existing
bounded syntactic JSON walker before typed projection, and a capped output
writer. If a whole projection cannot fit, omit it atomically with the fixed
reason; do not return truncated JSON or cut a UTF-8 sequence.

Retain call rows in ascending original `call_index`, independent of completion
or bidder grouping. When capture storage is exhausted, later rows are omitted
and counted. For final debug-value fitting, remove a captured response body,
then a captured request body, then the entire row at the highest retained index,
repeating until the value fits. Body removal sets `omitted_size`/`truncated`;
row removal also increments `dropped_calls`. Recount the exact serialized value
after counter/status changes, including JSON-string escaping and punctuation.
This is bounded by the fixed row/body counts, not an open-ended retry loop.
If assembly cannot finish within the remaining deadline/allocation allowance,
omit the whole debug object; do not perform a post-deadline trimming sweep or
return a false successful-empty object. HTTP fitting still drops debug wholly.

The existing auction response charge gains an explicit debug term covering:
retained capture allocation capacities, one active redaction input parse /
node / depth scratch peak, bounded MIME/header parser scratch (including
parameter storage), metadata and DTO assembly, an escaped debug
serialization scratch buffer, and output-fit bookkeeping. Derive it with
checked arithmetic from these caps and measured conservative parser/container
factors using the existing memory-factor qualification method. Prepay the
worst-case debug term for every admitted auction before reading its signals;
conditional capture does not justify an unfunded later allocation. Reserve
fallibly before copying/projecting, and measure hostile maximum-node, Unicode,
escaping, tiny-body, many-call, and empty/204-response cases. Fund request
captures even when every bidder returns an empty response.

The original-header count/representability helper must not allocate; full
MIME parsing and metadata projection run only after debug gates pass. The
locked MIME parser owns temporary source/parameter storage, so do not assume
zero-copy or fallible internal allocation. Bound input before invoking it,
measure hostile long/parameter-heavy values, include its coexistence peak in
the prepaid charge and drop parser storage immediately after producing fixed
tokens/eligibility. Qualify input-byte and parameter-count density independently,
including fixed parser overhead; do not treat body-node count as MIME scratch.
Over-limit MIME input omits that header and any nonempty
body requiring it with `omitted_size`; it never copies parameters into debug
or changes ordinary transport behavior. Nonrecoverable allocator OOM remains
outside the capture-refusal guarantee.

Preserve the inherited classified disclosure boundary and its separate charges:
normalized wire/node funding, retained per-scope address growth, reusable scrub
scratch, and the core's plain aggregate audit. Add debug storage to the response
phase; do not replace or consume those other allowances. The engine still accepts
the caller-owned `DisclosureAudit`, and core observes it once before releasing
the existing auction resources, including when debug is denied or omitted.

Direct engine calls without an HTTP boot envelope still enforce these fixed
debug caps; this does not certify an embedded host's total memory envelope.
For a non-HTTP caller, the engine caps the debug value at 64 KiB. An HTTP
caller also applies its installed `final_response_bytes` before attachment
is committed to output: count the base response including required existing
and debug-denial warnings through the structured JSON-size API, then check
the exact incremental `ext.debug` size, including punctuation and escaping.
If the debug value does not fit, discard it wholly and serialize the base
response. Do not drop bids, grow the cap, retry unbounded, or add a marker
that could itself overflow. A base response that already exceeds the cap
retains the existing bounded-serialization refusal behavior.

Capture allocation/projection failure omits debug data without changing
bidder execution or bid validity. Required 10002/10003 warnings retain their
normal diagnostic-budget/error behavior; they are not raw capture data.
CPU spent projecting debug counts against existing deadlines; no deadline
is extended. Check the clock between projections; once exhausted, stop
capture and retain whatever bounded metadata can be assembled without
delaying a terminal response. Never convert missing timing observations into
an exact provider guarantee.

HTTP auction responses to a recognized debug request use `Cache-Control:
no-store`, including account-denied requests and final-output omission.
Apply it through the existing response/egress path, not a new router or
config-reading middleware. Already-existing earlier framework failures keep
their own response ownership; diagnostic qualification does not claim to decorate provider-owned
responses or transport aborts. Raw/sanitized debug payloads are not logged;
logs may use only fixed omission categories and bounded aggregate counters.

## 7. Config Resolution and Error Precedence

Use one direct versioned `OperatorPolicy` containing `schema_version = 1`,
`host`, `account_defaults`, `accounts`, `bidders`, `features` and `floor_currency`.
The existing EdgeZero typed validate/diff/push path validates
and encapsulates this blob with the canonical SHA-256 `BlobEnvelope`; diagnostic qualification uses
the existing store/binding/key, extractor, and trust configuration. This digest
checks integrity, not a cryptographic publisher signature. No additional
runtime signing protocol or separate keys are introduced.

Omission inherits from product data or the parent scope in the same document.
Direct null is rejected except where generated bidder settings explicitly
admit it. Only the operator contract's nullable-field whitelist accepts strict
`{ clear = true }`; it clears optional state, restoring documented fallback,
not permission. Empty objects are no-op overlays. Explicit false, zero, and empty
strings/lists preserve their typed meaning wherever valid. An explicitly
present array replaces its corresponding default array, including `[]`.
Invalid effective required values fail through the existing whole-blob
loader validation; required fields cannot be cleared.
Host/account privacy fall-through applies only when a declared tri-state
field requests fallback after product, account-default and selected named-account
resolution, never merely because a raw key is absent. It is not a general
fallback between unrelated settings.

Malformed or integrity-invalid runtime data anywhere in the blob yields the
existing `MalformedHost` 500, even when it affects a different account or
the user has opted out. Backend/secret-backend/deadline unavailability keeps
the existing 503 with Retry-After; rejected keys retain their existing 500
classification. Missing required policy refuses; it never selects product
defaults, another source or last-known-good state. Store bindings and installed
limits stay host-owned, not a separate startup/source-mode configuration.
Shared validation semantics preserve SDK command ordering and the request
error/admission ordering below; parity does not mean identical side effects.

After EdgeZero ingress/routing/admission succeeds, the shared auction path
loads/validates config and resolves/clamps host policy, reads bounded input,
counts syntactic nodes, discovers account identity, resolves account state,
parses/prepares typed OpenRTB, then runs privacy/alias/auction processing.
Preserve this order, not historical full-JSON-tree extraction examples.
Account discovery must preserve the ordered publisher-path lookup without
converting unrelated opaque numeric tokens to `serde_json::Value`. Valid JSON
such as `banner.ext: {"opaque": 1e400}` must not turn a named disabled account
into the unknown-account fallback merely because a full Value parse cannot
represent that number. Discovery failure is not proof of absent identity.
This is an diagnostic qualification correction to the current discovery path, not evidence of a
successful outbound privacy bypass: later typed preparation may still reject
the request. Invalid JSON/node excess retain their earlier refusals.

Sync also checks its route CPU deadline after host resolution, body extraction,
parsing, and decision processing. The table assumes no earlier CPU refusal;
the final checkpoint can replace a business response with CPU-overload 503
without Retry-After. Keep that distinct from config-store unavailability 503
with Retry-After, and ingress/body RequestTimeout 408. Do not remove checkpoints
to force a business-precedence assertion or add a second config read.

| Competing faults                                                                                                      | Required outcome                                                                                            |
| --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Invalid runtime blob + opt-out / disabled account / malformed body                                                    | Config 500 before application body/account/opt-out work                                                     |
| Valid config + unparseable JSON + disabled account                                                                    | Request 400; account discovery not reached                                                                  |
| Valid config + over-node-budget parseable JSON + disabled account / 2.6 field violation                               | Node-budget 400 before account discovery                                                                    |
| Valid config + within-budget body + disabled account + missing imp / invalid channel / 2.6 field violation            | Account 503 before typed preparation                                                                        |
| Valid config + named disabled account + within-budget JSON with an unrelated opaque `1e400` token                     | Same account 503; numeric conversion failure cannot select the unknown account                              |
| Valid config + within-budget body + clean account + missing imp                                                       | Existing request-validation 400                                                                             |
| Valid config + host opt-out marker on cookie_sync + disabled account / invalid filter / missing consent               | Host opt-out 401 before account lookup, signing-key read and authenticated UID-cookie decode                |
| Valid config + no host marker + disabled account + otherwise valid signed UID-cookie opt-out                          | Existing account-disabled 400 before signing-key read/UID decode                                            |
| Valid config + allowed account + unavailable signing keyring + UID-cookie opt-out                                     | Signing-unavailable 503 before authenticated UID-cookie opt-out can be trusted                              |
| Valid config + allowed account + usable keyring + authenticated UID-cookie opt-out + invalid filter / missing consent | UID opt-out 401 before privacy/filter evaluation                                                            |
| Valid config + no host marker + disabled account without UID opt-out                                                  | Existing account-disabled 400                                                                               |
| Valid config + route CPU deadline expired at a checkpoint + otherwise applicable sync business refusal                | CPU-overload 503 without Retry-After at that checkpoint; final checkpoint may replace the business response |

Framing/admission/encoding/byte-cap failures at an earlier framework stage
still win; application fixtures must state which boundary they exercise.
Probe routes preserve their existing degraded status rather than inheriting
auction/sync fail-closed response mappings. Independently fetched malformed
account records and their 500/400 mappings are stored requests and accounts design work, not reachable
diagnostic qualification fixtures or a reason to add account storage now.

## 8. Updates, Freshness, and Recovery

Each request uses one validated snapshot for permissions, debug gates,
resource clamps, aliases, and response decoration. A store change while that
request runs does not change its decisions. Later requests and sync callbacks
resolve again rather than reusing policy from an earlier auction or sync URL.
Runtime settings cannot expand a boot-funded ceiling or replace compiled
bidder safety metadata. Do not reinitialize the admission envelope on reload.

Test through the generated App with a controllable store: valid revision A,
replace with B during a request, next request sees B, malformed/integrity-invalid
replacement fails closed, unavailable store uses its existing transient
mapping, deletion follows compiled missing-data policy, and valid recovery
restores normal behavior. Verify one config extraction per config-consuming
application request in these flows
and repeat the applicable update/failure cases for auction, cookie_sync,
setuid, and getuids, with existing probe behavior covered separately.
Fixtures use the current envelope builder and binding/key/extractor path, not
invented bare JSON overrides or a second loader.

The sync-consumer portion is already inherited from user sync:
[usersync_config_lifecycle.rs](../../../crates/auctioneer-core/tests/usersync_config_lifecycle.rs),
[usersync_precedence.rs](../../../crates/auctioneer-core/tests/usersync_precedence.rs)
(including setuid snapshot/recheck), and
[usersync_flow.rs](../../../crates/auctioneer-core/tests/usersync_flow.rs).
These cover one-read snapshots, next-request rechecks, whole-blob refusal,
redacted config/backend errors, outage, fail-closed deletion, and recovery.
Retain them as regressions. New diagnostic qualification work extends the auction/debug consumers,
gate/resource-clamp changes, and genuinely missing competing-fault cases;
do not recreate or relabel the existing sync lifecycle as new diagnostic qualification work.

The controllable-store App tests prove snapshot/update consumption, not a
backend's ability to refresh its source. Rechecked at EdgeZero pin
`596f9f4e23e43c8952a51ce6d92961d461032004`, Axum's local-file store reads its
map at startup and serves bounded reads from that resident snapshot. A typed
push/file replacement alone does not change a running native process's
effective config. Native variant, corruption, restoration and rollback runs
must install the intended store input before process startup, restart for
each replacement and observe the effective serving settings/identity. Keep
Task 8's mutable-store in-flight A/next-request B tests: they cover the
application contract without claiming native file hot reload. Record native
live reload as unsupported at this pin; it is not a blocker for diagnostic qualification behavior
or the restart-aware native acceptance matrix.

Record each backend's actual cache/refresh/restart semantics and a measured
visibility observation separately, not globally instantaneous rollout.
Until EdgeZero specifies a propagation bound, do not claim one or block all
diagnostic qualification behavioral work waiting for it. A cached valid older revision is distinct
from Auctioneer receiving an invalid revision and substituting an older policy.

## 9. Residual Acceptance Inventory

The implementation plan must carry these rows with exact test-file references
and evidence status. Existing coverage stays a regression requirement; do
not count it as newly implemented diagnostic qualification work merely by renaming tests.

| Surface                                                                 | diagnostic qualification disposition                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Account/base-bidder debug gates, signals, warning placement             | New diagnostic qualification tests across normal and early-return paths                                                                                                                                                                                                                                                                |
| Debug wire schema, redaction, positional timing, output/resource limits | New diagnostic qualification unit, engine, and actual-App integration tests                                                                                                                                                                                                                                                            |
| Transport formatter payload/credential omission                         | Inherited bounded fanout `b1561c0`; colocated `request_debug_omits_payload_url_headers_and_identifiers`, `response_debug_cannot_recover_sealed_body_or_headers` in `auctioneer-bidder/src/types.rs` and `metadata_debug_omits_the_raw_pool_and_credential_offsets` in `src/meta.rs`; not diagnostic qualification debug DTO/projection implementation |
| Hostile privacy inputs and policy composition                           | Inherited privacy enforcement `1bad3ff`; shared input fixture and independent 256-decision/idempotence oracle remain separate from new debug expectations                                                                                                                                                                         |
| GPP SID-selected section absent and fallback                            | Existing privacy enforcement coverage; retain named regression evidence                                                                                                                                                                                                                                                           |
| Fast/slow bidder per-call timing                                        | Existing fanout coverage; extend to debug call timing                                                                                                                                                                                                                                                            |
| getuids host-cookie merge                                               | Existing user sync coverage; retain regression evidence                                                                                                                                                                                                                                                                 |
| setuid activity/TCF ordering and exact 451 bodies                       | Existing user sync coverage; retain regression evidence                                                                                                                                                                                                                                                                 |
| SkipWhen SID exact-string behavior                                      | Existing privacy/usersync coverage; retain regression evidence                                                                                                                                                                                                                                                   |
| Cookie opt-out vs account/filter/consent                                | Existing user sync precedence and lifecycle coverage; retain whole-blob config refusal, not an unreachable separately malformed account-store fixture                                                                                                                                                                   |
| Deep typed overlays across sibling subtrees                             | Extend current suites across auction and sync consumers                                                                                                                                                                                                                                                          |
| Structural-budget/account/config error precedence                       | Extend competing-fault fixtures using section 7                                                                                                                                                                                                                                                                  |
| Snapshot/update/invalid/outage/deletion/recovery                        | Sync consumers inherited from user sync's lifecycle/precedence/flow suites; extend auction/debug-specific cases using section 8                                                                                                                                                                                         |
| Browser sync preflight/callback/cookie/denied origin                    | Existing user sync actual Chromium harness; retain its cross-origin, same-site loopback evidence boundary                                                                                                                                                                                                               |
| Reported intermittent `usersync_routes` failure                         | [Investigation #30](https://github.com/IABTechLab/auctioneer/issues/30): open, not reproduced; fixed clock is not a proved fix. Retain complete binary logs and investigate any recurrence; no sleeps, blanket serialization or raised limits to manufacture green verification                                                                       |
| Provider-added response headers                                         | [Target scope #50](https://github.com/IABTechLab/auctioneer/issues/50): application header count/byte checks exclude provider-added framing fields. Record target wire scope; successful application caps are not whole-wire bounds                                                                                                                  |
| Cloudflare/Spin/Fastly offline handler harness                          | diagnostic qualification artifact handoff/refusal tests, bounded fixtures/evidence and authorized corruption/recovery runbook preparation                                                                                                                                                                                              |
| Cloudflare/Spin/Fastly deployed handler execution                       | publisher pilot deferred/unexecuted live cases, config visibility and authorized corrupt-config injection/recovery; not an diagnostic qualification completion gate; keep compile/conformance/deployment evidence separate                                                                                                                        |
| Separate account store and deletion-patch policy                        | stored requests and accounts; not required or implicitly restored by diagnostic qualification                                                                                                                                                                                                                                                                    |

Redaction tests seed distinctive secrets in native/legacy/flattened fields,
URL paths/query/userinfo, mixed-case/duplicate headers, cookies, creative and
tracking content, opaque vendor payloads, malformed JSON, invalid UTF-8,
compressed content, and unknown formats. Assert secrets are absent from the
complete serialized debug subtree and captured logs, not only one field.
For each masking/omission canary, also require `body_status: captured` and exact
known-safe leaves/masks below all capture caps. This prevents whole-body refusal
from vacuously passing redaction. Keep deliberate overflow/unsupported fixtures
separate and assert their exact omission statuses. Vary permission, byte/node
shape and media headers independently; mutation controls must catch both a
leaking projector and one that silently omits all content.
Use no-bid fixtures to also assert absence from the entire response. Normal
bid creative fields remain their existing delivery surface, not debug
projections; under identical fixed-clock/outcome fixtures, assert ordinary
bid/outbound bytes are unchanged by capture. Debug CPU remains real auction
work and may consume deadline budget; the tests must not promise zero cost.

Reuse [privacy_fixture.rs](../../../crates/auctioneer-exchange/tests/support/privacy_fixture.rs)
for hostile input construction only. Preserve the independent `expected_wire`
oracle and exhaustive 256-decision/idempotence tests in
[scrub/composition_tests.rs](../../../crates/auctioneer-exchange/src/scrub/composition_tests.rs).
Debug projection needs its own unconditional omission/masking expectations:
data permitted in an outbound privacy projection is not automatically safe
in debug output. The inherited safe Rust `Debug` formatters also do not
implement this new body projector. Section 6's prepaid debug charge and
debug-on/off/denied allocation/concurrency qualification remain diagnostic qualification work;
do not charge unused capture work in earlier capabilities.

Gate tests include aliases of debug-disabled bases, multiple calls, OR/false
signal combinations, duplicate/encoded/malformed query/header inputs,
account denial on tmax/no-live-bidder exits, and absence of capture work when
unrequested/denied. Resource tests cover exact cap boundaries, debug-only
output overflow, valid UTF-8/JSON after omission, fallible allocation,
deadline expiration, and reservation release on success/error/cancellation.
Exercise the fixed call ceiling with funded synthetic 63/64/65-call profiles
whose existing call ceiling exceeds 64; separately vary smaller runtime ceilings
and permission-denied populations. Do not enlarge shipped caps to make the test
bind. Include escaping-heavy captures that fit retained/body caps but exceed
the serialized debug cap, with exact deterministic trimming assertions.
Keep unit tests beside their implementation; cross-module, actual-App,
allocation, and deployed-provider tests live separately. Name tests by
behavior, not by capability.

## 10. Provider and Pilot Evidence

Use the EdgeZero manifest/generated App, shared actions, and existing
admission/egress hooks. Do not build adapter-local routes or bypass those
hooks to make a deployed test pass. Reuse bounded fanout's target-indexed capability and
conformance matrix rather than inventing a new all-target production gate.

diagnostic qualification delivers the offline harness, isolated artifact handoff/refusal tests,
case templates and runbooks, including corruption/recovery preparation.
Credentialed real provider execution and the selected live limited-bidder
pilot belong to publisher pilot after Trusted Server embedding. The execution contracts below are retained
for that capability; offline simulated results never close their live rows.

Local browser evidence already exists in user sync `5367ab0`:
[check-usersync-browser.mjs](../../../scripts/check-usersync-browser.mjs),
the [run instructions](../../testing/usersync-browser.md), and the test-only
[usersync_browser_config.rs](../../../crates/auctioneer-adapter-axum/examples/usersync_browser_config.rs).
Chromium 153.0.8010.12 passed browser-generated preflight, credentialed CORS,
the emitted callback through a controlled local redirect, actual secure-cookie
acceptance/delivery, and denied-origin enforcement against the real Axum binary
and generated App. A separate pending-launch regression covers SIGTERM cleanup
and permission-failure reporting; it is not a substitute for the browser run.
The browser build uses `insecure-bidders` only for HTTP loopback origins.
Different ports are cross-origin but same-site: this proves neither cross-site
iframe/third-party-cookie acceptance, real bidder redirects, other browsers,
nor hosted-provider execution. Preserve the evidence rather than claiming it
as new diagnostic qualification provider coverage; see the [user sync execution record](../plans/2026-09-30-usersync-verification.md#october-5-stacked-hardening).

In publisher pilot, for each Cloudflare/Spin/Fastly deployment, record measuring commit,
EdgeZero pin, target/config identity, harness version, and results for a valid banner
auction, timeout/failed bidder, malformed request/config, denied debug,
redacted debug, bounded input/output, response-header fidelity, and
cancellation/completion behavior where observable. Use controlled mock
bidder endpoints and test credentials; do not commit real secret payloads.
Raw-parser/egress guarantees remain the provider/framework owner's evidence,
not a guarantee inferred from a successful Auctioneer handler response.

Hosted fixtures must be reachable through an explicitly bound, build-time
HTTPS endpoint in an isolated test artifact. Reuse the existing feature-gated
bidder-info override with a compiled reference adapter, record its metadata
digest/features and prove actual fixture dispatch; do not change committed
production metadata or introduce runtime endpoint overrides. That artifact
provides handler evidence, not production qualification or bidder certification.
Config push validates inputs and generates a valid integrity envelope, so a
rejected push cannot prove deployed-handler fail-closed behavior. Corrupt
stored-config cases require explicit authorization to mutate a dedicated test
deployment's provider-native binding/key, bounded corruption fixtures, actual
handler observation and observed restoration through the normal typed push.
Keep production/shared bindings untouched and document recovery for abrupt
termination; failed injection, stale reads or failed restoration are not passes.

Offline native behavior tests and all existing wasm compile checks are
required for the implementation PR. diagnostic qualification completion requires its new behavioral
rows, offline provider harness/artifact handoff/refusal tests and runbooks,
and section 10.1's Q2-A/B/C/reusable-E evidence. Credentialed live-handler
and corruption/recovery rows remain explicitly unexecuted under publisher pilot, outside
diagnostic qualification completion. Pending/unsupported target rows still block that target's
live-coverage/qualification claim; native evidence is not live-pilot readiness.
Per-target production certification remains a separate recorded decision.
Positive real response-body debug coverage requires consuming the trusted SDK
provenance under section 5.1. Task 3 completes that boundary's native transport
acceptance; engine/HTTP integration remains diagnostic qualification work and hosted execution
remains open under publisher pilot. Record Cloudflare's `Unknown` provenance and fail-closed body omission as
the expected provider limitation, never as successful positive body capture.

Fastly fixture deployment also needs a complete managed application-release
bundle, not just a WASM/package upload. Prepare the application CLI, package,
application and adapter manifests, and `release.json` metadata/digests matching
the isolated fixture artifact/features. Use the pinned SDK verifier and deploy
path; missing or mismatched bundle members fail before deployment. This adds
no separate promotion or signing protocol.

Spin's isolated deployment manifest must reference the exact component emitted
by the fixture build, with matching profile/features and verified digest. An
isolated Cargo target directory does not rewrite a manifest's static `source`;
never fall back to a repository-local component from an earlier build.

The [publisher pilot limited-bidder banner pilot](../../roadmap.md#8-limited-bidder-publisher-pilot-and-hosted-acceptance)
follows Trusted Server embedding integration and the selected bidder migration wave/host setup,
without requiring full catalog closure or video parity. It still depends on
selected bidder support/credentials, real consent and
identity flow, targeting and creative rendering, reference-data freshness,
Q2 deployment-envelope acceptance, and limited rollout/rollback. A passing
mock auction is neither bidder certification nor direct embedding completion.

### 10.1 Deployment Resource Qualification (Q2)

**Owner:** Auctioneer qualification tooling and the selected deployment's owner.
Q2 is the deployment-envelope acceptance item carried from the bounded fanout review.
diagnostic qualification must turn provisional resource assumptions into a reproducible qualification
workflow and tuned native artifact/configuration profile. publisher pilot executes Q2-D
on the selected actual deployment before its publisher pilot, using that
retained workflow. This adds no configuration loader, signing layer, mandatory
workspace crate, or dependency on metrics and analytics's full metrics program.

**SDK boundary:** Q1 (Axum raw connection/parser admission), Q3 (config-store
pre-extraction allocation bounds), and Q4 (provider transport lifecycle and
hosted guarantees) remain EdgeZero-owned tasks. Q1 is being handled in
EdgeZero; that is not evidence it has shipped or passed acceptance. Auctioneer
records the effective SDK revision/capabilities and runs application scenarios,
but does not duplicate the listener, parser, backend reader, or delivery driver.
An open SDK row does not block building Q2 tooling or unrelated diagnostic qualification behavior;
an unsupported required guarantee prevents qualifying that target's profile.

#### Profile and Evidence Contract

- Identify the Auctioneer commit, release artifact digest/build features, EdgeZero
  pin, target/runtime and actual platform limits, harness/fixture revision,
  reference-data revisions, and effective boot/runtime configuration identity.
  Identify the config backend's update mode (startup snapshot or live refresh)
  and bind digests to the serving snapshot, not merely the current disk/store
  value. Record restart boundaries and effective-setting observations for
  startup-snapshot variants. Include enforced SDK resource limits and any
  unqualified provider boundary.
  Include linked dependency development features, including configuration test
  overrides; fixture/override artifacts may have local acceptance but cannot
  receive production qualification.
  Bind the effective SDK ingress/store settings parsed from the exact sanitized
  child environment used for that process, not only compiled Auctioneer caps and a
  stored-config digest. SDK environment overrides may change the effective
  envelope while both those identities stay fixed. Reuse SDK structured parsers;
  export/hash only the allowlisted resolved settings, never a raw environment
  that can contain credentials. Environment-only setting changes invalidate
  old observations just as effective runtime-config changes do.
  Reports use a fixed allowlist: artifact/configuration digests, controlled
  fixture identifiers/digests, validated numeric limits, fixed target/version
  identifiers, bounded category codes and aggregate measurements. Do not
  persist raw configuration, auction/bidder bodies, credentials, cookies,
  consent strings, IPs, EIDs/user identifiers, URL userinfo/paths/query strings,
  free-form backend errors or raw debug payloads. "Exact inputs" means the
  effective safe profile plus immutable fixture references, not payload copies.
- Declare the load shape, concurrency, duration/repetitions, memory headroom,
  latency percentiles/thresholds and per-scenario outcome bounds before the run.
  Specify minimum required successful/admitted work and maximum tolerated
  rejection/error/timeout counts or rates for normal workloads; intentional
  saturation/failure scenarios instead specify their expected outcome bounds
  and healthy recovery criteria. Define latency start/end points and sample
  populations, report failures/timeouts separately, and do not silently exclude
  them to manufacture a passing success-only latency result. Evidence must
  show that the intended admitted concurrency, calls and byte/node load were
  exercised; generator underload or unexpected policy filtering cannot count
  as qualification. Report the exact accepted safe inputs; silently lowering
  fixture sizes or raising thresholds after observing results cannot pass.
  Commit a versioned candidate profile with actual numeric durations, repetitions,
  windows and criteria before executing it. Bind fixture/subcase IDs, a generator
  seed where relevant, and bounded hardware/OS/meter identities. These are
  candidate-specific criteria, not invented universal performance floors.
  Broad peak ceilings are envelope criteria, not tight memory-regression budgets
  or estimates of the minimum safe deployment size. A large margin between an
  observed peak and its predeclared ceiling does not establish unmeasured
  workloads, provider memory bounds or safe capacity below the funded envelope.
  Missing subcases/repetitions or shortened windows fail even if all six workload
  family names appear and an instantaneous peak reaches the target concurrency.
  Admission-witness maxima do not prove ordinary dispatch: reconcile successful
  ordinary request identities against the controlled bidder's call counts.
  Credit a GVL overlap only while a separate ordinary auction is observed
  holding actual bidder work throughout the lookup.
- Derive the funded resource envelope with the existing checked runtime model,
  including diagnostic qualification debug charges. The candidate must fit its real platform budget
  with the declared headroom. Preserve the current warn-mode boot policy:
  serving after an over-ceiling warning does not qualify a profile. Do not
  remove ledger terms or increase a claimed platform ceiling to hide a failure.
- Measure cold startup, warm serving baseline and peak live memory during each
  workload. Record the measurement source, sampling/high-water method, units
  and limitations. Native RSS, allocator usage, WASM guest memory and provider
  meters are distinct measurements, not interchangeable proof. A sampled
  average or missing short-lived peak cannot establish a hard peak bound.
  A supported high-water observation must be nonzero, service-only, use the
  declared meter and its matching limitation, and cover the required window
  without exceeding the actual measurement window. Peak plus headroom must fit
  both independent platform and installed ceilings, with checked arithmetic;
  retained growth has its own limit. Zero or overflowing readings cannot pass.
  Same-process lifetime high-water observations must not decrease across the
  cold/warm and declared scenario schedule; window-scoped meters remain distinct.
- Record latency distributions, admitted/rejected/error/timeout outcomes and
  gate recovery against the predeclared criteria. Measurements complement
  enforced bounds; they do not replace a missing admission or allocation cap.
- Emit a bounded, structured report with per-scenario pass/fail/pending or
  unsupported evidence and a separate overall qualification result. Missing
  mandatory evidence, identity mismatch, exceeded thresholds or required
  unsupported capabilities must never become an overall qualified result.
  Provide a repeatable invocation and exact artifact/profile pair for reruns.

Reports distinguish `local_acceptance` from `profile_qualification`.
Local acceptance describes the mandatory application/resource workload
evidence for the identified local artifact and its declared, explicitly
limited envelope. It does not establish an unsupported SDK guarantee or
production suitability. All mandatory local measurements and acceptance
criteria passing can complete Q2-C while a required unverified/unsupported
SDK boundary keeps profile qualification pending/unsupported. Missing local
evidence, identity mismatch, underload or failed thresholds cannot pass local
acceptance either. Test this combination explicitly: local acceptance passes,
required SDK evidence is absent/unsupported, overall profile is not qualified,
and publisher-pilot readiness is not granted.

#### Required Workloads

Use controlled bidder endpoints and the actual release application through
the existing EdgeZero generated App and shared handlers. Keep load-generator
and fixture-server resource usage separate from the measured service. Reuse
existing allocation guards, TCP fixtures and GVL cold-parse work where useful;
do not replace them with another application server or private auction path.

| Workload                         | Required Evidence                                                                                                                                                                                                                                                                                                                                                              |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Cold and warm baseline           | Configuration load, cold GVL parsing, warm caches, and retained reference-data costs; distinguish startup from steady serving.                                                                                                                                                                                                                                                 |
| Maximum admitted auction work    | Requests at the selected profile's byte/node and impression limits, supported FPD expansion, maximum funded bidder/call concurrency and response bookkeeping; justify limits that cannot peak simultaneously.                                                                                                                                                                  |
| Response/decode overlap          | Independent encoded/decoded caps, maximum permitted decompression, mixed success/error/timeout outcomes and retained output during controlled slow reads.                                                                                                                                                                                                                      |
| Debug states                     | Unrequested, denied and permitted maximum-bounded capture/output, including concurrent capture and peak release; safe diagnostics remain mandatory.                                                                                                                                                                                                                            |
| Concurrent auxiliary/config work | Status, catalog, existing sync routes where enabled, bounded extraction of the serving config snapshot and GVL cache misses overlapping funded auctions. Native config variants require restart at the inspected pin; live update overlap is exercised only by capable backends and the controllable-store App suite. No config variant may bypass the installed boot ceiling. |
| Saturation and recovery          | Excess admissions, queued outbound work, stalled I/O, cancellation/disconnect and timeout; bounded refusal behavior and a subsequent healthy wave demonstrate returned capacity without sustained growth.                                                                                                                                                                      |

For native measurements, install each controlled config variant through the
existing typed push before spawning the same inspected release artifact.
Verify its effective gates/clamps/alias settings with bounded observations
and bind evidence to that startup snapshot. If the file changes while it
runs, it still serves the earlier snapshot; do not record the new file digest
as the active config or claim an update took effect without restart. Each
variant/restoration uses a fresh process with separate cold/warm/meter
windows. Hosted update/visibility evidence is separate and follows that
backend's actual capability. A later SDK reload capability requires a newly
recorded pin/profile and rerun, not an invented Auctioneer loader.

Tuning uses the existing boot-profile and runtime-configuration mechanisms.
Reducing concurrency/caps or choosing a larger actual deployment budget is
allowed; bypassing privacy, weakening byte/node accounting or treating a
runtime override as funding above the boot ceiling is not. The actual baked GVL
LRU capacity remains funded: lowering an accounting-only `gvl_cache_entries`
declaration does not resize the separately constructed cache and is not an diagnostic qualification
tuning lever. Preserve cold/miss/overlap workloads at the real capacity. Rerun
acceptance
after tuning. Controlled bidders establish application/resource behavior,
not the selected real bidders' certification or creative-rendering evidence.

#### Deliverables and Acceptance Gates

The diagnostic qualification implementation plan must give these rows explicit task ownership and
evidence status, separating diagnostic qualification deliverables from deferred publisher pilot execution
rather than leaving Q2 as an advisory paragraph:

- [x] Q2-A: validated profile/report contract and negative tests for missing
      evidence, identity mismatch, unsupported required capabilities, invalid
      thresholds, missed workload targets, excessive failures/rejections/timeouts,
      shortened windows, skipped repetitions/subcases, changed effective SDK
      environment, biased latency populations, over-ceiling envelopes and qualification-result
      classification. Seed sensitive values and assert their absence from complete
      serialized reports and harness logs; raw payload/configuration fields must
      not be accepted by the report schema.
- [x] Q2-B: reproducible workload fixtures and release-artifact load harness,
      with ordinary unit tests beside implementation and TCP/allocation/process or
      provider-boundary tests in integration targets; behavioral test names only.
- [x] Q2-C: execute and record the cold/warm, maximum-admitted, debug and
      overload/recovery acceptance matrix against native Axum, with a tuned local
      profile and measured limitations. A passing local-acceptance result closes
      this row even when separately recorded SDK gaps leave profile qualification
      pending/unsupported. Local proof does not qualify hosted WASM or a publisher
      pilot.
- [ ] Q2-D (publisher pilot, deferred/unexecuted; not an diagnostic qualification completion gate): before a
      publisher pilot, execute that matrix on the selected actual
      deployment and publish the exact artifact/profile, accounted budget, peak
      memory, latency/outcomes, required SDK capability evidence and acceptance
      result. Other targets retain separate pending/unsupported rows.
- [x] Q2-E: provide a runbook with bounded rollout, monitoring/stop thresholds,
      prior known-good artifact/configuration, coordinated rollback and recheck
      steps. Restoring native startup-snapshot configuration requires restarting
      the serving artifact and observing recovery; writing the file alone does
      not complete rollback. Other backends require their actual visibility or
      restart procedure. Qualification is invalidated by changes to the artifact,
      effective profile, SDK/runtime, relevant reference data or platform budget; rerun the
      affected evidence rather than reusing an unrelated passing report.

Q2-A/B/C and the reusable Q2-E runbook are required diagnostic qualification implementation
deliverables. Q2-D and deployment-specific rollout evidence are publisher pilot-owned
execution, mandatory before that publisher pilot and not an diagnostic qualification completion
gate. A missing hosted environment leaves Q2-D open; it does not close actual
deployment qualification or authorize claiming pilot readiness. Do not require
every unrelated provider to qualify before merging behavioral work or starting
another capability. bounded fanout's implementation merge remains independent of Q2;
selected-target production qualification remains a recorded decision.
