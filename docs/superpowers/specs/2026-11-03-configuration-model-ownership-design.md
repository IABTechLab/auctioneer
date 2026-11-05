# Configuration Model Ownership

Status: approved dependency amendment to the operator-configuration design,
2026-11-03.
The maintainer approved consolidating shared configuration types and retaining
bidder-specific configuration with its bidder, then explicitly approved the
config/config-data package-role rename below. This supersedes the earlier
request to retain package roles. No serving or resource approval is implied.

## Goal

Declare Auctioneer-wide operator, host/account and effective
policy models together in `auctioneer-config`. Keep bidder-owned settings and
their validation in `auctioneer-bidder`. Core owns resolution, semantic admission
and lifecycle orchestration, not another copy of those data models.

This amends the file/ownership mapping in the
[operator configuration design](2026-11-02-operator-runtime-configuration-design.md)
and its implementation plan. The 2026-11-04 direction supersedes the earlier
startup/storage package: one direct `schema_version = 1` policy, existing typed
SDK commands and bounded extraction. No startup/root/source-mode model remains.
Runtime bounds and request-retention controls remain required; actual host
ingestion/adoption and capacity/lifetime proof stay in #135.

## Why A Direct Move Is Circular

Today `auctioneer-config` generates product data in `build.rs`, which consumes
`auctioneer-config-schema` with `build-support`. That schema crate also declares
the shared effective types and validators used by the renderers. Moving those
types into today's config library would require its build dependency to depend
back on the library being built. Importing its own library, duplicating the
models, or compiling their source through `include!` does not solve the cycle.

Core's plain operator models can move independently, but doing only that would
leave the shared model split. A facade reexport also does not change the actual
declaration owner.

## Chosen Package Roles

Reuse the existing two packages; do not add a third package:

| Current package | Destination role |
| --- | --- |
| `auctioneer-config-schema` | `auctioneer-config`: canonical model library and optional native build support; no build script |
| `auctioneer-config` | `auctioneer-config-data`: compiled product defaults, immutable bidder metadata, disclosure tables and GVL/GCL archive; existing build script |

The old `auctioneer-config-schema` package and directory name disappear at this
refactor's cutover. There is no package alias retaining that retired interface.
Workspace package/dependency metadata remains inherited. This is a role/name
change to two existing packages, not crate proliferation.

The smaller alternative is to make config-schema the canonical model owner and
retain today's package roles. It avoids data-path migration but leaves the
package named config owning generated data rather than configuration models.
Use the roles above to make names match the maintainer-approved boundary.

### Dependency Direction

```text
auctioneer-config       -> serde, validator, existing portable helpers/runtime
auctioneer-config-data  -> auctioneer-config
                        -> auctioneer-config(build-support) during its build
auctioneer-bidder       -> auctioneer-config + auctioneer-config-data
                        -> auctioneer-config-data during its build
auctioneer-core         -> auctioneer-config + auctioneer-config-data + bidder
adapter / CLI          -> their existing engine integration and shared models
```

`auctioneer-config` must not depend on generated product data, bidder
implementations, core, provider adapters or provider SDKs. Keep native parser,
source-analysis and generation dependencies behind the existing build-support
boundary; model/serving use must not activate them accidentally. No new feature
is needed to choose between two configuration mechanisms.

## Model And Data Owners

- Move shared source/policy/override declarations, field-override serialization,
  floor-currency configuration and pure scalar/shape validation into config.
  Effective host/account/privacy/activity/sync types already in the old schema
  crate stay with that library as its role/name changes.
- Keep build-only raw DTOs and quote/syn renderers in config's native build
  support. They are not a second runtime policy model. Preserve actual renderer
  validation, including callback/URL and privacy-key checks.
- Config-data owns generated values, lookup tables and embedded external data,
  not operator-policy declarations. Generic immutable metadata descriptors may
  be shared config types; their bidder-owned manifest source remains unchanged.
- Core retains explicit validation contexts, request-owned effective state,
  resolvers and errors involving registry/admission services. These are engine
  state, not independently authorable settings. Runtime budget derivations and
  SDK/provider types retain their existing owners; do not duplicate them here.
- Adapter bindings, physical facts and installed limits remain host-owned.
  They do not translate into a new startup DTO. The shared typed-policy semantic
  validator/resolvers use caller-owned input/context; actual host adoption is
  #135, not a new constructor, storage proof or serving-readiness claim here.

### Bidder-Owned Settings

Each bidder's `bidder.toml` remains the authoritative declaration of its own
host settings, alongside its local borrowed `validator::Validate` view and
protocol implementation. Do not add a central bidder-name list or validator.

Move the generated settings and endpoint-binding modules, including their
bounded structural/credential-reference helpers, from today's config crate
into auctioneer-bidder. Its normal build discovery emits these typed aggregates
from bidder-owned manifests. Generic immutable endpoint/metadata descriptors
remain shared; vendor-specific generated bindings do not live in config-data.
Retain the complete unfiltered catalogue for these renderers. Current discovery
consumes that catalogue and keeps only a reduced factory view; extend its owned
input/result to retain or hand off the complete manifest/field information,
rather than attempting to regenerate settings from that reduced view. Reuse
the same scanned catalogue, not another handwritten bidder list or a second
interpretation. Reference-only and inactive declarations still generate types;
factory, feature and target/publication eligibility remain independent gates.

The bidder build continues to consume config-data's metadata declarations for
reconciliation, registry generation and schema staging. Update the explicit
metadata-feature reconciliation edge to `auctioneer-config-data/<feature>`;
changing imports alone is insufficient. Preserve settings/endpoint controls
for reference-only, feature-disabled and target-ineligible declarations without
making their implementations routable.

The generated aggregate supplies one structural `Validate` implementation using
the existing limits. Shared orchestration invokes owner callbacks with their
existing requested/base/definition/enablement context for semantic validation.
Do not run the same structural scan twice after adding that implementation.

Settings-bearing shared models use a type parameter, for example
`OperatorPolicy<Settings>` and `BidderOverrides<Settings>`. They never depend on
a concrete bidder implementation or accept opaque arbitrary JSON instead.
Core binds the parameter to the discovered `BidderSettings` type once. Such a
type alias is composition, not another declaration of the fields or rules.
Default/Serde implementations must not invent a `Settings: Default` requirement
merely because an optional settings field or an empty map defaults.

Validation must work with the model feature alone, without build-support.
Activate the existing validator dependency in that feature. State the actual
`Settings: Validate` bounds on generic validation implementations or their
declarations; validator's derive copies declared bounds and does not infer this
constraint. Retain derive-based scalar rules rather than rewriting their
ranges. Model-only tests use a non-Default synthetic settings type with an
observable rejecting validator: a supplied entry is visited once, omission is
not visited, and the resulting nested errors contain only fixed codes and no
input parameters. Removing delegation must fail that control. Do not remove
independent per-request/programmatic binding or resource checks when eliminating
duplicate scans inside a single configuration-admission invocation.

Keep the common models independent of SDK traits. A transparent core integration
wrapper can implement those traits when orphan rules prevent implementing them
for a composed foreign model. This is preferable to pulling provider/SDK or
generated-data dependencies into native model build support.

Pure validation follows its model owner. Registry/default-dependent inherent
methods currently in core become orchestration functions; they cannot remain
foreign inherent implementations after a type move. Use a thin transparent
SDK-integration wrapper only where Rust trait ownership requires it. It must
contain the shared model, not repeat its fields or implement a second parser.
Preserve the actual secret-field metadata; do not assume an empty metadata list
without checking the existing contract.

## Migration And Safety

Perform declaration/data/settings moves as a coordinated compiling group on
the existing implementation branch. Keep actual old serving behavior until
the operator-runtime plan's separately approved reader cutover. A temporary
development commit is not a supported dual-format release.

Update root/workspace manifests, feature forwarding, code generation, explicit
compiler-fixture closures, maintenance CLI dependencies, ordinary imports and
active scripts/docs together. Source tests move with their owning models;
compiled-value and build-failure tests move with config-data. Bidder conformance
and generated settings contracts remain bidder-owned. Preserve allocator,
process-global and child-build isolation and the existing once-owned CI lanes.

Split today's mixed config build-failure suite deliberately. Data-generation
failures stay with config-data. Its settings/endpoint compiler consumers move
to the existing bidder compiler owner: they must obtain those outputs from
bidder's OUT_DIR, not the new data OUT_DIR. Construct the source tree in that
owner's disposable workspace, so bidder's fixed-root discovery and the data
builder's fixture selection see the same owned tree; do not add a production
build-source override. Preserve empty/reference-only/inactive inputs, namespace
collisions, insertion-order stability and credential-reference refusals. Rewire
that source closure before executing migrated compiler cases, not afterward.

Active archive readers include the CLI GVL maintenance/refresh modules, the
OpenRTB/catalogue maintenance paths, and the existing refresh/check scripts.
Update their current asset locations. The maintenance-only CLI must retain no
dependency on config-data's build script; reading the archive as an explicit
maintenance input is not such a dependency. Relocate archive files byte-for-byte.

Do not change serialized fields, null/omission/clear behavior, privacy rules,
URL safety, callback dispatch, limits, defaults, source pins or the resource
equation. Qualification records and external captures retain original bytes;
old package names in historical captures are data, not compatibility APIs.
Active current-source references change. Old records do not qualify a new
artifact merely because this is intended to be a behavior-preserving move.

## Acceptance

1. All shared configuration data declarations have one source owner in config;
   core/adapters compose or consume them, and bidder settings remain bidder-owned.
2. Actual Cargo metadata shows no config-to-data/bidder/core/provider cycle or
   native build-support activation in the serving model graph. Package count
   is unchanged and no handwritten source import is introduced.
3. The existing JSON/TOML and typed-setting controls retain strict shape,
   independent fallback, fixed/redacted errors and exact callback dispatch.
4. Record before/after executed case names and ownership. A moved or filtered
   case must not disappear; a skipped feature target is not a pass.
5. Independently guarded layouts/allocation controls, default snapshots,
   actual generated-settings child contracts and source-isolation controls
   pass. No ceiling or observation is widened to absorb a regression.
6. CLI normal/maintenance feature lanes and all adapter compile/lint lanes
   retain their previous dependency/feature guarantees. Provider execution and
   complete operator-runtime acceptance remain separate, not inferred from this.

Implementation plan:
[configuration model ownership](../plans/2026-11-03-configuration-model-ownership.md).
