# bidder catalog: Bidder Foundations and 14 Priority Flows

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

**Current acceptance amendment (AG approved 2026-10-30):**
[controlled-native delivery](2026-10-30-controlled-native-bidders-completion-design.md)
supersedes this document's proof-before-activation and identical-target-publication
requirements for bidder catalog. All 14 priority flows must pass native handler acceptance
with enforced limits and predeclared tested reservations. Complete worst-case
schema resource proofs move to resource qualification; new publications remain excluded from
guests. Source/protocol/privacy requirements and workload/ceilings are unchanged.

Date: 2026-10-16
Status: Implementation started; offline reference inventory slice in progress
Application baseline: `f870aaca1999cbd46ff1ae16905975f5dfd28e8a` (merged diagnostic qualification)
Branch: `bidder-catalog`, specification and SDK-baseline update against `main`
SDK baseline: [IABTechLab/edgezero PR #275](https://github.com/IABTechLab/edgezero/pull/275),
`f6c708ccb5be19ba9fa92575db3c08c0f17e0d72`; existing Rust/tool pins remain aligned.

The SDK transfer update does not implement bidder catalog foundations or requalify
historical diagnostic qualification resource evidence. Its exact dependency/CI/adapter checks
are recorded separately from catalog acceptance.

Implementation sequencing and proposed qualification limits are in the
[bidder catalog implementation plan](../plans/2026-10-16-bidder-catalog.md).

## 1. Goal and Delivery Order

Provide catalog-scale foundations and all 14 priority selected flows as
reviewed Rust code and metadata, without a hand-maintained central
registration list. Retain the full pinned catalog contract for remaining catalog closure.
Keep bidder migration, shared-feature support, and live certification as
separate claims.

### 2026-10-29 Shipped-Schema Qualification

AG approved focusing qualification on completing bidder catalog and assigning unnecessary
catalog-wide work to later capabilities. The
[shipped-schema amendment](2026-10-29-shipped-schema-qualification-design.md)
defines `S` for each adapter/feature lane: existing published bindings plus
the planned priority wave, closed over aliases, references and shared helpers.
Runtime-disabled shipped rows remain funded. An imported source-only schema
does not need serving-code emission or complete engine-resource qualification
before bidder catalog can finish.

This scopes sections 7 and 11 and the bidder catalog exit gate; it does not weaken their
required bounds for reachable shipped paths. Use actual artifact/layout and
execution evidence, distinguish proved/enforced/observed facts, and retain
source derivations only for required reachable gaps. Measurements are not
upper-bound descriptors. All-source correctness/provenance controls and the
261-schema inventory remain. Newly reachable schema/engine features belong
to their bidder expansion wave or remaining catalog closure; required bidder catalog work cannot be deferred that way.
resource qualification, Trusted Server embedding and publisher pilot retain their existing target/embedding/deployment owners.
The amendment is a written-review checkpoint, not serving-cutover approval or
evidence of bidder catalog completion.

### 2026-10-26 Scope Correction

AG approved bidder catalog = foundations plus full 14-priority selected-flow acceptance,
including Sharethrough, Rubicon and APS. APS requires its own reviewed
contract/evidence. All existing priority-local branch, schema/resource and
onboarding requirements remain; protocol candidates, imported metadata and a
partial checkpoint cannot close bidder catalog. Required priority-local branches are not
deferred to remaining catalog closure.

This correction initially assigned remaining migration to remaining catalog closure. The sequencing
amendment below brings post-pilot waves into bidder expansion; the plan's retained Tasks
20/21 terminal gates remain remaining catalog closure work and cannot gate bidder catalog closure. The complete
358-source inventory (261 bases, 97 aliases), 261 schemas/base bindings and 259 source
implementations remain required. Default/normal `bidders check`, wave and
full `--require-complete` semantics are unchanged; full completion still
requires every source row/branch and belongs to remaining catalog closure. No implementation or
qualification is claimed by this correction, and historical records remain
evidence of their original scopes.

Use demonstrated publisher demand/adoption to review exact membership in
smaller waves; the [roadmap](../../roadmap.md#demand-led-waves-and-remaining-closure)
assigns post-pilot expansion to bidder expansion and residual categories to remaining catalog closure. These are
not popularity claims. Trusted Server embedding/publisher pilot and stored requests through browser privacy compatibility IDs and requirements, and all resource qualification
boundaries below, remain unchanged.

### 2026-10-27 Post-Pilot Expansion

AG approved bidder expansion for additional bidder waves after the selected Trusted
Server/publisher testing under Trusted Server embedding/publisher pilot, before stored requests and accounts onward. bidder catalog retains all
14 priority flows and their existing schema/resource requirements. bidder expansion reuses
the same reviewed protocol, discovery, promotion, resource and evidence paths;
it does not wait for remaining catalog closure or introduce a second catalog/registry.

Freeze a finite, nonempty demand-led wave queue adding accepted flows beyond
the initial 14, with exact source/branch requirements before implementation.
Each wave needs current composed standalone/embedded
acceptance and affected build/resource gates. The earlier pilot's success is
not evidence that a new bidder is ready for traffic. Unsupported shared-feature
dependencies stay explicit and assigned to remaining catalog closure unless separately brought
forward through review. remaining catalog closure retains every residual row and the unchanged
358-source terminal reconciliation; full `--require-complete` need not pass to
close the frozen bidder expansion queue. See the [roadmap acceptance boundary](../../roadmap.md#2026-10-27-post-pilot-bidder-expansion).

The corrected delivery order is:

1. Registration, YAML-to-TOML import, configuration, validation, protocol,
   and resource foundations under bidder catalog.
2. All 14 publisher-priority bidder names in section 4, including APS, and
   bidder catalog closure under section 12.
3. Trusted Server embedding and selected publisher testing under Trusted Server embedding/publisher pilot.
4. Additional integrations in reviewed, demand-led bidder expansion waves after that testing.
5. Residual remaining catalog closure integrations and catalog closure, including later-feature-dependent rows.

This is one program with separately reviewed delivery slices, not one
all-bidders implementation PR. A later slice may need its own protocol
design before implementation. Foundation or partial priority-wave completion
alone is not bidder catalog closure; the complete section 12 bidder catalog acceptance is required.
The sequence expresses delivery priorities; selected embedding/testing may
overlap bidder catalog work under the unchanged prerequisites below.

The existing WASM memory-envelope overcommit is a separate readiness
prerequisite, **resource qualification: WASM Resource Envelope Remediation** (section 11.2).
It may proceed alongside catalog development; it is not hidden work inside
a bidder wave. bidder catalog owns catalog-attributable resource growth, not repairing
every pre-existing target envelope. Catalog migration/build completion and
absolute target memory qualification remain separate results. A green bidder catalog
relative gate cannot qualify an overcommitted WASM target.

Trusted Server embedding and publisher pilot selected-bidder acceptance may proceed once their
own prerequisites and selected bidder wave are ready. They do not wait
for bidder catalog closure or remaining catalog closure full-catalog closure. Credentialed provider execution
remains publisher pilot work; this specification does not move it back into diagnostic qualification or require
credentials to begin bidder development.

Normative parents are the [roadmap](../../roadmap.md),
[master design](2026-05-21-auctioneer-openrtb-server-design.md), and
[registration design](2026-09-27-bidder-registry-design.md). This document
refines the registration design for the inspected source layout, catalog
scale, opt-in defaults, protocol requirements, and staged delivery.
Where these refinements conflict with that older draft, this document
owns the bidder catalog contract. It preserves the existing privacy hardening and
intentional differences unless a reviewed row explicitly changes them.

## 2. Inspected Baseline, Not Implemented Foundations

The application currently has three default implementation modules:
`appnexus`, `generic`, and `pubmatic`. `mocktioneer` is available only
with `insecure-bidders`. Existing modules are not evidence of complete
compatibility with every source bidder branch.

The inspected production paths are:

| Surface                                                           | Current behavior                                                                                               | bidder catalog change                                                                                             |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `auctioneer-core/src/bidder_registry.rs`                              | Name match, generic fallback, cloned/leaked metadata                                                           | Generated factory join; borrow static metadata; fail on invalid bindings                              |
| `auctioneer-bidder/src/bidders/mod.rs`                                | Manual implementation module declarations                                                                      | Generate implementation modules and factories from adjacent declarations                              |
| `auctioneer-bidder/src/registry.rs`                                   | Duplicate insertion overwrites                                                                                 | Typed duplicate error; incumbent remains intact                                                       |
| `auctioneer-config/build_codegen.rs`                                  | Metadata DTO lacks an implementation binding and parameter-schema association                                  | Explicit binding and data-only schema descriptors                                                     |
| `auctioneer-bidder/src/types.rs`                                      | Sealed response parser accepts the standard bid-response envelope                                              | Budget-preserving support for required proprietary JSON envelopes                                     |
| `auctioneer-core/src/usersync/{catalog,choose,evict}.rs`              | Request-scoped catalog; several 256-name assumptions                                                           | Separately bound complete catalog work and caller-controlled lists                                    |
| `auctioneer-config/src/usersync.rs::validate_catalog_size`            | Literal `bidders + aliases > 256` catalog gate                                                                 | Replace this catalog-owned literal with `MAX_CATALOG_IDENTITIES = 512`, with build/runtime accounting |
| `auctioneer-runtime/src/{aux_work,budget}.rs`                         | One `MAX_SYNC_NAMES` supplies five independently live count limits; no distinct catalog-residency term         | Split count owners end to end; add disjoint resident/startup and request-local charges                |
| `auctioneer-core/src/host_primitives/gates.rs` and provider envelopes | Over-ceiling boot warns and installs; shipped WASM rows already fail absolute memory fit before catalog growth | bidder catalog pins baseline and catalog deltas; resource qualification owns absolute WASM fit/headroom before target readiness    |
| `auctioneer-privacy/src/sync/{mod,permission}.rs`                     | Candidate permission reads the supplied `BidderInfo` vendor; host aliases currently supply base metadata       | Carry each public definition's effective vendor separately from implementation/base provenance        |

These are implementation requirements, not claims that automatic
registration, schema enforcement, arbitrary envelope decoding, or
bidder credential binding already exists. Use existing runtime snapshots,
privacy projection, request sinks, response seals, and EdgeZero host paths.

## 3. Exact Reference and Coverage Ledger

### 3.1 Pinned Inventory

Use source revision `d7f8515b86258688304b0d9b6668c6a0e258bc9e`, the existing
behavioral baseline. It is not a claim that this is the latest market
catalog or that every endpoint remains operational.

The following counts and Git object IDs were inspected from that exact
revision, not estimated from the working directory:

| Source set          | Observed inventory                                            | Git object ID                              |
| ------------------- | ------------------------------------------------------------- | ------------------------------------------ |
| Bidder metadata     | 358 documents: 261 non-alias definitions, 97 aliases          | `79921209b19c85f0a2ed50c6651080e707ce7fbc` |
| Parameter schemas   | 261 JSON documents                                            | `210ccd2c6466edc72f0d738493119a448e11b7fe` |
| Adapter source tree | 259 implementation directories, plus a test-support directory | `801ecb3279997b0d1d668d72cc23259053d1138f` |
| Factory bindings    | 261 explicit base bindings                                    | `630fbe348b94d3196cb2a330a7f8ff0790eb5e03` |

Structured inspection of the 261 base metadata objects finds **78 declared
HTTP endpoints: 77 source-enabled bases and one source-disabled base,
`stroeerCore`**. Here HTTP means the object's top-level `endpoint` starts
with `http://`; source-enabled means `disabled` is not true, not that Auctioneer
may route it. Do not count URLs in sync/documentation fields or aliases in
this base population. Alias-declared and inherited effective endpoints need
their own audit. Three of the 77 are priority bases (`msft`, `openx`,
`sovrn`); the other **74**, including the already shipped `appnexus`, require
transport/provenance reconciliation in their owning waves. Section 6.1 also
retains the earlier shipped AppNexus routing-cutover control. These are source
HTTP counts, unchanged by effective HTTPS selection.
The inventory tool derives and pins these sets and counts from the same
metadata Git tree above, not a hand-maintained list of examples.

The pinned parameter-schema tree contains 164,518 raw blob bytes; its largest
member is `static/bidder-params/appnexus.json`, 3,778 bytes, blob
`37909b12066ad9239b488b9751d070212e90d61a`. These figures sum the sizes returned
by `git --no-replace-objects ls-tree -rl` for all regular blobs in that exact
tree, without decoding, newline conversion or reserialization. Inventory/import
checking recomputes the count, sum, maximum and maximum-member identity and
rejects drift against these literals; they are source witnesses, not compiled
validator or heap bounds.

The 260 directories in that adapter tree are 259 implementations plus
exactly `adapters/adapterstest/**`, source test scaffolding excluded from
implementation import. Record this pinned exclusion in the inventory;
it neither produces a factory nor requires a `bidder.toml`.

Source attribution: [metadata](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e/static/bidder-info),
[schemas](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e/static/bidder-params),
[adapter sources](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e/adapters).

Directories and names are not interchangeable units. For example,
`mediafuse` uses the `appnexus` implementation and `emx_digital` uses
`cadent_aperture_mx`; neither has a same-named implementation directory.
`ttd` is a baked alias of `thetradedesk`. The inventory must preserve
these relationships rather than generate fictitious implementations.

Before the first bidder wave, commit a machine-readable ledger under
`docs/bidders/`, with one record for every source public identity. Record:

- Exact case-sensitive public name, source paths and content digests.
- Base versus baked alias, resolved implementation key, alias parent,
  effective vendor identity, parameter-schema binding, and sync family.
- Migration state, planned wave, reviewed protocol contract, capabilities
  by channel/media/entrypoint, and intentional differences.
- Source disable/white-label flags and endpoint provenance, kept separate
  from reviewed compiled safety, direct-routing and opt-in policy.
- Fixture owners and counts, target-build evidence, live evidence status,
  and each blocking shared-feature dependency with its roadmap owner.

Keep the complete reference inventory/import staging separate from the
progressively shipped runtime catalog. Store all reference metadata and
schemas for auditing, but promote a definition into compiled production
metadata only with its real factory, required schema and alias parent.
Do not emit 358 live definitions during foundations and then exempt missing
factories because their ledger rows are unfinished or disabled. Every wave
keeps the strict join valid for all shipped definitions; remaining catalog closure full closure makes
the shipped source set equal all 358 inventory identities.

Use one strict dependency record everywhere in the ledger and capability
reports: `owner_capability`, `capability`, `blocks_bidder_migration`,
`unavailable_behavior`, `error_class`, and `unblock_evidence`. An unsupported
flow and its error must be stated even when it does not block bidder-local
migration. Unknown fields and unknown state/owner values fail checking.

The ledger is an audit/checking input, not a constructor table. Production
discovery must work without consulting it. Validate exact set equality
against the pinned source inventory, including unknown/missing/duplicate
rows, alias cycles, case-fold collisions relevant to sync, and orphaned
schemas. Source pin changes require explicit review and regenerated
provenance; re-pinning digests alone must not bypass the revision check.

Use states `inventoried`, `in_progress`, `blocked`, and `migrated`.
Record `disabled` as source/host policy and live certification as separate
fields, not substitutes for migration. A metadata-only, generic-fallback,
stubbed, or bidder-implementation-blocked row cannot be `migrated`.
Unavailable shared delivery is a separate capability status; it blocks
migration only when it prevents implementing or verifying bidder-local
behavior, as distinguished in section 9.

### 3.2 Completion Accounting

remaining catalog closure full-catalog closure reconciles all 358 source identities, the 261 base
bindings, their inherited or distinct schemas, and the 259 source implementation
directories. Here an implementation family means one compiled factory key;
public identity, source directory, and sync family are separate dimensions.
The 261 base bindings reuse 259 source implementations, while 97 aliases
add identities without adding implementations. Local-only `generic` and
feature-only `mocktioneer` are recorded
separately and do not inflate source coverage. A source-disabled bidder
still needs a reviewed implementation/binding to count as migrated; its
compiled safety disable stays effective unless an explicit reviewed
source-to-policy reclassification under section 6.1 changes it. A source
deployment default is not silently treated as a permanent safety judgment.
Non-routable definitions still require real implementations and source-owned
behavioral evidence; a disabled row is not a stub exemption.

No bidder disappears because it is difficult, inactive upstream, or
requires a later shared feature. If a required shared feature prevents
bidder-local implementation or verification, bring forward its smallest
reusable portion with a reviewed design and keep the original owning
capability accurate. Removing source scope or changing the catalog revision
requires an explicit product decision, not a green status from a shortened
ledger.

### 3.3 Evidence-Derived Migration State

`auctioneer bidders check` derives migration state; it must not trust a
hand-written `migration_state = "migrated"`. Normal checking permits
honest unfinished rows; `--require-complete` additionally enforces full
closure, so foundation/partial-wave CI need not pretend every row is done.
A migrated row requires a valid compiled binding, schema/settings mapping,
and a reviewed protocol branch manifest with runnable conformance evidence for every applicable
branch. Required request and response positive cases must actually produce
expected nonempty work/bids where the captured protocol does so; no-bid
fixtures alone cannot certify an implementation. Baked aliases require
their own identity/vendor/metadata cases even when transformation cases
are shared. Intentional differences require explicit reviewed records.

CI executes each declared test family through a fixed Rust driver, with
exact operation sets, row shapes, and positive/negative dispatch counts.
Evidence binds the source revision, fixture/schema digests, implementation
and shared-code input hashes, effective activation/routing and promotion
policy digests, target, features, and dependency lockfile.
Missing, failed, skipped, stale, mismatched, or unknown evidence cannot
produce `migrated`; cached reports never substitute for the CI test run.
Ledger test identifiers select supported drivers, not arbitrary commands.
Human protocol review remains required: a green driver cannot prove its
own branch manifest is complete against the source.

Use a producer/consumer contract, not content hashes as proof of execution.
The fixed Rust evidence driver starts a fresh run manifest in an exclusively
created output directory. It records the required producer set and a run ID;
in CI the expected ID comes independently from the current workflow run ID,
attempt and commit, not from a supplied evidence file. Local verification
uses a newly generated run ID, never a reused successful directory. Each
fixed behavior/build/resource producer writes its own receipt only after
executing its declared controls and recording exact dispatch counts. Receipts
bind both the run identity and the input identities above. Missing, skipped or
failed producers cannot be represented by an old success receipt.
Derive the normal run's required set from foundation controls and the current
shipped subset; a wave adds its reviewed selected-flow requirements. Full
completion independently requires the complete source branch/identity set,
not whichever subset a supplied run manifest names. This preserves honest
partial-foundation acceptance without allowing a shortened run to certify
remaining catalog closure full-catalog closure. bidder catalog instead requires section 12's complete
foundation/priority evidence; it does not redefine full checker completion.

`bidders check --evidence-root PATH --run-id ID` consumes these receipts;
the expected ID is supplied by the invoking run. A normal inventory check
without current execution evidence may accept an honest unfinished ledger
but cannot derive `migrated`. A final CI check depends on every required
producer, collects failure/missing status even when a producer fails, and
refuses incomplete runs. This uses existing CI trust and scoped artifacts,
not a new signing service. Replaying a previous run's success with identical
source/schema/code/lock hashes while the current producer is skipped must
fail a named CLI control. Test wrong-run, failed, missing and unknown producer
receipts separately from content staleness.

Negative controls must replace a real implementation with a no-work stub,
forge migrated state without evidence, omit a branch/fixture, rename an
operation, alter an implementation/shared dependency, and supply stale or
failed results. Each must fail the completion check. remaining catalog closure full closure requires
all 358 source rows to derive `migrated`, with zero migration-blocked,
inventoried, or in-progress rows; later shared delivery and live status
remain separate.

## 4. First Bidder Wave

The first wave must make these exact requested identities usable for
their reviewed banner/display flow, including parameter validation,
aliases, required host settings, privacy, and sync where configured:

| Requested identity | Source implementation/base | Note                                                                                                                                                               |
| ------------------ | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `msft`             | `msft`                     | Prove source-specific member consistency, impression transformation, HB-source settings, and 10-impression chunking; a shared endpoint is not protocol equivalence |
| `ttd`              | `thetradedesk`             | Baked alias; retain its requested identity and effective metadata                                                                                                  |
| `rise`             | `rise`                     | Explicit binding                                                                                                                                                   |
| `sovrn`            | `sovrn`                    | Explicit binding                                                                                                                                                   |
| `yahooAds`         | `yahooAds`                 | Preserve public spelling; internal Rust key is `yahoo_ads` under section 5.1                                                                                       |
| `unruly`           | `unruly`                   | Explicit binding                                                                                                                                                   |
| `sharethrough`     | `sharethrough`             | Explicit binding                                                                                                                                                   |
| `openx`            | `openx`                    | Explicit binding                                                                                                                                                   |
| `kargo`            | `kargo`                    | Explicit binding                                                                                                                                                   |
| `rubicon`          | `rubicon`                  | Source deployment disable is explicitly reclassified under section 6.1; requires reviewed endpoint/settings/authentication and remains opt-in                      |
| `pubmatic`         | `pubmatic`                 | Audit and complete existing bidder, not automatically a completed row                                                                                              |
| `triplelift`       | `triplelift`               | Do not conflate a separate native integration                                                                                                                      |
| `criteo`           | `criteo`                   | Explicit binding and host-setting review                                                                                                                           |
| `aps` (APS)        | Not in the pinned catalog  | Additional priority integration; review its own protocol, schema, endpoint/settings/authentication, privacy and onboarding contract before implementation          |

This is 14 requested names, not necessarily 14 new implementations.
Every row gets its own conformance fixtures and capability record.
The current source-backed wave contains the original 13 names; APS is not
present in that pinned source and cannot inherit another bidder's factory,
schema, endpoint, vendor ID or sync metadata. Keep APS in the same priority
delivery checkpoint with an explicit pending integration contract, not in the
remaining-catalog backlog. Add its reviewed implementation and declaration to
the existing discovery path; do not add a central registration entry or invent
source blobs. APS does not change the 358-row source inventory or its pinned
reference evidence. A 13-row wave check alone cannot complete the 14-flow
publisher checkpoint; its acceptance must also consume APS's independently
reviewed contract and executable evidence.
The pinned `msft`, `openx` and `sovrn` metadata declares HTTP endpoints.
AG's 2026-10-28 decision in section 6.1 approves HTTPS-by-default selection
for those same destinations without separate vendor TLS-mapping evidence.
Their effective metadata, source URL/blob provenance, transport-difference
records and deterministic route tests still require implementation and binding;
selection alone is not priority-flow acceptance, connectivity or live approval.
An unavailable row cannot satisfy that checkpoint. Unapproved HTTP dispatch
must fail route qualification even with valid activation/settings/credentials.
For `msft`, require 10/11/21-impression call-boundary controls, consistent
versus conflicting member IDs, and the reviewed display/video HB-source
defaults and overrides. These source-derived cases must distinguish it
from the existing bidder; reuse requires passing them, not a label.
Unsupported video/native/AMP/pod behavior stays explicitly disabled or
rejected until its owning shared contract is available. This first-wave
checkpoint does not waive priority-local branches or claim complete video
delivery. bidder catalog versus remaining catalog closure closure follows the explicit
[2026-10-26 correction](#2026-10-26-scope-correction), not a reduced source ledger.

## 5. Automatic Registration, Without Central Bidder Names

### 5.1 Chosen Mechanism

Use build-generated discovery. A central match or macro list violates
the requirement even if moved to another crate. Linker inventory is not
selected: ordinary generated Rust follows existing codegen practice
without introducing linker-retention discovery across four runtimes.
Runtime plugin loading and remote executable downloads are out of scope.

All bidder implementation work stays in the existing `auctioneer-bidder` crate;
there is no new implementation crate. Public concrete types use `FooBidder`
names, with no old-name compatibility aliases. Bounded metadata
constants use bidder terminology without changing their values or accounting.
Platform crates `auctioneer-adapter-{axum,cloudflare,fastly,spin}` and their provider
terminology remain unchanged, as do immutable source evidence and wire names.

Each implementation lives under
`crates/auctioneer-bidder/src/bidders/<bidder_key>/mod.rs` with an adjacent
`bidder.toml`:

```toml
key = "yahoo_ads"
factory = true
```

Use the [common bidder layout](../../bidders/README.md#bidder-layout) for every
implementation: `mod.rs` owns the trait/factory, private transforms and inline
unit tests; bidder-specific captures live in its `fixtures/` tree. Do not
scaffold separate unit/request files. Only conformance tests use separate files,
with shared-inventory, process and cross-crate controls in their actual owners.

**Bidder-owned asset cutover, approved 2026-10-23:** this declaration becomes
the single bidder-owned TOML document for registration, public metadata, alias
associations and provenance; section 10 defines its storage boundaries. The
coordinated [migration plan](../plans/2026-10-23-bidder-owned-assets.md) supersedes
the earlier two-field-only declaration and split config/reference locations.
Existing readers have not yet made that cutover. `factory` is an explicit
boolean declaration, not a migration-completion assertion. The directory name
equals `key`, which is a non-keyword lowercase Rust identifier.
Public bidder names are data, not Rust identifiers, and retain their exact
source spelling, including digit-leading names. The importer maps source
implementation-directory names to internal keys by splitting a lowercase
or digit followed by uppercase, and an uppercase run before its final
capital followed by lowercase, with `_`; then ASCII-lowercase. Preserve
existing underscores. Prefix a digit-leading result with `bidder_`, and
suffix a Rust keyword with `_bidder`. Reject other characters, invalid
identifiers, helper-name conflicts, or collisions; never silently add
numeric suffixes. Persist source-path/key pairs in the audited ledger.

Examples are `33across` -> `bidder_33across`, `adkernelAdn` ->
`adkernel_adn`, `sspBC` -> `ssp_bc`, and `yahooAds` -> `yahoo_ads`.
All 259 pinned implementation directories map to distinct keys by this
rule; tests enumerate that full mapping plus keyword and collision cases.
Aliases and bases reusing code follow the resolved source factory binding,
not a guessed transformation of their public names. This mapping is import
tooling, not a central production registration list.

The conventional factory is `create_bidder() -> Box<dyn Bidder>`.
It is synchronous, infallible, and does no I/O or configuration lookup.
Protocol-specific settings are supplied through validated request/host
context, not global state hidden in a constructor.

`auctioneer-bidder/build.rs` discovers declarations in this fixed committed
directory, validates them, and generates both module inclusion and a
lexicographically sorted factory slice into `OUT_DIR`. Every directory requires
`bidder.toml`. A `factory=true` declaration requires `mod.rs`; a reference-only
`factory=false` declaration has neither `mod.rs` nor promoted public definitions.
Reject code without its factory declaration and factory declarations without
code, including inactive features. Imported data cannot become executable or
callable merely because its folder was added. Shared helpers remain ordinary files,
not implementation directories. Move the existing `bidders/url/mod.rs`
helper to `bidders/url.rs` during foundation cutover, preserving its
tests and API. Do not exempt helper directory names from discovery;
reject a declaration whose key collides with an ordinary helper module.
No Rust-source trait scanning, arbitrary path expression, user-supplied
constructor text, or production discovery directory override is permitted.
The external `adapters/adapterstest` inventory exclusion is not a Rust
discovery exemption: every immediate directory under the fixed production
`bidders/` root, even one called `adapterstest`, requires its declaration;
executable declarations also require `mod.rs`. Nested modules belong to their
declared implementation;
repository test-support trees live outside this discovery root.

Expose the generated factory slice through
`compiled_bidders() -> &'static [BidderRegistration]`; each registration holds
its implementation `key` and conventional `create` function pointer.

Generated module declarations use `#[path = "<validated source path>"]`
to include each committed `<bidder_key>/mod.rs` from the `OUT_DIR` module. This
specific generator-owned path attribute is permitted: it is constructed
from the canonical fixed source root and validated key, never declaration
text, runtime data, or an arbitrary path expression. Reject escaping paths
and symlinks. Compile a nested-module fixture from a different working
directory; text rendering alone does not prove module resolution.

Validate all declarations before feature filtering. A declared Cargo
feature must exist. Apply an identical predicate to the implementation
module, factory entry, and associated metadata inclusion. Test-only
source substitution stays behind the existing harness boundary.
Feature ownership must be declared and forwarded in both bidder and config
manifests. A bidder-only feature does not implicitly reach config codegen.
Validate data-only feature descriptors against factory declarations before
filtering, and reject missing forwarding/mismatched predicates. Metadata
generation still has no dependency on bidder code or constructor execution.

Emit `cargo::rerun-if-changed` for the discovery root and discovered
directories, declarations, source files, and manifest/feature inputs.
Directory tracking is necessary for new/deleted members; tracking only
the previous file list is insufficient. Rebuild generated output entirely
from the current validated set so stale `OUT_DIR` files cannot keep a
deleted registration. Cargo's documented directory change detection is
the mechanism, not an assumption inherited from flat metadata generation;
see the [Cargo build-script contract](https://doc.rust-lang.org/cargo/reference/build-scripts.html#change-detection).

In one reused target directory, execute a synthetic bidder, add another,
edit it, rename its complete declaration/source/metadata set, then remove
it and rebuild. The final executable must no longer expose or execute the
removed bidder. Deleting only a declaration or source fails loudly;
restoring it must recover without `cargo clean`. Repeat nested helper
edits and feature off/on/off with actual module/factory execution checks,
not only snapshots of emitted text.

### 5.2 Explicit Data Bindings

Every baked base definition declares `implementation = "<bidder_key>"`.
The public name and implementation key are distinct: public `yahooAds` binds
to `implementation = "yahoo_ads"`, and independent public bases may share a key.
Metadata generation emits data-only bindings beside the existing static
metadata as `BidderImplementationBinding { bidder, implementation }` rows
through `bidder_implementation_bindings()`. Here `bidder` is the exact public
name and `implementation` is its internal key. Generation does not import
`auctioneer-bidder` or execute factories.
Missing/unknown bindings never fall back to `generic`. An explicitly
reviewed generic binding is allowed only with fixtures proving that
integration really uses the generic wire protocol.

Several bases may share an implementation while retaining independent
endpoints, metadata, vendor identity, parameter rules, host settings,
runtime switches, and response attribution. Internal implementation keys do
not become public bidder names, info fields, sync keys, or privacy identities.

### 5.3 Immutable, Validated Startup Join

`auctioneer-bidder` validates the metadata/factory join before invoking any
factory. Duplicate bidders, factories, bindings, missing/dangling
bindings, unavailable feature factories, and invalid baked aliases are
typed errors, including for disabled entries. Unused factories are allowed.

The generated runtime catalog indexes each compiled public definition,
including flattened baked aliases, with its effective factory binding
and canonical-base provenance. Construct a separate instance for each
such definition in deterministic order, borrowing existing static
metadata without production `Box::leak`. Baked aliases add catalog
entries/instances, not new implementation families; inherited validators
may share immutable caches. Cache one
`Result<BidderRegistry, RegistryBuildError>` per process/isolate.
Application initialization and every direct helper propagate its failure;
no panic, empty-catalog fallback, partial publication, or repeated retry.
Duplicate explicit registration leaves the incumbent unchanged.
Registry failure is not ordinary invalid runtime configuration that health
or status may degrade past. Initialize through `configure_app`; direct
runtime resolution, including absent-store defaults, and catalog helpers
propagate the same cached typed failure.

Auctions, runtime alias validation, info handlers, and sync consume the
same validated catalog. Provider adapters retain the shared EdgeZero app
configuration path and gain no bidder-name lists. The portable exchange
continues to permit explicitly injected test/embedded registries.

Required onboarding proof: in an isolated fixture source tree, add a
previously unknown implementation directory, declaration, metadata, and
tests; rebuild and execute it without editing any central production
registry/module/factory list. A second proof adds a base using an
existing bidder implementation with metadata alone. A source-text grep is
not enough.

## 6. Metadata, Aliases, and Runtime Activation

### 6.1 Baked Versus Runtime

Bidder code, binding, schema, vendor identity, capabilities, endpoint
policy, sync definitions, and protocol-specific non-secret constants
remain reviewed build inputs. Operators may enable/disable shipped
eligible bidders and use existing timeout/sampling/sync overrides through
the current validated configuration push. No rebuild is needed for that.

Newly imported integrations are opt-in by default. Add a data-only baked
`defaultEnabled` policy, distinct from the existing non-overridable
`disabled` safety flag. Preserve current defaults for already shipped
definitions. Retain source `disabled` separately in import provenance;
the importer maps it to the safety flag unless an identity-specific reviewed
policy record explicitly reclassifies that source deployment default. Such
a record names the exact source blob, old/new policy, rationale and review
evidence; re-import may not erase it, and runtime configuration cannot create
or override it. No production bidder-name match implements this mapping.

For the priority wave, this design explicitly reclassifies Rubicon's source
deployment default: retain `source_disabled = true`, compile
`disabled = false`, and set `defaultEnabled = false`. Its source row contains
a regional endpoint placeholder and credential placeholders. This design
treats that as an unconfigured deployment default, subject to endpoint/auth
review before promotion, not proof of permission for a live connection.
Promotion must supply the reviewed regional HTTPS
endpoint policy and settings/secret contract before it is callable. A runtime
enable without valid required settings/credentials produces no calls. Keep
other safety disables intact; this is not a global reinterpretation of source
flags. Record the policy review honestly, without inventing a reviewer or
live certification.

For a base, **family eligibility** is `!disabled` combined with its
runtime `enabled` override, falling back to `defaultEnabled`. An alias
cannot bypass a disabled or family-ineligible base; an alias-specific
disable affects neither the base nor siblings. Sync also applies its
existing independent enablement rules. Carry `defaultEnabled` as internal
eligibility data, not an added public bidder-info field. List/detail/all
info views retain all definitions, while their enabled-only filters use the
same **direct-identity eligibility** predicate as auction and sync; sync
additionally applies its independent sync-specific gates. Activation is not
a claim of credentials, supported
shared delivery features, or live certification.

Here family eligibility is keyed by the canonical **public base**, not
section 3.2's implementation-family/factory key. Independent bases sharing
code, such as `appnexus` and `mediafuse`, keep independent activation and
safety state. A factory-keyed veto or enable map must fail their control.

Direct-routing disposition is separate from family eligibility. A reviewed
definition is `direct` with a validated endpoint template, `alias_only`, or
`unavailable` with a fixed reason. Represent absent endpoints explicitly;
never manufacture a dummy HTTPS URL. A `direct` identity requires its family
and own activation to allow it and a reviewed endpoint/settings contract.
An `alias_only` parent can enable its family but is never directly callable,
sync-selected or included in enabled-only info. An `unavailable` definition
is likewise non-routable. Invalid direct definitions still fail the strict
startup join; disablement does not excuse a malformed _routable_ definition.

List/detail/all status uses that same direct predicate, not family eligibility
alone. Unfiltered and base-only views retain alias-only/non-routable bases
with non-active status, while eligible baked aliases may be active.

Preserve `whiteLabelOnly` as `alias_only`, not as a safety disable. The pinned
`teqblaze` parent has no endpoint, while aliases such as `360playvid` declare
their own. Enabling that family and an eligible alias permits the alias's
reviewed endpoint, never direct parent dispatch. Do not inherit the parent's
`alias_only` disposition into its baked aliases; validate their effective
endpoint/routing separately. Genuine base safety disables still veto them.
Runtime aliases cannot supply endpoints or turn an `alias_only`/unavailable
base into a routable identity; reject that target rather than fabricate one.

**Endpoint transport policy, AG decision 2026-10-28:** default to HTTPS.
This supersedes earlier requirements for separately evidenced TLS mappings,
including `appnexus`/`msft`; no separate vendor documentation is required merely
to replace a source HTTP scheme with HTTPS. Change only the scheme, preserving
the source authority (including any explicit port), path, query and bidding
protocol. Preserve exact source URLs and immutable blob witnesses as provenance,
and record the effective transport difference against this decision. This
selection does not prove connectivity, protocol equivalence or live authorization,
and does not itself promote metadata or complete deterministic route acceptance.

HTTP requires an explicit scoped exception. Preserve only the existing
`insecure-bidders` Mocktioneer loopback exception here, without broadening its
scope. Add no global HTTP fallback and never auto-downgrade on an HTTPS error.
Do not invent endpoints or change source activation through transport selection.
Live authorization and deployed traffic testing remain publisher pilot work.

Native import/check records a scheme-only selection in an optional metadata
artifact `https_default` table: exact `source_url`, `selected_url`, and
`inherited_from` only when the endpoint comes from an alias's parent. An own
nonempty endpoint takes precedence. HTTPS, missing and alias-only source
definitions have no such record. The pinned inventory yields 122 selections:
78 declared HTTP bases, 27 own-endpoint aliases and 17 inherited aliases.
The 77 source-enabled HTTP bases remain a separate activation fact.

These records are unvalidated template provenance, not executable definitions,
URI-safety approval or proof of connectivity. Native `--check` reconstructs
them from immutable source objects, compares all fields and presence, and
rejects records on non-metadata witnesses in every actual manifest, including
supplemental-only owners. Independent protocol notes remain review-owned.
The serving cutover still requires its strict source/policy binding and the
existing destination validator; it cannot treat this table as an enable bit.

For source-disabled `stroeerCore` and endpoint-less `ix`, retain
the source endpoint/absence as provenance and compile an `unavailable`,
safety-disabled definition with no callable endpoint. Factory, schema and
protocol evidence remain required for migration. HTTPS selection does not lift
a source disable; that requires a separate reviewed reclassification, such as
Rubicon's existing scoped decision above. URL validation remains unconditional
for every effective `direct` endpoint, even when runtime activation is off.

Apply this disposition-dependent rule across the complete inventory, not
only the examples: absent, placeholder or otherwise unusable destinations stay
unavailable; a source HTTP scheme alone is not an unavailable disposition.
For source-enabled HTTP base `mediafuse`, select the same destination over
HTTPS under this policy, preserving source flags and its AppNexus binding.
`enabled_http_base_defaults_to_https_with_source_provenance` is the named import
control: assert HTTPS selection, exact source URL/blob and transport-difference
preservation, and no unapproved HTTP dispatch. Selection is not promotion or
factory/protocol qualification. Endpoint-less disabled `adxcg`, `avocet` and
`pangle` remain unavailable/safety-disabled import controls; HTTPS supplies
neither a missing destination nor approval to clear their source flags.

The existing AppNexus HTTPS override and `msft` both have the pinned source URL
`http://ib.adnxs.com/openrtb2`. AG's decision approves choosing
`https://ib.adnxs.com/openrtb2` for each, not because of the override's TLS
comment or URL/factory equality. Task 6 must bind AppNexus's exact source
URL/blob and intentional transport difference and test effective routing before
the shipped representation cutover; no separate vendor TLS mapping is pending.
Preserve shipped defaults, and keep `msft`'s protocol/settings review independent.
Task 20's later complete AppNexus protocol audit does not postpone this binding
control. Each wave records effective HTTPS selection or a scoped HTTP exception
or explicit unavailable disposition, with provenance; remaining catalog closure reconciles all 78
source HTTP bases and separately audits effective aliases.
An unavailable disposition cannot satisfy a priority usable-flow requirement
or be advertised as callable merely because its protocol migration is complete.

Baked aliases introduced together with a new opt-in base inherit that
base's family activation by default, so enabling the base can activate
its intended alias cohort (including `thetradedesk`/`ttd`). A newly imported
alias of an already shipped base instead receives explicit
`defaultEnabled = false`; otherwise importing it could activate a new sync
destination through the base's existing enablement. The importer records this choice
from the pre-wave compiled set and reviewed promotion set, not mutable
migration-state labels. Preserve existing defaults on re-import. Operators
can explicitly enable such a narrowed alias at runtime, but alias `true`
never bypasses a base veto. Runtime aliases inherit base eligibility. A
source alias's baked safety disable remains non-overridable. Onboarding
documents whether base enablement suffices or alias opt-in is also needed.

This deliberately refines the older registration draft's omitted-switch
behavior for new catalog entries. Importing metadata must not silently
add hundreds of cooperative sync destinations. The importer must set an
explicit activation policy; source `disabled` is not used as a temporary
runtime switch. Setting an eligible base's runtime `enabled = true`
activates it without changing code. In-flight requests retain their
captured runtime snapshot.

### 6.2 Baked and Runtime Aliases

Flatten baked alias inheritance during validated metadata generation;
retain its public identity, source parent, permitted metadata overrides,
and effective vendor/sync/schema relationships. Detect cycles, unknown
parents, conflicting fields, and duplicate identities before publication.
Do not turn all aliases into identical clones or count them as new code.
In particular, a baked alias's GVL vendor ID is its own declared source
value; absence stays unknown/zero, never inheritance from the parent.
Host/runtime aliases retain their existing base-vendor semantics except
for the auction-only request override described below.

Preserve declared versus inherited source disable provenance before applying
policy, then compute effective compiled safety after the reviewed base mapping.
`magnite` omits a disable and inherits Rubicon's mapped compiled policy; an
importer must not freeze the old inherited source `true` as an independent
alias veto. Conversely `appStockSSP` explicitly declares `disabled: true` and
keeps that veto even when its `teqblaze` family is enabled. Re-import must bind
every reclassification to the same identity/source blob; a changed blob needs
renewed review. Test both inheritance directions, wrong-identity/blob records
and runtime attempts to change routing/endpoint policy.

Runtime aliases target an existing compiled base through validated
host/request rules. A host or request alias must not shadow any compiled
public identity, including a baked alias, by exact name or ASCII-folded
name. This intentionally rejects the previous exact-name base-shadowing
behavior: accepting it would let a remapping bypass a source identity's
baked safety disable or change its advertised provenance. No compatibility
shim is required. A runtime overlay may still replace a previously
configured host alias's target, subject to the same validation.

Preserve exact public name matching in the auction and the reviewed
ASCII-folded lookup semantics in sync. Reject ambiguous catalog
collisions under either consumer's rules; never let map insertion order
choose the privacy identity. A host runtime alias selects its declared
base's reviewed implementation, metadata, schema, and destination policy;
it cannot supply replacement values for them. Preserve the existing
request-local `ext.prebid.aliasgvlids` contract: it applies only to an
alias declared by the request or effective host alias map,
validates `1..=u16::MAX`, and feeds that alias's effective privacy identity.
It does not mutate baked metadata, affect another alias, or create a base.
Direct requests for baked aliases use their reviewed effective vendor
metadata, not an undeclared `aliasgvlids` override.

Public info views use provenance, not membership in the factory or
registry slice, to distinguish bases from aliases. `baseadaptersonly=true`
excludes both baked and runtime aliases, while retaining every non-alias
base even when several bases share a factory. List/detail/all views retain
the exact public spelling; baked-alias details expose `aliasOf` with the
canonical base and the alias's effective metadata. Keep a source alias's
immediate parent separately for audit. Request-only aliases stay outside
the public info catalog, as today.

### 6.3 Catalog Scale Is Not Auction Scale

Introduce a shared `MAX_CATALOG_IDENTITIES = 512` for compiled public
identities plus additional host aliases. It covers all 358 source names
and local definitions. Retain the separate 64-entry runtime host-alias
overlay cap and surface-specific identity byte limits: runtime/request alias
names and bases remain capped at 64 bytes, while inherited sync-alias
names retain their existing 128-byte allowance. The aggregate ceiling
does not widen that overlay or any name limit. The effective host map may
also contain inherited host-default aliases; do not misdescribe today's
overlay cap as a 64-entry cap on that merged map. Source baked aliases are
compiled identities, distinct from inherited host-default aliases. Count a host
alias once in the effective merged host map. Refuse 513 total identities
rather than truncate. Validate
this at build and runtime alias resolution, before constructing a larger
request-scoped catalog.

In `auctioneer-config/src/usersync.rs::validate_catalog_size`, replace the
literal in the existing checked-add `total > 256` check with the shared
catalog constant; retain overflow rejection and update the error's limit.
This is a catalog gate, not a caller-list gate. Both build-time
metadata validation and runtime host-alias validation must exercise that
same owner at 511/512/513, including bases, baked aliases and host aliases.
The semantic constants live in `auctioneer-runtime` with the auxiliary limits;
use its workspace dependency from both normal and build dependencies of
`auctioneer-config`, since this validator is compiled in both contexts. There
is no new constants crate or reverse runtime-to-config dependency.

Separate catalog iteration from user-supplied lists: retain existing
256-entry filter/priority input ceilings, group limits, cookie limits,
sync response limits, and auction impression/bidder/call limits. The
chooser and eviction-family projection must accept a 512-entry catalog
without accepting oversized caller lists. `all` selection still obeys
the configured output limit and privacy/enablement rules. Charge the
entire catalog scan/projection even when only one result is returned.

Split the runtime `MAX_SYNC_NAMES` ownership, not just handler/config
validation. Through `AuxWorkLimits`, normalization, materialization guards,
and `aux_work_limits_from_envelope`, `catalog_names` permits 512;
`requested_names`, `priority_entries`, each `filter_names_per_list`, and
requested-name `debug_rows` retain distinct 256-entry owners, with 32
priority groups. Equal production values are not a shared semantic limit.
Do not feed all five fields from one constant or widen debug rows merely
because the catalog grew.

The concrete cutover is the single `AuxWorkLimits` literal in
`auctioneer-runtime/src/budget.rs::aux_work_limits_from_envelope` (currently
lines 1119-1124). Replace its five `MAX_SYNC_NAMES` assignments with five
named semantic constants: `MAX_CATALOG_IDENTITIES = 512`,
`MAX_SYNC_REQUESTED_NAMES = 256`, `MAX_SYNC_PRIORITY_ENTRIES = 256`,
`MAX_SYNC_FILTER_NAMES_PER_LIST = 256`, and `MAX_SYNC_DEBUG_ROWS = 256`.
Keep the separate 32-group and 64-entry host-overlay limits. Split the
corresponding shared validation in `auctioneer-runtime/src/aux_work.rs`, the
filter/requested/priority/debug owners in `usersync/choose.rs`, and catalog
versus priority owners in `usersync/evict.rs`. Carry each constant through
its actual materializer/refusal path; a renamed but still shared constant
does not implement this split.

Under the current coefficients, chooser bytes are
`(catalog + requested + 2*filter + priority + debug)*256 + groups*128`.
Growing only the catalog from 256 to 512 therefore adds 65,536 bytes;
this arithmetic anchor is not permission to omit other simultaneously
live catalog maps or copies. Tests use unequal count inputs and change
one field at a time, including both filter lists and explicit debug-row
refusal. Exercise 255/256/257 caller entries separately from catalog
255/256/257 and 511/512/513, a full catalog with small caller lists/output
limit one, and allocation peaks with catalog and chooser live together.
Removing any summand or its enforcement must fail a targeted guard check.
The shipped five-field tuple is independently pinned to the literals
`(512, 256, 256, 256, 256)` in source assignment order
`catalog_names / priority_entries / requested_names / filter_names_per_list / debug_rows`.
The interleaved `priority_groups` assignment is separate and pinned to 32;
it is not a sixth catalog/caller owner. Test unequal synthetic values too,
so swapping priority and requested fields cannot hide behind equal defaults.
Widen each of the four caller-sized owners to 512 independently: its
257-entry refusal and the shipped-tuple guard must fail. This detects
widening, which a term-removal control or an expected value derived from
the same production constants cannot detect.

For cooperative expansion, exclude effectively disabled identities before
shuffling and family deduplication: an imported opt-out alias must not
consume a family ahead of an enabled integration. Preserve explicit
requested-name diagnostics and the existing first-attempt family rule
for explicitly requested identities, including later privacy/filter
denials; do not retry a denied family through a different vendor to seek
a grant. This cooperative eligibility refinement is an intentional bidder catalog
difference from user sync's all-configured expansion and needs an audited fixture
row. Test disabled-before-enabled and enabled-before-disabled orders,
separately for requested and cooperative names.

Do not merely replace each `256` literal with `512`. Update only owners
of catalog-sized work, fund their actual scratch/retained terms, and
prove the unchanged input/output bounds separately. No existing memory
profile is qualified for the expanded catalog by inheritance.

## 7. Parameter Schemas and Protocol Settings

### 7.1 Schema Ownership and Enforcement

Serving/resource acceptance in this section applies to the complete shipped
set `S` from the [qualification amendment](2026-10-29-shipped-schema-qualification-design.md),
not all unshipped catalog validators. Keep full-source inventory and existing
correctness/compatibility controls separate. Verify actual generated/linked
membership; an unselected owner that remains resident cannot be uncharged.

Import committed parameter schemas with source digests. Bind each base
explicitly; baked aliases inherit or override only as the source contract
permits. Review the dialect, formats, references, regex features, and
coercion behavior across the entire pinned set. Preserve distinctions
such as integer/string alternatives and missing versus null.

Generate schema-specific validation code during the build using a maintained
JSON Schema implementation, not a hand-written subset of the standard.
`auctioneer-bidder/build.rs` discovers the shipped schema bindings and emits
validator declarations/dispatch into `OUT_DIR`; the qualified library's
compiler or procedural macro generates the checks. No central bidder-name
list or runtime schema compiler is introduced. The candidate is Rust
`jsonschema` with its documented
[compile-time validator macro](https://docs.rs/jsonschema/latest/jsonschema/#compile-time-validator-macro),
default HTTP/file resolution disabled and explicit draft selection. These
are capabilities to verify, not a claim that all schemas, WASM targets or
resource bounds are qualified. Pin the version/features in the implementation
plan and root workspace dependencies before adding it. Runtime helper code
required by generated checks still belongs in the dependency/artifact audit;
moving compilation to the build does not make its runtime cost zero.

Generated validator marker structs and generated parameter-data structs are
different things. Plain Serde types do not by themselves preserve all schema
constraints, including combinators, formats and patterns. Prefer existing
`validator::Validate` derives and reviewed custom validators for typed host
configuration/settings. Do not hand-translate the imported schemas into a
second competing constraint source, coerce values or add generated data
types merely to deserialize a parameter subtree twice. A bidder can use
typed parameter views when its protocol needs them, with their own accounting.

Build/compile each distinct effective shipped schema once and share its generated
entry point across inheriting aliases. Runtime calls validate the borrowed
final parameters with that code; they neither parse nor compile schemas.
Resolve committed local references only during the build. Unknown remote/file
references, reference cycles that defeat a finite work bound, unsupported
validation keywords, or regex semantics without a reviewed bounded
implementation fail the build. Annotation keywords are not validation
failures. Explicitly configure format validation from the reference contract.
Prefer the linear regex engine; a required unsupported pattern needs a
reviewed protocol/schema decision, not silent removal of the constraint.
Qualify any library-generated lazy regex/cache initialization separately:
fund it before first use and share it once, never compile per impression.

Approved compiler extension (2026-10-21): carry a separate checksum-pinned
patch adding `initialize_regexes() -> bool` to each generated validator. It
forces every existing regex constructor, including after an earlier failure,
using the unchanged engine, pattern and builder limits. Construction is shared
with validation through the same initialize-once storage; repeat/concurrent
calls neither reconstruct a successful regex nor retry a cached failure. A
validator without regexes returns true. This is constructor initialization,
not schema compilation or a promise that later searches cannot grow caches.
Custom-format owners and complete C/I/V/work/cache qualification remain separate;
this API alone authorizes no production schema integration.

The macro is a candidate, not a guaranteed implementation. If qualification
fails for a required schema, semantic feature or target, record a concrete
reproducer and keep schema integration/foundation acceptance blocked. Inventory,
discovery and other independent foundation work may continue without enabling
unqualified validation. Review a compatible maintained ahead-of-time generator
or a bounded upstream macro fix, then rerun the complete shipped qualification set.
If neither is viable, a startup-compilation alternative requires an explicit
spec/plan amendment and user approval before implementation, including renewed
work, initialization, residency and all-target bounds. There is no automatic
runtime fallback, hand-written schema subset, bidder omission or allow-all mode.

Validate the bidder's final scoped parameter object after the existing
participation/supplement/alias merge, without exposing competitors or
reconstructing data removed by privacy. Validate before that impression
can fund an outbound body. Keep input byte/node/depth bounds and bound
schema evaluation work for the committed schemas; a JSON node bound
alone does not establish a CPU bound for arbitrary regex/combinators.

Each generated validator must have a reviewed, source-hash-bound
`SchemaWorkBound`: a conservative cost function over the admitted
instance bytes/nodes/nesting, covering references, combinator fan-out,
regex scans, collection comparisons, and unsuccessful validation as well
as success. Only committed schemas whose supported keywords admit this
bound may initialize. A hard evaluation counter in the qualified engine
may replace conservative precharging only if it stops work before the
same limit; an elapsed timer after a synchronous call is not a bound.

The foundation plan must set finite single-evaluation work, aggregate
auction work, build-output/resident-cache memory, and peak evaluation scratch
limits before engine integration. Core funds scratch under auction
admission and debits the conservative evaluation charge before calling
the engine, including on failing impressions; no reparse/clone or eager
full-error-tree collection is uncharged. Check the original deadline
before every evaluation. Resource exhaustion is a fixed bidder-attributed
code-5 construction-resource refusal, not invalid-parameter code 2;
original-deadline expiry keeps the existing auction deadline disposition.
Refusal must not discard previously valid sibling work or start an
unfunded evaluation. If the candidate engine cannot establish these
bounds, its qualification fails; passing a benchmark is not a substitute.

Acceptance independently varies schema cost, input size, impression
count, single/aggregate work, and scratch bounds, with exact-limit and
one-over cases, a failed expensive instance, an exhausted pre-call budget,
and hostile supported regex/combinator/reference cases. Record cost-bound
inputs and independent observed work/allocation controls for every member of
`S`; retain the independent all-261 source census and existing corpus controls.
Shared validators count once in resident memory, never
as free per-impression work.

An invalid impression yields bidder-attributed code 2 with a bounded
fixed diagnostic, removes only that bidder's invalid impression, and
does not invalidate valid sibling impressions or other bidders. If no
valid impressions remain, make no call. Never echo instance values,
untrusted property names, full validator errors, credentials, or schema
text into diagnostics. Test alias schema selection and merged params,
not only direct bidder calls with already-valid inputs.

### 7.2 Host Settings and Credentials

Some required protocols need more than an endpoint: the source contains
authentication, supply identifiers, and bidder-specific constants.
Inventory these before a wave is called implementation-ready.

Non-secret constants belong in strict baked metadata. Operator-specific
values use typed validated runtime settings through the existing
configuration pipeline, namespaced by public base identity; schemas
and consumers live with the bidder contract. Runtime settings cannot
replace code, binding, vendor identity, capabilities, or endpoint policy.
Do not introduce an unvalidated string map of arbitrary headers/URLs.

Secret bytes stay in EdgeZero's existing secret-store path. Runtime
configuration may name reviewed secret references, not contain secret
values. After eligibility and privacy permit work, core resolves only
the participating requested identity's resolved references into a bounded request-owned
redacting view and passes it through a provider-neutral context. Coupled
credentials use one versioned secret bundle, read once for that bidder;
rotation must not mix a username from one version with a password from
another. Secret lookup stays inside the original auction deadline and
host admission budget. Bidders perform no lookup/I/O and do not cache
credentials in singleton instances. Share the view across its calls,
not across users or snapshots. Info/debug output never includes the
bundle or its values. Missing/invalid required host settings or
secrets fail that bidder closed, without outbound work or secret-bearing
logs; the catalog remains discoverable for operator diagnosis.

The foundation implementation plan must define the concrete typed
settings/secret-view API, byte/count limits, readiness diagnostics,
allocation charges and rotation consistency before implementing an
authenticated bidder. Reuse existing EdgeZero primitives; do not design
another config-signing layer or secret backend. A wave blocked on an
unsettled credential contract is not complete.

The approved [priority service contract](../plans/2026-10-26-priority-bidder-services.md)
brings only two shared prerequisites forward into bidder catalog: explicit, expiring
operator USD-per-unit floor-rate snapshots, and one bounded coherent
credential bundle per admitted requested identity. Resolve requested/base
settings before reading; aliases may deliberately name different references,
so do not cache by implementation or reuse another alias's bundle. Share the
bundle across that identity's synchronous construction calls, then drop it
before constructing the next identity. Its working allowance is prepaid once
in sequential per-auction scratch, not multiplied by call or catalog count.
Privacy, valid parameters, remaining call capacity and the original deadline
precede reads. Full bid-response conversion and external rate feeds remain shared auction foundations.

#### Generated Non-Secret Settings

AG approved bidder-owned field declarations on 2026-10-24. Adjacent
`bidder.toml` declares optional fields under `[settings.fields]`; the initial
closed kinds are `optional_i64` and `optional_string`. The approved credential
prerequisite adds only `optional_secret_reference`: a borrowed-string projection
whose present value must contain 1..=128 UTF-8 bytes and no ASCII controls.
Omission/null remain absent; references are not trimmed or case-normalized.
Invalid values are rejected during config admission with a fixed error, before
any store operation. This is not a general schema language. Discovery validates names and consumes the same manifest
snapshot for metadata and settings generation. `auctioneer-config` owns the
generated types; no central family list, runtime directory scan or additional
crate is required. A schema declaration does not activate a factory or promote
a public definition.

Generate Rust with `quote!` and `syn`, not escaped source-string assembly or a
runtime template engine. `quote`/token support are workspace-managed build/test
dependencies only; generated portable types have no templating dependency.
Identifier and literal interpolation must not splice unchecked Rust source.

Runtime overrides have an explicit implementation discriminator and object
payload, for example:

```toml
[bidders.publisher_alias.settings]
implementation = "msft"
[bidders.publisher_alias.settings.values]
hb_source = 0
```

Both envelope fields are required. Reject positional sequences, unknown fields,
unknown implementation keys, coercions and integer overflow. Missing/null
optional fields are absent; zero and ordinary empty strings remain explicit
values. A present empty secret reference is invalid rather than a fallback.
The public wrapper is opaque and exposes stable named projections rather than
ordinal enum variants. All settings/view Debug output redacts values.

Resolve identity first. Before constructing a participating bidder request,
check **both** requested and base entries against the independently compiled
binding of the actual resolved base. A wrong discriminator, even with an empty
payload, is a fixed bidder-local code-5 refusal, never an absent-setting
fallback. Host and request aliases use the same path. Config keys may name
request aliases whose binding is not known until the auction; validating such
keys against the pre-alias public name at config load would choose the wrong
implementation. Eligibility, privacy and endpoint authorization remain separate.

The original extracted bidder map moves into a pruned request-owned wrapper;
unrelated config/account overlays drop before fanout. Lookups and field-wise
requested-before-base precedence borrow it without cloning strings or building
a merged map. Defaults/normalization belong to the consuming bidder **after**
precedence: MSFT zero/missing values become 5/6, while an explicit alias zero
must first override a nonzero base. TTD's optional `supply_id` stays a typed
string; endpoint/path approval is not a settings validation side effect.

The predeclared typed-settings ceiling is 16 KiB serialized, including its
envelope and escaping, 1,024 nodes and depth 16. Existing config extraction
independently admits/funds original bytes and parsed values; push/load validates
the typed representation without a second JSON tree. Programmatic construction
must apply the same limits before exposing borrowed views. Existing config
funding owns moved storage, not a new copied per-bidder context term. Native
allocation controls are evidence for the move/borrow operation, not target-wide
schema, residency or WASM qualification. This non-secret foundation does not
complete coupled credentials or the MSFT/TTD executable protocol ports.

## 8. Bidder Transformation Contract

### 8.1 Request Construction

Bidders remain pure synchronous transforms. They receive the bidder's
privacy-projected request plus validated host context. All requests use
the existing sink's reserve-before-write transaction, metadata bounds,
endpoint validation, call budget, and finalization. Do not clone an
entire auction or prebuild an unbounded vector of calls/bodies.

Port grouping/chunking, method/path/query, headers, parameter placement,
multi-format handling, IDs, version fields and response correlation from
the reviewed protocol. Preserve exact impression coverage per call.
Some protocols split one multi-format impression into separate calls;
the impression ID alone cannot identify which format a response answers.
Add a bounded, immutable per-call correlation context to the sink-owned
metadata for such protocols. It may retain reviewed media-selection or
protocol correlation facts, not request bodies or arbitrary JSON. Fund
it before accepting the call and preserve its association through
finalization, positional fan-out results, and parsing, including responses
without a media hint. Do not recover correlation by retaining/re-reading
the sent body or by consulting another call's context. The foundation
plan must define its typed API and byte/count limits before the priority
wave. Test two calls covering the same impression with different formats,
responses without `mtype`, reversed completion order, and one failed call.

Track parser errors and valid sibling work independently. Required
partial construction failures need a bounded nonfatal diagnostic
channel; the existing semantic `Err` still discards accepted calls for
a genuinely bidder-fatal failure. Do not smuggle partial errors through
`Ok(())` or change rollback behavior without corresponding tests.

Every bidder-specific query/path value uses the existing structured URL
and encoding APIs. Extend approved endpoint macros only with a reviewed
contract. Request data may select a reviewed authority only through a
baked allowlist; it cannot become an arbitrary host, credential, scheme,
fragment, or redirect target. Apply section 6.1's AG-approved HTTPS default
without changing authority/path/query or bidding protocol; retain the original
URL/blob and effective transport difference. HTTP dispatch needs an explicit
scoped exception; keep Mocktioneer's existing feature-gated loopback scope.
Missing/unusable destinations or unresolved protocol implementations remain
unavailable/blocked. No global HTTP fallback or auto-downgrade on HTTPS failure.

Transport compression, connection handling, redirects, deadline cutoff,
cancellation and delivered body/header bounds remain exchange/EdgeZero
responsibilities. Bidder code must not add provider clients, retries,
timers, or secondary requests outside the shared call budget.

#### Canonical Request Admission

AG approved this boundary on 2026-10-27: shared ingress owns Auctioneer's canonical
OpenRTB 2.6 decoding, structural validation and rejection of legacy privacy
locations. Bidder code does not decode the complete request again or import
another server's framework DTOs for stored requests, analytics, cache controls
or other unused route features. This is an intentional request-boundary
difference, not a claim of complete source-wrapper decoder equivalence.

A bidder still validates every extension value its protocol consumes before
credential selection or any sink transaction. Rubicon's consumed set comprises
the supported supply-chain locations, legacy EIDs, DSA, first-entry multibid
`maxbids`, rewarded flags, GPID and slot/adserver fields. Validate supported
shadowed locations before choosing a winner, including an otherwise
parameter-invalid impression. Shared typed native fields are not reparsed.
Validation borrows the admitted tree; it does not clone, serialize/reparse or
build an alternative request representation.

Keep declared scalar/container types and widths, without inventing required
fields, enum-membership checks or a framework multibid clamp. Missing/null
values follow the consumed field's established contract. A null first multibid
entry is absent; later entries and selector fields unused by this transformation
remain outside its guard. Opaque extension JSON does not acquire an invented
grammar. Generic FPD map keys remain case-sensitive; supported DTO members keep
the reviewed single-spelling/ambiguity policy rather than guessing lost key
order. Consumed GPID/slot-data variants are admitted in one stage before secrets,
even where the reference wrapper's exact outer-key spelling delayed its own
type error to an impression-local stage.

An invalid consumed conversion field is a fixed, bidder-attributed code-2
refusal for the whole scoped bidder request. Test malformed fields in both
impression orders, shadowed values, absent credentials, zero sink transactions
and refunded/unspent permits. Keep genuinely impression-local parameter,
`rp.target`, media and floor failures distinct so valid siblings still execute.
This does not waive schema/resource qualification or authorize promotion.

### 8.2 Responses and Protocol Envelopes

Keep response bodies sealed. Add a reusable typed JSON-envelope parse
operation inside that boundary for bidders not using the standard
OpenRTB response. At baseline, transport bounds decoded bytes,
`BidderHttpResponse` carries the exchange's `max_parsed_nodes`, and the
parser owns its recursion guard; there is no exchange-supplied depth
field. Keep those owners explicit. The foundation plan must qualify a
fixed nesting guard and its error classification for the new operation;
a configurable depth budget would be an explicit API/accounting change.
Use fixed error classes, including refusal of unchargeable raw-value
capture. No borrowed/raw byte escape hatch, response cloning, or
bidder-supplied replacement limit is permitted. Non-JSON protocols need
a separately reviewed bounded parser before their wave can claim support.
Qualify retained heap and transient scratch for each reviewed envelope
DTO: syntactic node limits alone do not bound arbitrary typed allocation.
Candidate-local repeated decoding shares the original call's parse-work
and scratch allowance, not a fresh budget per candidate. Independently
vary decoded bytes, nodes, nesting, and typed expansion at their limits.

New envelope and candidate-local typed parsers must preserve the existing
`BidderErrorKind::ResponseParse` provenance for malformed, structurally
over-budget, and refused raw-value decode failures. Preserve it in both
the fatal error vector and a successful `BidderResponse::errors` vector
containing valid siblings. Post-decode semantic failures remain `Other`;
never reconstruct parser provenance from numeric codes or messages.
Acceptance must observe the resulting diagnostic qualification debug error class through both
channels, independently of whether a debug body projector is supported.

Preserve source no-bid/status behavior, currencies, deal metadata,
creative fields and bid-local errors. An envelope or whole-response typed
decode failure may fail the response where that is the protocol's rule;
once candidates have decoded, one semantic/classification-failed bid
must not erase valid bids in the same or another seat. A protocol using
independently decoded candidates must preserve their isolated typed
failures too. Fixtures must distinguish whole-envelope typed failures,
candidate-local typed failures where applicable, and post-decode
semantic failures; a classifier-only test cannot stand for all three.
Unknown impression IDs,
invalid prices, unsupported formats and invalid media declarations are
attributed and dropped through the shared validation rules.

Use the shared media classifier with explicit bidder hints where
required. Do not hardcode banner or duplicate divergent classifiers.
Maintain requested-name attribution through response bundles, warnings,
debug rows, targeting, and sync. Binding reuse does not merge privacy
decisions or bidder budgets. `DebugBodySchema` currently selects both
request and response projection. A proprietary/asymmetric protocol must
default to `Unsupported`, omitting both bodies, unless both directions
have reviewed bounded projectors. One OpenRTB-shaped direction does not
justify `OpenRtb` for the pair. A future per-direction selector requires
an explicit reviewed contract change. Test request and response omission
separately; parser-error provenance remains available independently of
body disclosure. Never use generic serialization as a projector.

## 9. Sync, Privacy, and Shared-Feature Boundaries

Import and validate each sync family's URL templates, supported types,
UID macro, host requirements, skip conditions, and default response.
Use the existing callback renderer, encoded privacy macros, selected
signal precedence, host/bidder permission gates, signed-cookie format,
rotation, transactional eviction, and complete-cookie 4-KiB ceiling.
No sync support is invented for a source row that requires unknown host
configuration. Cookie integrity still does not authenticate callback UID
ownership, and bidder catalog does not inject stored buyer IDs into auctions; shared auction foundations
owns that explicitly reviewed path.

### 9.1 Sync Identity and Family State

Carry each compiled public definition's validated `effective_vendor_id`
explicitly through catalog `Entry`/`Candidate` and `SyncerLookup` to Purpose 1
permission evaluation. Keep requested name, canonical-base/factory
provenance, effective metadata, and sync family distinct. Do not obtain
every baked alias's vendor from its base's `BidderInfo`. Test distinct
alias/base vendors with opposite consent in both directions, plus an
alias with no vendor, through the real resolver and emitted sync URLs.
Sync endpoints do not acquire auction-only `aliasgvlids` handling, and
this change does not add a per-candidate vendor gate to `/setuid`: retain
its existing host-permission/disabled-syncer ladder and UID trust boundary.

A cookie slot is keyed by sync family, not public identity. A UID written
through one family member can suppress another member's sync and is
visible as that family's stored state; it is not proof of another vendor's
permission or UID ownership. Preserve intentional family sharing, not
per-alias cookie isolation. Families follow reviewed source sync metadata
and inheritance, not factory equality: an alias with its own sync contract
must not be collapsed onto its parent's family merely because it is an
alias. Test same-family/different-vendor members in both orders, with
opposite grants and absent/live/expired cookies. First-attempt family
deduplication may suppress work but cannot manufacture a permission grant
or trigger a retry through a more permissive vendor. shared auction foundations must independently
authorize any later stored-UID auction ingestion.

The pinned metadata has 196 source-defined sync identities in 187 distinct
families before enablement/TLS/host-setting filtering; 162 identities have
no source-defined sync contract beyond possible support hints. These are
structural counts, not 187 ready or simultaneously storable destinations.
They follow the source's non-`supports` definition and alias inheritance;
the import report must reconcile them and list family membership, shared
members, configured versus host-required families, and source/host disables.
Host additions and intentional changes get separate totals. Keep the
4-KiB complete-cookie limit, signing overhead, eviction ordering and atomic
write refusal unchanged. No claim that all catalog families fit at once;
exercise high-occupancy selection/eviction without cross-user state.

### 9.2 Privacy and Shared Delivery

Privacy always evaluates effective bidder/vendor identity before any
bidder transformation. Preserve the privacy enforcement disclosure classifier, COPPA,
USP fail-closed decisions, activity controls and alias handling. A
bidder may not restore denied IDs/FPD/precise geo from original request
bytes, copied headers, another bidder's params, host state or cookies.
Unknown/absent vendor identity does not imply consent. GPP/TCF support
is recorded truthfully; unsupported jurisdictions do not become enabled
because the source metadata advertises a capability.

The capability ledger distinguishes protocol-level video/native support
from shared delivery support. stored requests and accounts owns stored requests/accounts; shared auction foundations owns
currency/multibid/stored-UID ingestion; creative caching owns caching; video delivery owns full
video/pods; first-party data through browser privacy compatibility own the remaining roadmap surfaces. For each required
dependency state the exact unavailable behavior and returned error. Do
not silently downgrade video to banner, discard required protocol data,
or advertise a route/format the server cannot deliver.

Migration completion means all bidder-local transformations, metadata,
schemas, privacy integration and applicable sync contracts are implemented
and verified, including branches used by future entrypoints. Such
branches may be exercised using reviewed injected typed context without
shipping the later route, cache, video delivery or player integration.
Those unavailable shared flows remain separately blocked capabilities
under their roadmap owners, not missing bidder code disguised as
complete. Stubs, omitted branches or a mandatory unavailable service that
prevents implementing/verifying the bidder still block migration. bidder catalog
closure does not require delivering every stored requests through browser privacy compatibility server feature.

## 10. Rust Import and Fixture Tooling

Add offline `auctioneer bidders` subcommands in the existing CLI for source
inventory/import, deterministic checking, and coverage reporting. Keep
build-only metadata/factory helpers in their owning existing crates;
do not add one workspace crate per bidder, a new tool under `tools/`,
or a registry-service product. Dependencies are declared centrally.

The import reads a locally supplied checkout at the exact pinned Git
object, never fetches or runs source programs in a Cargo build. Read
content and file modes from the pinned tree/blobs, not merely a working
tree whose `HEAD` matches: uncommitted edits must not change an import
labelled with the pinned revision. Verify this with a dirty-working-tree
control as well as wrong-revision and re-pinned-digest controls. Use
structured YAML/JSON/TOML parsers. Reject duplicate keys, escaping paths,
symlinked inputs, unknown executable/config fields, wrong revision,
missing source files and unaccounted metadata. Alias resolution follows
reviewed source inheritance, not a shallow generic merge. Unknown
metadata is reported for mapping, not silently discarded by serde.

Reference staging may use canonical JSON before promotion to runtime TOML.
Keep all 358 declared metadata objects and 261 raw schemas separate from
the shipped subset by explicit admission state, not separate bidder directories,
with a source license copy, source/output witnesses and
prominent conversion notices. Preserve known source-ignored spellings as
labelled facts, not effective settings. A native-only event preflight must
reject alias/tag expansion and enforce byte/node/depth limits before the
first YAML tree allocation, including inventory's parse. `serde_yaml_ng
0.10.0` remains the converter; pinned `yaml-rust2 0.11.0` supplies that
event admission without adding a runtime YAML loader. This reference stage
cannot authorize a factory, endpoint or runtime definition.

### Catalog Storage

**Single-folder ownership, approved 2026-10-23.** Every bidder-specific input
belongs below `crates/auctioneer-bidder/src/bidders/<implementation_key>/`.
There is one editable `bidder.toml` for registration, effective public metadata,
aliases and provenance. This replaces the former split between config data,
central per-bidder reference collections and source folders. The
[migration plan](../plans/2026-10-23-bidder-owned-assets.md) tracks implementation;
this section does not claim the existing readers or files have been migrated.

| Contents                                                                                 | Location                                                                                |
| ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Registration, effective public definitions, aliases and all bidder-specific provenance   | `<implementation_key>/bidder.toml`                                                      |
| A single canonical parameter schema                                                      | `<implementation_key>/parameters.json`                                                  |
| Additional independent public schemas sharing that implementation                        | `<implementation_key>/schemas/<public_name>.schema.json`, explicitly associated in TOML |
| Protocol code, private transforms and inline unit tests                                  | `<implementation_key>/mod.rs`                                                           |
| Separate conformance tests, when needed                                                  | `<implementation_key>/tests/conformance.rs`                                             |
| Protocol/parameter captures and bidder-specific source data used by tests                | `<implementation_key>/fixtures/`                                                        |
| Truly cross-bidder inventory, source-binding witnesses, legal notices and test machinery | Existing shared owners; never a second editable catalog                                 |
| Design, review and qualification reports                                                 | `docs/bidders/`                                                                         |

Each manifest explicitly declares `factory=true` or `factory=false`. Only the
former can have executable source; section 5.1's source/declaration consistency
checks apply. Effective definitions exist only after reviewed promotion and
require a real factory. Imported-only manifests declare no effective definitions.
Source activation, a schema, a folder, a compiled factory, or an imported HTTPS
endpoint independently grants no routing or migration approval. APS may retain
its real factory with no promoted definition. Import must never invent Rust
stubs, use Generic as a substitute, or silently promote the source catalog.

Public definitions and aliases retain exact public names, their own vendor,
sync and activation decisions, and explicit canonical-schema associations.
Two bases sharing an implementation are not aliases. Keep their source schemas
independent, even when their bytes match. Resolve ownership through the audited
factory binding, not name guessing. An alias referring to an existing schema
does not require a duplicate schema file.

TOML provenance records immutable repository/revision identities, source paths,
Git object IDs, source lengths/digests, retained artifact paths/digests, and
documented transformations or reviewed policy differences. Artifacts from
different pins name their own source identity; adding APS must not silently
change the catalog pin. Locally authored definitions declare local origin
without invented upstream provenance or human approval. Retained paths are
confined to the owning bidder folder and refer to regular non-symlink files.
Do not hash the containing TOML into itself. Independent source pins and
conformance expectations remain load-bearing when a manifest is re-pinned.

Both artifact witnesses and nested `retained` tables use `source_path`,
`source_bytes` and `source_sha256`. Artifact fields identify the upstream file;
the `retained` fields identify the local copy, with its path relative to the
owning bidder folder. Retained copies are still verified independently, and
permitted transformations still require explicit notes. Reject the former
`path`/`bytes`/`sha256` names rather than accepting compatibility aliases.

**Metadata-retention amendment, approved 2026-10-24:** metadata artifacts retain
their repository/revision, source path, Git blob, byte length, SHA-256 and
reviewed differences in `bidder.toml`, but have no local YAML capture or
`retained` record. Import and source-conformance checks read the pinned external
Git objects. Remove the 358 main-catalog YAML copies and the supplemental APS
copy; retain JSON schemas and protocol/parameter captures unchanged. Fresh
main-reference staging contains 521 files: 259 manifests, 261 schemas and the
shared license. Relevant source interpretations and ignored spellings belong
in TOML notes; there is no second editable metadata catalog or runtime YAML loader.

Use the same schema filename for each implementation. Parameter test captures
are separate documents under `fixtures/parameters.json`, not alternative
schema filenames. Attribution belongs in the root README; shared license
material has one cross-bidder owner rather than a copy in each bidder folder.
License-source object witnesses may remain in the shared reference inventory.

Mocktioneer is reference-derived, not local-only: its supplemental source is
`https://github.com/stackpop/prebid-server.git` at
`c9ae35110d45f4dca35efc064d97f955a48445bf`, on `feature/mocktioneer` when
verified. That adapter is absent from the fork's then-current `master`.
Keep its source witnesses separate from the original catalog and APS pins.
Preserve the existing feature-gated loopback metadata and bounded Rust protocol
implementation as explicit differences; provenance is not a parity claim,
new routing approval or schema-validation activation.

This provenance covers every input category, not only fixture JSON:

| Category                         | Required provenance                                                                                                                                                                                              |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| External implementation          | Exact revision, source implementation tree and relevant source-file object IDs/lengths/digests; referenced shared helpers and protocol/test producers; coverage associations and intentional port differences    |
| Parameter schemas                | Original path, revision, blob, raw length/digest and draft; explicit owning public definition/canonical association                                                                                              |
| Protocol/parameter JSON captures | Source/producers and raw artifact digests, capture/transformation method, operation/row shapes and counts, and independently captured expected outputs                                                           |
| Metadata and aliases             | Original metadata objects, exact public identity, source binding/parent and source-versus-effective field mapping; reviewed endpoint/default/vendor/sync differences, including inherited versus explicit values |
| Attribution                      | Source license identity and retained notice/digest at the applicable immutable revision                                                                                                                          |

The Rust port is not a verbatim source copy. Its TOML associations identify the
source implementation and explain differences; do not fabricate byte equivalence
or copy foreign-language source into the repository. The bidder catalog plan's Task 16 evidence
receipts bind actual Rust files, shared dependencies, generated validators,
schemas, metadata and captures to the tested commit/lock/toolchain/features and
run identity. Do not hand-maintain a guessed current commit or self-referential
manifest hash as a substitute. Changes invalidate the affected evidence closure;
updating a checksum alone grants neither review nor migration credit. Evidence
receipts are immutable results, not a second editable provenance catalog.

Original capture bytes remain verbatim. Moving an existing JSON provenance
manifest into TOML is an explicit, tested representation change, not permission
to rewrite its captured inputs or expected answers. Preserve its witness set
and delete the redundant JSON manifest after consumers switch. TOML has no null:
keep source null/missing distinctions and source-ignored spellings in original
metadata snapshots, never invent a null encoding or silently turn them into
effective settings. JSON Schema remains JSON, with its original draft and bytes.

Physical ownership does not reverse crate dependencies. The config build owns
data-only parsing, validation and metadata generation; it reads the fixed sibling
`../auctioneer-bidder/src/bidders` relative to its manifest, never bidder Rust code,
constructors or another crate's `OUT_DIR`. The existing bidder-to-config
dependency remains one-way. Both builders consume the same strict structural
contract; inactive inputs are validated before feature filtering. Only explicit
`insecure-bidders` ownership can select the existing insecure URL policy.
Provenance and reference captures must not inflate guest metadata allocations
or enter request-time parsing. Keep generated Rust/validators in `OUT_DIR`.

This sibling-source contract is workspace-only while packages are unpublished.
Config-only copies and provider qualification/preparation workspaces must include
the data closure and shared build reader. Missing inputs fail rather than
falling back to old metadata trees. Register directory/member/reference/feature
inputs for incremental rebuilds. Do not introduce runtime filesystem discovery,
a new crate, a central bidder roster, or arbitrary source/constructor paths.

Import/check and schema controls share these local files, not duplicate copies.
Import publishes only a fresh staging tree, never overwrites reviewed TOML/code,
and preserves atomic no-replace publication and no-write checking. A coordinated
cutover removes the legacy metadata/schema/provenance readers; no production
dual-read or compatibility fallback remains. Shared historical acceptance
records are not rewritten to make their old paths appear current.

Disable checkout newline normalization for captured JSON, source licenses and
TOML witnesses, not only raw schemas. Keep the raw schemas' additional
`-whitespace` Git attribute. Exercise Git's actual checkout filters
with `core.autocrlf=true` on protected LF/CRLF schema, metadata, manifest,
license and witness inputs; an unprotected LF positive control must transform,
so disabling conversion cannot make the guard vacuous.

Generate reviewable TOML and provenance without overwriting hand-written
bidder code. Import/check output is stable across source file order.
Failed import must not leave a partially updated catalog. `--check`
performs no writes and returns nonzero for drift. Imported source URLs
are data requiring safety review, not permission to rewrite production
destinations or weaken validation.

Preserve external reference captures and source/license attribution;
do not generate expected outputs by executing the Auctioneer implementation
being tested. All in-repository tooling is Rust. External source capture
is a separately documented provenance step, not checked-in foreign-language
programs or an implied capability of a validate-only command.

Each bidder owns request/response fixtures with exact operation sets,
row shapes, source digests, and positive/negative counts. Drivers assert
nonzero/exact dispatch counts and reject unknown operations. Compare
actual transformed output, including partial errors, rather than just
fixture hashes. At test/check time, verify the immutable source revision
and capture/fixture digests against the committed provenance manifest;
do not accept a new revision just because its digests were re-pinned.
Where the source checkout is supplied, compare the referenced pinned
blobs as well. Corpus edits must fail before behavioral dispatch on a
digest mismatch. Put unit tests with implementation modules; integration
drivers live with their actual cross-crate/process boundary owners; bidder-local
captures and conformance live in the bidder folder described above.

## 11. Resource, Portability, and Verification Contract

### 11.1 Accounting Owners

Apply the [shipped-schema amendment](2026-10-29-shipped-schema-qualification-design.md)
to the validator population, not the equation: `S` contains every shipped
validator/helper in the actual target/feature artifact, including disabled
rows and existing bindings. Fixed catalog metadata and any other linked owners
still contribute their real costs. Artifact measurements corroborate but do
not replace required proved/enforced admission facts. Unused future engine
branches need no speculative private-layout reconstruction for bidder catalog.

`auctioneer-runtime/src/budget.rs` owns the `CatalogResourceCosts` input,
inspectable derived terms, checked arithmetic and multiplicities; this is
an extension of the existing equation, not another budget system.
Generation has two disjoint owners, with no config-to-bidder dependency:

| Owner                                                                            | Contribution and verification                                                                                                                                                                                                                              |
| -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `auctioneer-config/build_codegen.rs`                                                 | Emits bounds for its generated metadata/binding static data only; a source-schema association/digest is metadata, not a bound for generated validator code/data                                                                                            |
| `auctioneer-bidder/build_schema.rs`, called by `build.rs`                            | Owns generated validator code/data and any lazy-cache descriptors; emits target-specific scalar bounds tied to schema, generator/helper source, target ABI, features, toolchain and lockfile identities from the reviewed qualification record             |
| `auctioneer-bidder/src/catalog.rs`                                                   | Aggregates those descriptors with its registry/index/bidder costs into one checked `CatalogResourceCosts`, including C/I/V/K; deduplicates shared allocations and checks that its components have matching qualification identities before materialization |
| `auctioneer-bidder/tests/catalog_alloc.rs` and `auctioneer-cli/src/bidders/resources.rs` | Verify allocation bounds and post-build artifact/code/data contributions against the descriptors on each applicable target; a build script running before macro expansion cannot measure the final executable merely by counting emitted source bytes      |

The schema generator fails on missing, stale or unsupported cost records;
it cannot substitute source-file sizes or hand estimates for qualified
upper bounds. The catalog aggregator is the single place computing C from
metadata, validator and registry residency; core passes its result to the
existing runtime derivation rather than recomputing it. Provider adapters
do not estimate these components independently. Qualification records include
failed initialization, cache sharing and target-specific layout/work bounds.
Independently vary metadata, validator static data, lazy caches and registry
contributions: each must reach C exactly once. Missing/stale descriptors and
swapping one owner's contribution for another fail before constructors.

| Lifetime                   | Required term and multiplicity                                                                                                                                                                                                                                                                       |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Artifact/isolate residency | `catalog_resident_term = C`, once per isolate: disjoint generated metadata/validator static data, registry/indexes, bidder instances, any distinct lazy caches and attributable executable-code residency owned by the target ceiling, including disabled definitions; shared allocations count once |
| Initialization             | `catalog_initialization_term = I`, once per isolate: temporary registry/join/lazy-cache initialization and failure-cleanup scratch, excluding resident `C`; schema code generation is build work, not guest initialization                                                                           |
| Auction                    | `schema_validation_term = V` and `catalog_context_term = K` per admitted auction; `K` funds introduced settings/secret/correlation/diagnostic work not already charged by an existing owner                                                                                                          |
| Auxiliary request          | `A = aux_term`: rederive catalog/alias/chooser/info projection and serialization scratch inside the existing auxiliary peak and its existing permit/handoff equation, never `C` per request                                                                                                          |

**Keep the existing additive accounting model.** The code has no startup/
steady lifecycle partition today, and bidder catalog does not introduce one. It retains
all existing simultaneous-population reserves and adds initialization scratch
once even if the implementation normally initializes before admission. This
is a conservative bound, not a claim that every funded byte is simultaneously
resident. In particular, do not replace the current checked sum with a
`max(startup, steady)` that can hide `I` behind a larger auction term.

The exact existing-term mapping in
`auctioneer-runtime/src/budget.rs::derive_isolate_budget_with_baked_config` is:

| Symbol | Existing source term, retained without a phase split                                         |
| ------ | -------------------------------------------------------------------------------------------- |
| `B`    | `limits.baseline_bytes`, including its already documented adapter startup/residency reserves |
| `G`    | `gvl_terms`, including cache and request-pinned populations                                  |
| `Q`    | `max_concurrent_auctions * per_auction_peak`, before the new `V`/`K` additions               |
| `E`    | `egress_term`                                                                                |
| `A`    | `aux_term`, including both auxiliary and probe-config permit populations                     |
| `T`    | `terminal_term`                                                                              |
| `P`    | `probe_output_term`                                                                          |
| `H`    | `pre_admission_parser_term`                                                                  |
| `S`    | `max_active_body_streams * per_stream_peak`                                                  |

The new checked sum is
`total = B + G + Q + E + A + T + P + H + S + C + I + N*(V + K)`,
where `N = max_concurrent_auctions`. `V` and `K` enter the existing
`AuctionCharges` receipts and their `per_auction_peak` derivation; the expanded
notation above shows their multiplicity, not a second charge after including
them in `Q`. Recomputed `A` includes its introduced per-permit catalog scratch;
do not append that scratch again. The baseline input `B` remains the matched
pre-bidder catalog allowance. Document existing catalog/data/code reserves already inside
it; `C`, `I`, `V` and `K` charge non-overlapping additions. Do not subtract an
unmeasured old reserve or an unrelated saving to make catalog growth fit.
Record generated code/static-data artifact growth and its target memory owner:
guest linear-memory data belongs in `C`; new native/host/JIT code residency
also enters `C` where the target ceiling owns it, separately from the unchanged
pre-bidder catalog allowance `B`. If the platform's memory definition or observation is
unavailable, record that limitation under resource qualification rather than assuming a zero
physical cost or claiming absolute fit. A smaller package alone does not prove
a smaller resident bound.

The scalar descriptor must be available before materializing new registry
state or lazy caches so the admission proof does not allocate first. Extend
the existing derivation API with that mandatory descriptor on production boot,
strict-validation and relative-gate paths. Descriptors must be finite verified
upper bounds, not allocator observations rounded down to current consumption;
missing generated-validator bounds block foundation qualification even if the
observed sample is small. Changing to phase-aware reclamation requires resource qualification's
separate lifetime review and tests; it is not an implicit bidder catalog optimization.

`baked_config_bytes` remains the host/account-default cloning allowance.
It must not absorb bidder metadata through its per-request
`CONFIG_PARSE_FACTOR = 128` charge. Resident catalog/schema memory enters
the isolate total once, not once per auction or auxiliary permit; request
scratch enters its actual concurrency owner. Disabled entries still
consume resident memory, but an auction must not clone the full catalog.
Keep one checked `BAKED_CONFIG_BYTES` cache in
`auctioneer-core/src/host_primitives/gates.rs`, limited to the exact
`(host_default(), account_default())` tuple. Move its `get_or_init` block
from `install_boot_envelope` into section 11.2's non-installing
`resolve_budget_inputs` facade or its private shared helper, so boot and
strict callers use the same cached counting result. Do not append catalog
data to that tuple or require gate installation to initialize this cache.
An owning core test grows metadata/schema/registry residency while holding
both defaults fixed: the serialized-default allowance must remain identical
and only the catalog descriptor changes. Runtime controls independently
vary `C`, `I`, `V`, `K`, auctions and auxiliary permits with literal expectations:
one byte of `C` or `I` costs one isolate byte, including with zero live auctions;
one byte of `V` or `K` costs one byte per admitted auction, not per alias.
Configured concurrency remains at least one as required by the existing scalar
validator; an idle startup does not set its reserved auction population to zero.

**Resource protocol cutover.** Native resource exports, qualification profiles,
observations and evaluation reports use schema version 2 when carrying this
descriptor. This is a hard cutover, not an optional extension of version 1.
Their shared `resources.catalog_costs` object requires all four integer fields:
`resident_bytes`, `initialization_bytes`,
`schema_validation_per_auction_bytes` and `context_per_auction_bytes`.
Zero is explicit, never a deserialization default for an absent descriptor or
member. Reject version 1, unknown versions, missing/unknown/duplicate members,
wrong numeric types and arithmetic overflow at the active consumer boundary.
Candidate workload, fixture, provider-control and runner protocols retain their
own versions; this resource change does not renumber unrelated protocols.

Native export includes the four corresponding derived terms alongside its
existing terms. The harness independently rederives total and these terms,
and verifies `effective_profile_sha256` over compact serialization of the typed
`resources` object, using its declared field order (not JSON key sorting).
Qualification copies the exact descriptor into profile and observations,
includes it in both resource and full-profile identity hashes, and rederives
the four terms in the report's accounted result. Independently changing any
descriptor member must change the resource identity, affect its owning term,
and make unrebound observations fail identity matching. Forged derived terms
or a stale resource digest refuse before measurement.

Historical captures remain byte-identical version-one evidence for their
pinned executable and CLI; the active version-two command rejects them rather
than claiming a fresh qualification. Their integrity checks continue without
reevaluating them. Test-only fixtures may explicitly construct new version-two
inputs from historical shapes, with literal costs and rebound digests, but
must not label these synthetic inputs as new captures or add a production
migration/defaulting path. Before shipping positive catalog costs, exercise
nonzero descriptors through native serialization, real CLI parsing/evaluation
and the harness's export/binding boundary, not only runtime unit tests.

Use literal unequal terms to exercise every existing summand, independent
auxiliary/probe-config populations, and arithmetic overflow. Increasing `I`
must increase total even when auction demand dominates it. Removing a term
or multiplying `C`/`I` by config factors/permits must fail its owner test.
The foundation plan must supply concrete descriptor bounds before cutover.
Further tests independently vary catalog size, schema-cache sharing,
initialization scratch/failure, auction concurrency and auxiliary permits,
and detect omission or double charging of each new term.

Re-derive auxiliary-route work at the admitted catalog size as well:
`/info/bidders`, detail, and `/info/bidders/all` must fund their catalog
maps, effective alias/activation views, serialization scratch, and output
bytes, including disabled definitions. Prove the complete aggregate fits
the installed auxiliary reservation and response cap at 512 identities;
do not assume the existing flat `AUX_MANIFEST_BYTES` reservation still
establishes the bound. Preserve all route admission and egress ownership.

Resolve aliases and activation by catalog indexes, not repeated linear
searches through all definitions for each alias. Use a deterministic
indexed join plus ordered rendering. Resolve baked aliases once during the
validated startup join. For each runtime host alias, allow at most two
base-index probes, each with at most `ceil(log2(N+1))` name comparisons for
`N` compiled identities and `H` effective host aliases, including inherited
host defaults, with `N+H <= 512`. At the aggregate limit, use 512 compiled
identities/no host aliases and 448 compiled identities/64 host aliases;
the latter allows at most 1,152 probe comparisons. Also merge 64 inherited
and 64 distinct runtime aliases with 384 compiled identities: the combined
128 aliases permit at most 2,304 probes' name comparisons, not a new
128-entry runtime-overlay allowance. Ordered rendering has
a separate linear bound of `4*(N+H)` name comparisons, at most 2,048 in
all three rows; already ordered indexes must not be sorted anew per alias.
Count probe and rendering comparisons separately. Late-key and absent-key
controls must reject the per-alias full-catalog scan, not merely assert a
loose global threshold. These are name comparisons, not a bound on string
length: existing name-byte caps remain enforced. Instrument the actual
bounded ordered-index search and reuse its resolved entries through
config validation, auction, sync and info paths. Merely switching to
`BTreeMap::get` is not proof of this exact comparison bound.
Small serialized output is not a CPU bound for an unauthenticated info
request that first builds the full view.

### 11.2 Target Resource Acceptance

Package size, accounted demand and observed resident memory are different
results. The current WASM envelopes already fail absolute fit before bidder catalog.
The [resource qualification baseline decomposition](2026-10-17-resource-envelope-design.md#2-source-grounded-baseline)
includes the current normalization and request/debug increments. Its complete
source-derived expressions have these numeric-only lower bounds, before the
positive target-layout and boot baked-default terms:

| Target          | Numeric demand lower bound, bytes | Declared ceiling, bytes | Approximate numeric demand/ceiling |
| --------------- | --------------------------------: | ----------------------: | ---------------------------------: |
| Cloudflare      |                   138,047,236,632 |             128,000,000 |                           1,078.5x |
| Fastly          |                    23,589,436,420 |             128,000,000 |                             184.3x |
| Spin hosted row |                    23,589,436,420 |             134,217,728 |                             175.8x |

These are conservative source-accounted bounds, not executed reports or
measured GB of live heap. resource qualification retains the older 58,064,844,288/10,259,037,696-byte
golden subtotals as history, not the current problem's full magnitude. Exact
totals additionally include target-layout and baked-default terms. The existing
`envelope_warns_over_ceiling_on_*` tests expect
`BudgetOverCeiling` and zero margin. Fastly/Spin already have one admitted
auction and their single-auction bound exceeds the ceiling: changing only
concurrent-auction admission cannot fix those profiles.

#### Absolute-Fit Prerequisite: resource qualification

resource qualification owns remediation of these pre-existing envelopes in the runtime/core
and provider profiles. Its dedicated [design](2026-10-17-resource-envelope-design.md)
and [plan](../plans/2026-10-17-resource-envelope.md) are in this
same bidder catalog PR. Policy/accounting foundations and scoped ownership repairs are
implemented; complete remediation and WASM fit remain open. AG authorized
proof-backed unchanged-workload repairs and accounting corrections, not a
lower-workload package. On 2026-10-19 AG explicitly approved continuing bidder catalog
catalog implementation with resource qualification readiness blockers retained. This does not
bypass bidder catalog's schema, allocation-descriptor or matched-catalog gates. The resource qualification documents
require exact normalized baseline term reports, lifetime and
population audit, bounded sharing/streaming opportunities, and explicit
versioned workload decisions at the initial checkpoint. Even numeric-only
gaps need approximately 99.47-99.91% demand reduction; no independently
justified unchanged-workload package closes them at the planning baseline.
The checkpoint presents workload/profile proposals for all three WASM targets;
an explicitly approved partial package is a normal initial disposition, not
permission to call residual demand fitting. Any lower workload ceiling
requires a reviewed product/profile decision; no silent call/body/node cap
reduction, removed charge or larger deployment ceiling is allowed. A
coefficient/lifetime reduction needs new independent bounds and execution
evidence. Per-target blockers and the next required evidence stay tracked
even while catalog migration proceeds.

Share **input resolution**, not only the final arithmetic. In
`auctioneer-core/src/host_primitives/gates.rs`, extract the existing boot input
preparation into a public, non-installing `resolve_budget_inputs` facade,
re-exported through `host_primitives`. It calls the existing crate-private
`configure::extraction_limits_for(Some(raw_limits))`: zero
`max_runtime_config_bytes`, `max_runtime_config_blob_bytes` and
`max_config_json_nodes` resolve respectively to the SDK's effective backend,
blob and node defaults; nonzero declarations remain unchanged. Copy these
three effective caps into the normalized row. Resolve `baked_config_bytes`
from the counting serializer over exactly
`(auctioneer_config::host_default(), auctioneer_config::account_default())`, using the
same checked cached result as boot. Return both inputs; no gates, registry
or provider bindings are installed to obtain them. An external strict caller
must use this public facade, not duplicate the crate-private resolver or
silently pass a raw row or zero baked-default allowance.

Boot installation, adapter resource-profile export and the bidder catalog CLI strict
resource checker use those resolved inputs plus the same target constants,
owning construction costs and catalog descriptor. Call the same
`derive_isolate_budget_with_baked_config` derivation extended under section
11.1. The existing zero-baked-default `validate_isolate_limits` call is
insufficient unchanged. Require section 11.1's checked **additive total** plus
headroom to fit `min(configured_isolate_bytes, platform_memory_bytes)`;
there is no separate startup/steady equation or lower total for strict mode.
bidder catalog's matched reports implement this shared preparation now; resource qualification owns
repairing absolute target fit, not permission to leave two input contracts.

Make parity **differential across the real consumers**. Start with the same
raw declaration, exercise actual boot installation in a fresh process, then
compare its retained normalized caps/baked bytes/derived terms against the
strict path. Include all-zero caps, each independently zero, unequal nonzero
caps and nonzero baked defaults; assert the three effective cap values and
the complete tuple's byte count against independent expectations. Skipping
normalization or baked counting in either consumer must fail its named case.
Calling the pure derivation twice with already-normalized inputs is not this
proof. The CLI also needs an actual-command zero-cap control so bypassing its
core facade cannot escape a core-only test.

Every control that installs boot gates runs in a dedicated **single-test
binary**, with one fresh child process per distinct installing row and a
bounded, counted driver. `BOOT_GATES`, `BAKED_CONFIG_BYTES`,
`PROBE_EGRESS_GATE` and `OVER_CEILING_WARNED` cannot be reset by a test mutex or
`--test-threads=1`. Assert the pre-install state before each child installs;
never test a second row against the first row's cached gates. Keep the pure
arithmetic/default-byte-isolation/C/I/V/K/A/auxiliary-probe/ceiling sweeps in
a separate non-installing target. Independently vary I under dominant auction
demand, K and auxiliary/probe populations, as well as unequal ceilings and
exact/over fit. Only the policy differs: boot currently warns on over-ceiling
demand while strict qualification rejects it. Any future phase-aware resource qualification
amendment must change the shared derivation and both consumers together, with
new lifetime proofs; it cannot introduce a private, more permissive equation.
The headroom floor
is `ceil(effective_ceiling/16)`, with at least 256 MiB on native: 8,000,000
bytes on the current Cloudflare/Fastly rows and 8,388,608 on Spin. A one-byte
placeholder does not meet the contract. Pin those independent literals and
test the floor plus under/at/over fit using unequal configured/platform
ceilings. bidder catalog native checks retain the already-passing 256-MiB floor.
The dedicated [resource qualification acceptance policy](2026-10-17-resource-envelope-design.md#31-shared-headroom-policy)
requires one shared minimum-headroom policy in the resource checker and
qualification evaluator. Its [platform validation contract](2026-10-17-resource-envelope-design.md#32-platform-and-separate-stack-validation)
also requires the SDK memory-envelope result and justified separately capped
stack bounds; primary-memory fit alone cannot pass required-fit mode.
At the pinned SDK revision host-ingress accounting is unknown on all four
targets, while Axum also lacks ceiling/population facts and Cloudflare lacks
an inbound bound. Therefore aggregate required-fit exit zero additionally
depends on [resource qualification's external prerequisites](2026-10-17-resource-envelope-design.md#53-auctioneer-and-edgezero-responsibilities),
not only local memory repair. Report completed Auctioneer patches, numeric fit and
external blockers separately; no `Indeterminate` result becomes a pass.
The [resource qualification audit checkpoint](2026-10-17-resource-envelope-design.md#41-required-audit-and-feasibility-record)
also requires complete named-term records, dominant-cost and full-gap decisions,
and AG's approval of an immutable exact-path package for changes outside the
existing unchanged-workload/accounting authorization. Scope checks
do not authenticate that approval or convert partial progress into aggregate fit.

Warn-and-install, synthetic ceilings, package-size results and local native
measurements cannot close resource qualification. It blocks WASM memory-readiness claims and
the corresponding publisher pilot deployments, not starting bidder catalog registration or bidder
work. Full four-target runtime readiness requires both resource qualification and the bidder catalog
catalog checks below; catalog completion alone must not be called that.

#### Catalog-Attributable Gate: bidder catalog

For each of the four actual target rows, freeze a machine-readable pre-bidder catalog
baseline: normalized limits, every derived term, baked-default allowance,
catalog ownership split, SDK/toolchain/build identity and absolute-fit
status. This is a new source-bound accounting record, not a rewrite of diagnostic qualification
measurements. The WASM absolute-fit status starts failed/blocked, not zero
cost or a passing memory qualification.

bidder catalog's required gate, reused by remaining catalog closure, checks catalog-attributable growth against this matched
baseline. Before implementation/measurement, the reviewed foundation plan
must pin per-target resident, startup, validation and auxiliary delta
allowances, plus changed bidder construction/response-parse contributions
under their existing owners, with finite numeric bounds. Missing allowances
refuse the gate; observing a larger cost does not authorize re-pinning them
without review.
Use the same workload and target constants on both sides. Check exact
catalog-delta ownership/multiplicity against independent expectations,
checked target-specific layout/work bounds against those allowances, and no
increase or omitted charge in unrelated terms. Report each component:
an unrelated reduction cannot hide an over-budget catalog component.
Allocator/runtime observations separately check those bounds where the
target can execute; an unavailable meter is a pending observation, not a
failed or forged result of the checked equation.

For already-fitting native profiles, absolute fit/headroom must also remain
passing. For pre-existing overcommitted WASM profiles, passing the relative
gate means only bounded, fully funded catalog growth; it does not mean
fit under the declared platform ceiling. Report relative acceptance and
absolute readiness separately; neither `--require-complete` nor a passing
build may promote the latter.
The checker recognizes `resource qualification` as a target-readiness dependency owner; that
pre-existing blocker alone does not mark bidder-local migration blocked.
Failure of a required catalog build or relative gate blocks acceptance of
the affected shipped set under its owning capability, bidder catalog, bidder expansion or remaining catalog closure; remaining
catalog work cannot become a new prerequisite for bidder catalog closure.
When resource qualification changes a workload or accounting owner, review/rebase both
baseline and candidate together, preserving the recorded old result and
catalog delta; never reset a baseline merely to absorb a regression.

Record target-appropriate initialization/cold/warm allocation observations
for the introduced catalog/schema costs with credential-free local
execution and isolated bindings where supported. A substituted native
baseline is not a measured WASM baseline. Missing runners/meters, including
a Spin guest that never boots, remain explicitly pending/blocked, not
passing observations. These missing observations block target qualification,
not deterministic catalog migration/build completion. They do not authorize
bidder omission or a full-catalog runtime-readiness claim; the bidder catalog checker
must retain the blocked observation and resource qualification owner in its capability report.

These are bidder catalog catalog resource checks, not credentialed deployed-handler
tests or publisher certification. The latter, actual deployment Q2-D and
authorized corrupt-config injection/recovery remain publisher pilot. No hosted
credentials are required to implement bidders or run checked accounting.

### 11.3 Required Proofs

Foundation acceptance includes:

| Surface                | Required proof                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Discovery              | Unknown synthetic bidder executes without editing central production lists; existing-bidder metadata-only addition also executes; external source test support is not a production discovery exemption                                                                                                                                                                                                                                                               |
| Incremental generation | Generated path modules compile; add/edit/rename/delete and off/on/off feature changes affect actual execution in one reused target directory; stale output cannot retain removed bidders                                                                                                                                                                                                                                                                             |
| Invalid artifact       | Duplicate/missing/feature-mismatched bindings fail before constructors; failure is cached; no partial registry                                                                                                                                                                                                                                                                                                                                                       |
| Metadata identity      | Complete pinned membership and alias graph, distinct effective vendor/schema/sync state, case-sensitive public names; host/request shadowing of compiled bases and baked aliases rejected before work                                                                                                                                                                                                                                                                |
| Info views             | Base-only omits baked/runtime aliases, not bases sharing factories; detail/all preserve alias provenance and effective metadata; activation and sync-only gates independently varied                                                                                                                                                                                                                                                                                 |
| Activation             | New imports do not activate automatically; runtime disable/re-enable works without reconstruction; baked disable wins; reviewed source-default reclassification, non-routable disabled definitions and alias-only family/direct routing are independently tested                                                                                                                                                                                                     |
| Catalog scale          | Named validate_catalog_size cutover at 511/512/513; five unequal count inputs, literal shipped tuple (512,256,256,256,256), caller-owner widening and term-removal controls, plus unchanged cookie/output bounds                                                                                                                                                                                                                                                     |
| Schemas                | All pinned dialect/features reviewed; known invalid/valid/merged/aliased params; zero uncontrolled fetches; precharged single/aggregate work and scratch, failed/expensive evaluations and exhaustion controls                                                                                                                                                                                                                                                       |
| Authentication         | Missing/malformed/rotated references, distinct base/alias settings, no cross-request state or diagnostic disclosure                                                                                                                                                                                                                                                                                                                                                  |
| Requests               | Real sink, grouping/chunking, zero/one/many calls, independently bounded body/meta/correlation context and deterministic partial failures; same-impression media-specific calls keep their context under reordered results                                                                                                                                                                                                                                           |
| Responses              | Real sealed parser, independently varied byte/node/nesting/typed-allocation bounds, standard/proprietary envelopes, mixed good/bad bids across seats; parser provenance preserved through fatal and partial-success channels; both-direction debug omission                                                                                                                                                                                                          |
| Privacy                | Independent deny axes and mixed aliases on actual serialized outbound bytes for every shipped implementation family                                                                                                                                                                                                                                                                                                                                                  |
| Sync                   | Host/config/privacy precedence, actual alias/base vendor divergence, hostile values for every privacy macro, source family counts/membership, shared-family grant isolation and unchanged signed-cookie limits                                                                                                                                                                                                                                                       |
| Budget                 | Runtime-owned C/I/V/K multiplicities, separately qualified metadata/validator descriptors and one checked catalog aggregator, core default-byte isolation, differential raw/zero-cap boot-versus-strict preparation and derived terms in process-isolated controls, target-specific catalog delta allowances, full-catalog info/all scratch and comparison bounds; independent expectations detect omissions, 128x resident multiplication and caller-limit widening |
| Portability            | Native behavior/fit plus actual Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1`, Spin `wasm32-wasip2` builds/package checks and matched per-target catalog-delta gates; absolute WASM fit remains resource qualification, with pending runtime observations reported separately                                                                                                                                                                                           |
| Completion             | State derived from current binding/schema/branch/fixture/build evidence; forged migrated status, stubs, omitted operations, stale hashes, same-input prior-run replay and failed/skipped/missing producer evidence rejected                                                                                                                                                                                                                                          |

Run secure-default and feature-enabled tests separately to avoid Cargo
feature unification erasing secure-default coverage. Workflow guards
must enumerate actual integration targets, protect the commands/features
and trigger/step execution, and preserve existing package ceilings.
No ignored test may be credited without its explicit driver.

Every delivery wave reruns the shared regression gates and its real
provider application builds and target resource gates. Record exact
source/toolchain/SDK/features, catalog size, package sizes, schema
initialization cost, and memory/CPU growth. Do not raise deployment
ceilings, change the established measurement
or build profile to disguise code growth, or claim all-target viability from
native compilation. Every target builds the same shipped catalog at each
wave, and the full source catalog at remaining catalog closure closure; pre-existing
absolute-fit failure remains blocked under resource qualification and a new catalog-relative
failure blocks the corresponding bidder catalog/bidder expansion/remaining catalog closure slice. None may be waived by an
ignored gate or silent per-target bidder omission. A packaging change needs
explicit review and cannot silently shrink full-catalog coverage.

Targeted guard-removal checks must cover registration fallback, schema
rejection, alias/vendor identity, runtime opt-in, mixed valid/invalid
sibling work, sealed parse budgets, privacy axes, exact fixture dispatch,
and source-pin/set-equality checks. Vary independent limits independently;
do not give both sides of a boundary the same fixture value.

## 12. Delivery and Exit Criteria

### Foundation Slice

Deliver the pinned ledger/import checks, automatic registration cutover,
validated baked aliases and runtime activation defaults, scaled/funded
catalog paths, schema-engine qualification/enforcement, and the shared
protocol/correlation/settings/secret interfaces needed by the priority
wave. Update the older registration draft and architecture to reflect
what actually ships. Preserve diagnostic qualification behavior/evidence boundaries; no claim
that old resource captures qualify the new catalog.
Freeze the four matched accounting baselines and approve the numeric catalog
delta allowances before the first catalog measurement. Track resource qualification's exact
WASM blockers separately; its absolute-fit repair need not delay this slice.

### Priority Slice

Deliver the 14 requested names in small PRs after foundations. APS requires its
separate reviewed integration contract rather than a fabricated source import.
Each PR includes metadata/schema/settings mapping, independently sourced behavioral
fixtures, independent failure/privacy/budget tests, target build evidence,
and precise capability/live-certification status. Missing real credentials
do not prevent deterministic protocol tests; fabricated live readiness
does not close the row.
Author and approve the source-specific wave plan/branch manifest first,
then implement it as a separate acceptance step. Record the frozen plan
identity in evidence; implementation cannot quietly redefine its own
required driver set or count writing the wave plan as delivery.

### Post-Pilot Waves (bidder expansion) And Residual Migration (remaining catalog closure)

After the selected Trusted Server embedding/publisher pilot test path passes, deliver the frozen bidder expansion queue
without waiting for stored requests through browser privacy compatibility. Order smaller reviewed waves by demonstrated
publisher demand/adoption, using the roadmap's categories, not guessed rankings.
Group by protocol and shared prerequisites within that evidence-led order,
not alphabet alone. Review each wave's exact membership and blocking
dependency list before implementation.
Reusing a factory requires proof of protocol equivalence, not a shared
vendor/URL. Existing implementations receive the same audit as new ones.
Repeat the applicable composed standalone/embedded, activation/privacy/sync,
schema/resource and target gates for the actual expanded set. Preserve separate
rollout authorization and readiness claims. Rows outside the reviewed queue or
requiring later shared features remain assigned to remaining catalog closure; no row disappears when
the selected queue finishes. Task 20's full-catalog terminal command remains an
remaining catalog closure requirement, not an bidder expansion partial-wave success criterion.

### bidder catalog Closure

Under the [2026-10-26 correction](#2026-10-26-scope-correction), bidder catalog is complete
only when foundations and the full 14-flow priority checkpoint, including
Sharethrough, Rubicon and APS, pass with current executable evidence. Require
registration/onboarding, schema-engine and complete shipped-set resource qualification
under the [2026-10-29 amendment](#2026-10-29-shipped-schema-qualification),
metadata/schema/alias/settings/secret/sync contracts, all required priority-local
branches, and composed privacy/context/original-deadline/gzip acceptance.
No mandatory priority-local branch may be dropped, relabeled as shared parity,
or transferred to remaining catalog closure; a passing partial-flow check with unfinished required
branches is not bidder catalog closure. Preserve explicit later shared-delivery restrictions
under section 9 and all migration-state rules in section 3.3.

Repeat required native/secure/feature/provider build/package and all four
matched catalog-relative gates for the actual shipped set, with native
fit/headroom preserved and full-scale foundation controls retained. The
complete inventory remains checked, not falsely marked migrated. Tasks 20/21
and a green full-catalog `--require-complete` are remaining catalog closure gates, not bidder catalog prerequisites.
Keep resource qualification blockers, unavailable target observations, Trusted Server embedding and publisher pilot
credentialed deployment/publisher qualification separate. Protocol candidates
or local test totals alone cannot close bidder catalog.

### Full-Catalog Closure (remaining catalog closure)

remaining catalog closure is complete only when all 358 source rows derive `migrated` from
section 3.3's current executable evidence, with no unaccounted, stubbed,
fallback, or bidder-implementation-blocked migration row; the
14-flow priority checkpoint, including APS, and registration onboarding proof
pass; all metadata/schema/alias/settings contracts validate; and the required
native/secure/feature/provider build
and catalog-relative resource gates pass at the full catalog size, with
native fit/headroom preserved. Keep pre-existing WASM fit failures and
pending target observations in the result; they are not passed gates or
omitted bidders. resource qualification is independently required before claiming WASM
memory readiness, and publisher pilot owns actual deployment/publisher qualification.
Shared delivery capabilities may remain explicitly blocked under stored requests through browser privacy compatibility
after their bidder-local logic is complete and verified as section 9
requires. Retain live certification
and later shared-feature scope explicitly; do not convert local evidence into
publisher pilot deployment or publisher qualification.

The scope correction does not itself authorize unreviewed protocol or resource
changes or establish acceptance. Retain the reviewed foundation plan and
separately reviewable priority, bidder expansion and remaining catalog closure wave contracts; historical preparation
and verification records remain limited to the evidence they actually captured.
