# Operator Runtime Configuration And Bootstrap

Date: 2026-11-02

Status: Draft for maintainer review. This document proposes contracts; the new
types, authoring layout and bootstrap paths below are not implemented.

Tracking: [configuration epic #124](https://github.com/IABTechLab/auctioneer/issues/124),
[contract task #125](https://github.com/IABTechLab/auctioneer/issues/125).

## 1. Outcome And Scope

For a fixed release, adapter and supported feature set, two operators must run
the same executable bytes with different validated configuration. Building,
linking or packaging the executable must not read an operator's policy file,
account roster, domains, deployment bindings or credentials.
Audit SDK macros/manifests and provider packaging as well as Auctioneer's
build script: an operator value must not re-enter compiled code through a
different generator. Deployment bindings/configuration may differ; the
released executable/component bytes for that adapter must not.

This contract covers compiled product defaults, startup/deployment settings,
reloadable policy, secrets, validation, ownership and the authoring cutover.
The [field inventory](2026-11-02-operator-runtime-configuration-inventory.md)
is part of the contract. It records current owners separately from proposed
destinations; a classification is not a claim that a runtime reader exists.

This work does not add bidders, a plugin loader, new auction features, a new
configuration framework, a separate embedding configuration system, or a
new workspace crate. It does not qualify a guest deployment or change the
resource equation, workload defaults, measured floors or provider ceilings.
The [resource-qualification epic](https://github.com/IABTechLab/auctioneer/issues/9)
retains those readiness outcomes.

## 2. Verified Starting Point

The source baseline is Auctioneer commit
`7d7fee7704e2d9552457068ac7de4861b6c7169e`. The adjacent capability-naming
cleanup changes descriptions and paths, not this configuration ownership.

- `auctioneer-config/build.rs` reads `auctioneer.toml`, validates the baked
  host/account tables, and emits the values exposed by `host_default()` and
  `account_default()`. It also compiles a separate `runtime-bootstrap.toml`.
- `AuctioneerConfigSource` currently requires both `baked` and `runtime`.
  The CLI parses the runtime subtree strictly but accepts opaque baked
  subtrees; the build does the converse. This is not one shared validator
  of the complete authoring file.
- `AuctioneerAppConfig` is an optional-field overlay. Runtime resolution
  still clones the compiled defaults, then applies host, account, bidder
  and feature overrides. Host fields have runtime setters, but most
  deployment-wide account defaults and both bootstrap fields remain compiled.
- The runtime owner already distinguishes missing configuration, corrupt
  configuration and transient backend errors. Missing configuration is
  refused by the checked-in bootstrap setting. `/status` has a separate,
  bounded degraded disposition; that is not permission to serve auctions.
- Config extraction uses SDK-owned byte/value limits and a deadline. Core
  checks the App's captured caps against installed boot funding before a
  store read. Apps constructed before boot do not magically acquire later
  extractor limits.
- Adapter profiles own provider facts and raw resource declarations. Core
  normalizes them; `auctioneer-runtime` owns the checked additive accounting.
  The compiled default pair has an independent clone charge today.
- Gates, host primitives and several initialization results are process/
  isolate scoped. A new configuration API cannot pretend these are
  independently replaceable per App or per request.

Implementation references are collected in section 15. Claims about the SDK
must use the dependency revision in this baseline's `Cargo.toml`, not the
head of a moving branch or a different local checkout.

## 3. Approaches And Decision

| Approach | Benefit | Cost / reason not selected |
| --- | --- | --- |
| Product defaults + typed startup + replaceable policy | Reuses the current typed resolver and host services; releases are operator-neutral; the first read stays bounded | Requires an explicit split and migration of existing baked settings; recommended |
| Move every setting into the pushed runtime payload | One apparent source | Circular trust, locator and allocation limits; a blob could authorize its own read or resize live gates; rejected |
| Keep custom builds and add more optional overrides | Smallest immediate diff | Leaves compiled operator fallbacks, deployment-dependent builds and a second embedded source of truth; rejected |

Use the first approach. Defaults remain useful, but they are reviewed product
data, never the last operator configuration captured during a release build.

## 4. Ownership And Lifecycles

| Class | Owner / source | When it changes | Admission boundary |
| --- | --- | --- | --- |
| Product constants and defaults | Versioned Auctioneer source | New release | Build/source validation and existing protocol/resource controls |
| Startup settings | Operator deployment configuration, or the embedding host | Restart/redeploy the affected configuration domain | Typed startup validation before policy extraction or normal request admission |
| Runtime policy | One complete operator document from the selected source | Next request using a successfully validated snapshot | Bounded loading, pure typed/semantic validation and installed ceilings |
| Secret values | Host secret service | According to that purpose's existing rotation contract | Existing bounded, purpose-specific secret reader; never ordinary policy |
| Derived state | Owning resolver, resource model or SDK | Recomputed from admitted inputs | Not independently operator-settable |

An immutable default for an operator setting is allowed only when it is
operator-neutral and documented. Physical resource names, account identities,
customer domains, host vendor identity and credentials are not release inputs.
Empty/unconfigured values must not fabricate an integration or grant permission.

Do not replace generated values with Rust `Default` indiscriminately: current
effective resource defaults contain zeros and current UserSyncConfig supplies
an example external URL. Product defaults must be valid neutral values, not
accidental zero limits or a fictitious live service. Represent user-sync as an
optional configured service: absent configuration is an explicit unconfigured
state, not an empty URL that passes a validator. When unconfigured, sync routes
refuse with a fixed unavailable result without reading signing credentials or
constructing callback URLs; auctions and info do not require a sync service.
Configuring it requires a valid explicit external origin and all existing
cookie, callback and origin checks. Neutral sync preferences may still have
product defaults. This is a proposed effective-model change owned by #126/#127.

Bidder implementations, declarations, supported features, parameter schemas,
provenance, routing safety restrictions and generated registrations remain
compiled. Operator enablement, documented host settings, timeouts and supported
runtime aliases remain policy. An enable flag cannot manufacture an unlinked
implementation, override an immutable disable, enable guest-ineligible code,
or replace a bidder's schema or endpoint-safety rules.

No policy snapshot may change a store locator, trust binding, admission count,
extractor cap, parser implementation, authorization primitive or resource fact.
Existing privacy policy settings remain configurable where the current policy
contract allows it; this design does not silently make every switch immutable
or change consent enforcement. Validation and load failures must never turn
enforcement off.

## 5. Proposed Typed Boundary And Crate Ownership

Keep the existing crates and dependency direction:

- `auctioneer-config-schema` keeps portable effective DTOs, bidder-owned schema
  declarations and native build support. Native generators stay out of serving.
- `auctioneer-config` owns a reviewed product-default artifact and its effective
  values. Its build script no longer opens the operator authoring file or
  `runtime-bootstrap.toml`. Bidder/disclosure/GVL generation remains.
- `auctioneer-core::config` owns the startup DTO, runtime DTO, semantic validation
  and snapshot construction. CLI, standalone handlers and embedders use it.
- Adapters translate their deployment surfaces and supply actual resource and
  host-service facts. Core does not read provider environment variables.
- `auctioneer-runtime` remains the sole resource derivation/validation owner.
  CLI diagnostics consume the same resolved inputs without installing globals.

The following are proposed API roles, not existing APIs:

1. `AuctioneerStartupConfig`: strict deployment intent, including the selected
   configuration source, extraction settings and permitted resource settings.
2. `ConfigValidationContext`: explicit borrowed product defaults, compiled
   catalog/feature eligibility and the relevant startup/envelope limits. It
   replaces implicit reads of process-global boot state in pure validation.
3. `ValidatedPolicySnapshot`: privately constructed, immutable policy plus
   validated identity and ownership metadata. Its constructor accepts only
   bounded input or a host-owned typed value with an explicit reservation.

Do not eagerly resolve and clone a full HostConfig for every account and
bidder. Preserve the current demand-scoped resolution and budget lookup; any
new retained index or cache needs its own concrete ownership and accounting.

One application-level configuration owner resolves all auction, sync, info and
status callers. Source selection does not fork their policy logic. Existing
exchange inputs (`HostConfig`, selected `AccountConfig`, response budgets and
features) stay independent of HTTP/runtime configuration DTOs.
Do not deserialize public input directly into today's effective output DTOs:
they are not uniformly strict or self-validating. Keep strict input types and
one explicit validated construction path to effective state.

## 6. Authoring Cutover

Replace the operator file's `baked` root with `startup`; retain a separate
`runtime` subtree that is the only application policy sent by `config push`.
The recommended strict runtime scopes are:

- `host`: host policy, including auction/resource requests, GDPR/CCPA/LMT,
  cookies, user-sync, aliases and the status banner.
- `account_defaults`: a deployment-wide account overlay over product defaults.
- `accounts`: publisher-specific overlays over those deployment defaults.
- `bidders`: supported per-identity operator settings and requests.
- `features` and `floor_currency`: their existing policy concerns.

This is a hard authoring/schema cutover, not a dual-format parser. Reject
`baked`, the retired flat runtime layout, unknown fields and mixed old/new
documents. Preserve required OpenRTB identifiers and external attribution.
The cutover must provide an explicit field mapping and validated configuration
diff; it must not guess operator values from the machine that built the CLI.
No permanent legacy parser, automatic fallback or migration command is required.
Any separate migration utility would need its own concrete use case.

Require `runtime.schema_version = 1` in the new application payload; no missing
version default. This is distinct from the SDK envelope's version `1`, which
currently establishes envelope format only. The authoring wrapper projects
only the runtime subtree, including its application schema version. A legacy
payload with a valid SDK envelope/digest must still fail the new app schema.

Illustrative new authoring fragment, not accepted by today's code:

```toml
[startup.config]
source = "store"
store = "auctioneer_runtime"
key = "auctioneer_runtime"
max_config_fetch_ms = 2000

[runtime]
schema_version = 1

[runtime.host.gdpr]
enabled = true

[runtime.host.gdpr.tcf2]
enabled = true

[runtime.account_defaults]
debug_allow = false
```

The source/key in this example must be bound by the selected deployment;
Fastly specifically constrains the config key to its logical store alias.
It is not a new physical store declaration or an operational configuration.
Wire/version checking must use the existing SDK envelope contract and a
reviewed application schema identity; do not conflate those two versions or
invent compatibility from a matching content digest.

The former global targeting overlay moves into `account_defaults`; its fields
do not remain as a second precedence layer. Per-account response-cap defaults
must be included explicitly in the shared budget resolver, not lost because
they are not fields of today's effective `AccountConfig`.

## 7. Source Selection And Bootstrap

Select exactly one source at startup:

| Mode | Policy owner | Failure behavior |
| --- | --- | --- |
| `store` | The declared EdgeZero config binding/key | Missing, malformed or unsupported data refuses policy-dependent routes; backend/deadline failures retain the existing transient disposition |
| `host` | A validated snapshot supplied by the embedding host | Missing or incompatible host state refuses; no fallback store or second App is created |
| `product_defaults` | Explicit operator selection of product-only policy | No store read; still validates against catalog, authorization and installed resource limits; never selected because a required load failed |

`store` is the standalone default. An absent binding/blob is not an implicit
request for `product_defaults`. Embedding selects `host` explicitly. A source
change, trust/physical binding change or extractor-limit change requires a new
configuration domain; a policy push cannot perform it.

Bootstrap order is mandatory:

1. Establish the release/host's pre-existing bounded startup-input and logging
   boundary, including logging/health binding materialization before normal
   policy extraction. Privileged input is not an allocation proof by itself.
2. Decode/validate startup settings without reading the policy they bound.
   Apply the existing product/adapter defaults for omitted startup values;
   explicitly invalid values are errors, not requests for defaults.
3. Resolve actual adapter/host facts and normalize byte/value/deadline and
   permitted resource settings once. Derive the complete budget with the
   same normalized inputs that installation and CLI diagnostics will use.
4. Install the single shared gates, extraction/trust/source policy and host
   primitives in the existing bootstrap order. Do not configure Apps with
   wider captured SDK defaults and hope a later boot repairs them.
5. Only then build/configure the serving App and admit policy-dependent work.
   Loading and validation occur while the appropriate funded work receipt is
   held; initialization outside a request has its own funded startup owner.

Domain validation failures (source, trust/bindings, extraction bounds or
resource settings) are fatal. Health is a separate, frozen startup component:
preserve Axum's fatal health-resolution errors and the edge adapters' existing
health-only degradation to `Closed`, with a fixed safe diagnostic. An edge
health-resolution failure may leave otherwise valid auctions/status available;
it cannot authorize health, bypass policy loading or change the domain's source.
A health installation conflict remains fatal on every adapter. Do not let a
generic startup deserializer accidentally broaden or erase that exception.

The complete normalized SDK extraction tuple is part of the installed context:
`max_backend_bytes`, `max_blob_bytes`, `max_json_depth`, `max_json_nodes`,
`max_secret_bytes`, `max_total_bytes` and `timeout`. Existing boot rows directly
fund the backend/blob/value caps; newly exposed settings must also have a
mapped enforcement/accounting owner in #128/#130. An unmapped field stays a
fixed product/SDK setting, not a silent new operator knob. Check captured Apps
against the complete accepted tuple; do not approve a wider depth/secret/total
allowance merely because the three older comparisons pass.

The bootstrap implementation task must freeze the raw startup-input limits
and parser/storage reservation before measurement or provider adoption. It
must identify which host/SDK operation bounds the read, structure and parse.
Reusing a helper that only reads a whole TOML file or applies environment
overrides is not sufficient. If the SDK has no adequate hook, that raw-input
path stays blocked until a reviewed SDK extension or bounded owner exists;
the host's already typed startup API can be developed independently.
Where a provider cannot check a length before producing a guest value, a
published provider maximum plus an independently funded materialization bound
may supply the limit. A post-allocation check alone is not that proof; missing
provider facts retain the selected path's explicit prerequisite owner.

This design does not approve an arbitrary new numeric startup reservation.
No provider task may claim a safe bootstrap by marking those figures pending,
using an unbounded read, or choosing them after a measurement to obtain green.

Keep one configuration domain for the current process/isolate globals.
Identical per-request reinstallation is idempotent. A different valid startup
configuration must fail as a conflict rather than silently inheriting the
first installation or replacing counters that live requests still hold.
Fatal domain validation publishes no new startup policy/gate state. A host
logger or closed health diagnostic may already exist; this is not a promise
to roll back unrelated global host state. Native-fatal health resolution also
keeps dispatch closed. Multi-domain engine instances would
require a separate ownership change, not a convenience overload here.

## 8. Resolution, Replacement And Failure Semantics

Each runtime publication is a complete operator document, not a patch against
the previous publication. Optional fields inherit reviewed product defaults
or an explicit parent scope in the same document. Removing a document entry
removes that operator override; a previous release's baked entry must not
reappear. Never merge a new payload into a process-global previous payload.

Preserve these independent resolution rules:

- Host = product HostConfig + runtime host overlay, followed by installed
  bounds. A typed requested cap is not an allocation entitlement.
- Account = product AccountConfig + `account_defaults` + the selected named
  account. Keep the distinction between configured, host-provided and unknown
  fallback identity; a synthesized `unknown` name cannot select an account.
- Response caps = observable bidder, alias base, selected account, deployment
  account defaults, runtime host, product host/historic fallback, then the
  existing funded min and warning behavior. Resolve each cap independently.
- Bidder settings, activation, sync-family permissions and timeout fallback
  retain their actual existing per-field rules. An alias cannot bypass its
  base's eligibility or a disabled sync family.
- Arrays replace rather than append. Ordinary nested objects merge by their
  defined field/key rules within this document. Activity entries merge by key
  and independently supplied `default` and `rules` fields. Supplied rules
  replace the complete ordered array, including `[]`; omitted fields inherit.
  A parent `default = false` with a child `rules = []` must still deny by
  default. Never replace that whole entry with a new allow-by-absence entry.
- Missing optional values mean inheritance. `false`, `0`, empty lists and empty
  maps retain field-specific meanings, not one generic truthiness rule.

Freeze the following reset rules in the new strict source model. A nullable
effective field accepts its normal typed value or the strict inline value
`{ clear = true }`; omission inherits. The latter clears the optional effective
value. It is represented by the same tagged object in JSON policy, not by a
TOML-only preprocessing pass or a new CLI command. Reject extra keys, false,
unsupported fields and direct nulls. This small field-level union applies
only to the nullable fields below, not arbitrary JSON paths or patching.

| Field | Missing | Explicit clear value |
| --- | --- | --- |
| Host cookie `domain`, `optout_cookie`; user-sync `redirect_url` | Product/default parent behavior | Clear the optional value; configured sync itself still requires its external origin |
| Host user-sync service | Unconfigured unless explicitly supplied in this document | Unconfigured; no inherited endpoint is retained |
| Account `truncate_target_attribute`; cookie-sync `default_limit`, `max_limit`, `default_coop_sync`, `priority_groups` | Inherit deployment account defaults, then existing field fallback | Set effective None, restoring that field's documented no-value/host fallback; empty groups remain different from None |
| Account treatment `enabled`, `access_allowed` | Inherit account defaults | Restore host treatment fallback, not an unconditional grant |
| Ordinary booleans, required strings/numerics, tri-states, namespace/maps/lists and features | Their documented inheritance rule | Reject; tri-state `unset` remains the explicit account-to-host fallback |
| Generated bidder-owned settings | Their current owner-declared per-field fallback | No new clear union; preserve only nulls explicitly admitted by the existing generated settings contract and reject unknown keys |

Absence of a fresh publication is not this field-level inheritance mechanism.
For example, a named account can set `truncate_target_attribute = { clear = true }`
to clear a parent's optional truncation setting. The CLI, extractor and host
constructor must agree on that operation before it is published. Required
fields cannot be cleared, and a generated bidder parameter remains governed
by its own protocol schema, not this policy override vocabulary.

Structural, scalar and cross-field validation share the same code for CLI,
bounded store extraction and host-provided typed snapshots. Validate the
effective result before publication, including account defaults, namespace
collisions, compiled catalog/feature restrictions, cookie/sync safety,
bidder-owned settings, currency freshness rules and both independent cap
ceilings. Do not validate after clamping and thereby hide an invalid field.
Preserve the separately documented successful clamp/warning behavior for
otherwise valid requested values that exceed installed capacity.

Capture one snapshot for each request and derive all its host/account/bidder/
feature/debug/sync decisions from that snapshot. Never re-read the config
mid-auction. Different requests may legitimately use different publications.

For store mode, keep current request-time extraction and its refusal behavior;
this design adds no last-known-good fallback or background refresh cache.
For host mode, the host may atomically replace its current validated snapshot.
A failed candidate leaves the host's explicitly managed current snapshot
unchanged; it must not publish an unvalidated candidate as current. This is
not permission for Auctioneer to mask a failed required store load. Admission
and version-retention limits in section 11 still apply to host publication.

Keep the runtime owner's closed error taxonomy: malformed/integrity/version/
structural/invalid-key failures are permanent refusals, transient store/secret/
deadline failures are 503 with the established retry behavior, and unknown
errors fail closed. `/status` may expose its current fixed degradation code,
not values, keys, credentials, account names or raw validator diagnostics.
Invalid configuration-domain startup is never a normal serving state; the
health-only exception in section 7 is deliberately narrower. Where the host provides
bounded pre-established liveness it may report unready. This does not promise
that an aborted native process still has a listening `/status` endpoint.

## 9. Trust, Secrets And Environment

Use the existing EdgeZero push/extraction/store-registration path. A checksum
is content integrity, not proof of who wrote the store. Retain the actual
envelope checks and the deployment's authenticated writer/binding boundary;
do not claim an additional signature check exists or sign the runtime process.
Adding a new cryptographic configuration scheme is out of scope.

Keep secret values outside operator policy and build artifacts. Cookie keys
and bidder credential bundles retain their existing bounded readers and
rotation semantics. Health setup currently reads platform values directly;
the startup/provider tasks must establish its pre-extraction materialization
bound rather than claim the runtime extractor already covers it. Startup may
identify allowed logical bindings; a policy field cannot select an arbitrary secret backend,
raise the read allowance or repurpose a credential for another consumer.
Retain signed-cookie active/previous-key behavior and the distinction between
cookie integrity and authentication of a callback's claimed UID.

Deployment adapters own environment/binding translation. Auctioneer-owned
variables use `AUCTIONEER__SECTION__FIELD`; single underscores remain inside a
component. Update readers, manifests, CLI, scripts and docs together; no legacy
aliases. Existing provider/SDK names are not renamed by this contract.

There is no ambient environment overlay on every request. For standalone use,
the bounded authoring/load operation applies its explicit overlay once, then
validates the result. For push, the CLI's explicitly selected input is emitted
as policy; serving does not add the worker machine's environment a second time.
Host mode receives the host's final typed configuration, not an implicit
Auctioneer scan of the process environment. Diagnostics report fixed reason
codes and reviewed field paths only; rejected values and secret locations are
not copied into logs, error parameters, public status or config exports.

## 10. Trusted Server Integration

Trusted Server owns its EdgeZero App, validated Settings, stores, secrets,
timers, outbound transport, admission and egress lifecycle. Auctioneer accepts
host-supplied configuration through its core contract; it must not instantiate
a second standalone App, private store, router or HTTP hop to fetch policy.

This is the integration target, not a claim that the current host already
implements it. Source inspection used Trusted Server commit
`5ee01b365c2b509b4e21134cb31502ef3f2b86a6`; its configuration/services paths
match the older issue reference for this review, but the whole revisions are
not asserted equivalent. It locks EdgeZero `87456c708ffe9edf86b3a0f8b06dfd15d60b07b5`
while Auctioneer pins `775366fdac923135e8430b1e3dffebedc648ef7c`. Its existing
Settings loader is not Auctioneer's bounded extractor, and typed validation
does not establish bounded ingestion. Task #34/#135 must reconcile SDK type/
service compatibility and the selected host's startup-read ownership before
accepting an actual integrated artifact. Do not link incompatible SDK types
under the assumption that their crate/type names establish identity.

The embedding task must supply a snapshot compatible with the compiled
catalog, product-default revision and installed host resource envelope. A
typed host value still passes Auctioneer's semantic validation and structural/
resource bounds before the private snapshot constructor admits it. Public
struct construction is not a bypass for the store path's validation.
In particular, a typed value with short strings can retain oversized backing
capacities. Count/validate bounded logical content before copying; normalize
into funded owned snapshot storage rather than adopting arbitrary host
capacities as if serialized length proved their size. The original Settings
allocation remains the host's accounted owner, including during refused
normalization. A borrowed zero-copy path needs a separate host capacity/lifetime
proof; it is not the default exemption for typed input.

Route registration uses EdgeZero's existing host registration mechanism;
configuration work does not authorize cross-crate private imports or a
provider-specific engine fork. The host and standalone forms must produce
the same effective policy, exchange input and refusal for equivalent admitted
inputs, while reporting their different resource/transport facts honestly.

Task [#135](https://github.com/IABTechLab/auctioneer/issues/135) blocks acceptance
of the actual integrated artifact in
[#76](https://github.com/IABTechLab/auctioneer/issues/76). It does not block
independent host-contract review or bridge development, and acceptance of one
selected host path does not require all four standalone adapter ports.

## 11. Resource And Evidence Contract

Moving values from generated statics to operator-owned heap changes storage
ownership even when effective policy is identical. Do not delete the old
default-clone term and describe the difference as free capacity.

- Keep product-default cloning/storage independent of the runtime blob cap.
  Name it for product defaults rather than operator-baked config; measure the
  actual product-default artifact through the current non-materializing
  serialization owner.
- Keep backend/envelope bytes, typed-policy bytes and SDK JSON value count as
  independent extraction caps. Syntax-node counts used by auction/cookie
  paths are a different unit. A small byte cap does not waive a node charge.
- Charge startup decoding, retained startup state and validated policy
  storage with explicit owners. No startup work occurs before its reservation
  merely because normal request gates do not exist yet.
- Store mode preserves per-admitted-work extraction/clone charges. Host mode
  does not zero those charges just because it supplies an Arc or skips a store
  read; any tighter owner-specific model requires independent bounds first.
- Bound host snapshot construction and retirement. One current snapshot,
  candidates, and versions retained by auction, aux and probe callers can
  coexist. Do not assume only two versions: stalled requests can retain many
  distinct generations. The owning reservation must live as long as the
  snapshot; release/update refusal cannot strand unfunded retained versions.
- Binding names, startup maps, keys, credential metadata and temporary copies
  count where retained. Shared catalog/schema data remains charged by its
  existing resident/initialization/auction owners, not smuggled into policy.
- Resource intent, platform facts and measured costs remain distinct. An
  operator can choose supported deployment limits, not declare a smaller
  actual decoder/layout cost or turn an unknown SDK fact into a known one.
  Requested factors/reserves must meet current floors and independent
  platform/configured ceilings. Derived totals and measurement claims are
  never accepted as operator input.

Task [#130](https://github.com/IABTechLab/auctioneer/issues/130) must retain one
shared checked additive derivation for boot, diagnostics and qualification,
including all existing terms and configuration's new ownership. No new
startup/steady `max()` equation, hidden baseline substitution, changed
measurement threshold, smaller default workload or higher ceiling is approved.
Existing over-ceiling boot warnings and stricter native schema admission are
not silently reclassified as physical fit by this refactor.

An executable hash identifies code, not a deployment's policy. Reuse the
existing artifact/evidence identity contracts and bind startup configuration,
effective limits and applicable policy to the actual qualification run.
Different operator settings can share code bytes but cannot automatically
share readiness evidence. Old captures remain immutable; changed ownership
or frozen profile bindings need explicit new evidence, not rewritten results.

## 12. Required Acceptance And Negative Controls

Every implementation task owns its affected controls; do not defer safety
testing to final Trusted Server or publisher testing.

| Surface | Required proof / control |
| --- | --- |
| Release neutrality | Build once without an operator file; run that unchanged artifact with two isolated operator configurations and show different expected policies and identical executable hashes. Removing/adding/changing a local operator file cannot affect a release build. |
| Classification | Cover every current field group in the inventory. Reject unknown startup/policy fields and all retired authoring roots. Any deliberately removed field has a named migration/refusal, not silent ignore. |
| Shared validation | Exercise the same scalar and cross-field invalid cases through CLI, actual store extraction and host-typed constructors; assert exact fixed categories. Include a wrong application schema version under a valid SDK envelope and recomputed correct digest. Validation is non-installing and uses the supplied context, not whichever global profile a test installed first. |
| Bootstrap | Under/at/over input and extraction bounds independently; malformed explicit domain values, absent required fields, missing source/binding, preboot App, captured caps above funding, conflicting second installation and concurrent identical initialization. Fatal domain validation performs no policy read or startup-policy/gate publication. Separately assert native-fatal versus edge-closed health resolution and fatal health installation conflicts. |
| Replacement | Publish A, then B omitting an account/alias/override, and prove A's data is gone. Invalid B cannot masquerade as a valid fresh/default document. Hold requests on distinct valid generations and prove each uses one coherent snapshot. |
| Resolution | Vary product, host, account-default, selected-account, bidder and alias values independently. Assert empty/false/zero/null behavior by field and valid/malformed explicit clears, including inherited nonempty before-states for every clear case, fallback account identity, independent activity default/rules inheritance, whole-rule-array replacement and separate transport/decoded clamps. Include parent activity default false plus child rules [] and assert that default false survives. |
| Privacy and credentials | Isolate GDPR/account opt-in, COPPA, malformed selected USP, LMT and activity decisions. Preserve cookie-key rotation and hostile secret-in-flag redaction controls. A missing/corrupt/unknown policy cannot grant via defaults or another source. |
| Resource ownership | Independent expected deltas for each configuration term and its real multiplicity; distinct envelope/blob/value caps, asymmetric call/bidder limits, concurrent old/new snapshots, candidate refusal, release on cancellation, boot-vs-report input parity and both configured/platform ceilings. |
| Adapter parity | Axum, Cloudflare, Fastly and hosted Spin each consume startup settings through their actual supported surface and prove one unchanged artifact with two isolated configs. Native tests are not a guest execution claim. |
| Embedding | The selected real host artifact uses host settings/services with no mandatory Auctioneer store, second App or HTTP hop. Compare effective policy and controlled engine results with standalone use. |

Use existing fake bidders for shared infrastructure. Unit tests stay inline;
integration and conformance tests remain with their owner. Preserve allocator,
process-global and provider lifecycle isolation. No ignored failures, lighter
workload, increased ceilings, self-derived expected bounds or workflow-source
snapshots are introduced to make the migration pass.

## 13. Execution Boundaries And Review Gates

This draft defines the contract for #125. It is not authorization to implement
the whole epic before maintainer review, and it does not mark #125 accepted.

Review the contract and inventory in a documentation-only design PR.
Implementation and its tests belong in a separate PR after design approval;
do not append runtime changes to the design PR or count its merge as completion
of the configuration epic's implementation tasks.

| Issue | Deliverable / dependency |
| --- | --- |
| #126 | Prepare reviewed operator-neutral defaults and remove build-time operator-file access, coordinating the optional sync-service effective model with #127. No intermediate serving cutover may install invalid/fictional defaults. |
| #127 | One strict runtime model, shared pure validation and explicit resolution; add account-default coverage and the hard schema cutover. Depends on product defaults. |
| #128 | Typed startup/bootstrap and conflict-safe installation; freeze bounded raw-input/parser ownership before adoption. Depends on the approved contract and shared model. |
| #130 | Account for changed startup/policy/snapshot ownership through the shared derivation. Coordinate with #128; no provider or embedded serving path is accepted before its bounds are funded. |
| #129 | CLI authoring, validation, explicit clear/cutover instructions and export/push behavior using the same admitted types; it must not inject operator data during release builds or add permanent legacy serving aliases. |
| #131 / #132 / #133 / #134 | Provider-specific startup/binding translation and unchanged-artifact acceptance after shared bootstrap/accounting. These are independent ports, not four prerequisites for one embedded target. |
| #135 | Host-supplied typed startup/policy contract, validated snapshot lifetime and actual-host integration prerequisites; coordinate with host-contract task #34. |
| #136 | Operator-neutral release, deployment and cutover documentation matching implemented behavior and actual selected-path evidence. |

Before implementation, review the proposed runtime scope regrouping, optional
reset semantics, typed startup source and complete field dispositions. Before
provider adoption, review the exact startup input/parser limits and resource
ownership package from #128/#130. These gates have concrete child owners;
unknown SDK functionality stays a named prerequisite, never an invented API.

## 14. Draft Verification And Limits

This is a documentation-only design task. Source inspection and independent
document review are not runtime or deployment acceptance. The local checks
cover new-document references, TOML example syntax, naming and whitespace;
no Cargo, guest, hosted or new resource measurements are claimed. Maintainer
approval of this written contract is still required before implementation.

Read-only reviews checked the field/build/default/reset contract and the
SDK/provider/host/bootstrap contract independently. Corrections preserve
independent activity default/rule inheritance and distinguish fatal domain
startup from health-only degradation; health materialization remains an
explicitly owned gap. Both reviewers reread the revised documents and found
no remaining material planning issues in their respective scopes.

Local verification checked 46 file links across the two new documents, parsed
the proposed authoring example as TOML, and passed naming and whitespace
checks. Those checks do not establish production-schema acceptance. Only the
two draft documents changed in the isolated design branch; no runtime or
configuration implementation was started.

## 15. Source References

- [Current authoring root](../../../auctioneer.toml),
  [strict source wrapper](../../../crates/auctioneer-config-schema/src/lib.rs),
  [build and generation owner](../../../crates/auctioneer-config/build.rs).
- [Effective DTOs](../../../crates/auctioneer-config-schema/src/types.rs),
  [runtime DTO and effective validation](../../../crates/auctioneer-core/src/config/mod.rs),
  [resolution](../../../crates/auctioneer-core/src/config/resolve.rs),
  [response-cap precedence](../../../crates/auctioneer-core/src/config/budgets.rs).
- [Compiled bootstrap input](../../../crates/auctioneer-config/data/runtime-bootstrap.toml),
  [runtime loading and failure owner](../../../crates/auctioneer-core/src/config/runtime.rs),
  [App configuration/captured limits](../../../crates/auctioneer-core/src/configure.rs).
- [Adapter resource profile](../../../crates/auctioneer-core/src/host_primitives/resource_profile.rs),
  [bootstrap order](../../../crates/auctioneer-core/src/host_primitives/bootstrap.rs),
  [shared gates and normalization](../../../crates/auctioneer-core/src/host_primitives/gates.rs),
  [resource derivation](../../../crates/auctioneer-runtime/src/budget.rs),
  [auxiliary ownership](../../../crates/auctioneer-runtime/src/aux_work.rs).
- [Logical store/handler declarations](../../../edgezero.toml),
  [bounded cookie-key reader](../../../crates/auctioneer-core/src/handlers/usersync_keys.rs),
  [dependency revision](../../../Cargo.toml).
- Pinned SDK [envelope integrity/version](https://github.com/IABTechLab/edgezero/blob/775366fdac923135e8430b1e3dffebedc648ef7c/crates/edgezero-core/src/blob_envelope.rs),
  [extraction policy](https://github.com/IABTechLab/edgezero/blob/775366fdac923135e8430b1e3dffebedc648ef7c/crates/edgezero-core/src/config_store.rs),
  [authoring loader](https://github.com/IABTechLab/edgezero/blob/775366fdac923135e8430b1e3dffebedc648ef7c/crates/edgezero-core/src/app_config.rs).
- Trusted Server [Settings payload loading](https://github.com/IABTechLab/trusted-server/blob/5ee01b365c2b509b4e21134cb31502ef3f2b86a6/crates/trusted-server-core/src/config_payload.rs),
  [host services](https://github.com/IABTechLab/trusted-server/blob/5ee01b365c2b509b4e21134cb31502ef3f2b86a6/crates/trusted-server-core/src/platform/types.rs),
  [locked dependency](https://github.com/IABTechLab/trusted-server/blob/5ee01b365c2b509b4e21134cb31502ef3f2b86a6/Cargo.lock).
