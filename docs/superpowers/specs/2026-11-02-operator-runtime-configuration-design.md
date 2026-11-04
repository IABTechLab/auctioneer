# Operator Runtime Configuration

Date: 2026-11-02

Status: Active contract for PR #140, revised to the explicit maintainer direction
on 2026-11-04. Finish one runtime configuration through existing EdgeZero typed
validate/diff/push and bounded typed consumption. This supersedes the earlier
startup-package gate, startup/runtime authoring wrapper and source-mode design;
it is accepted implementation scope, not a claim that serving cutover is done.

Section 5.1's bidder-validation amendment was approved on 2026-11-02. Existing
staged types and dated audits are implementation history, not authority to keep
a separate startup config, loader or retained-snapshot publication subsystem.

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

This contract covers compiled product defaults, runtime policy, host-owned
deployment boundaries, secrets, validation, ownership and the authoring cutover.
The [field inventory](2026-11-02-operator-runtime-configuration-inventory.md)
retains its field census and privacy/semantic rules. Its original proposed `S`
startup destinations and `runtime.schema_version` spelling are superseded by
sections 4, 6 and 7 here: host facts stay host-owned and `schema_version` is at
the payload root. A classification is not proof that a runtime reader exists.

This work does not add bidders, a plugin loader, new auction features, a new
configuration framework, a separate embedding configuration system, or a
new workspace crate. It does not qualify a guest deployment or change the
resource equation, workload defaults, measured floors or provider ceilings.
This scope excludes an Auctioneer startup file/config/loader, source-mode
config, new numeric startup reservation, global host snapshot pool or SDK startup hook.
Actual Trusted Server integration and adapter/provider ports remain child tasks.
The [resource-qualification epic](https://github.com/IABTechLab/auctioneer/issues/9)
retains those readiness outcomes.

## 2. Historical Starting Point

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

## 3. Accepted Decision

Use one strict `OperatorPolicy` model as both the direct operator-file payload
and the runtime payload. Reuse EdgeZero's existing typed validate/diff/push;
apply the same shared validators at push and bounded SDK consumption. The typed
host contract reuses policy validation/resolution over explicit caller-owned
input/context; actual host ingestion/adoption and capacity/lifetime proof remain
#135, not PR #140 readiness claims. Any SDK wrapper is a zero-layout trait boundary only, not a
new wire root, projection or second configuration model.

Host/adapter physical facts, store/trust bindings and installed limits remain
outside that policy under their existing owners. Runtime policy requests only
existing supported caps within installed capacity; it cannot fund its own read
or resize live gates. Compiled neutral defaults remain code-owned product data,
never operator config or an implicit fallback after a required load fails.

## 4. Ownership And Lifecycles

| Class | Owner / source | When it changes | Admission boundary |
| --- | --- | --- | --- |
| Product constants and defaults | Versioned Auctioneer source | New release | Build/source validation and existing protocol/resource controls |
| Host/adapter deployment facts | Existing host/adapter APIs, services and bindings | Existing restart/redeploy lifecycle | Existing host validation, normalization, boot funding and installation; no Auctioneer startup DTO |
| Runtime policy | One complete direct `OperatorPolicy` document | Next request consuming a successfully validated document | Bounded typed loading, shared semantic validation and installed ceilings |
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

## 5. Shared Typed Boundary And Crate Ownership

Keep the existing crates and dependency direction:

- `auctioneer-config` keeps portable source/policy/effective DTOs, shared pure
  validation and optional native build support. It has no build script or
  engine, bidder, SDK or compiled-data dependency.
- `auctioneer-config-data` owns a reviewed product-default artifact and its effective
  values. Its build script no longer opens the operator authoring file or
  `runtime-bootstrap.toml`. Bidder/disclosure/GVL generation remains.
- `auctioneer-bidder` owns generated concrete settings and endpoint bindings,
  alongside discovered bidder-local semantic validators.
- `auctioneer-core::config` composes shared models with those settings and owns
  semantic resolution, the SDK boundary and bounded typed consumption. CLI,
  standalone handlers and embedders use that orchestration.
- Adapters translate their deployment surfaces and supply actual resource and
  host-service facts. Core does not read provider environment variables.
- `auctioneer-runtime` remains the sole resource derivation/validation owner.
  CLI diagnostics consume the same resolved inputs without installing globals.

These package roles incorporate the maintainer-approved
[ownership amendment](2026-11-03-configuration-model-ownership-design.md),
implemented without increasing the package count. The starting-point section
records the earlier package names; its historical build behavior is not the
new serving contract.

The shared boundary consists of `OperatorPolicy` and `ConfigValidationContext`:
explicit borrowed product defaults, compiled catalog/feature eligibility, time
and host-owned normalized limits. Validation is non-installing and independent
of whichever global boot state a test installed first. Shared typed-policy helpers
accept caller-owned input/context and reuse the same validator/resolution, not
a parallel host/startup model or a new normalization constructor. Actual host
allocation ingestion/adoption is #135. Validated
request state is not a new publication subsystem or mandatory snapshot type.

Do not eagerly resolve and clone a full HostConfig for every account and
bidder. Preserve the current demand-scoped resolution and budget lookup; any
new retained index or cache needs its own concrete ownership and accounting.

One application-level configuration owner resolves all auction, sync, info and
status callers. Store and typed-host boundaries do not fork policy logic. Existing
exchange inputs (`HostConfig`, selected `AccountConfig`, response budgets and
features) stay independent of HTTP/runtime configuration DTOs.
Do not deserialize public input directly into today's effective output DTOs:
they are not uniformly strict or self-validating. Keep strict input types and
one explicit validated construction path to effective state.

### 5.1. Bidder-Owned Settings Validation

The shared validator orchestrates configuration admission; each bidder owns
the meaning of its protocol settings. Do not move bidder-specific conditions
into a central core validator or treat strict deserialization as complete
semantic validation. Generated settings enforce types, unknown fields and shared
byte/structure/reference bounds. The original baseline lacked bidder-owned
callbacks; the staged implementation follows the contract below. Its existence
does not establish serving activation or allocation admission.

Keep `bidder.toml` as the source of typed settings declarations and generated
borrowed accessors. Use one bidder-owned `validator::Validate` implementation
for field constraints and custom cross-field validation. Do not add another TOML
validation language, runtime schema compiler or duplicate settings DTO. The
generated settings aggregate is owned by `auctioneer-bidder` under the ownership
amendment and already implements generated structural `Validate`. Keep that
structural boundary and use a bidder-local borrowed semantic validation view, with
`Validate` derive/custom rules, projected borrowed fields and the same
pure semantic helpers as request construction. No settings clone, second
config parse or bidder factory invocation is needed for this projection.

Extend the existing discovered registration, not a second registration list.
A factory whose manifest declares settings must supply an owner-local
`config.rs`, wired by ordinary `mod config;`. The implementation's `mod.rs`
exposes a narrow owner-level `validate_config` function as a thin adapter to
that view's `Validate::validate`. Generated wiring references
`implementation::validate_config`, not a private child path. This adapter
is registration plumbing, not a second validation implementation or an
alternative optional hook. Discovery derives its requirement from the settings
declaration and emits a compiler-checked function reference with the factory's
actual feature/target eligibility. Missing or incompatible callbacks fail the
build; do not silently substitute success. A settings-free factory needs no
empty config module; shared validation rejects an incorrectly bound settings
object. Reference-only packages do not become executable validation providers.

There is one typed input and one validator invocation for each relevant bidder
configuration identity. Supplied-value checks and effective-requiredness checks
are internal rules of the same validator, not selectable validation modes.
Names below are proposed bidder-crate API roles, without core/HTTP/SDK types:

```rust
pub struct BidderConfigInput<'a> {
    pub requested: Option<&'a BidderSettings>,
    pub base: Option<&'a BidderSettings>,
    pub definition: Option<&'a BidderInfo>,
    pub enabled: bool,
}

pub type BidderConfigValidator =
    for<'a> fn(BidderConfigInput<'a>) -> Result<(), BidderConfigError>;
```

For settings-owning implementations, shared orchestration forms a bounded
identity union: known effectively enabled public identities/configured aliases,
plus every explicit settings entry, including disabled and request-dependent
entries. Visit each identity once
using existing indexes and borrowed keys. After shared structural and binding
checks, its bidder-owned validator checks all supplied requested/base values,
including disabled values or those shadowed by an override; a local constraint
must not disappear because a different value won resolution. Preserve current
missing/null settings behavior; no new clear union applies.
An identity whose implementation has no settings declaration has no callback
to run; it follows the same shared admission path with zero callback calls,
and any incorrectly attached settings object is refused. This is absence of
bidder configuration, not an alternative validation mechanism.

The same validator checks merged requiredness for known eligible enabled
identities, even when neither settings object is present. It uses the generated
fieldwise requested/base view and documented protocol defaults; `enabled` and
`definition` come from shared resolution, never a bidder-selected bypass.
Shared code retains enablement, namespace, catalog and immutable binding
checks. Use the existing alias/base resolution,
not a new all-fields-or-nothing merge. Required settings that are intrinsically
configuration-only can be checked here; a field that may validly come from an
auction/impression must not become a mandatory deployment setting. An absent
optional field is not automatically an error.

Do not use this hook to prohibit existing request-local alias behavior. For a
settings entry whose observable identity depends on a future request, validate
its declared implementation's supplied values through this same contract,
with `definition = None` and `enabled = false` until a request resolves it.
Retain binding, effective fallback and request-dependent requiredness checks
at the request boundary.
For identities resolvable from the document/catalog, validate their bindings
and effective settings before accepting the snapshot. No future request may
use the configuration callback to bypass exchange binding or parameter checks.

`BidderConfigError` carries only reviewed static field/reason identifiers.
Convert any `ValidationErrors` inside the bidder without formatting, logging or
retaining its value parameters across the callback boundary. Validator-derived
errors can temporarily allocate/copy values; the configuration work owner must
fund and bound that failure path, not label the callback allocation-free.
Shared public diagnostics remain fixed categories and approved paths, without
echoing instance names, settings or secret locations. Keep configured secret
references syntax/purpose-checked; do not read secret contents, authenticate a
credential, consult ambient environment, call a network service or initialize
mutable process caches from this callback.

`ConfigValidationContext` supplies the validated catalog/callback view without
installing a serving registry or constructing bidders. CLI, actual SDK store
extraction and caller-owned typed-policy validation run the same shared
orchestrator before accepting a candidate. A callback rejection rejects that
candidate; it never silently disables the bidder or publishes the rest of a
partly validated policy. A rejected host value is not adopted; a failed required
store load still refuses the request as specified in section 8.

Validation loops use bounded identity/alias indexes and borrowed views; do not
resolve/copy all accounts for every bidder or add a separately maintained
callback registry. Include any additional index/storage/error peak in the
existing configuration/catalog owners before serving cutover. Request-time
parameter schemas, endpoint/sink safety, privacy and secret-loading preflight
remain independent mandatory checks, not replacements for policy validation.

Required controls vary supplied values and effective requiredness independently
within this single contract: disabled entries with invalid supplied values,
enabled identities without settings,
fieldwise alias inheritance, explicit zero/empty/null, implementation mismatch,
request-supplied fallbacks, exact redacted errors and zero factory/store/secret
calls. Omit or miswire a callback in a synthetic discovered package and require
a real build/behavioral failure. Shared tests use the existing capable fake
and the existing synthetic settings compiler workspace, not a real bidder as
a stand-in or a fake variant added to production generated settings.

## 6. Authoring Cutover

The operator file is the direct staged `OperatorPolicy` payload, with required
`schema_version = 1` and these strict scopes:

- `host`: host policy, including auction/resource requests, GDPR/CCPA/LMT,
  cookies, user-sync, aliases and the status banner.
- `account_defaults`: a deployment-wide account overlay over product defaults.
- `accounts`: publisher-specific overlays over those deployment defaults.
- `bidders`: supported per-identity operator settings and requests.
- `features` and `floor_currency`: their existing policy concerns.

This is a hard authoring/schema cutover, not a dual-format parser. Reject
`baked`, `startup` and `runtime` root wrappers, retired flat overlay keys,
unknown fields and mixed old/new documents. No backward wire aliases.
Preserve required OpenRTB identifiers and external attribution. The root and
policy objects are objects/tables; positional arrays are not authoring syntax.
The cutover must provide an explicit field mapping and validated configuration
diff; it must not guess operator values from the machine that built the CLI.
No permanent legacy parser, automatic fallback or migration command is required.
Any separate migration utility would need its own concrete use case.

Require root `schema_version = 1` in the application payload; no missing
version default. This is distinct from the SDK envelope's version `1`, which
currently establishes envelope format only. File decoding, SDK serialization
and the typed host API use the same model, without subtree projection. A legacy
payload with a valid SDK envelope/digest must still fail the new app schema.

Illustrative direct authoring fragment; serving acceptance remains cutover work:

```toml
schema_version = 1

[host.gdpr]
enabled = true

[host.gdpr.tcf2]
enabled = true

[account_defaults]
debug_allow = false
```

Wire/version checking must use the existing SDK envelope contract and a
reviewed application schema identity; do not conflate those two versions or
invent compatibility from a matching content digest.

The former global targeting overlay moves into `account_defaults`; its fields
do not remain as a second precedence layer. Per-account response-cap defaults
must be included explicitly in the shared budget resolver, not lost because
they are not fields of today's effective `AccountConfig`.

## 7. Existing Host And Extraction Boundary

Host/adapter owners retain physical resource facts, store/key/trust and secret
bindings, health/logging setup, extraction limits and installed gate capacities.
The inventory's former `S` proposals do not create runtime setters for these
fields. Keep existing normalization, validation, conflict/idempotence behavior
and bootstrap order; install funded host primitives and extraction limits before
configuring serving Apps. A preboot App does not acquire narrower caps later.

No runtime payload can select `store`, `host` or `product_defaults` modes, change
a binding, authorize its own extraction, lower an actual cost or resize gates.
The existing store boundary and a typed host call are integrations of the same
policy model, not operator-selectable source modes. Required policy absence or
failure never selects product defaults. Runtime requests retain existing valid
clamp/warning behavior within host-owned capacity, after semantic validation.

Preserve fatal host-domain validation, Axum-fatal health-resolution errors and
edge health-only degradation to `Closed` with safe diagnostics. A health
installation conflict remains fatal everywhere; degraded health cannot bypass
policy loading or open authorization. Actual provider materialization and
Trusted Server ingestion bounds remain with their child integration owners,
not a new startup loader or numeric reservation gate in PR #140.

The complete normalized SDK extraction tuple is part of the installed context:
`max_backend_bytes`, `max_blob_bytes`, `max_json_depth`, `max_json_nodes`,
`max_secret_bytes`, `max_total_bytes` and `timeout`. Existing boot rows directly
fund the backend/blob/value caps; retain explicit enforcement/accounting for
the full tuple without adding operator knobs. Check captured Apps
against the complete accepted tuple; do not approve a wider depth/secret/total
allowance merely because the three older comparisons pass.

Serving SDK extraction, validation and temporary/per-request retained allocations
remain bounded and funded by existing owners before use. Shared typed-policy
semantics do not establish host allocation ingestion/adoption bounds; #135 owns
that actual integration and its capacity/lifetime proof. A post-allocation size check alone does not establish a
materialization bound; unknown provider facts remain explicit child-task gaps.
This runtime cutover neither adds an SDK startup hook nor requires a new
startup package, multi-domain engine or snapshot-publication service.

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
API must use that same model and shared resolution semantics before publication.
Exhaustive field oracles use nonempty supplied defaults in the shared model/resolver
tests; actual CLI/SDK tests prove boundary delegation, not every clear against
already-neutral compiled host defaults. Required fields cannot be cleared, and
a generated bidder parameter remains governed by its own protocol schema, not
this policy override vocabulary.

Structural, scalar and cross-field validation share the same code for CLI,
bounded SDK extraction and caller-owned typed-policy validation. Validate the
effective result before admission, including account defaults, namespace
collisions, compiled catalog/feature restrictions, cookie/sync safety,
bidder-owned settings, currency freshness rules and both independent cap
ceilings. Do not validate after clamping and thereby hide an invalid field.
Preserve the separately documented successful clamp/warning behavior for
otherwise valid requested values that exceed installed capacity.

Capture one policy generation for each request and derive all its host/account/
bidder/feature/debug/sync decisions from it. Never re-read config mid-auction.
Different requests may legitimately use different publications. Preserve early
release of unrelated accounts before fanout; coherent decisions do not require
retaining the entire policy tree throughout an auction.

Keep existing request-time store extraction and refusal behavior, without
last-known-good fallback or background cache. Shared typed-policy validation
rejects invalid caller-owned values, but does not ingest/adopt them or qualify
host update/storage behavior; that remains #135. Existing per-request retained
allocations stay bounded/funded through final drop, including overlapping
requests. A failed required store load is never hidden by prior host state.

Keep the runtime owner's closed error taxonomy: malformed/integrity/version/
structural/invalid-key failures are permanent refusals, transient store/secret/
deadline failures are 503 with the established retry behavior, and unknown
errors fail closed. `/status` may expose its current fixed degradation code,
not values, keys, credentials, account names or raw validator diagnostics.
Invalid host-domain initialization is never a normal serving state; the
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
the host/provider tasks must establish its pre-extraction materialization bound
for actual integration, not claim the runtime extractor covers it. Hosts own
allowed logical bindings; policy cannot select an arbitrary secret backend,
raise the read allowance or repurpose a credential for another consumer.
Retain signed-cookie active/previous-key behavior and the distinction between
cookie integrity and authentication of a callback's claimed UID.

Deployment adapters own environment/binding translation. Auctioneer-owned
variables use `AUCTIONEER__SECTION__FIELD`; single underscores remain inside a
component. Update readers, manifests, CLI, scripts and docs together; no legacy
aliases. Existing provider/SDK names are not renamed by this contract.

There is no ambient environment overlay on every request. Standalone serving
uses its existing host-owned deployment boundary; requests consume that
installed domain and accepted policy, not a fresh environment scan. Push emits
the direct typed policy through the existing
SDK flow; serving does not overlay the worker's environment onto that policy.
Local CLI authoring-read and once-only capture hardening is separate under 9.1.
The typed host API receives the same final policy model, not an implicit
Auctioneer scan of the process environment. Diagnostics report fixed reason
codes and reviewed field paths only; rejected values and secret locations are
not copied into logs, error parameters, public status or config exports.

### 9.1. One Configuration Loading Pipeline

The 2026-11-04 maintainer direction supersedes the earlier startup/runtime
scope interpretation of the 2026-11-03 amendment. There is one direct policy
document and no separate loader, legacy format or alternate validator. Neutral
compiled defaults are code-owned data, not another operator configuration.

Keep the existing `run_config_validate_typed`,
`run_config_diff_typed` and `run_config_push_typed` entry points with one
`OperatorPolicy` model and the shared application validators. An SDK trait
wrapper, if needed, preserves the exact root layout and secret metadata paths;
it adds no startup/runtime subtree, policy projection or second DTO.
Preserve SDK envelope integrity, secret metadata, store selection, command
flags and existing remote reread/concurrent-change behavior. Do not call that
behavior a compare-and-swap guarantee or invent another loader.

Bounded local authoring-file/manifest/environment materialization and reuse of
one accepted local input without raw/typed rereads are now **separate CLI
hardening**, not prerequisites for this cutover. Today's SDK supports the typed
single-config commands but does not provide those extra guarantees. The
follow-up must cover the real reads and intermediate ownership, use independent
initial ceilings rather than limits inside the file, and test changes to file
and environment after acceptance. Any prepared handoff is invocation-local,
not a configuration source or cross-request cache. Track this explicitly in
[#142](https://github.com/IABTechLab/auctioneer/issues/142) under configuration
epic #124; do not mark it implemented by using the current SDK helpers.

This does not waive serving extraction bounds, privacy/semantic validation,
safe diagnostics or existing host trust/resource controls. It replaces, rather
than defers, the former numeric startup/storage approval package. #128/#130
retain the actual runtime bounds/accounting work; provider/host ingestion
acceptance remains separate. Deferring local CLI hardening proves no serving
or provider ingestion bound.

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

PR #140 supplies reusable shared typed-policy semantic validation/resolution
over explicit caller-owned input/context, with no new normalization constructor
or host storage proof. Task #135 must implement actual ingestion/adoption of the
same `OperatorPolicy`, compatible with catalog, product-default revision and
host envelope. It owns the allocation/capacity/lifetime proof for the selected
copying or borrowing path; short logical values or serialized length do not prove
backing capacities. Host Settings/source allocations stay host-owned, including
on rejection. None of that actual host serving readiness is claimed by semantic
parity tests or public struct construction.

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
- Keep typed-policy parsing, semantic validation, callback-error temporaries
  and retained effective storage within the existing configuration/work owners.
  No new startup decoding/state reservation is part of this runtime cutover.
- Store consumption preserves per-admitted-work extraction/clone charges and
  early release of unrelated policy data before fanout. Overlapping request
  views/generations remain under existing admission/retention owners through
  final drop/cancellation; do not assume exactly two generations.
- Actual host ingestion/adoption and allocation/capacity/lifetime evidence are
  #135 work, not a new PR #140 constructor/storage package. An Arc or skipped
  store read proves no allocation saving or serving readiness. No global host
  snapshot pool or retained-snapshot publication subsystem is required.
- Binding names, keys, credential metadata and temporary copies count with their
  existing owners where retained. Shared catalog/schema data remains charged by
  its resident/initialization/auction owners, not smuggled into policy.
- Resource intent, platform facts and measured costs remain distinct. Runtime
  policy may request supported existing caps, but cannot choose installed limits,
  declare a smaller decoder/layout cost or turn an unknown fact into a known one.
  Host-owned factor/reserve declarations must meet current floors and independent
  platform/configured ceilings. Derived totals and measurement claims are
  never accepted as operator input.

Task [#130](https://github.com/IABTechLab/auctioneer/issues/130) must retain one
shared checked additive derivation for boot, diagnostics and qualification,
including all existing terms and configuration's actual ownership. No new
startup/steady `max()` equation, hidden baseline substitution, changed
measurement threshold, smaller default workload or higher ceiling is approved.
Existing over-ceiling boot warnings and stricter native schema admission are
not silently reclassified as physical fit by this refactor.

An executable hash identifies code, not a deployment's policy. Reuse the
existing artifact/evidence identity contracts and bind host deployment inputs,
effective limits and applicable policy to the actual qualification run.
Different operator settings can share code bytes but cannot automatically
share readiness evidence. Old captures remain immutable; changed ownership
or frozen profile bindings need explicit new evidence, not rewritten results.

## 12. Required Acceptance And Negative Controls

Every implementation task owns its affected controls; do not defer safety
testing to final Trusted Server or publisher testing.

**Latest directed scope alignment, 2026-11-04:** Exhaustive nullable/clear and
semantic field oracles have one shared model/resolver owner with caller-owned
input/context and nonempty supplied defaults. The canonical direct payload and
zero-layout SDK trait wrapper use the same underlying model, removing subtree
projection drift. Actual CLI/SDK tests still verify payload/object guards and
validator delegation, including bidder-settings miswiring; lifecycle tests own
A/B and no fallback. Do not add a validator, rebuild synthetic CLI defaults or
require a duplicate field-by-path Cartesian matrix for clears unobservable with
neutral compiled host defaults. Retain every existing test, negative control and
limit. This changes acceptance ownership explicitly, not pending command status
or safety/storage requirements; actual host integration remains #135.

| Surface | Required proof / control |
| --- | --- |
| Release neutrality | Build once without an operator file; run that unchanged artifact with two isolated operator configurations and show different expected policies and identical executable hashes. Removing/adding/changing a local operator file cannot affect a release build. |
| Classification | Cover every field group in the inventory under the revised ownership. Reject unknown policy fields, startup/source/binding/installed-limit setters and all retired root wrappers/flat keys. Removed fields have a named migration/refusal, never an alias or silent ignore. |
| Shared validation | Exhaustive representable scalar/cross-field/catalog/cap and bidder supplied/effective, disabled and alias semantic oracles belong once to existing shared validators/resolvers over caller-owned input/context, with independent expectations. Keep missing/miswired callback discovery controls and explicit-context negative controls. No new validator/constructor, installed-global dependence or host storage/readiness claim. |
| Actual CLI / SDK boundaries | Independently verify direct schema-version-1 payload identity, strict object guards, serialization/overlays and shared-validator delegation through existing typed validate/diff/push and actual bounded store extraction, including bidder-settings miswiring. Retain malformed/null/unknown/strict-clear and wrong-app-version refusals under valid SDK envelope/digest; typed tests record exclusion of unrepresentable shapes. No exhaustive field-matrix replay. |
| Host boundary | Under/at/over extraction bounds independently; preboot App, each captured SDK tuple field above accepted funding, missing binding, conflicting/identical initialization and zero store/secret reads on pre-read refusal. Preserve host boot/health fatal-versus-Closed behavior without a new startup DTO or numeric gate. |
| Replacement | Actual lifecycle owners publish A, then B omitting an account/alias/override, and prove A's data is gone. Missing/corrupt/invalid required policy must not fall back or masquerade as a valid fresh/default document. Hold requests on distinct valid generations and prove each uses one coherent snapshot; preserve same-binary/two-config evidence. |
| Resolution (shared owner) | Vary supplied product defaults, host, account-default, selected-account, bidder and alias values independently in shared model/resolver tests. Exhaustively assert empty/false/zero/null behavior by field and valid/malformed explicit clears, including nonempty supplied inherited before-states for every clear case, fallback account identity, independent activity default/rules inheritance, whole-rule-array replacement and separate transport/decoded clamps. Include parent activity default false plus child rules [] and assert that default false survives. |
| Privacy and credentials | Isolate GDPR/account opt-in, COPPA, malformed selected USP, LMT and activity decisions. Preserve cookie-key rotation and hostile secret-in-flag redaction controls. A missing/corrupt/unknown policy cannot grant via defaults or another source. |
| Resource ownership | Independent expected deltas for actual SDK/request terms and multiplicities; distinct envelope/blob/value caps, asymmetric call/bidder limits, overlapping request views, admission refusal, cancellation/final release, boot-vs-report parity and both ceilings. Actual host allocation ingestion/adoption proofs remain #135. |
| Adapter parity | Each actual adapter port proves one unchanged artifact with two isolated runtime policies using its host-owned bindings/limits. These remain child-task acceptance; native tests are not guest execution. |
| Actual embedding (#135, separate) | The selected host artifact must prove ingestion/adoption capacity/lifetimes and use host settings/services without a mandatory Auctioneer store, second App or HTTP hop. Compare controlled engine results; PR #140 typed semantics are not this acceptance. |

Use existing fake bidders for shared infrastructure. Unit tests stay inline;
integration and conformance tests remain with their owner. Preserve allocator,
process-global and provider lifecycle isolation. No ignored failures, lighter
workload, increased ceilings, self-derived expected bounds or workflow-source
snapshots are introduced to make the migration pass.

## 13. Execution Boundaries And Review Gates

PR #139 merged the original design. The explicit 2026-11-04 maintainer direction
accepts the revised single-runtime scope for the existing PR #140; no new spec
or design PR is required. This document edit does not complete an issue or
establish runtime/deployment acceptance.

| Issue | Deliverable / dependency |
| --- | --- |
| #126 | Prepare reviewed operator-neutral defaults and remove build-time operator-file access, coordinating the optional sync-service effective model with #127. No intermediate serving cutover may install invalid/fictional defaults. |
| #127 | One strict runtime model, shared pure validation and explicit resolution; add account-default coverage and the hard schema cutover. Depends on product defaults. |
| #128 | Actual runtime reader cutover through bounded SDK typed extraction and shared validation; preserve existing host boot order, bindings and installed limits. No startup config, source modes or loader. |
| #130 | Preserve the shared budget derivation, independent product-default storage and actual policy/validation/retention owners within existing limits; no new numeric startup package. |
| #129 | CLI authoring, validation, explicit clear/cutover instructions and export/push behavior using the same admitted types; it must not inject operator data during release builds or add permanent legacy serving aliases. |
| #131 / #132 / #133 / #134 | Separate actual adapter/provider ports and unchanged-artifact acceptance using host-owned deployment surfaces. Compile-time shared-caller edits do not close these tasks. |
| #135 | Actual host ingestion/adoption, allocation/capacity/lifetime proof, services and SDK compatibility with #34 using the shared typed-policy validator/resolvers; no new snapshot publisher or readiness claim from PR #140 semantics. |
| #136 | Operator-neutral release, deployment and cutover documentation matching implemented behavior and actual selected-path evidence. |

PR #140 acceptance requires actual reader/default/CLI cutover, once-owned shared
semantic oracles, actual CLI/SDK payload guards and validator delegation,
lifecycle A/B/no-fallback controls, preserved bounds/accounting and
same-binary/two-config evidence. This scope alignment completes no pending check.
Provider/host adoption retains its own ingestion and physical-fit evidence;
unknown SDK facts are child prerequisites, never invented hooks. The superseded
startup input/parser/numeric-reservation package is not an active review gate.

## 14. Historical Design Verification And Limits

The following records the 2026-11-02 design-only review, not current approval
status. Its startup-package assumptions are superseded by the 2026-11-04 scope
above. Historical checks do not verify the current document revision or runtime.

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

- [Authoring file awaiting cutover](../../../auctioneer.toml),
  [staged source wrappers to retire](../../../crates/auctioneer-config/src/lib.rs),
  [canonical policy model](../../../crates/auctioneer-config/src/policy.rs),
  [build and generation owner](../../../crates/auctioneer-config-data/build.rs).
- [Effective DTOs](../../../crates/auctioneer-config/src/types.rs),
  [runtime DTO and effective validation](../../../crates/auctioneer-core/src/config/mod.rs),
  [resolution](../../../crates/auctioneer-core/src/config/resolve.rs),
  [response-cap precedence](../../../crates/auctioneer-core/src/config/budgets.rs).
- [Product-data exports and bootstrap retirement owner](../../../crates/auctioneer-config-data/src/lib.rs),
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
