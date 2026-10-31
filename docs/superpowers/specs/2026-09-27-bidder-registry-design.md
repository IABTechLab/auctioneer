# Automatic Bidder Registration

Date: 2026-09-27
Status: Draft for user review; design direction approved
Worktree: `.claude/worktrees/m4-privacy` (`m4-privacy` branch)

M7 refinement (2026-10-16): the
[catalog specification](2026-10-16-m7-bidder-catalog-design.md) carries this
unimplemented direction forward using the current `src/bidders/<bidder_key>/mod.rs`
layout and adjacent `bidder.toml` declarations with `create_bidder()` factories.
All bidder implementations remain in the existing `auctioneer-bidder` crate,
with public concrete types named `FooBidder` and no compatibility aliases.
It also specifies opt-in defaults for newly imported definitions, complete
catalog/alias accounting,
schema/protocol foundations, and explicit rejection of runtime aliases
shadowing compiled public identities. Those refinements supersede the original
flat-file discovery proposal, new-bidder defaults, and alias-shadowing
behavior in this draft; the declaration and helper names below follow M7.
Neither document claims that discovery has already shipped.

## 1. Purpose and Boundary

Remove the manually maintained mapping from public bidder names to bidder
implementations. Discover bidder registrations during the build, assemble
one validated registry during application initialization, and retain the
existing request-scoped runtime controls.

Here, "dynamic registry" means automatic registration of compiled code,
not loading code after deployment or mutating a global registry when
configuration changes.

| Change                                                                             | Rebuild/deploy? | Mechanism                                               |
| ---------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------- |
| Enable or disable an already shipped, baked-enabled bidder                         | No              | Existing runtime config push                            |
| Change its runtime timeout or sampling setting                                     | No              | Existing runtime config push                            |
| Add a host alias to an existing base bidder                                        | No              | Existing validated runtime alias map                    |
| Add a new bidder implementation                                                    | Yes             | Bidder source, local declaration, metadata, tests       |
| Add a new base bidder using an existing implementation                             | Yes             | New baked metadata with explicit implementation binding |
| Change endpoint, vendor ID, capabilities, sync metadata, or implementation binding | Yes             | Reviewed source/metadata change                         |
| Override a baked `disabled = true` safety switch                                   | Yes             | Reviewed baked metadata change                          |

Operators do not edit Rust or rebuild to activate an eligible bidder that
the artifact already contains. Developers may change code and rebuild for
new integrations, as agreed. No new workspace crate, EdgeZero change,
runtime plugin loader, admin registration API, or remote metadata store is
part of this work.

This is a focused architecture improvement, not a reduction of M4 privacy
scope and not a new production-certification milestone.

## 2. Baseline and Existing Behavior

The inspected application baseline is
`d1f1bde25b02169b8eba04ce0dca23061c757203`; worktree HEAD is `d7b0d88`,
which adds the initial M4 design. Relevant sources:

- [Master design](2026-05-21-auctioneer-openrtb-server-design.md), sections 2.1,
  5.9, 6, and 9.1: dependency boundaries, deterministic ordering, baked
  metadata, and runtime overlays.
- [Config generation](../../../crates/auctioneer-config/build.rs): already
  discovers bidder-info TOMLs and emits metadata accessors.
- [Core registry](../../../crates/auctioneer-core/src/bidder_registry.rs):
  chooses implementations with a hard-coded name match; unknown names
  silently select `GenericBidder`. It also clones and leaks metadata
  even though the generated metadata already has a static lifetime.
- [Bidder modules](../../../crates/auctioneer-bidder/src/bidders/mod.rs):
  manually declares implementation modules alongside the shared `url`
  helper.
- [Registry type](../../../crates/auctioneer-bidder/src/registry.rs): stores
  entries in a `BTreeMap`; duplicate registration currently overwrites.
- [Application configuration](../../../crates/auctioneer-core/src/configure.rs)
  and [info handlers](../../../crates/auctioneer-core/src/handlers/info_bidders.rs):
  initialization and observable catalog integration points.

The master design already prohibits a config-to-bidder dependency cycle.
This design preserves that boundary. The M7 refinement above now owns the
specific registration contract; implementation must update the master
design's registry/code-generation descriptions and the architecture guide
to distinguish generated factories from generated metadata.

## 3. Chosen Approach

Use a build-generated bidder factory catalog, paired with explicit
bindings in baked bidder metadata.

Alternatives considered:

- A central macro or registration function is simple, but still requires
  editing a central list for each new bidder implementation.
- Linker-based self-registration removes that list, but adds another
  discovery mechanism whose behavior must be qualified across all target
  runtimes. This project already uses build-time generation.
- Generated registration follows that existing pattern, produces ordinary
  Rust modules and function references, and requires no runtime discovery
  machinery. This is the selected approach.

The build reads committed files only. At runtime, startup joins baked
bidder metadata to compiled factories; requests apply runtime policy to
the resulting immutable registry. No build-time factory is executed and
no runtime file scan occurs.

## 4. Identities and Ownership

Keep these identities distinct:

| Identity                            | Meaning                                            | Example         |
| ----------------------------------- | -------------------------------------------------- | --------------- |
| Implementation key (`<bidder_key>`) | Internal implementation selector                   | `generic`       |
| Base bidder name                    | Baked integration identity and metadata owner      | `example_ssp`   |
| Requested bidder name               | Base name or validated alias visible in an auction | `publisher_ssp` |

Several base bidders may bind to the same implementation key. Each still has
its own endpoint, GVL vendor ID, capabilities, sync metadata, disabled state,
and runtime settings. Implementation keys must never replace public bidder
names in privacy evaluation, alias resolution, bid seats, targeting, warnings,
metrics, or bidder-count limits.

| Component         | Responsibility                                                                                                                          |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `auctioneer-config`   | Parse and validate metadata/binding syntax; generate data-only accessors                                                                |
| `auctioneer-bidder`   | Discover bidder declarations at build time; expose compiled factories; validate and assemble registry instances                         |
| `auctioneer-core`     | Initialize/cache the production registry; propagate initialization errors; supply the same catalog to config, auctions, and info routes |
| `auctioneer-exchange` | Consume the existing registry and request-scoped policy; no discovery logic                                                             |
| Provider adapters | Continue building the shared application; no provider-specific bidder lists                                                             |

Keep the dependency direction `auctioneer-bidder -> auctioneer-config`.
`auctioneer-config` must not import bidder implementations, including through
build dependencies. Cross-catalog validation happens in `auctioneer-bidder`,
not by making the config build script link application code.

## 5. Bidder Discovery Contract

### 5.1 Local Declarations

M7 supersedes the original flat-file discovery proposal. Each implementation
has an adjacent `bidder.toml` declaration under
`crates/auctioneer-bidder/src/bidders/<bidder_key>/mod.rs`:

```text
appnexus/mod.rs
appnexus/bidder.toml
generic/mod.rs
generic/bidder.toml
mocktioneer/mod.rs
mocktioneer/bidder.toml
pubmatic/mod.rs
pubmatic/bidder.toml
url.rs
```

Example `appnexus/bidder.toml`:

```toml
key = "appnexus"
```

Example `mocktioneer/bidder.toml`:

```toml
key = "mocktioneer"
required_feature = "insecure-bidders"
```

Declarations are strict TOML: only `key` and optional `required_feature`
are accepted. `key` is nonempty, matches the directory name, and is a
lowercase Rust module identifier, excluding keywords. The corresponding
`<bidder_key>/mod.rs` must exist. Paths, constructor expressions, URLs, and
arbitrary Rust tokens are not declaration fields. A declared feature must exist in
this crate's Cargo feature table; unknown features are errors, not silently
disabled bidders.

The deterministic source-directory-to-key mapping in M7 section 5.1 preserves
source paths, prefixes digit-leading results with `bidder_`, and suffixes Rust
keywords with `_bidder`; it never derives implementation keys from public names.

Every immediate implementation directory requires both `bidder.toml` and
`mod.rs`; a missing member fails the build. Helpers such as `url.rs` and
`media_type.rs` remain ordinary modules and are not discovered as bidders.
A declaration that collides with a manually declared helper module is a build error.
Production discovery is restricted to this committed source directory;
do not add an environment-controlled production plugin path.

Each bidder module provides one conventional factory:

```rust
pub(super) fn create_bidder() -> Box<dyn crate::Bidder>;
```

The factory is infallible, synchronous, and performs no I/O. It must not
read request/config-store data, initialize networking, or retain user
state. Existing `Bidder: Send + Sync` and request/response conversion
contracts remain unchanged. Nontrivial future constructor requirements
need a deliberate contract change, not hidden fallbacks or panics.

### 5.2 Generated Catalog

A new `auctioneer-bidder/build.rs` parses declarations using structured TOML
APIs and emits module declarations plus a sorted factory table into
`OUT_DIR`. It does not parse Rust source to discover trait implementations.
The Rust compiler checks the factory signature and trait bounds.

The public catalog contract is equivalent to:

```rust
pub struct BidderRegistration {
    pub key: &'static str,
    pub create: fn() -> Box<dyn Bidder>,
}

pub fn compiled_bidders() -> &'static [BidderRegistration];
```

Generate both implementation-module inclusion and factory-table entries;
moving only the name match to another hand-maintained file does not meet
the requirement. Public concrete types and intentional re-exports use
`FooBidder`, with no old-name compatibility aliases. Re-exports are not the
registration source, and new bidders must work without adding them.

Validate declaration shape and duplicate keys before applying feature
selection. Apply the same feature predicate to the generated module and
its factory entry. An inactive bidder implementation is not compiled or
included in the factory table. With default features, Mocktioneer must remain absent from
both generated factories and baked bidder metadata; the existing
`insecure-bidders` forwarding chain enables both together.

Sort by implementation key, independent of filesystem iteration order. Register
Cargo change tracking for the discovery directory, declaration files,
source files, and relevant manifest inputs. Adding, editing, renaming, or
removing a bidder implementation must update an incremental build without
`cargo clean`. Feature changes must also regenerate the correct catalog. Generated
output stays in `OUT_DIR`, never in the checked-in source tree.

Extract pure parsing/validation/rendering helpers for tests, following the
existing config codegen pattern. Test discovery against temporary fixture
trees without modifying the developer's live source directory.

## 6. Explicit Metadata Bindings

Every baked bidder-info TOML gains a required Auctioneer-owned top-level
`implementation = "<bidder_key>"` field, distinct from the public name. For
example, the public `appnexus` definition declares:

```toml
implementation = "appnexus"
endpoint = "https://ib.adnxs.com/openrtb2"
gvlVendorID = 32
```

A new `example_ssp.toml` may explicitly use `implementation = "generic"` while
retaining its own public bidder name and metadata. Public `yahooAds` instead
binds to `implementation = "yahoo_ads"`. Reusing the generic bidder
implementation does not make an arbitrary provider protocol compatible; that integration
still needs reviewed metadata and protocol fixtures.

Missing, empty, or syntactically invalid bindings fail metadata generation.
Do not infer a binding from the bidder name or silently select `generic`.
Reject duplicate bidder names across the active secure/insecure inputs
before emitting metadata or validating host aliases.

Keep `BidderInfo`, `bidders()`, and `bidder(name)` as the existing metadata
surface. Generate a separate data-only binding table from the same parsed
records, with a contract equivalent to:

```rust
pub struct BidderImplementationBinding {
    pub bidder: &'static str,
    pub implementation: &'static str,
}

pub fn bidder_implementation_bindings() -> &'static [BidderImplementationBinding];
```

Here `bidder` is the exact public name and `implementation` is the internal
implementation key, never a replacement public identity.

This avoids adding internal constructor selection to public bidder-info
JSON or changing the runtime configuration schema. Both generated tables
must describe exactly the same active bidder set in lexicographic order.
Metadata generation validates binding syntax; registry assembly validates
that the selected implementation actually exists in the compiled catalog.

The metadata remains baked, including endpoints, endpoint macros, host
allowlists, compression, privacy/vendor data, capabilities, and syncers.
Neither runtime overlays nor request extensions can replace `implementation` or
introduce a base bidder. Existing test-only metadata-directory overrides
remain test-only and must produce matching metadata and binding tables.

## 7. Registry Assembly and Initialization

### 7.1 All-or-Nothing Construction

`auctioneer-bidder` owns a fallible assembly function accepting static bidder
metadata, its bindings, and a factory catalog. Keep this independently
testable without initializing the production singleton.

Before invoking factories, validate:

1. Bidder names and factory keys are unique.
2. Every bidder has exactly one binding; no binding names a missing bidder.
3. Every binding resolves to one available compiled factory.

Validate disabled bidders too. A bad binding must not hide until an
operator enables that bidder. An unused factory is allowed and does not
create a bidder, an info entry, or an outbound destination by itself.

Only after full validation succeeds, instantiate each bound bidder once
in bidder-name order. Separate base bidders receive separate bidder
instances even when they share a factory. Borrow the existing
`&'static BidderInfo`; do not clone and `Box::leak` production metadata.
Keep lexicographic registry ordering and existing lookup semantics.

Make duplicate `BidderRegistry::register` calls return an error without
overwriting the incumbent. Preserve explicit construction for unit tests
and embedded exchange callers; those callers may inject dummy bidders
without using generated discovery. Production always uses the validated
catalog join.

Use typed initialization errors for duplicate bidders/factories/bindings,
missing bindings, bindings without metadata, and unavailable implementation
keys. Diagnostics may identify the committed public bidder names and
implementation keys. Do not dump metadata records, endpoint credentials,
configuration blobs, or user data. Never repair these errors by dropping a
bidder or substituting a
bidder implementation.

### 7.2 Shared Application Integration

Replace the match in `auctioneer-core/src/bidder_registry.rs` with the generic
assembly call. Cache its success or failure once per process/isolate,
using a `OnceLock<Result<BidderRegistry, RegistryBuildError>>` or equivalent
fallible singleton. Accessors must propagate a cached failure, not panic,
retry indefinitely, or expose a partially populated registry.

The shared `configure_app` callback validates/initializes the catalog
before an application can dispatch handlers. Initialization is idempotent
when serverless providers build an app per request; concurrent builds
construct at most one production registry. An invalid catalog rejects app
initialization through the existing framework error path. It is an artifact
error, not a bidder-local auction error. Any caller-visible error rendering
must remain bounded and omit internal diagnostics.

Update `live_names`, the metadata-list helper, runtime host-alias
validation, auction handling, and info handling to consume this same
validated result. Helpers used directly in tests must retain the failure
contract even when app construction is bypassed. Do not introduce a
second registry or independently reconstruct an advertised bidder list.

This does not alter provider admission/egress behavior or claim that app
construction occurs before raw ingress on every provider. Verify the
shared application-construction path on Axum, Cloudflare, Fastly, and Spin;
no provider owns a separate registration mechanism.

The registry is immutable after publication. Runtime config refreshes do
not construct bidder implementations, rebuild maps, or change global
membership. Count its one-time metadata/index/instance allocation in the existing startup
baseline; do not add discovery or constructors to the per-auction path.

## 8. Runtime and Observable Behavior

Retain the current `AuctioneerAppConfig` bidder overrides: `enabled`,
`timeout_ms`, and `sample_rate`, plus the existing host alias map. An
operator can disable a compiled, baked-enabled bidder with:

```toml
[runtime.bidders.appnexus]
enabled = false
```

Setting it to `true`, or removing that runtime disable, permits the bidder
again without rebuilding. The usual eligibility, account, media-type,
privacy, sampling, and budget checks still apply. "Enabled" is not a
promise that every request is sent to the bidder.

Preserve current precedence: `BidderInfo.disabled = true` always wins over
runtime enablement. For an existing integration operators intend to toggle,
ship baked `disabled = false` and use the runtime override as the switch.
Do not change the default-enabled behavior for omitted overrides in this
work. This avoids silently changing existing deployments' bidder sets.

Aliases are request/config names resolved to existing base bidders, not
new compiled implementations. Base disables affect aliases; an alias-only
disable does not disable its base or sibling aliases. Keep current alias
validation, timeout/sampling resolution, and requested/core-name rules.
Do not add a blanket rejection of unknown runtime override names here:
some overrides can refer to aliases declared by a later auction request.
Such an override never creates a base bidder.

Auctions and `/info/bidders` surfaces retain the same resolution policy,
ordering, filters, and public shapes. Disabled entries stay in the compiled
catalog; `enabledonly` controls visibility rather than physical registry
membership. Do not expose factory keys as bidder names or as new info JSON
fields. Request-defined aliases remain request-local.

Use the existing config push and request-snapshot semantics. A request
already using an older snapshot may finish with it; this work adds no
global hot-reload event or immediate cross-isolate propagation guarantee.
Existing fail-closed config behavior and privacy/SSRF checks are unchanged.

## 9. Migration and Developer Workflow

Migrate these bindings explicitly:

| Public bidder name | Implementation key | Availability            |
| ------------------ | ------------------ | ----------------------- |
| `appnexus`         | `appnexus`         | Default                 |
| `pubmatic`         | `pubmatic`         | Default                 |
| `generic`          | `generic`          | Default                 |
| `mocktioneer`      | `mocktioneer`      | `insecure-bidders` only |

Add declarations/factories for the existing bidder implementations, update
metadata and metadata fixtures with bindings, introduce generation/validation, and
remove the manual implementation match/module list in the same coherent
change. Update concrete-type consumers to `FooBidder` without compatibility
aliases. Existing runtime config blobs need no schema migration or repush.

To add a new implementation, a developer adds its module, adjacent
declaration, bidder-info TOML, and integration tests, then rebuilds and
deploys. No edits to a central factory match, bidder-module list, core
handler, or provider adapter are required. A genuinely new dependency or
Cargo feature may still require a manifest change; automatic registration
does not automate dependency management.

To add a new base bidder using `generic` or another existing implementation,
add metadata with an explicit binding and integration tests, then rebuild.
This does not require a new factory declaration. To activate a bidder
already in the artifact, only push the existing runtime configuration.

Keep onboarding instructions next to the bidder crate. Update the master
design sections 2.1 and 6 and the architecture guide during implementation.
Document required `implementation` fields for downstream maintained bidder-info
files; removing the implicit generic fallback is an intentional build/
initialization compatibility break, not a runtime wire-format change.

## 10. Verification and Acceptance

The implementation plan must include these focused checks:

| Surface                      | Required evidence                                                                                                                                                                     |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Automatic discovery          | A temporary fixture adds source, declaration, and metadata; the bidder is constructed and used without changing any central registry/module list                                      |
| Declaration validation       | Malformed/unknown fields, invalid identifiers, mismatched directory keys, missing source, duplicate keys, helper collisions, and unknown features fail clearly                        |
| Compiler contract            | Wrong/missing factory signature fails compilation; factory table retains trait bounds                                                                                                 |
| Incremental builds           | Add, edit, rename, remove, and toggle a feature using a reused target directory; output/catalog follows inputs without a clean build                                                  |
| Metadata generation          | Required binding syntax and duplicate bidder names are checked; info and binding tables have identical membership/order                                                               |
| Catalog join                 | Missing/duplicate/dangling bindings and missing factories fail before any factory is called, including for disabled bidders                                                           |
| No implicit generic          | Misspelled implementation keys fail; only an explicit `generic` binding selects that implementation                                                                                   |
| Shared implementation        | Two bidders using one factory retain distinct metadata, instances, vendor identity, endpoint selection, and response attribution                                                      |
| Registry safety              | Duplicate insertion leaves the old entry intact; ordering is deterministic; production metadata is borrowed                                                                           |
| Initialization               | Repeated/concurrent app construction is idempotent; failures are cached, prevent dispatch, and expose no partial catalog or unbounded error body                                      |
| Direct helper calls          | With app construction bypassed, catalog access and runtime alias validation propagate a cached registry error without panicking, retrying construction, or returning an empty catalog |
| Runtime toggles              | Disable and re-enable without rebuilding or reconstructing bidder implementations; in-flight requests keep their resolved snapshot                                                    |
| Precedence/aliases           | Baked disable wins, base/alias switches behave as before, and aliases do not create factories or mutate global membership                                                             |
| Public catalog               | Auction, runtime alias validation, list/detail/all info views agree; existing filters/order/JSON remain stable; factory keys are not exposed                                          |
| Feature parity               | Default builds omit Mocktioneer metadata and implementation; feature-on builds include both; a deliberately mismatched pair fails initialization                                      |
| Transport/privacy regression | Existing bidder request/response fixtures, privacy scrubs, endpoint validation, sampling, timeout, and deterministic cap behavior still pass                                          |
| Portability                  | Native Axum tests plus actual Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1`, and Spin `wasm32-wasip2` application builds; check default and insecure feature catalogs   |

Fixture generators and source-directory substitution are test harness
facilities, not production configuration. Existing config build-failure
fixtures must receive valid bindings so unrelated negative cases continue
to test their intended validation rule. Add a production-catalog test that
joins the actual generated metadata and factories, not only synthetic data.

Native tests exercise behavior; cross-target compilation proves build
compatibility only, not deployed provider behavior. Keep the existing
provider smoke/conformance coverage and do not claim certification from
these checks.

Completion means no central implementation-selection list is required,
all shipped bindings validate, runtime toggles remain rebuild-free, and
the above regression/portability checks pass. This document defines the
work; it does not claim the registry has already been changed.
