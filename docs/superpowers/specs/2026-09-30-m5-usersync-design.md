# M5 User Sync Design

Date: 2026-09-30
Status: Draft for user review
Worktree: `.claude/worktrees/m5-usersync` (`m5-usersync`)

## 1. Purpose and Scope

Implement the three user-sync endpoints in the master design:
`POST /cookie_sync`, `GET /setuid`, and `GET /getuids`. This milestone owns
their configuration, UID cookies, candidate selection, URL construction,
privacy decisions, browser integration, and bounded handler execution.

Auctioneer is still in development. Existing internal Rust APIs, placeholder
types, and their tests may change together. Do not retain compatibility
aliases, permissive implementations, boxed callbacks, or marker fields
solely because an earlier milestone introduced them. Preserve the intended
observable behavior, not the scaffolding used to compile unfinished code.

M5 can be designed and implemented while M3b and M4 are under review.
Integrate in order: M3b, M4, then M5. Provider production qualification is
not a prerequisite for writing or testing user-sync logic. M5 must still
use the admission, configuration, clock, privacy, and egress contracts
provided by those milestones; it must not bypass them.

The worktree starts at M4 commit `f91c23edbe965561f962e937dc1af1e1ba4f275f`.
The uncommitted M4 review fixes are not copied or committed by this design
task. Reconcile the final M4 changes before integrating M5.

### In Scope

- Real sync privacy evaluation, including host-cookie GDPR permission,
  per-bidder GDPR permission, activities, and GPP/USP signal handling.
- Cookie serialization, opt-out, host-cookie merge, expiration, deletion,
  attributes, and priority-aware size eviction.
- Issued-cookie integrity, atomic secret-store key rotation and rejection of
  unsigned state, under the approved
  [cookie integrity addendum](2026-10-13-usersync-cookie-integrity-design.md).
- Cooperative selection, filters, aliases, shared syncer families, and limits.
- Safe callback and bidder URL templates, preserving bidder UID macros.
- Baked defaults and pushed runtime settings through the existing config path.
- HTTP behavior, credentialed browser access, diagnostics, resource accounting,
  native integration tests, and the existing multi-target build matrix.

### Not Included

- Full migration of upstream's bidder catalog. The master design only schedules
  four reference adapters; catalog migration needs a separate milestone.
- Automatic bidder registration, which has its own design and is not an M5
  prerequisite. Consume the registry API, not a second hard-coded bidder list.
- Server-to-server execution of sync URLs, a persistent UID database, cookie
  encryption or authenticated bidder-UID ownership, plugins, runtime bidder
  code loading, or a new workspace crate. Cookie integrity is included above;
  signing alone cannot authenticate the public callback's supplied UID.
- New US-state GPP enforcement or Privacy Sandbox cookie-deprecation support.
- Auction debug payloads and full telemetry, which remain M6 or later.
- Automatic ingestion of `uids` into auction bidder requests. That path is
  not implemented by the current auction engine and is not specified by the
  master's M5 endpoint scope. `/getuids` exposes the map to clients; M5 must
  not claim that a stored UID is already delivered to bidders in auctions.
  A separately reviewed auction identity projection change must define
  precedence and privacy scrubbing before adding that integration.

## 2. Sources and Authority

The [master design](2026-05-21-auctioneer-openrtb-server-design.md), sections
7.8, 8.2-8.6, 9, 11, 14, and appendices B, D, and F, defines the milestone.
The [M4 design](2026-09-26-m4-privacy-design.md) and
[GPP envelope design](2026-09-29-gpp-envelope-module-design.md) define shared
parsers and privacy foundations. The
[M3b plan](../plans/2026-07-10-m3b-multi-adapter-production-fanout.md) defines
admission and response lifetime requirements.

The behavioral reference is the
[pinned source snapshot](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e)
at commit `d7f8515b86258688304b0d9b6668c6a0e258bc9e`, not upstream HEAD.
Reference areas are sync endpoint responses, cookie encoding and eviction,
syncer selection, effective host/account/bidder settings, privacy enforcement,
and name-matching semantics. The requirements below describe observable
behavior rather than adopting the reference's source files or functions.

Reference source was inspected from the locally available pinned source
snapshot. Fetching its raw GitHub pages during this draft returned cache
misses; no new upstream revision or live provider behavior was verified.
Implementation fixtures must record the source commit, generator version,
and attribution rather than depending on that local filesystem path.

This document specifies the proposed M5 contract. Upon approval it supersedes
the master's unfinished sync API examples and the explicitly listed
clarifications in section 13. Unlisted endpoint semantics continue to follow
the master. Claims of byte parity require committed reference fixtures;
similar-looking implementation and reference error messages are not sufficient evidence.

## 3. Ownership and Structure

| Owner                   | Responsibility                                                                      |
| ----------------------- | ----------------------------------------------------------------------------------- |
| `auctioneer-config`         | Shared sync configuration types, baked metadata, code generation, common validation |
| `auctioneer-config-schema`  | Existing source-document structure; no bidder implementation dependency             |
| `auctioneer-core::config`   | Runtime overlays, validation, and one resolved config snapshot per request          |
| `auctioneer-privacy::sync`  | Input normalization, consent usability, host and bidder permission decisions        |
| `auctioneer-core::usersync` | Pure cookie, candidate, catalog, template, and response-format modules              |
| `auctioneer-core::handlers` | Thin HTTP pipelines, config extraction, admission/egress integration                |
| `auctioneer-runtime`        | Existing clock/RNG, bounded parsing, checked resource derivation                    |
| Provider adapters       | Existing shared application wiring and response/header fidelity                     |

Use focused modules such as `cookie`, `catalog`, `choose`, `template`,
`wire`, and `limits` under `auctioneer-core::usersync`. These modules take typed
values and injected clock/RNG inputs rather than `RequestContext` wherever
possible. Only handlers perform framework work. Neither cookies nor privacy
evaluation makes network requests.

No public trusted-server user-sync SDK is promised by M5. Pure modules make
later extraction possible without introducing another crate now. The
alternative of a dedicated crate adds a boundary without a current second
consumer; putting all behavior into handlers would make precedence and
cookie algorithms unnecessarily difficult to test.

### EdgeZero Handler Integration

Use EdgeZero's existing application path, not an adapter-specific router:

- Declare `cookie_sync` (POST), `cookie_sync_preflight` (OPTIONS), `setuid`
  (GET), and `getuids` (GET) in `edgezero.toml`, each with `class = "aux"`
  and all four existing adapters. Mirror method/path/id and body caps in
  `auctioneer-runtime::body_caps`; keep its manifest parity test authoritative.
- Export thin `#[action]` functions from `auctioneer-core::handlers`. Each uses
  `with_aux_egress(ctx, <handler>_body)` and the existing
  `edgezero_core::app!(..., configure = configure_app)` generated `App`.
  Preserve the admission hooks and completion-owned permit; do not acquire
  another permit or call a handler directly from a provider adapter.
- Resolve config once with the existing `resolve_runtime_config` path.
  Apply browser-response policy from that same validated snapshot, without
  a second middleware config read. Use `RequestContext` for bounded input
  access and EdgeZero `Response`/`Body` for output.
- The stock query extractor alone does not implement strict percent/UTF-8
  validation and first-duplicate-wins semantics. Use a bounded sync parser
  over `ctx.uri().query()`, retaining a structured form-decoding library;
  do not replace framework dispatch to obtain these endpoint semantics.
- Test through `App::build` and `dispatch_ingress`, in addition to pure
  unit tests. Exercise admission refusal, config failure, routed OPTIONS,
  response headers, cancellation, and terminal completion. Adapter entry
  points continue using their existing `run_app::<auctioneer_core::App>()`.

## 4. Configuration and Catalog

### 4.1 Configuration Shape

Extend the existing baked/override model, not the historical standalone
host/account config-store scheme. `auctioneer.toml` remains the source:

| Source path                                        | Fields                                                                                                 |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `baked.host.host_cookie`                           | `cookie_name`, `family`, `domain`, `ttl_days`, `max_cookie_size_bytes`, `optout_cookie`                |
| `baked.host.user_sync`                             | `external_url`, `redirect_url`, `cooperative.enabled_by_default`, `priority_groups`, `allowed_origins` |
| `baked.account.cookie_sync`                        | `default_limit`, `max_limit`, `default_coop_sync`, optional `priority_groups`                          |
| `runtime.host_cookie`                              | Optional per-field host cookie overrides                                                               |
| `runtime.user_sync`                                | Optional per-field host sync overrides                                                                 |
| `runtime.accounts.<id>.cookie_sync`                | Optional per-account sync overrides                                                                    |
| `runtime.bidders.<name>.usersync_enabled`          | Optional sync disable/enable control, bounded by baked eligibility                                     |
| `baked.host.gdpr.tcf2.purpose_one_treatment`       | `enabled`, `access_allowed`; both default true as in the pinned reference                              |
| `baked.account.gdpr.purpose_one_treatment`         | Optional per-field overrides of the host treatment settings                                            |
| `runtime.gdpr.tcf2.purpose_one_treatment`          | Optional host treatment overrides                                                                      |
| `runtime.accounts.<id>.gdpr.purpose_one_treatment` | Optional account treatment overrides                                                                   |

Keep baked bidder endpoints, UID macro text, syncer keys, supported types,
GVL IDs, and regulation-skip metadata in bidder-info. Normalize existing
`UserSync` metadata into a typed `SyncerConfig`; replace the free-form type
strings with validated `Iframe` and `Redirect` values. An integration without
sync metadata remains a known bidder with no configured syncer.

There is no API compatibility requirement for the old `UserSync` or
`SyncerLookup` shapes. Update code generation, defaults, all struct literal
callers, fixture builders, schema tests, and docs atomically.

The fixed Auctioneer cookie name is `uids`. `host_cookie.cookie_name` names a
separate host identity cookie to import under `host_cookie.family`; it does
not rename `uids`. Require a nonempty name and family, disallow `uids` as the
host identity name, and validate cookie names and attributes as cookie/header
data. Reject control characters, CR/LF, invalid domains, and overflow in TTL
arithmetic. Require positive TTL days; default 90. Preserve the logical size
setting: zero means no operator eviction threshold, otherwise at least 500
bytes. It never disables the hard output limits in section 10.

Opt-out configuration absent or both strings empty disables host opt-out.
Exactly one empty string is invalid. `Some(false)`, zero, empty strings
where allowed, and an explicit empty priority list must not be treated as
missing. Account priority groups, when present, replace the host list even
when empty; otherwise inherit it. No generic RFC 7396 engine or unrelated
runtime schema rewrite is needed.

Resolve cooperative mode in order: request `coopSync`, account
`default_coop_sync`, host default. Resolve the selection limit from request,
then account default, then unlimited; a nonpositive request/default limit
means unlimited, as in the pinned baseline. A positive account `max_limit`
caps that result. Hard resource ceilings still apply. Preserve integers
without lossy numeric casts. Invalid JSON types remain request errors.
At the `/cookie_sync` body-parse stage, require a JSON object root: arrays,
scalars and null are `400` before account, cookie or privacy processing.
Framing, body-byte and JSON-node limits still take precedence. This does not
change deferred validation of the object's legacy GDPR field in section 5.

Validate all new baked fields during generation and runtime overrides in
the existing loader/push path. Validate the effective merged host and
account settings without cloning large inherited structures for every
account. An invalid runtime blob fails closed through existing loader
errors; it cannot fall back to permissive privacy. A disabled account maps
to the master sync-specific `400`, not the auction's `503`.

Retain account selection provenance separately from a diagnostic resolved ID:
`Default` for a missing/empty account and `Named(id)` for a nonempty supplied
account. `Default` never looks up an `accounts.unknown` entry. Literal
`Named("unknown")` does, and an unconfigured `Named(id)` retains its original
ID while falling back to baked/default policy. Reuse or factor the existing
typed account merge helper; do not synthesize an auction JSON body to select
an account. `/cookie_sync` and `/setuid` use the same selection rules.

`external_url` must be explicitly configured as an absolute HTTPS base URL,
with optional base path, no credentials, query, or fragment. Committed demo
defaults may use an explicitly documented example origin, never inferred
`Host` or forwarded headers. Operators must set their real public callback
origin before deployment. A missing/invalid effective origin must not cause
sync URLs to be guessed. Follow section 8 for the local-test feature.

### 4.2 Identity and Eligibility

Build a request-scoped view from the compiled bidder registry, validated
host aliases, and runtime settings. Do not rebuild adapter implementations
or maintain another registry. Distinguish:

1. Requested bidder name: retained for response attribution.
2. Normalized bidder/host-alias name: ASCII lowercase for lookup and filters.
3. Base bidder identity: owns implementation and GVL metadata.
4. Syncer family key: shared cookie slot and per-request sync deduplication key.

There are no request-defined aliases or alias GVL IDs on these endpoints.
Host aliases resolve through the same validated base mapping as auctions.
An invalid or disabled-base alias cannot fall back to a colliding base name.
Use the base vendor ID for privacy and the requested normalized name for
component activity matching. A shared family does not grant permission to
all of its bidders.

Validate the effective namespace after baked/runtime alias merging, before
building the sync view. Reject distinct alias keys with the same ASCII-folded
name, even when they reference the same base. If an alias folds to a base
bidder name, allow the existing explicit exact-spelling alias override only;
a differently cased shadow is invalid configuration. An exact alias override
has precedence over that base name, consistently with the auction alias
resolver. Validate case-fold collisions in bidder-override keys as well.
Never resolve a collision by insertion order, map iteration, or last write.
Apply the same checks to generation, push validation, and runtime load.

Baked bidder/syncer disablement cannot be undone by a push. Runtime bidder
disablement suppresses syncing as well as bidding. `usersync_enabled=false`
can independently disable syncing; sampling and auction timeout do not
change sync eligibility. Preserve lookup information for disabled entries
so `/setuid` can return the documented disabled-syncer error.
For an alias, both the base and alias controls must permit the operation:
base `enabled=false` or `usersync_enabled=false` is an unconditional veto.
Alias `true` cannot override a base veto; alias `false` only narrows access.

Candidate availability contains configured syncers in deterministic name
order before seeded shuffling. For shared families, multiple definitions
may retain different bidder endpoints, but incompatible declared default
callback behavior must be resolved through the bidder identity, never an
arbitrary family-to-bidder reverse lookup.

## 5. Privacy API and Policy

### 5.1 Replace the Stub

Replace `gvl_marker`, `syncer_config_marker`, back-compat signal aliases,
and boxed allow-all closures. Use a concrete context with named methods
and closed result enums. The function/type names may evolve during planning;
the responsibilities and ordering below are the contract.

The input contains bounded wire privacy fields, endpoint kind, resolved
account/host configuration, a `GvlStore` reference, a `Gcl` reference, and
one `now: OffsetDateTime` sampled for the request. The syncer argument is a
typed identity/config view, not a string marker. No synthetic `BidRequest`
is constructed to call the auction resolver.

Separate these outcomes:

- Request extraction error: invalid SID/GPP, unresolved malformed legacy
  GDPR, missing required consent, or a resource limit. Handler maps to `400`.
- Host-cookie permission: allowed, denied, or malformed nonempty TCF.
  Preserve the third state until the endpoint-specific permission stage.
- Candidate decision: allowed, activity/privacy denied, disabled syncer,
  or regulation-scope skip. These are not request-level errors.

Expose `cookie_sync_host_permission`, `cookie_sync_candidate_permission`,
and `setuid_permission` as distinct operations. `/setuid` never calls the
chooser's bidder permission ladder. An enum/`Result` must carry the deferred
malformed-TCF state; a boolean alone loses the required `400` versus `451`
distinction. Keep raw consent and UID data out of `Debug`/error formatting.

### 5.2 Signals and GPP

Reuse the independently testable M4 envelope parser and underlying TCF/USP
parsers. Keep source selection and endpoint error policy in a separate sync
normalizer. Auction normalization cannot be reused wholesale.

- Preserve the decoded wire string `gpp_sid` byte-for-byte for `SkipWhen`
  matching and macro output. Do not sort, deduplicate, or stringify it again.
- Parse a separate numeric SID view. Proposed Auctioneer domain: ASCII decimal
  positive IDs `1..=65535`, comma-separated, plus the standalone literal
  `-1` meaning no section applies. Empty/absent means unspecified applicability,
  not `-1`. The sentinel belongs to the applicability signal, never to the
  envelope's positive section-ID set; the M4 envelope parser is unchanged.
  This distinction follows the [IAB applicability contract](https://github.com/InteractiveAdvertisingBureau/Global-Privacy-Platform/blob/main/implementation.md#34-applicable-sections).
  Reject zero, other negative values, `-01`, mixed/repeated sentinel lists
  such as `-1,2` or `-1,-1`, empty components, plus signs, and whitespace.
  Positive IDs may have leading zeroes or duplicates; retain their raw text.
  Bound entries to 64 before allocation. This deliberately widens positive
  IDs beyond the reference helper's signed-eight-bit range while accepting
  its standard no-applicable-section sentinel; rejecting mixed sentinel
  lists is an explicit Auctioneer validation rule.
- Wire GDPR signal: SID containing 2 means Yes; standalone `-1` or any other
  valid nonempty SID list means No; otherwise use legacy `gdpr`. `-1` overrides
  legacy GDPR, including an invalid legacy value, and activates no GPP section.
  Preserve `-1` in macro output and exact-string `SkipWhen` matching. Defer
  malformed legacy rejection until this precedence is resolved. A sentinel
  does not bypass malformed-envelope/resource checks or activity controls.
- Legacy wire coercion remains endpoint-specific: `/cookie_sync`'s `gdpr`
  field accepts JSON numbers 0/1, with absent/null ambiguous; other field
  shapes are deferred invalid values. `/setuid` accepts query values `0`/`1`, with absent or exactly
  empty (`gdpr=`) ambiguous. Other nonempty query values, including whitespace,
  are deferred invalid values. Empty query acceptance does not permit the
  JSON string `""` as a legacy grant or a valid JSON GDPR signal.
- Keep `parsed_request_signal` separate from `normalized_request_signal`.
  The latter substitutes the raw host default on Ambiguous, never geography.
  Configuration enablement is a separate value, not a rewritten signal.
- TCF selection follows the master's active-section rule: section 2 is
  selected only when SID contains 2. Absent section falls back to legacy;
  an active present empty/invalid value does not reactivate a legacy grant.
- USP selection uses a nonempty section 6 only when SID contains 6; otherwise
  legacy USP. Validate the selected value; malformed USP is cleared without
  a hard error and without falling back from an invalid selected value.
- Nonempty malformed/unsupported GPP envelopes are hard `400`, even when
  legacy consent grants or enforcement is disabled. Envelope limits are
  terminal `400`, not malformed-cookie-style fallback.
- Unknown envelope sections remain opaque, as in the M4 envelope design.
  M5 adds no US-state decoders. Structural validity is not a claim that every
  section payload is semantically valid or enforced.
- GPP versus legacy value conflicts use the selected GPP value. They do not
  expose raw consent in diagnostics or broaden permissions.

These are deliberate Auctioneer selection rules, not claims that both pinned
Upstream endpoints share this exact selection algorithm. Section 13 records
the known reference differences and requires separate parity/divergence tests.

Use the shared effective GDPR configuration, trusted web channel,
purpose/vendor/restriction logic, consent-selected GVL version, and GCL
authenticity checks. Never use auction `PassID` as a substitute for sync
permission. Define `gdpr_enforcement_active = host.gdpr.enabled &&
effective_web_channel_enabled && normalized_request_signal == Yes`.
The channel/TCF enablement resolution is the existing account-over-host
tri-state ladder. An inactive path skips TCF permission parsing, but never
skips the request-extraction checks or activity evaluation.

When active, host permission evaluates **Purpose 1 only**, using the host
vendor ID and no bidder-name vendor exception. Candidate permission evaluates
Purpose 1 using the base bidder's vendor ID and base-name configuration,
including applicable vendor exceptions. A missing, zero, absent-from-GVL,
or deleted bidder vendor ID denies candidate permission even if the host
vendor is zero. Host ID zero is the explicit host-only bypass below.

For nonzero vendors, first establish nonempty syntactically valid consent,
GCL authenticity, and the required GVL/vendor. Failure is denial (or the
deferred syntax error), not an unenforced-purpose grant. Then honor the
master's non-overridable effective publisher `NotAllowed` restriction,
including conflicting consent/LI restrictions for the same purpose/vendor,
before every disabled-enforcement or treatment bypass. Duplicate same-type
records are not a conflict. If Purpose 1
enforcement is disabled, allow. Otherwise, when the TC string has
PurposeOneTreatment and effective treatment `enabled` is true, use effective
`access_allowed`; neither bit alone grants permission. Otherwise evaluate
Purpose 1 with the effective full/basic algorithm and vendor enforcement
settings. Never enable the Purpose 2 LI-transparency override here.

Treatment settings are resolved per field: account override, then host, then
the reference true/true defaults. They are new M5-owned configuration because
auctions do not evaluate Purpose 1. Test false/true/absent combinations and
publisher restriction precedence. Factoring shared consent state and purpose
helpers is permitted; creating a synthetic auction request is not.

The master's non-standard-publisher auction exemption is not an automatic
sync exemption. A required unavailable GVL, unusable CMP, or stale GCL denies
permission; it does not fetch on the request path or permit by default.
Syntax errors and unusable authenticated state remain distinguishable
internally. This preserves Auctioneer's existing restriction-first hardening
rather than claiming literal parity with every upstream early return.

Host vendor ID zero bypasses only the host-company GDPR permission check.
It does not bypass candidate GDPR, activity, disabled-syncer, resource, or
request-extraction checks. Activity evaluation reuses M4's matching and
first-match rules without broadening purpose permissions.

### 5.3 Endpoint Decision Matrix

| Condition                                                | `/cookie_sync`                          | `/setuid`                                         |
| -------------------------------------------------------- | --------------------------------------- | ------------------------------------------------- |
| Parsed GDPR Yes, selected consent empty                  | `400`                                   | `400`                                             |
| Parsed Ambiguous, host default Yes, consent empty        | `400`                                   | Defer to host permission                          |
| Invalid GPP/SID or unresolved invalid legacy GDPR        | `400` before activity                   | `400` before activity                             |
| Malformed nonempty TCF, GDPR permission path inactive    | Ignore for permission                   | Ignore for permission                             |
| Malformed nonempty TCF, active path, host vendor zero    | Host allowed, candidates privacy-denied | Host allowed, subject to activity/disabled checks |
| Malformed nonempty TCF, active path, nonzero host vendor | `200`, no selected syncers              | After activity allows, permission-path `400`      |
| Activity denies and TCF is malformed nonempty            | Candidate denied                        | `451`, empty body; wins over deferred TCF error   |
| Usable consent denies host storage                       | `200`, no selected syncers              | `451`, documented denial body                     |
| Host permission allows, bidder GDPR denies               | Candidate denied                        | Not a bidder-permission gate                      |
| Matching `SkipWhen.gpp_sid`                              | Candidate skipped                       | Not consulted                                     |
| Syncer disabled                                          | Candidate disabled                      | `400`, no cookie write                            |

`/cookie_sync` response `status` still reflects live cookie entries, not
privacy permission, even when `bidder_status` is empty. A decoded cookie's
own opt-out flag is enforced as well as the separate host opt-out cookie.

USP is parsed and propagated on `/cookie_sync`; `/setuid` has no legacy
`us_privacy` parameter. Preserve the master's candidate ladder, which does
not add a CCPA sale-opt-out denial stage. The pinned chooser defines a CCPA
callback but does not invoke it. Do not claim that carrying USP alone is a
new per-syncer CCPA denial policy; a policy change requires an explicit
design change and corresponding endpoint tests.

## 6. UID Cookie Model

Use a typed cookie model with a deterministic family-key map, expiration
timestamps, and opt-out state. Clock time is an input, never a direct system
call inside cookie algorithms.

The inner payload format is padded URL-safe base64 of JSON with `tempUIDs` entries
`{ "uid": "...", "expires": "<RFC3339>" }` and optional `optout`.
Emit omission of empty `tempUIDs` and false `optout` in the same manner as
the reference encoder. The pure payload codec records corrupt input as fresh
state for reference qualification; production readers must first authenticate
the `v1.key-id.payload.mac` envelope and reject both invalid MACs and corrupt
payloads with 400. There is no unsigned fallback. Missing or invalid secret
keyrings return a fixed 503 at the existing UID-read stage; earlier denials
retain their order. The [integrity addendum](2026-10-13-usersync-cookie-integrity-design.md)
owns rotation, one bounded secret read, deletion and trust semantics.
Resource-limit failures are not silently ignored.
Do not accept alternative encodings by guessing or double-decode values.

Retain structurally valid expired entries on decode. A live entry requires
`now < expires`; equality is expired. Candidate suppression and response
`status` consult live entries. `/getuids` returns stored entries, including
expired ones, matching the reference accessor, not an undocumented active-only
view. A malformed timestamp makes the cookie invalid. Duplicate JSON family
keys use the parser's documented last-value-wins behavior; input bounds apply
to all visited nodes, not only surviving map entries.
Validate every encountered timestamp before an overwritten entry is
discarded. Cookie and entry field names are matched case-insensitively.
Repeated non-null `tempUIDs` maps merge; `tempUIDs: null` resets that map.
Duplicate scalar entry fields use the final value; `uid: null` clears the
UID, while null `expires` and `optout` leave the preceding value intact.
These are explicit codec
semantics, not consequences of a general-purpose map deserializer.
Encode expiration from its parsed timestamp, normalizing redundant
fractional zeros and zero-offset spelling without losing actual precision
or a nonzero offset. Equivalent timestamp text must not change eviction.

Host opt-out wins before reading `uids`. Cookie `optout=true` drops all IDs
and forbids writes and host identity merging. Otherwise import the configured
host identity cookie only into an absent or empty-UID family slot. A nonempty
existing value is retained even if expired, matching the reference fill rule.
Imported and newly synced values receive 14-day entry expiry. An empty callback
UID removes that family; it does not store an empty UID. Preserve the baseline
`audienceNetwork` UID `0` special case when that family is represented.

Use a standard HTTP cookie parser/formatter available in the dependency
graph, or add a portable focused dependency after verifying its behavior.
Do not build headers by concatenating unchecked cookie or query values.
Inspect all Cookie header entries in wire order; first matching cookie name
wins. Tests pin repeated-header and duplicate-name behavior on each adapter.

`Set-Cookie`: name `uids`, `Path=/`, expiry `now + ttl_days`, optional
configured Domain, no HttpOnly. Retain the master's UA compatibility rule:
Chrome/CriOS major >= 67 adds Secure and SameSite=None; missing, invalid, or
other UAs omit both. This is baseline behavior, not a promise that every
modern browser will accept third-party cookies. Browser rejection is distinct
from a server cookie serialization or permission failure.

### Eviction and Atomic Writes

Resolve priority groups once for selection and eviction. Map bidder names
to family keys, discard unknown names from the priority projection, and
assign a shared family its highest declared priority. Work on a request-local
copy; never mutate configuration lists while ejecting.

Measure the exact complete serialized cookie field value, including name,
base64 padding, version/key-ID/MAC overhead, Expires, Path, Domain, Secure,
SameSite and deletion's Max-Age. Do not measure
only JSON or the base64 payload. For a positive configured cap:

1. If the complete cookie fits, write it.
2. If a sole remaining entry cannot fit, return `400`, no Set-Cookie.
3. If priority entries exist and only one non-priority entry remains while
   the incoming family is non-priority, return the documented `200` warning
   and do not write a partial cookie.
4. Otherwise evict the oldest non-priority entry, or the oldest entry in
   the lowest populated priority group. Repeat with strictly fewer entries.

Use earliest expiration as oldest; break ties lexically by family key.
Skip empty/nonresident priority groups, and never compare bidder names
directly to cookie keys. Empty-cookie clearing must still emit a bounded
valid cookie, and attribute overhead alone exceeding the cap is a `400`.
Prepare the entire response before attaching Set-Cookie so serialization,
header-cap, or permission errors cannot commit a cookie accidentally.

## 7. Candidate Selection

Implement the master's ordered chooser with an injected seeded RNG:

- Nonempty requested list, cooperative off: shuffle requested only.
- Nonempty requested list, cooperative on: requested, then each priority
  group, then available, independently shuffled within each group.
- Empty requested list, cooperative on: priority groups, then available.
- Empty requested list, cooperative off: available only.

Deduplicate normalized bidder names. For each remaining candidate, evaluate
in this order: known bidder, configured syncer, unused family, allowed sync
type, no live cookie entry, activity, GDPR bidder permission, syncer enabled,
regulation skip. Record a family's first encounter before filters/privacy,
as the reference chooser does; an alias rejected by a later check still
prevents retrying that family under a different alias in the same request.
Only selected candidates count toward the logical limit.

`filterSettings` uses the `image` key for redirect syncs, not `redirect`.
Accept include/exclude and either `"*"` or a string array as in Appendix B.
Match names case-insensitively using the chosen ASCII normalization. Missing
filters permit available types; select iframe before redirect when both
are permitted and configured, matching the reference default.

`SkipWhen.gdpr` consults the parsed wire scope, not an auction geo signal or
the enforcement enable flag. `SkipWhen.gpp_sid` compares the original decoded
string exactly: `"2,6"`, `"6,2"`, and `"02,6"` are distinct. A skipped
candidate does not trigger a request-level error.

Successful responses preserve the original requested spelling for explicit
candidates and the catalog spelling for added candidates. Rejected candidates
appear only in requested debug output, not as synthetic failed sync URLs.
Build debug rows lazily and within the response budget.

## 8. URL and Callback Construction

Sync URLs are instructions for the browser, not outbound server requests.
The flow is browser -> bidder sync endpoint -> Auctioneer `/setuid` callback.
M5 does not need another EdgeZero outbound API, nor does URL validation prove
the bidder's browser redirect or third-party cookie acceptance works.

Use a small closed template representation, not a general expression/template
engine. Validate and compile static bidder templates during generation;
validate effective host callback templates whenever config is resolved.
Allowed tokens follow master section 8.6. Reject unknown/unclosed tokens,
credentials, fragments, authority substitution, unsafe schemes, private or
loopback literals outside the test feature, and output/header controls.
Request `Host`, Origin, forwarded headers, bidder names, and consent values
cannot change the configured authority.

There are two runtime phases:

1. Assemble the callback from trusted external origin, normalized callback
   bidder identity, sync type, and the bidder's literal `user_macro` token.
   Preserve privacy tokens for the next phase. Embed the callback into
   `RedirectURL` using the reference escaping rules.
2. Resolve privacy tokens from the source-selected values defined below. Encode
   each value for its actual nesting depth; never rescan inserted request
   data as template syntax. The browser/bidder later replaces the literal
   bidder UID macro with a UID. Auctioneer never invents or resolves that UID.

| Privacy token | Value before nesting-aware encoding                                                            |
| ------------- | ---------------------------------------------------------------------------------------------- |
| `GDPR`        | `parsed_request_signal` after SID precedence: Yes -> `1`, No -> `0`, Ambiguous -> empty string |
| `GDPRConsent` | Selected TCF value from section 5.2, or empty if absent                                        |
| `USPrivacy`   | Selected validated USP value from section 5.2, or empty if absent/cleared                      |
| `GPP`         | Original validated envelope string, or empty if absent                                         |
| `GPPSID`      | Original decoded SID string, including standalone `-1`, or empty if absent                     |

Never serialize `normalized_request_signal` or enforcement enablement into
`GDPR`: the host default is local permission policy, not a caller-supplied
scope assertion. An ambiguous request therefore emits `gdpr=` in a callback
that includes the macro, regardless of host default. `/setuid` treats that
empty value as ambiguous and applies its current host default only for
permission. Test both defaults; use usable consent when default Yes requires
it. Explicit 0/1 and SID-derived scope must survive the same round trip.

In a callback, `SyncerKey` retains its historical token name but substitutes
the callback bidder identity, not the cookie family. AppNexus can therefore
call back as `bidder=appnexus` while storing under family `adnxs`. Resolve
family only after `/setuid` validates bidder identity. Do not emit
`bidder=adnxs` unless it is an actual configured bidder or host alias.
Custom callback templates must contain exactly one decoded `bidder` query
key, bound to `SyncerKey` or `BidderName`. Missing, hard-coded or duplicated
bidder keys are configuration errors, including percent-encoded duplicates.
The external callback base must be an explicit canonical HTTP(S) authority
with no surrounding whitespace or parser-repaired scheme spelling; optional
base paths remain supported. Validation and rendering must use the same
authority interpretation.

`UserMacro` substitutes trusted bidder-specific syntax such as `$UID`; do
not leave a literal `{{.UserMacro}}` in the emitted URL. Reference fixtures
must demonstrate that the bidder can replace the token after the prescribed
URL decoding and that a UID containing `&`, `+`, `%`, `=`, quotes, or Unicode
arrives as one value, not as injected query parameters.

An empty or absent `user_macro` means the bidder uses an append-only UID
callback, as PubMatic iframe syncing does. In this mode the final decoded
callback must end exactly in `&uid=` (or `?uid=` if there are no prior query
parameters). The bidder appends its encoded UID to that empty final value.
Insert all other parameters, including account and privacy, before `uid`.
Reject custom templates that omit or duplicate `uid`, prefill it in this
mode, or put parameters/path/fragment data after it. Token-replacement and
append-only modes each have real-metadata round-trip fixtures; testing only
an AppNexus-style replacement token is insufficient.

Account selection must survive the round trip. For `Default`, omit the
callback account parameter; never emit a synthesized `account=unknown`.
For `Named(id)`, propagate exactly that original nonempty ID as one encoded
parameter, including literal `unknown` and IDs not currently configured.
Insert it before a terminal append-only `uid=` and before embedding the
callback. Reject callback templates containing their own `account` parameter
rather than permitting duplicates or a stale hard-coded account. Each
request reads current configuration, so the guarantee is preserved selection
identity, not a snapshot locked across the browser round trip. Do not trust
the account parameter as authentication; it selects the same public account
policy model as the original request.

Also force PubMatic's redirect mode in reference fixtures: its literal
`#PMUID` token must survive query-value/nested callback encoding without
becoming a URL fragment. Simulated substitution must recover the exact UID
and account and preserve the image callback format `f=i`; iframe-only
PubMatic coverage does not exercise this branch.

The `insecure-bidders` feature permits only the established literal localhost
authorities and arbitrary local ports. Apply the same feature-controlled rule
to the host callback origin and bidder sync URLs. No runtime flag enables it.
Resolved URLs must satisfy the same authority checks as their validated
templates, without re-encoding the whole URL and destroying bidder macros.

## 9. HTTP Pipelines and Browser Access

All handlers use one validated runtime config snapshot, one sampled clock
instant, and the request's existing admission/response lifetime. Preserve
master semantic check order after framework ingress acceptance:

| Endpoint       | Ordered application stages                                                                                                                                                                                                                                                    |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `/cookie_sync` | Bounded body; wire parse; effective host; host opt-out; account; UID decode/host merge and cookie opt-out; privacy extraction; host permission; filters/limit; chooser; URLs; response                                                                                        |
| `/setuid`      | Bounded structural query parse; effective host; host opt-out; bidder validation; format validation; account; syncer resolution; UID decode/host merge and cookie opt-out; privacy extraction; activity; deferred host permission/disabled check; cookie transaction; response |
| `/getuids`     | Effective host; host opt-out; UID decode/host merge and cookie opt-out; read-only response                                                                                                                                                                                    |

Config-store extraction may need to precede body consumption because it
borrows `RequestContext`. Do not claim that body parse errors override a
framework/config-extraction failure that has already prevented the handler
from running. Pin these infrastructure outcomes separately from the master's
application-level host/account ordering. No second config read is allowed.

On an admitted `/cookie_sync` request, the handler's bounded-body stage
first rejects any nonempty `Content-Encoding`, then rejects a declared
`Content-Length` over 32768 bytes, both before polling the body. Preserve
Appendix F's exact 400 responses and encoding-before-size precedence.
Check all encoding field values, not only a first empty value. Framing
ambiguity and invalid/duplicate Content-Length remain EdgeZero's earlier
ingress responsibility. The aux route class and manifest body-cap mirror
do not implement these application checks automatically. Even when declared
length is absent or acceptable, enforce the independent streaming cap.
Test zero body polls for both pre-read failures through actual App dispatch;
after successful config resolution they receive the same CORS/no-store
decoration as other application errors.

Parse query parameters with a structured URL form parser and reject malformed
percent encodings/invalid UTF-8. Decode exactly once. First occurrence of a
repeated parameter wins; enforce total bytes and parameter count before
materialization. `/setuid` accepts `f=i` or `f=b` case-insensitively; absence
uses the syncer's configured default when present, otherwise blank for an
iframe-capable syncer or image for a redirect-only syncer. Invalid/empty
explicit format is `400`.

Keep endpoint wire details explicit:

- `/cookie_sync`: `application/json; charset=utf-8`, trailing newline,
  `bidder_status: []` when none selected, status based on live UID entries.
  No HTML escaping of URL `&`, `<`, `>`; normal JSON string escaping still
  applies. `debug` is the bounded sync chooser diagnostic, not auction
  `ext.debug`; it is present only when requested and nonempty.
- `/setuid`: image returns the pinned 86-byte 1x1 PNG from upstream's
  `util/httputil/pixel.png`, with `Content-Type: image/png` and
  `Content-Length: 86`; Appendix F contains its exact bytes and digest.
  Blank returns empty `text/html`; opt-out/activity denial has an empty body
  where the master requires it. No Set-Cookie on any failed permission,
  invalid input, serialization refusal, or eviction-warning response.
- `/getuids`: `text/plain; charset=utf-8`, JSON plus newline, omit empty
  `buyeruids` so empty/opted-out output is `{}` plus newline. No writes.

Appendix F owns fixed status/body/header mappings. Use endpoint-specific
response builders rather than a universal error helper that adds a newline
or Content-Type to every error. Rust parser-generated diagnostic suffixes
are fixed safe messages, not reference implementation details or echoed input.
An error fixture must specify status, headers, body, newline, and cookie
presence, including combinations of simultaneous invalid conditions.

### Credentialed Browser Requests

Add validated exact-origin `user_sync.allowed_origins`; default empty permits
same-origin use but grants no cross-origin JavaScript access. No wildcard
with credentials, suffix matching, or arbitrary Origin reflection. Origins
have scheme/host/port only; reject `null`, credentials, paths, and controls.

After successful config resolution, for an allowed request origin return
that exact origin with `Access-Control-Allow-Credentials: true` and merge
`Vary: Origin` on application success and error responses. Decorate bounded
application responses inside the shared handler path using that same
snapshot, preserving their endpoint-specific status, body, and headers.
Do not let an ordinary parsing, permission, or serialization rejection
escape as an undecorated framework error after config has been resolved.
Add bounded `OPTIONS /cookie_sync` preflight handling
for POST and Content-Type: allowlisted origin/method/header combination
returns `204` with allow headers; other preflights return `403`. No wildcard
request-header reflection. OPTIONS is an additional route-table entry, not
a fourth user-sync feature endpoint. Unsupported actual origins receive no
CORS permission and do not bypass normal privacy processing.

The CORS guarantee starts only after a validated snapshot is available;
it does not extend to every framework-generated response:

| Response source                                                                           | CORS policy                                                                       | Cache policy                         |
| ----------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ------------------------------------ |
| Application success/error or preflight after valid config                                 | Allowlist from the one request snapshot; a denied preflight gets no allow headers | `no-store`                           |
| Config extraction/validation failure                                                      | No CORS allow headers; preserve the existing config error status/body             | `no-store`                           |
| Admission refusal, ingress/routing failure, escaped framework error, or terminal fallback | No CORS allow headers; do not load config or reflect Origin to decorate it        | `no-store` on Auctioneer-owned responses |
| Transport abort or provider-generated response outside Auctioneer                             | No response-header guarantee                                                      | No response-header guarantee         |

EdgeZero's synchronous error renderer receives an `EdgeError`, not the
request Origin or a runtime config snapshot; admission refusals bypass
handler middleware altogether. Preserve these boundaries. An early 503
may therefore be visible to cross-origin JavaScript only as a network/CORS
failure. Do not add pre-admission config I/O, a global last-good allowlist,
or a provider-specific router to make that failure readable. Missing,
malformed, duplicate, or unapproved Origin headers grant no CORS permission.

No CORS headers are needed to make an image or iframe callback load; do not
require Origin on `/setuid`. CORS is not consent, account authentication, or
CSRF protection. A cross-origin JavaScript `/getuids` read needs the same
allowlist/credentials treatment. Capture browser-shaped preflight, POST,
redirect/callback, and readback flows in integration tests.

All Auctioneer-owned sync responses, including errors and Set-Cookie responses,
carry `Cache-Control: no-store`. Extend the existing bounded error renderer
and admission/fallback response builders as well as sync response builders;
a handler-only decorator cannot cover early failures. The fixed no-store
header may be applied to all Auctioneer framework errors, since those hooks
need not have a matched sync route. Charge the extra header against the
static refusal/terminal response bounds. Do not cache personalized sync
URLs or UID maps.
No raw consent, UID, Cookie/Set-Cookie, query string, or resolved sync URL
goes into logs, metric labels, or debug errors. Requested bidder attribution
is bounded request data, never a dynamic metric label. Fixed diagnostic
categories and registered bidder names are sufficient for operational tests.

## 10. Bounds and Resource Integration

Register the sync routes in the existing aux admission class, not the
auction fanout pool and not the isolated liveness probe class. Update the
aux peak derivation to cover the new work before route activation. The aux
permit stays owned by response completion through terminal egress; handlers
must not acquire a second permit or release it after body construction.

| Surface                        | Contract                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `/cookie_sync` body            | Existing 32 KiB route cap, bounded read and JSON nodes; no compressed inbound body                                                                                                                                                                                                                                                                         |
| GET query                      | At most 32 KiB encoded query, at most 32 parameters; count duplicates and unknown fields                                                                                                                                                                                                                                                                   |
| Cookie input                   | Existing admitted header-byte/count limits, checked again before handler-owned copies; decoded bytes and JSON nodes bounded before allocation                                                                                                                                                                                                              |
| Requested bidders/filter names | At most 256 entries per list and 128 UTF-8 bytes per name; count before deduplication                                                                                                                                                                                                                                                                      |
| Available syncers/priorities   | At most 256 catalog names, 32 groups, and 256 total priority entries; overflow rejected as invalid config                                                                                                                                                                                                                                                  |
| Runtime host aliases           | Inherited host-overlay bound: at most 64 entries and 64 UTF-8 bytes per alias; baked catalog names retain the 128-byte sync-name bound                                                                                                                                                                                                                     |
| Emitted sync URL               | At most 32 KiB per URL, checked during each encoding/substitution stage                                                                                                                                                                                                                                                                                    |
| Response body                  | At most 256 KiB total, using a bounded serializer including debug rows and the trailing newline                                                                                                                                                                                                                                                            |
| Response headers               | At most 16 application-owned field values, including duplicates, CORS, Vary and Set-Cookie, checked after browser-policy decoration. At most 16 KiB serialized header bytes; `/setuid` checks non-cookie header bytes independently of the complete signed cookie field. Handler caps derive from the same runtime constants used by auxiliary accounting. |
| Set-Cookie                     | At most 4 KiB complete signed field value, including version/key ID/MAC and attributes, independently of the logical eviction threshold                                                                                                                                                                                                                    |
| Privacy parsing                | M4 parser ceilings, further bounded by the endpoint's input budgets; no recoverable limit fallback                                                                                                                                                                                                                                                         |
| Templates/config strings       | At most 8 KiB per template, 2 KiB external URL; config-owned arrays/strings participate in config storage/node accounting                                                                                                                                                                                                                                  |

Response header limits apply only to the application-owned map after browser
policy. Fields added later by an adapter/provider, such as framing or server
metadata, are outside these count/byte checks; these are not complete wire-level
header limits. Accounting reserves the larger of the ingress profile allowance
and the fixed sync header constants. A profile reserving 128 fields therefore
does not raise application enforcement above 16. The same distinction applies
to the header-byte allowance. No universal provider-added field count is assumed.

These are proposed M5 safety ceilings, not baseline interoperability claims.
Reject request-derived amplification with `400` and a fixed resource-limit
message; reject invalid operator configuration through the existing config
error path. A whole-request response limit does not silently emit a partial
candidate list or a partial cookie. Cooperative logical unlimited never
means unbounded storage or work. A future larger bidder catalog must revisit
the catalog/resource ceilings explicitly rather than silently overflowing.

The pre-M5 native profile's request-target limit was smaller than the proposed
query allowance. M5 updates the target/parser profile coherently to admit
33,792 bytes of request target, with a 51,200-byte admitted H1 head floor below
the SDK's enforced 65,536-byte cap. Parser storage and the whole memory
envelope are recalculated and their limits tested. Do not increase only a handler limit
that ingress can never admit. Where a provider imposes a smaller immutable
limit, document the effective limit and test safe refusal; do not claim every
256 KiB GPP string can fit in a GET callback. No provider limit is silently
reinterpreted as consent denial.

Derive the aux work bound from simultaneously live config state, raw body or
query, decoded cookie, typed nodes, selected privacy data, retained parsed GVL
handles, chooser state, intermediate escaped URLs, and response/header output.
Account for parsed vendor lists retained by aux requests as well as auctions;
the global LRU size alone does not bound in-flight handles. Charge strings
after worst-case percent/base64/JSON expansion and use checked arithmetic.
Either reuse the neutral reservation primitive or generalize its auction
name when genuinely shared; do not charge sync allocations to auction permits.

Aux config extraction, GVL loading, parsing, serialization, cancellation, and
all early errors must fit the derived work reservation. Keep existing
`with_aux_egress` and bounded framework error handling, extending only the
parts needed for the new bodies/headers. Add a deadline check around bounded
CPU stages; the provider's documented egress guarantee remains unchanged.
This is application resource integration, not a new provider certification
project and not a physical-OOM proof.

## 11. Acceptance Tests

Use behavior names, not milestone names, for new files and tests. Fixtures
use typed serialization or `serde_json::json!`; literal raw JSON is reserved
for malformed syntax, duplicate-key, and byte-exact wire cases.

| Area              | Required evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Configuration     | Baked generation, runtime push/load parity, missing/present/false/zero/empty semantics, missing versus literal `unknown` account and unconfigured named account, invalid names/domains/templates, one snapshot within each request across concurrent config change                                                                                                                                                                                                               |
| Catalog           | Known-without-sync, aliases, colliding disabled aliases, case-fold collision rejection and exact alias precedence, shared family with different vendor IDs, base sync disable cannot be overridden by alias, no auction-sampling influence                                                                                                                                                                                                                                       |
| Cookies           | Padded base64/RFC3339 fixtures, corrupt and oversized input, duplicate headers/keys, expiry equality, host fill rules, both opt-out sources, empty UID deletion, Chrome/CriOS attributes                                                                                                                                                                                                                                                                                         |
| Cookie integrity  | Independent HMAC vector, tampered/unsigned rejection on every reader, unknown/retired keys, rotation, lazy bounded secret read, deadline/cancellation, 4-KiB complete writes and real-browser deletion/readback; no callback UID ownership claim                                                                                                                                                                                                                                 |
| Eviction          | Complete header measurement, account priority override including empty, ties, empty/nonresident groups, shared family priority, sole non-priority warning, oversize single entry, no partial writes                                                                                                                                                                                                                                                                              |
| Selection         | Four cooperative/requested combinations, seeded ordering, dedup before later denial, filters/type preference, limit counts successes only, unknown/disabled/already-synced status                                                                                                                                                                                                                                                                                                |
| Privacy           | Every row in section 5.3; active/absent/empty GPP sections; standalone `-1` vs missing/empty SID, `-1` overriding invalid legacy GDPR, inactive embedded sections and exact raw SID matching, zero/other negative/mixed/repeated sentinel rejection; empty query GDPR vs invalid JSON string; GVL version unavailable; deleted CMP/vendor; stale GCL; Purpose 1 and treatment config, full/basic/vendor exceptions and restrictions; host vendor zero does not bypass candidates |
| Templates         | Host vs family identity, real AppNexus-style UID macro, real PubMatic append-only iframe plus account propagation with terminal `uid=`, nested escaping, default/named account provenance, ambiguous GDPR round trips under both host defaults without emitting a defaulted signal, explicit/SID-derived GDPR and raw `-1` forwarding, hostile macro-looking input, no authority changes, exact secure/insecure feature pairing                                                  |
| Wire precedence   | Two simultaneous faults for every adjacent pipeline stage; safe parser diagnostics; exact headers/newlines, pinned PNG byte equality/digest, successful image decoding and `Content-Length: 86`, no-store and no unwanted Set-Cookie                                                                                                                                                                                                                                             |
| Browser access    | Allowed and denied/malformed/duplicate origins, credentialed preflight, Vary preservation, no Origin on callback, full cookie_sync -> simulated bidder substitution -> setuid -> getuids flow; post-config application errors retain CORS, config/overload/framework failures have no allow headers, all Auctioneer-owned responses have no-store                                                                                                                                    |
| Framework wiring  | Manifest/class/cap parity including OPTIONS, generated App on all adapters, one config read, existing aux wrapper, no adapter-specific router; actual dispatch covers early refusal and completion ownership                                                                                                                                                                                                                                                                     |
| Resource lifetime | Body/node/query/URL/response/header boundaries, nonempty Content-Encoding and declared-length prechecks with zero body polls plus independent streaming overflow, no repeated parse amplification, allocations under derived aux charge, cancellation and egress permit retained until completion                                                                                                                                                                                |
| Regression        | Auction behavior unchanged, M4 privacy/scrub suites unchanged, probe still works under aux saturation, every supported native/WASM build still compiles                                                                                                                                                                                                                                                                                                                          |

Commit small reference-derived fixtures rather than relying on a network
checkout at test time. Keep divergence fixtures separate from exact baseline
fixtures. Include the original 86-byte PNG asset with source provenance; do
not substitute a generated equivalent or assert only its signature/length.
Test at least the two real reference bidders and a local simulated
sync endpoint; Generic with no sync metadata must not be accidentally made
syncable. Tests must exercise actual registered handlers and emitted headers,
not only pure functions or an always-allow privacy fake.

Default and `insecure-bidders` selections, strict Clippy, formatting, the
existing target checks, and native handler integration are required. Cloud
compile checks are not live-browser or provider execution evidence. Record
any unexecuted provider/browser test honestly; do not mark it passed because
the adapter compiled.

## 12. Delivery Order

1. Configuration types, catalog view, validation, limits, and immutable
   reference fixtures. Update the stub API and all callers deliberately.
2. Pure cookie and eviction modules with fixed clock tests.
3. Sync normalization and concrete privacy permission API with decision-matrix
   tests, including deferred host-cookie errors.
4. Candidate selection and two-stage URL construction with seeded RNG and
   simulated bidder callback tests.
5. Aux resource integration, response builders, handlers, CORS, and route
   registration. Keep routes absent until real permission evaluation is wired.
6. Full integration/regression/portability validation and documentation updates.

The [implementation plan](../plans/2026-09-30-m5-usersync.md) turns these
into test-first tasks with explicit file ownership and acceptance commands.
This documentation task does not implement
handlers, activate routes, change CI branch filters, commit M4 fixes, or
push a PR. Review the spec and plan together before executing the plan.

## 13. Explicit Clarifications and Divergences

On approval, update the corresponding master sections alongside implementation
so there is one current contract rather than another layer of review-round
exceptions:

| Topic                   | M5 contract                                                                                                                                                                                                                     |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Internal compatibility  | Replace unfinished APIs and marker fields; no back-compat wrappers required                                                                                                                                                     |
| Host cookie name        | Separate identity source; `uids` remains the fixed Auctioneer cookie name                                                                                                                                                           |
| Cookie integrity        | M5 signs issued state; unsigned/invalid state is rejected, unavailable keys are 503, empty writes delete the cookie, and the complete field is capped at 4 KiB. Encryption, UID ownership and replay protection are not implied |
| GPP selection           | Follow the master's active-SID rules, not the pinned endpoint's presence-only section-2 selection or `/setuid` combined signal/consent fallback                                                                                 |
| GPP conflicts           | Selected GPP wins; unlike pinned cookie_sync's USP conflict error, a legacy conflict alone is not a hard rejection                                                                                                              |
| GPP SID domain          | Positive IDs through 65535 plus standalone `-1` applicability sentinel; missing/empty is distinct; zero, other negatives and mixed/repeated sentinel lists rejected; envelope IDs stay positive                                 |
| Unknown GPP payloads    | Structural envelope validation only; no implied US-state enforcement                                                                                                                                                            |
| GDPR macro and callback | Serialize the parsed wire signal, not host-defaulted scope; ambiguous emits empty and missing/empty query GDPR remains ambiguous; JSON string values remain invalid                                                             |
| Callback tokens         | `SyncerKey` means callback bidder identity; `UserMacro` becomes the bidder's literal UID token, never a literal unresolved Auctioneer tag                                                                                           |
| Append-only callbacks   | Empty UID macro requires terminal `uid=`; generated account/privacy parameters precede it                                                                                                                                       |
| Callback account        | Preserve default-versus-named selection and the supplied ID; never synthesize `account=unknown`                                                                                                                                 |
| Alias namespace         | Reject case-fold collisions; preserve explicit exact-name alias precedence and base disable veto                                                                                                                                |
| Eviction                | Measure all actually emitted attributes; lexical tie-break; skip empty priority groups; no reliance on unspecified map iteration or bidder/family coincidence                                                                   |
| Input diagnostics       | Safe fixed parser suffixes, not verbatim Rust errors that can echo identifiers/consent                                                                                                                                          |
| Image response          | Original pinned 86-byte PNG and `Content-Length: 86`; verify the source bytes and decode, not a hand-authored replacement                                                                                                       |
| Browser integration     | Snapshot-based CORS on application responses; pre-config/framework failures stay without allow headers; no-store on every Auctioneer-owned response; OPTIONS adds one aux route through the generated EdgeZero App                  |
| Resource ceilings       | Hard bounded parsing/output even when an operator's logical cookie or candidate limit is unlimited                                                                                                                              |
| Scope claims            | Sync endpoints complete does not mean full bidder catalog migration or automatic auction UID enrichment complete                                                                                                                |

Existing intentional differences remain: early host opt-out, account-presence
priority override, the shared priority source for selection and eviction,
request-extraction errors before `/setuid` activity, and disabled-syncer
rejection on `/setuid`. Do not reduce privacy scope or add auction-style
fallback to make a test pass.
