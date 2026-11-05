# Bidder Catalog Program Reference (bidder catalog, bidder expansion and remaining catalog closure)

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

**Status: historical checklist, superseded for bidder catalog execution and completion.**
The controlled-native implementation and recorded local acceptance are complete
on the bidder catalog branch; merge review and remote CI remain separate. Unchecked items
below retain their original program scope, not an outstanding bidder catalog task count.
Do not mark deferred or superseded work complete merely to reconcile checkboxes.

> **CI snapshot retirement, 2026-10-30 (AG direction):** The Rust
> workflow-configuration snapshot target `ci_workflows` was retired on this date
> at AG's direction. References below to that target are historical, not
> instructions to recreate it or add workflow-YAML snapshot tests. Actual
> evidence-driver, producer-status and resource-acceptance requirements remain.

**Current bidder catalog authority:** follow the approved
[controlled-native completion plan](2026-10-30-controlled-native-bidders-completion.md)
for activation and resource acceptance. It supersedes proof-before-activation
and identical-target publication below, not the frozen 14-flow protocol contract.
Full source/ABI worst-case schema certification now belongs to resource qualification. Native
tested reservations, enforced limits, headroom and real handler acceptance stay
mandatory; guest readiness remains blocked.

Detailed task checkboxes and named-commit checkpoints below retain their
historical scope; they are not the current native completion verdict. Use the
[native verification record](../../testing/evidence/controlled-native-bidders-20261030.md)
and the current completion plan for that verdict. AG's frozen wave approval is
recorded separately in `priority-approval.toml`, which binds both original
contract files by digest without rewriting their planning-status fields.

| Scope | Current Authority |
| --- | --- |
| bidder catalog controlled-native delivery | [Completion plan](2026-10-30-controlled-native-bidders-completion.md), [native verification](../../testing/evidence/controlled-native-bidders-20261030.md) and [subsequent SDK verification](../../testing/evidence/edgezero-pr275-update-20261030.md) |
| resource qualification formal schema certification and WASM readiness | [resource qualification plan](2026-10-17-resource-envelope.md) |
| bidder expansion post-pilot expansion | [Roadmap ordering and wave requirements](../../roadmap.md#2026-10-27-post-pilot-bidder-expansion) |
| remaining catalog closure residual catalog closure | [Roadmap ownership](../../roadmap.md) and Tasks 20/21 below |

> **Historical execution guidance:** the detailed tasks below record the
> original program plan and its checkpoints. Do not restart them as current bidder catalog
> acceptance gates. Execute new work through its owning capability's approved
> plan using superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Close bidder catalog with automatic registration, bounded catalog/protocol foundations and full selected-flow acceptance for all 14 priority identities (including Sharethrough, Rubicon and APS); expand the bidder set after Trusted Server/publisher testing under bidder expansion; reconcile residual pinned-catalog migration under remaining catalog closure.

**Architecture:** Generate factories and schema-specific validation code during the build, joining factories with independently generated data-only bindings at initialization. Keep all bidder implementations in the existing `auctioneer-bidder` crate. Keep the complete reference inventory separate from the progressively shipped catalog; every shipped identity has a real implementation. Extend existing resource, privacy, configuration and EdgeZero paths rather than creating a second registry service or host stack.

**Tech Stack:** Rust 1.98.1, existing Cargo workspace and EdgeZero PR #275 at the workspace pin (see the [outbound adoption plan](2026-10-30-edgezero-outbound-adoption.md)); Serde/JSON/TOML, existing `validator`, offline Git object reads, `serde_yaml_ng = "=0.10.0"`, candidate `jsonschema = { version = "=0.58.6", default-features = false, features = ["macros"] }`. See the [subsequent SDK update](../../testing/evidence/edgezero-pr275-update-20261102.md) for the current source delta and verification; earlier captures remain historical.

---

## Historical Scope And Preconditions

- Specification: [bidder catalog design](../specs/2026-10-16-bidder-catalog-design.md).
- Planning baseline: `be057264cb1e676132a5a678e665928d7fa7d7fe`, plus the accompanying spec self-review corrections. Implementation has started with the offline reference inventory slice below; incomplete tasks remain pending.
- Source revision: `d7f8515b86258688304b0d9b6668c6a0e258bc9e`. Preserve the four Git object IDs in spec section 3.1.
- Exact source scope: 358 identities, 261 bases, 97 aliases, 259 implementation directories, 261 parameter schemas and 261 base bindings. Exclude only the pinned external `adapters/adapterstest/**` scaffolding.
- At remaining catalog closure closure the default source set is all 358 source identities, with separately tracked APS and local `generic`; `insecure-bidders` additionally includes local `mocktioneer`. Neither APS nor local definitions inflate source migration coverage.
- This is the bidder catalog/bidder expansion/remaining catalog closure capability program and detailed foundation plan. Task 18 first authors and obtains review of source-specific wave plans in a separate checkpoint, then implements them; this document does not pretend one generic recipe implements 259 different protocols.
- **resource qualification remediation is not implemented by this catalog plan.** Its dedicated [design](../specs/2026-10-17-resource-envelope-design.md) and [plan](2026-10-17-resource-envelope.md) are included in this same bidder catalog PR for review. Existing WASM absolute-fit failures remain Open. This catalog plan funds and checks catalog deltas, retains native fit, and builds the same shipped catalog on all targets. It does not qualify WASM memory or execute publisher pilot's credentialed tests.
- **Execution authorization, 2026-10-19:** AG approved proceeding with bidder catalog catalog implementation while retaining resource qualification readiness blockers. This permits independent catalog work, not marking resource qualification complete, lowering workload limits, bypassing schema/descriptor qualification or promoting a relative catalog result to target readiness. Continue local resource qualification repairs under their existing proof requirements; SDK facts, absolute fit and remaining allocation/stack proofs retain their owners.
- No new workspace crate beyond the explicitly approved `auctioneer-macros` compile-time refactor, checked-in foreign-language program, plugin loader, central bidder-name constructor list, new config signing layer, or provider SDK dependency in the portable engine.
- Keep unit tests next to their implementation. New `tests/` targets below are integration, allocator-process, CLI, or actual child-build tests. Test names describe behavior, never capability numbers.
- Commit each tested task separately; push each repair round. Do not add authorship trailers. Preserve unrelated edits and historical resource captures.

### Historical Execution Priorities (2026-10-29)

This section captures the pre-amendment state, not the current publication
count, activation policy or completion verdict. Its remaining-gate instructions
were superseded for bidder catalog by the controlled-native completion plan linked above.
Formal certification retains its resource qualification owner; expansion and full-catalog work retain
bidder expansion/remaining catalog closure ownership. The original checkpoint text follows for traceability.

Publication checkpoint at `da1d3038` (2026-10-30): the 262 bidder packages
declare 17 factories, but publish only four all-feature definitions, three in
secure-default builds. Thirteen of the 14 priority identities remain unroutable.
The approved wave requirements and source/compiler receipts are not migration
or onboarding acceptance. Tasks 3/4/7/12 must supply qualified, nonzero funding
and serving admission; Tasks 15/16/18/19 must still close decoding, real
execution evidence, publication and composed handler acceptance. None is marked
complete by test-layout, SDK-adoption or evidence-record repairs.

1. Preserve the implemented registry/API repair checkpoint, including fixtures
   that previously replaced an existing registration. There is no production
   replacement bypass. Immutable failures remain distinct in CLI validation
   and cached/default handler paths. This is not the full funded join below.
2. Freeze the per-adapter/feature shipped qualification set `S` from generated
   bindings plus the frozen priority wave, including existing bindings, aliases,
   references, formats/helpers and disabled shipped definitions. Follow the
   [qualification amendment](../specs/2026-10-29-shipped-schema-qualification-design.md).
   First produce a reachable gap table and reuse existing evidence. Do not
   continue general nested-literal/backend proof work without a need in `S`.
3. Qualify and integrate `S`, using actual native/guest artifacts and bounded
   execution probes plus the source-based work/scratch and owner facts needed
   for its admission contract. Keep proved/enforced/observed claims distinct.
   Close the nonzero descriptors, precharged cached registry, indexes and
   parameter-validation wiring together; never install zero or observed-only
   placeholders as purported bounds. The current partial cache/API work does
   not complete this gate.
   AG approved fail-closed artifact staging: generate/link `S` for application
   inspection before qualification, with an explicit unqualified state and no
   serving reachability or numeric cost defaults. Refuse activation and funded
   resource export before registry materialization, initialization or gate
   installation. This breaks the artifact/qualification sequencing cycle, not
   the funding requirement; repeat correspondence on the final admitted build.
4. Complete selected-flow acceptance for all 14 priority identities. Every
   priority family now has protocol candidate code, including the shared floor
   conversion/credential prerequisites and Sharethrough/Rubicon consumers.
   Protocol/decoder boundaries, reviewed destinations, settings, activation,
   sync and composed handler flows remain; candidate code is not promotion.
   See the [priority execution record](2026-10-23-priority-bidder-parallel-execution.md)
   and [shared-services record](2026-10-26-priority-bidder-services.md).
5. Wire current executable evidence into promotion/onboarding and CI.
   The immutable supplemental source checkout and exact nonzero source probe
   are now wired and locally verified; hosted CI execution is not claimed.
   The CLI's current wave report is design-only, with zero promoted
   rows; Tasks 18/19 need actual producer evidence and current package/resource
   gates, not an inventory-only completion claim.

For remaining Task 3/12/17 instructions, a new serving/resource qualification
set means `S`, not every unshipped source schema. Existing all-261 source,
semantic/compiler and captured-outcome controls remain intact. bidder expansion owns new
reachable schema/engine surfaces before its corresponding wave; remaining catalog closure owns the
remaining universal catalog reconciliation. resource qualification retains absolute target-fit
and SDK facts; publisher pilot retains actual hosted meters and live acceptance. Any
unproved owner actually needed by `S` stays in bidder catalog. Review the written amendment
before changing qualification/promotion behavior. No task becomes complete
merely because its future-only work gets an explicit owner.

Selected-source checkpoint (2026-10-29): ordinary modules under the existing
source-conformance target now join generated publications with the frozen
priority wave and supplemental priority membership. Exact source/generated
results are 16/17 roots, 17/18 ancestry-inclusive members and 15/16 effective
schemas in secure/development lanes. Both require four generated regex helpers
plus the shared hostname regex and UUID callback. Thirty-four focused controls
pass per lane; the report explicitly denies linked/serving qualification.
The [cohort and gap table](../../bidders/schema-qualification.md#selected-source-and-generated-membership)
records the remaining artifact, C/I/V, funded registry and onboarding work.
Execution priority 2 is not complete until actual per-adapter/feature artifacts
are reconciled; priorities 3-5 remain open. Preserve the 261-schema source census.

Selected-link checkpoint: the same cohort now builds standalone native and
three guest probes, preserving profiles/options and binding staged schema bytes,
compiler/features, auxiliary sources and artifact identities. Native probes
execute the 236-case selected capture subset and mixed-input sequences, with
exact per-family counts and explicit missing-capture names. These are not actual
adapter applications, resource descriptors or serving activation. Application
reconciliation, complete required semantics, native sections, guest execution,
reachable bounds and priorities 3-5 remain pending; see the
[probe boundary](../../bidders/schema-qualification.md#selected-schema-link-probes).

Selected regex-heap checkpoint: the existing type-layout API and pinned
constructor paths cover the cohort's one scalar automatic matcher without
another vendor patch. A six-term conditional native retained-heap model covers
the five regex helpers and is corroborated by constructor-only cold/repeat
observations. This is not all schema state or a qualified C/I/V descriptor.
Release/guest layout correspondence, code/data ownership, other schema state,
initialization/search work and funded serving integration remain in priority 3;
see the [explicit boundary](../../bidders/schema-qualification.md#selected-regex-retained-heap).

Selected cache-heap checkpoint: the same source-bound cohort now joins existing
serial cache lifecycle bounds with actual generated wrapper layouts and format
callback storage. Native-only captured parameter and existing owner-authored
adversarial workloads replace empty-object-only observations; the consumer
checks every owner/input/verdict and carries
retained storage into subsequent transient peaks, then requires full release.
This is a conditional native storage candidate, not a completed V/work/guest or
serving descriptor. No engine, schema, limit or active binding changes. The
[cache boundary](../../bidders/schema-qualification.md#selected-validation-cache-heap)
keeps the remaining priority-3 obligations explicit.

Selected WASI execution checkpoint: existing command probes now run against
the repository-pinned Wasmtime instead of treating compiler output as execution.
The source-table driver preserves raw malformed inputs and mixed sequences;
artifact/runtime witnesses and CI prerequisites remain mandatory. This is
local WASIp1/P2 behavioral evidence only. At that checkpoint the unknown target
still lacked a safe validation command; the later integer-ABI result below
addresses it. Guest memory/work, descriptors and serving integration remain
in priority 3; see the [WASI boundary](../../bidders/schema-qualification.md#selected-wasi-command-execution).

Inline-cache correspondence checkpoint: the versioned query now executes in
native release and both WASI probes, with target word size, base cache layout
and per-schema wrappers checked against independent selected helper counts.
This does not qualify native models as guest heap bounds or finish priority 3;
the [inline-layout boundary](../../bidders/schema-qualification.md#selected-inline-cache-layouts)
keeps nested layouts, work and application correspondence explicitly open.

Selected integer-ABI execution checkpoint: the unknown target now executes
original captured inputs through fixed numeric exports, never the unsupported
Rust-reference retention ABI. Both feature cohorts pass all four compiler
targets and native/three-guest replay. Known unavailable binding imports trap
on call; an exact import policy and a deliberate execution-trap control prevent
default-value substitution. The 243 shared controls pass, including two
loopback-only offline-reference reruns. This closes that standalone execution
gap, not application correspondence, resource bounds or serving admission.
See the [scope and limits](../../bidders/schema-qualification.md#selected-integer-abi-guest-execution).

Non-serving staging checkpoint: selected validators now use the shared selector
and quoted, private declarations backed by verified content-addressed `OUT_DIR`
snapshots. Source and generated snapshots are watched; changed output bytes
refuse before incremental compilation. Normal/staged registry, App and budget
controls distinguish an unchanged normal build from a typed unqualified refusal.
The source-cohort consumer checks actual generated membership in both feature
lanes. This is not linked application inspection or qualification: the
[staging boundary](../../bidders/schema-qualification.md#non-serving-application-staging)
retains all four application/package checks, reachable resource bounds,
precharged admission and final admitted-artifact correspondence in priority 3.

The requested structural refactors have implemented checkpoints: bidder-owned
assets/provenance, the shared fake and test isolation, ordinary module ownership,
generated settings/endpoint bindings, bounded browser output, and the
[compiler-derived field metadata](2026-10-27-openrtb-field-metadata-derive.md).
They are not reasons to restart the bidder ports or replace every build script
with a macro. File discovery, provenance and config generation retain their
existing build-script owners.

Application/layout checkpoint (2026-10-30): all four real adapter applications
pass selected-validator retention inspection in both feature lanes. Eight
compiler queries obtain private syntax/NFA/one-pass layouts from the exact
application dependency feature sets on native and all three guest targets.
Standard object/DWARF/WASM parsers perform static inspection; no debugger or
probe execution supplies a layout fact. These are non-serving staged artifacts,
not admitted-artifact residency or completed C/I/V bounds. The conditional
syntax, NFA, one-pass and finite half-search models still require their complete
composition, work/frame and target correspondence before serving admission.

Selected semantic checkpoint: all selected owners now have bidder-local strict
TOML controls bound to their parameter-schema digests. The 153/161-row batches
retain 14 imported source expectations separately from 139/147 authored rows.
Both lanes pass native and all three guest targets, with fresh and reused
caches. The bare guest receives an independently verified input-only table;
its original capture ABI and 236-row population remain unchanged. This closes
the added semantic checks, not priority 3's resource/prepaid serving integration
or priorities 4-5's onboarding evidence.

The response-decoder session now has a test-only bid-local isolation prototype,
with compact failure storage, shared node/byte visits and whole-source rollback.
The primitive-array allocation failure is repaired without changing the response
charge or weakening its 20% guard. Fifteen focused controls and the 1,377-test
bidder/runtime regression pass. Task 15 remains open: these observations do not
register a decoder, fund cumulative work or expose sealed response bodies.

Closeout continuation (2026-10-30):

- The sealed response's existing admission bit now freezes the original node
  limit after any decode attempt. Eight new controls first failed seven cases,
  then passed; the scoped bidder/response/fake/isolation regression passes
  1,223 tests with nine existing ignores. The test-only source visitor also
  constructs the actual standard response/seat types rather than layout-equal
  private copies, removing an unfunded conversion assumption. This does not register the test-only
  source/candidate decoder or finish Task 15.
- The selected syntax-construction leaf no longer combines guest private slots
  with host AST/HIR sizes. All 23 stack/payload layouts and the verified pointer
  ABI are mandatory inputs. Each target's compiler facts supply its own requested
  allocation projection; source-count traversal remains a host-side tool over
  the pinned selected grammar. The model sources are digest-bound in the report.
  Eleven focused controls and eight actual compiler queries across the native
  and three guest targets pass. This is a conditional syntax component, not complete C/I/V, machine-frame or
  serving qualification. Constructor composition, other program/cache owners,
  validation work and their final application correspondence remain required.
- Fixed CLI source-conformance evidence executes two exact Rust controls with
  current source/build/run bindings, explicit target selection and bounded child
  cleanup. Both producer slots are independently required. The earlier full
  16-case CLI baseline passed with four test threads. After the consumer-lease
  repair, all 19 scoped unit controls and the real producer/cache-flow/replay
  regression pass; strict CLI lint and formatting also pass. The final scoped
  run is not a repeat of the full 16-case baseline. Semantic/artifact/fixture-count/onboarding producers remain
  explicitly missing, with zero promotion credit and no bidder catalog acceptance claim.

The retained-NFA leaf now accepts explicit target root, collection, atomic and
pointer layouts, with no host-size substitution. Eight focused controls first
failed against refusal stubs and then passed; the expanded native/three-guest
compiler matrix also passes. Reports bind each lane/cohort and the model and
topology producers. Final report-provenance replay passes all eight compiler
queries, and all 23 retained-NFA owner controls pass. This is still a conditional retained component: constructor
composition, other program/cache owners, complete validation work and frames,
funded descriptors and serving admission remain open bidder catalog requirements.

resource qualification's remaining absolute-fit/SDK proofs block affected target readiness, not
this authorized catalog work. Trusted Server embedding and publisher pilot hosted/publisher testing
remain separate; additional bidder waves follow that pilot in bidder expansion, with
residual catalog closure in remaining catalog closure. Do not bring the long tail back into bidder catalog.

### Deferral Boundary

This table records existing approved ownership, not new deferrals. bidder catalog still
owns its schema/protocol/resource foundations, the strict funded registry and
all required branches and composed acceptance for the 14 priority identities.
Candidate code, unavailable evidence or a failed qualification is unfinished
bidder catalog work, not permission to move it to a later capability.

| Work Outside bidder catalog | Existing Owner |
| --- | --- |
| Trusted Server in-binary integration and host bridge | Trusted Server embedding |
| Authorized live provider/config-recovery tests and limited publisher pilot | publisher pilot |
| Additional demand-led bidders after the selected pilot | bidder expansion |
| Residual catalog migration and terminal full-catalog reconciliation | remaining catalog closure |
| Stored requests/accounts, full currency/multibid/UID integration, creative caching and full video/native/AMP delivery | Existing stored requests through browser privacy compatibility roadmap owners |

The scoped floor conversion and credential services brought forward for the
priority bidders stay in bidder catalog. Unsupported future shared-delivery features do
not excuse missing priority-local behavior. resource qualification's remaining proofs continue
under its separate remediation plan; they block affected target readiness,
not the already authorized catalog implementation. See the
[roadmap ownership table](../../roadmap.md#2-capability-ownership).

### 2026-10-26 Scope Correction

AG approved bidder catalog = foundations plus full 14-priority selected-flow acceptance,
including Sharethrough/Rubicon and APS's independent reviewed contract/evidence.
Tasks 1-19 retain every existing priority-local branch, schema/resource and
composed onboarding requirement; partial protocol candidates or imported
metadata are not bidder catalog completion. Required priority-local branches cannot move
to remaining catalog closure or be relabeled as later shared parity.

This correction initially assigned Tasks 20/21 to remaining catalog closure. The 2026-10-27
amendment below shares Task 20's wave procedure with bidder expansion but retains both
terminal full-catalog gates under remaining catalog closure. bidder catalog closure cannot wait for them.
Keep the complete source inventory, default/normal `bidders check`,
wave checks and full `--require-complete` semantics unchanged; incomplete
remaining rows still refuse full completion without blocking valid bidder catalog closure.
No task checkbox or execution/qualification claim follows from this correction.
Historical records below retain their original verification scopes.

Use smaller reviewed waves ordered by demonstrated publisher demand/adoption,
not unsupported popularity rankings; exact membership awaits evidence and
review. The [roadmap](../../roadmap.md#demand-led-waves-and-remaining-closure)
assigns post-pilot waves to bidder expansion and the residual set to remaining catalog closure. Trusted Server embedding/publisher pilot,
stored requests through browser privacy compatibility IDs/requirements and resource qualification readiness boundaries stay unchanged.

### 2026-10-27 Post-Pilot Expansion

AG approved bidder expansion after the selected Trusted Server/publisher test acceptance
under Trusted Server embedding/publisher pilot and before stored requests and accounts onward. Preserve bidder catalog's 14-priority scope and
all schema/resource/promotion safeguards. Task 20's per-wave procedure serves
bidder expansion and later remaining catalog closure work; its full-catalog terminal gate and Task 21 remain remaining catalog closure
requirements, not prerequisites for the finite bidder expansion queue. The dedicated
[bidder expansion steps](#bidder-expansion-post-pilot-bidder-expansion) below freeze that queue from
pilot findings and demonstrated demand before implementation. This is
sequencing approval, not approval of an invented roster or a readiness claim.

### 2026-10-28 Endpoint Transport Decision

AG approved HTTPS-by-default selection under [spec section 6.1](../specs/2026-10-16-bidder-catalog-design.md#61-baked-versus-runtime),
superseding separate vendor TLS-mapping evidence requirements, including for
AppNexus/MSFT. Change only the scheme; retain authority/port, path/query and
bidding protocol, exact source URLs/blobs and an effective transport-difference
record. HTTP requires an explicit scoped exception; preserve only the existing
`insecure-bidders` Mocktioneer loopback exception here. No global HTTP fallback
or automatic downgrade on HTTPS failure. Missing/placeholder/unusable
destinations stay unavailable, and source disables stay effective unless
separately reclassified. Rubicon's regional/settings/credential work stays bidder catalog;
live authorization/testing stays publisher pilot. This policy decision proves neither
connectivity nor completed metadata binding, route tests or promotion.

#### Transport-Selection Preparation

This native import/check slice does not perform Task 6's serving cutover.
Store a strict optional `https_default` record on metadata artifacts, with the
exact `source_url`, a `selected_url` differing only in scheme, and optional
`inherited_from` when an alias uses its parent's endpoint. Preserve original
Git/byte witnesses. This record is a default selection, not URL-safety,
connectivity, endpoint approval, activation or a resource descriptor.

- [x] Add typed metadata and colocated controls for HTTP selection, HTTPS/no
      selection, missing/alias-only sources, inheritance and explicit overrides.
- [x] Generate the record during import and compare it during `--check`;
      independently test each field against tampering and preserve reviewed
      implementation/definition data during the source projection.
- [x] Reconcile all 358 source identities, including 78 declared HTTP bases
      (77 source-enabled), without concrete bidder cases in shared tests.
      Refresh only importer-owned provenance notes/selection records in the
      adjacent manifests. Keep existing definitions, features and captures.
- [x] Replace the pending wave's obsolete HTTP-review label with
      `https_default_pending`, preserving pending status and all requirement
      counts. Run source/import/wave and ownership checks, strict lint, and
      independent review before commit/push. This is not wave promotion.

**Execution record, 2026-10-28:** native import/check now owns 122 typed
selections across 77 folders: 78 bases (77 source-enabled), 27 own-endpoint
aliases and 17 inherited aliases. Whole-manifest comparison over all 262
folders preserved definitions, factories, features, settings and original
witnesses; independent review checked 495 retained artifacts. Only selection
records, the obsolete generic review note and four authored status notes changed
in manifest provenance. Source URLs and selected URLs remain unsplit.

The authored pending wave changes three routing labels and one superseded TLS
evidence sentence, preserving all 13 identities and 113 operations. Its new
design digest is `aa9d38e3e6221c13d13b28ba18684e041f48e3c15608aa60ee11db86924a0c24`.
Four authored factory-fixture current-manifest bindings and three matching
test-only constants were renewed. Protocol inputs/expected outputs, parameter
schemas, original captures and historical qualification bundles did not change.

Observed reds covered the missing DTO, absent generated records, an altered
source URL being accepted, protocol-witness scope, supplemental-only scope,
the legacy wave label and the existing URL-format guard. Each passed after its
specific repair. The scope fix validates every actual manifest before optional
expectation matching; protocol review notes remain independently editable.
The formatter recognizes the two new URL fields generically, without bidder
exceptions or a weakened prose limit.

| Verification | Result |
| --- | --- |
| Maintenance CLI unit/import/source/wave matrix | 165 / 8 / 21 / 9 passed |
| Final normal-feature CLI bidder/resource namespace | 255 passed |
| Final pending-wave digest/identity integration check | 1 passed |
| Shared native configuration-schema library | 225 passed |
| Baked config, codegen, binding and URL-safety targets | 17 / 2 / 3 / 74 / 87 passed |
| Bidder library, registry/source conformance and workspace isolation | 1,100 / 23 / 16 passed |
| Separately driven immutable main/supplemental source checks | 9 passed |
| Development-feature URL safety and registry conformance | 87 / 23 passed |

These are affected-component executions, not distinct-proof totals or a full
workspace test claim. The initial unfiltered normal-feature CLI run had 729
passes, two existing ignores and one missing-source-environment failure; the
correctly bound final bidder/resource namespace above includes that test.
It is not relabeled as a second complete 732-test run. The full maintenance
matrix preceded the final policy-caption/digest renewal; the final normal CLI
namespace and digest integration check cover those changes.

Strict workspace Clippy passed under `-- -D warnings`, without new lint
allowances. Formatting, whitespace and both raw-fixture gates passed (188 raw
fixtures across 54 files). The normal-feature operator CLI builds; its help
retains `config`, `deploy` and `serve` alongside the maintenance commands.
Initial FIFO/socket fixture `EPERM` errors were
rerun with permission, not ignored or relaxed. Spec and independent quality
reviews approved after resolving the supplemental-scope and URL-format issues.
No serving endpoint, activation, dependency/toolchain/SDK pin, workload limit,
budget coefficient, schema engine or resource descriptor changed. Serving
metadata binding, destination validation and complete priority-flow/schema/
resource acceptance remain open; this is not bidder catalog or resource qualification completion.

### Bidder-Owned Asset Cutover

AG approved one authoritative folder per implementation on 2026-10-23. Follow
the [asset migration plan](2026-10-23-bidder-owned-assets.md) before extending the
old storage paths: `bidder.toml` owns registration, effective definitions,
aliases and provenance; local JSON schemas and captures stay beside it.
That plan supersedes this document's original two-field declarations and split
config/reference asset locations, not its protocol, source, safety or resource
requirements. Earlier checked preparation tasks and evidence below describe
the paths they actually exercised; they are not proof of this later cutover.

**Metadata boundary amended 2026-10-24:** AG approved removing 358
main-reference metadata YAML captures plus one supplemental APS metadata YAML
capture. Keep repository, immutable revision, source path, blob ID, byte length,
SHA-256 and reviewed differences in `bidder.toml`; metadata has no local YAML
or retained-path/digest witness. Import and source checks still read the pinned
external Git checkout, without publishing metadata YAML. JSON schemas and
protocol/parameter captures remain byte-identical. This supersedes earlier
metadata-retention requirements, not source pins, attribution, promotion or
qualification. The older retention steps and results below are historical,
not verification of this approved removal.

- [x] Record the approved ownership rule in the bidder catalog spec and bidder rules.
- [ ] Execute and verify the coordinated manifest/config/discovery/import/schema/
      provenance cutover; leave this open until all active consumers and copy
      closures use it, including the approved 2026-10-24 metadata-removal boundary.
      Imported-only folders must stay non-callable.

**SDK source continuity checked, 2026-10-16:** Compared the complete Git trees
at `596f9f4e` and `f6c708cc`, not just commit messages. The `edgezero-core`
tree ID is identical (`2830a47879f68d972a119c99cc841867b72ddb79`), as are
root `Cargo.toml`, `Cargo.lock` and `.tool-versions`. The only adapter-crate
changes are CLI repository/diagnostic URLs; SDK CLI changes also add URL tests.
No runtime adapter implementation or indirect core constant changed. Reproduce
with `git diff 596f9f4e f6c708cc -- crates Cargo.toml Cargo.lock .tool-versions`
in the SDK checkout. This round adds no compile or resource-qualification claim.

### Naming Cutover

This naming-only checklist does not implement discovery or promotion and does
not complete Tasks 5-8. Keep all bidder implementation work in `auctioneer-bidder`.
The planned source-directory mapping uses `<bidder_key>`, with `_bidder` for
Rust keywords; implementation keys are not derived from public bidder names.

- [x] Rename public concrete types and their consumers/re-exports to `FooBidder`: `AppnexusBidder`, `GenericBidder`, `PubmaticBidder`, and feature-gated `MocktioneerBidder`.
- [x] Rename bounded metadata constants to `META_BIDDER_REGION_BYTES`, `MAX_BIDDER_HEADERS`, and bidder-named private companions; preserve every numeric bound, layout and accounting term.
- [x] Cut over owned bidder documentation, tests and consumer imports together, with no compatibility aliases, duplicate old-name exports/fields or shims. Preserve platform crates `auctioneer-adapter-{axum,cloudflare,fastly,spin}`, provider terminology, exact source paths/evidence, wire identifiers and imported-schema terminology.
- [x] Keep behavior unchanged: public names, feature gates, defaults, protocol bytes, privacy, registry, sink/parser and resource semantics are unaffected by renaming. The planned declaration/factory/binding contract below remains pending implementation.
- [x] Run focused bidder/metadata/sink/registry and feature-gate tests, then compile all workspace consumers on the native host, including tests/examples/benches and all four provider crates. Check secure-default and `insecure-bidders` consumers separately; record actual commands/results. Native-host compilation is not actual WASM-target compilation or qualification and does not establish bidder catalog discovery or promotion acceptance.

Naming-cutover verification (2026-10-16, native host; pinned SDK/source
checkout prerequisites supplied through their existing environment variables):

- `cargo test --locked -p auctioneer-bidder --all-targets -j 2 -- --quiet`: 167 passed under secure defaults.
- `cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders:: -j 2 -- --quiet`: 68 passed, including the keyword-suffix controls first observed failing with the old spelling.
- `cargo test --locked -p auctioneer-cli --test bidders_source --test bidders_import -j 2 -- --quiet`: 14 passed; the committed reference snapshot and source bindings/inventory remain unchanged.
- `cargo test --locked -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core -p auctioneer-runtime --all-targets --features insecure-bidders,conformance-suite -j 2 -- --quiet`: 2,040 passed, zero failed, two existing ignored tests.
- `cargo test --locked -p auctioneer-bidder --lib a_bidder_without_a_debug_schema_defaults_to_unsupported -j 2 -- --quiet`: one passed after correcting that test's renamed identifier.
- `cargo clippy --locked --workspace --all-targets --features insecure-bidders,conformance-suite -j 2 -- -D warnings`, `cargo fmt --all -- --check`, and `bash scripts/check-raw-json-fixtures.sh`: passed. These are not live-provider or actual WASM-target qualification results.

### Operator Tooling Consolidation

Operator maintenance commands now belong to `auctioneer-cli`; see the
[consolidation plan](2026-10-16-cli-tool-consolidation.md) for scope and
verification. Default builds retain config, deployment and qualification
commands. The explicit maintenance-only build excludes application dependencies
so archive repair remains buildable independently. The catalogue package stays
a build-time library for config generation, not a second executable.
This tooling change does not complete registration or schema qualification.

### Delivery Order

| Slice                              | Tasks | Exit                                                                                                | Required acceptance checkbox                                                                                                                   |
| ---------------------------------- | ----- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Reference and decisions            | 1-3   | Exact inventory, scoped schema/dependency qualification, predeclared limits                         | Task 3: `schema_engine_qualification` plus Task 1/2 source/import targets                                                                      |
| Accounting groundwork              | 4     | Mandatory scalar descriptors, shared input resolution and additive ownership before new allocations | Task 4: `catalog_alloc`, pure `catalog_resource_profile`, process-isolated `boot_strict_parity`, CLI resource controls and runtime owner tests |
| Registration                       | 5-8   | Strict progressively shipped join, incremental discovery, fallible initialization                   | Task 8: `registry_build`, with Task 7 lifecycle refusals                                                                                       |
| Catalog behavior                   | 9-11  | Activation, aliases, sync/info and count ownership                                                  | Task 11: `bidder_catalog_views` plus Task 10's distinct-owner controls                                                                         |
| Protocol contracts                 | 12-15 | Bounded generated validation, context and response paths                                            | Task 15: `response_envelopes` plus Task 12-14 validation/context/correlation targets                                                           |
| Evidence and foundation acceptance | 16-17 | Current-code evidence driver, matched accounting and required checks                                | Task 17: every listed acceptance command and four matched resource reports                                                                     |
| Priority delivery and bidder catalog closure    | 18-19 | All 14 selected flows and required priority-local branches; schema/resource and onboarding acceptance | Task 19: `bidder_onboarding`, source-backed wave plus independent APS evidence, repeated foundation/build/resource gates; no Tasks 20/21 dependency |
| bidder expansion post-pilot expansion           | bidder expansion / 20 wave procedure | Frozen demand-led queue after selected Trusted Server embedding/publisher pilot testing, before stored requests and accounts onward | Each reviewed wave's current composed acceptance and affected schema/resource/build gates; no full-catalog completion requirement |
| remaining catalog closure residual waves                 | 20    | Remaining source branches and identities, including later-feature-dependent rows                    | Task 20 terminal gate: full-completion command, exactly 358 migrated source identities                                                                       |
| remaining catalog closure full-catalog closure            | 21    | Evidence-derived full migration, not deployment certification                                       | Task 21: full-completion rerun and actual full-catalog target/package gates                                                                    |

A slice is not complete merely because its prose or files were committed;
the named acceptance checkbox and all preceding required controls must pass.

Do not land an intermediate PR that enables metadata with missing factories, activates new imports, or introduces unfunded allocations. Keep coupled data/code/resource changes in one task's commit when necessary; red tests may exist locally, not as a broken shared branch.

## Concrete Foundation Decisions

These are **proposed predeclared ceilings**, not measurements or claims of qualification. Approval of this plan approves their initial evaluation; Task 3 must establish independent upper bounds before runtime integration. Exceeding a ceiling fails qualification and requires a reviewed amendment, not an adjusted-until-green constant.

| Owner                                                             |                                                                                                   Initial ceiling |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------: |
| Compiled identities plus effective host aliases                   |                                                                                                               512 |
| Requested / priority entries / each filter / requested debug rows |                                                                                      256 each, distinct constants |
| Priority groups / runtime host-alias overlay                      |                                                                                                           32 / 64 |
| One imported schema / all reference schema source bytes           |                                                                                                    16 KiB / 1 MiB |
| Schema input object, before engine call                           |                                                                64 KiB serialized; 4,096 nodes; container depth 32 |
| One conservative schema evaluation charge                         |                                                                                              4,194,304 work units |
| Aggregate schema work per auction                                 |                                                                                             67,108,864 work units |
| Schema evaluation scratch peak `V`                                |                                                                                      512 KiB per admitted auction |
| Generated-validator static data and distinct lazy caches          | 12 MiB, included in catalog residency; generated executable code has a separately recorded target residency owner |
| Disjoint catalog residency `C` / startup scratch `I`              |                                                                                                    16 MiB / 8 MiB |
| Compiled linear regex / its lazy-DFA cache                        |                                                     64 KiB / 64 KiB per pattern, included in cache/scratch bounds |
| One typed runtime settings input                                  |                                                                                     16 KiB; 1,024 nodes; depth 16 |
| One coupled secret read                                           |                      One version-1 bundle per admitted requested identity; 4 KiB backend and accepted bytes; at most 16 JSON nodes |
| Credential components / retained payload                           |                  Username/password 1..=256 UTF-8 bytes each; tracker 0..=128; complete retained credential payload at most 1,074 bytes |
| Credential parse / sequential construction bank                    |                  86,528-byte parse-heap envelope within one 128-KiB bank per auction; existing metadata separately funds emitted authorization |
| Aggregate retained settings/secret request views                  |                                                         1 MiB per auction; settings borrow the validated snapshot |
| Correlation metadata                                              |                                            One row per covered impression, at most 8 bytes/row plus 64 bytes/call |
| Partial construction diagnostics                                  |                                              At most 32 fixed/bounded entries per bidder; charge before retention |
| New proprietary JSON envelope nesting                             |                                                  Fixed 64 containers; standard response parser behavior unchanged |
| Info aggregate body / introduced auxiliary peak                   |                                                                           2 MiB body / 8 MiB per auxiliary permit |
| Complete UID cookie / sync output limits                          |                                                          Existing 4 KiB cookie and existing sync limits unchanged |

The credential rows above supersede the original generic 8-KiB-read/32-KiB-parse
proposal under AG's approved [priority service contract](2026-10-26-priority-bidder-services.md).
The version field plus username/password/tracker are the four strict wire fields;
unknown or duplicate fields refuse. Secret references and captured physical store
names have independent 128-byte bounds. The bank covers parse/read/encoding and
binding/layout work in the complete exchange construction descriptor, not a new
catalog `V`/`K` term or a per-bidder multiplier. Aliases may resolve different
references; one identity's construction finishes and drops its bundle before the
next read. Neither the original proposal nor the settings-view ceiling may be
used to underfund this implemented bank. Full source-bound and physical/provider
qualification remain distinct, as recorded in the service checkpoint.

A work unit is a conservative, version-bound operation allowance, **not a microsecond**. Task 3 derives coefficients from qualified engine paths for node visits, input-byte scans, combinator/reference fan-out, regex program size and collection comparisons. Use checked arithmetic; overflow refuses before evaluation. A finite constant with no derivation is not a work proof.

Predeclare the **total catalog-attributable accounted delta** per actual target: native 96 MiB, Cloudflare 128 MiB, Fastly 64 MiB, Spin 64 MiB. Apply the component ceilings above as well; unrelated reductions cannot cancel a component breach. These are regression allowances, not available physical headroom. Charge verified descriptors, not every allowance's maximum automatically.

Native demand plus at least 256 MiB must still fit its effective ceiling. resource qualification separately requires at least `ceil(effective_ceiling/16)` headroom: current Cloudflare/Fastly 8,000,000 bytes and Spin 8,388,608 bytes. No WASM relative result can be promoted to absolute readiness.

### Dependency Qualification

- Add versions centrally in root `Cargo.toml`; consuming manifests use `workspace = true`.
- Reuse the already locked YAML fork `serde_yaml_ng 0.10.0` for native CLI import only. Reject duplicates, non-data tags, alias/merge constructs, excessive structure and unknown fields through structured parsing. Do not add a YAML runtime loader.
- Native inventory/import also pins `yaml-rust2 = { version = "=0.11.0", default-features = false }` as an event-only preflight. The Serde library resolves anchors before exposing values, so alias/tag and byte/node/depth admission must run before its first tree allocation. This adds no YAML dependency to a runtime adapter. Preserve source-ignored historical spellings explicitly; do not silently normalize them into effective settings.
- Reuse existing `validator::Validate` derives/custom validators for typed configuration, bidder declarations and settings wherever applicable. Keep the existing cross-field/SSRF validators when a per-field derive cannot express their context. Neither a plain Serde struct nor `validator` attributes alone are a replacement for the imported JSON Schema semantics.
- Candidate `jsonschema 0.58.6` uses its `macros` feature, no default network/file/TLS, async resolver or arbitrary-precision features. `build.rs` emits declarations for the library's compile-time generator, not a handwritten schema compiler or a runtime cache of parsed schemas. Request-time parameter validation remains necessary; only schema compilation moves out of the guest.
- Candidate declarations explicitly select the audited draft (Draft4 where declared), `validate_formats = true`, committed resources, and `pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 }`. Generate only the boolean `is_valid` method when sufficient. The candidate macro's resolver/offline behavior and each option must be inspected and tested; runtime `.offline()` configuration is not evidence about a separate procedural macro.
- Verify the resolved candidate APIs/features and all actual deployment targets in Task 3. Documentation: [compile-time validators](https://docs.rs/jsonschema/latest/jsonschema/#compile-time-validator-macro), [format policy](https://docs.rs/jsonschema/latest/jsonschema/#note-on-format-keyword), [regex configuration](https://docs.rs/jsonschema/latest/jsonschema/#regular-expression-configuration). Retain exact checksums and relevant compiler/runtime-helper source paths in the qualification record. The exact candidate is installed only for native development controls; the native YAML dependencies above are pinned. The [preparation record](../../bidders/schema-qualification.md) distinguishes executed probes from the remaining engine gate.
- If qualification fails, land the non-behavioral inventory/report work, keep runtime schema cutover pending, and bring a reviewed replacement pin, bounded engine extension or compatible regex/format decision back to the spec/plan. Do not ship an allow-all validator, silently omit schemas, raise limits automatically, or describe a benchmark as a proof.

The schema macro is **unqualified**: neither these documents nor read-only
review establishes that its APIs, offline behavior or targets work. Task 3
must produce the actual execution/link evidence before schema acceptance.
Its explicit failure/recovery checkpoint does not block independent inventory
and discovery work.

## File And Interface Map

| Area                           | Existing files / proposed focused files                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reference CLI                  | Modify `crates/auctioneer-cli/src/main.rs`; create `src/bidders/{mod,source,inventory,import,check,evidence,resources}.rs` within that crate                                                                                                                                                                                                                                                                                                                                      |
| Reference data                 | Keep each implementation's metadata/provenance in `bidder.toml` and verbatim JSON schemas and protocol/parameter captures under `crates/auctioneer-bidder/src/bidders/<key>/`; no metadata YAML or retained witness for metadata. Shared immutable `source-inventory.toml` and `source-bindings.toml` live under `crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/`. Source-only packages supply no runtime definitions; production never performs a filesystem scan. |
| Review records                 | Keep `docs/bidders/{README.md,catalog.toml}` and qualification/wave reports separate from compiled inputs; runtime build never reads the migration ledger                                                                                                                                                                                                                                                                                                                     |
| Metadata codegen               | Modify `crates/auctioneer-config/{build.rs,build_codegen.rs,src/types.rs,src/lib.rs}` and `build_bindings.rs`; shared `bidder_manifest.rs` represents bidder-owned TOML and `build_catalog.rs` validates its build-time inputs. Schemas stay with their bidder owners.                                                                                                                                                                                                            |
| Discovery and generated checks | Create `crates/auctioneer-bidder/{build.rs,build_discovery.rs,build_schema.rs}`; modify `src/bidders/mod.rs`, move `src/bidders/url/mod.rs` to `src/bidders/url.rs`; include generated validator declarations through `src/schema/mod.rs`                                                                                                                                                                                                                                         |
| Bidder declarations            | Add adjacent `bidder.toml` and `create_bidder()` to existing `src/bidders/<bidder_key>/mod.rs` implementations; later waves add their own directories                                                                                                                                                                                                                                                                                                                         |
| Registry/schema/context        | Modify `crates/auctioneer-bidder/src/{registry,request_info,meta,sink,types,bidder,lib}.rs`; create `src/{catalog,host_context,response_decode}.rs` and `src/schema/{mod,input}.rs`                                                                                                                                                                                                                                                                                               |
| Core integration               | Modify `crates/auctioneer-core/src/{bidder_registry,configure}.rs`, `host_primitives/{gates,mod}.rs`, `config/{mod,resolve,runtime,budgets}.rs`, `handlers/{auction,info_bidders}.rs`, `usersync/{catalog,choose,evict}.rs`; create `src/bidder_context.rs`                                                                                                                                                                                                                       |
| Sync permission                | Modify `crates/auctioneer-privacy/src/sync/{mod,permission}.rs`; its integration tests live in `crates/auctioneer-privacy/tests/`                                                                                                                                                                                                                                                                                                                                                     |
| Portable exchange              | Modify `crates/auctioneer-exchange/src/{context,alias}.rs`, `split/mod.rs`, `run/mod.rs`; create `src/parameter_validation.rs`                                                                                                                                                                                                                                                                                                                                                    |
| Budget owners                  | Modify `crates/auctioneer-runtime/src/{lib,aux_work,budget}.rs`; create `src/catalog_budget.rs` for descriptor/receipt logic if it keeps the existing equation readable                                                                                                                                                                                                                                                                                                           |
| Provider profiles              | Modify each `crates/auctioneer-adapter-{axum,cloudflare,fastly,spin}/src/envelope.rs`; native profile export also changes `src/resource_profile.rs`                                                                                                                                                                                                                                                                                                                               |
| Input/parity controls          | Public non-installing core `resolve_budget_inputs`; pure `tests/catalog_resource_profile.rs`, isolated single-test `tests/boot_strict_parity.rs`, and CLI `tests/bidders_resources.rs` actual-command controls                                                                                                                                                                                                                                                                |
| Acceptance                     | New focused integration targets named in tasks; existing `test.yml`, `format.yml` and vendor workflows remain the owning CI paths                                                                                                                                                                                                                                                                                                                          |

Proposed APIs are deliberately provider-neutral:

```rust
pub struct CatalogResourceCosts {
    pub resident_bytes: u64,
    pub startup_scratch_bytes: u64,
    pub validation_scratch_per_auction: u64,
    pub host_context_bytes_per_auction: u64,
}

pub enum RegistryBuildError {
    DuplicateIdentity,
    DuplicateFactory,
    MissingFactory,
    InvalidBinding,
    InvalidAlias,
    FeatureMismatch,
    ResourceBounds,
}

pub enum ParameterVerdict {
    Valid,
    Invalid,           // bidder-attributed public code 2
    ResourceRefused,   // bidder-attributed public code 5
    DeadlineExpired,   // existing original-deadline disposition
}
```

Keep diagnostics fixed/redacted; a variant must not hold raw instance values, validator text or secret bytes. The catalog cost builder consumes generated bounds before initializing lazy metadata or validators. Boot and direct-helper initialization use the same bounded builder and existing runtime reservation machinery; do not allocate the catalog merely to discover its descriptor.

## Task 1: Establish Reference Inventory And Provenance

**Files:** CLI `src/bidders/{mod,source,inventory}.rs`, `main.rs`; `crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/{source-inventory,source-bindings}.toml`; `docs/bidders/{catalog.toml,README.md}`; new CLI integration target `tests/bidders_source.rs`; `.github/workflows/test.yml` and its colocated scripts for source prerequisites.

- [x] Add strict CLI commands `bidders inventory --source PATH --output PATH` and `bidders check --source PATH`. Distinguish invalid input (exit 2), incomplete/check failure (exit 1), and successful normal inventory/check (exit 0).
- [x] In the same task that introduces `bidders_source`, add CI's separate pinned checkout at `vendor/bidder-reference` and `AUCTIONEER__BIDDER__SOURCE_DIR`, plus exact checkout/pin/command guards in core `ci_workflows.rs`. The existing OpenRTB round-trip checkout has a different pin and must not be repurposed. Required source tests fail on missing prerequisites; do not defer setup to Task 17 or silently skip them.
- [ ] Write failing source-reader unit tests for wrong revision even with consistently re-pinned digests, dirty working trees, replacement refs, symlink/tree modes, duplicate/missing names and the exact source-support exclusion. Use literal revision/object/count expectations.
- [x] Read pinned objects with `git --no-replace-objects` and literal argument arrays, never a shell or a source program. Bound subprocess output/time and aggregate import bytes; dirty worktree changes neither accepted bytes nor provenance.
- [ ] Capture/audit the 261 source factory bindings as data in `source-bindings.toml`, tied to the exact source blob and reviewer/date. This is reference evidence, not production registration. Do not claim automatic syntax extraction from an unimplemented source-language parser, or invent human approval.
- [x] Derive 358/261/97/259 set equality, exact internal-key mappings and 196 sync identities/187 families/162 without sync. Recompute the exact schema tree's 261 raw blob lengths: 164,518 bytes total, maximum 3,778 at appnexus.json, blob `37909b12066ad9239b488b9751d070212e90d61a`; reject mismatch against spec section 3.1's tree/pin and literal witnesses.
- [ ] Record local-only definitions separately from the source inventory, using the actual Auctioneer discovery/binding inputs rather than a second maintained constructor list.
- [ ] Derive the top-level declared HTTP endpoint set structurally over bases only: exactly 78, of which 77 have `disabled != true` and one is disabled (`stroeerCore`). Pin the priority intersection to msft/openx/sovrn and the remaining source-enabled count to 74, including shipped appnexus. Report member/source-blob identities, source activation and routing-review state. Audit declared/inherited alias endpoints separately; sync/documentation URLs cannot inflate the base count.
- [ ] Run `cargo test --locked -p auctioneer-cli --test bidders_source`; wrong-revision/set mutations must fail the named assertions. Commit only after the new target executes a nonzero exact fixture count.

**Started slice:** `bidders inventory` records immutable membership, source
blob/SHA-256 witnesses, implementation-directory keys, schema byte witnesses
and the 78/77 HTTP population. It emits `reference_inventory_only`, never
runtime metadata, migration approval or callable-endpoint policy. The YAML
dependency was centralized now because structural endpoint inventory needs
it; no YAML parsing enters guest code. `bidders check` now validates the
complete reference artifact, refuses forged completion, and distinguishes
drift from invalid input. Version 2 also reconciles the source's exact
sync-definition predicate and key-only alias inheritance, without inheriting
vendor IDs or approving endpoints. The complete factory map is captured as
data with `review_status = "pending"`; its human review remains required.
Task 1 is not complete: audited binding approval, the migration ledger and
remaining source controls still need implementation. Task 2 owns semantic
import/promotion and the named enabled-HTTP routing-policy control; recording
an HTTP source endpoint in this slice does not authorize it.

## Task 2: Deterministic Import And Progressive Promotion

**Files:** CLI `src/bidders/{import,import_packages,import_projection,metadata,yaml,inventory,source,check}.rs`; root/CLI manifests; `crates/auctioneer-bidder/src/bidders/`; new `tests/bidders_import.rs`.

- [x] Add `serde_yaml_ng = "=0.10.0"` centrally and consume it in CLI. Write duplicate-key, unknown-field, alias/tag/merge, escaping-path, malformed source and stable-order failures before implementing import.
- [x] Implement `bidders import --source PATH --staging PATH --check` as a data-only conversion of all source metadata/schemas. Write no executable bidder code. Failed import leaves no partial published staging tree; check mode never writes.
- [x] The 2026-10-23 cutover kept raw schemas/metadata and per-bidder TOML in their implementation folders, outside documentation. Cross-bidder inventory witnesses now live in `crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/` after the [shared test isolation](2026-10-25-bidder-test-isolation.md) cutover. The [asset cutover plan](2026-10-23-bidder-owned-assets.md) superseded the original central snapshot layout; its 2026-10-24 amendment removes metadata YAML and retained witnesses for metadata only. Keep every remaining captured artifact/witness protected against Git checkout newline conversion, with an actual unprotected LF/CRLF positive control. Production promotion remains separate.
- [ ] Represent complete inventory and shipped bindings separately. Normal checks allow unmigrated inventory rows; there is no missing-factory exemption for any promoted runtime definition.
- [ ] Add `bidders promote --wave MANIFEST --check`: validate a reviewed source-identity set and emit an atomic promotion patch for real bindings/schemas. Unknown identities, missing implementations or alias parents refuse promotion.
- [ ] Import new bases opt-in; aliases promoted with a new base inherit its opt-in cohort. A new alias of an already shipped base gets explicit false. Re-import preserves existing decisions. Test both cohorts and later operator enablement, not just all-disabled data.
- [ ] Preserve source-disabled/white-label/endpoint provenance separately from compiled safety, family activation and direct-routing policy. Implement section 6.1's explicit data-only Rubicon deployment-default reclassification (`source_disabled=true`, compiled `disabled=false`, `defaultEnabled=false`), retaining the regional endpoint/settings/secret prerequisites. Pin its exact source blob and policy rationale; no invented human approval or production name exception. Test re-import preserving that decision, an unreviewed reclassification refusing promotion, and a genuine base safety veto remaining effective.
- [ ] Stage `teqblaze` as alias-only and its endpoint-bearing baked aliases independently; keep source-disabled `stroeerCore` and endpoint-less `ix` unavailable rather than fabricating URLs or lifting disables through HTTPS selection. This classifies policy, not implemented protocol evidence: promotion still requires a real factory/schema and reviewed source behavior. Add import cases for each shape before the runtime cutover.
- [ ] Add `enabled_http_base_defaults_to_https_with_source_provenance` over pinned source-enabled HTTP `mediafuse`: select HTTPS under AG's 2026-10-28 decision, preserving authority/port, path/query, bidding protocol, exact source URL/blob, enabled source provenance and AppNexus binding. Assert the effective transport-difference record, no unapproved HTTP dispatch, no global fallback and no downgrade on HTTPS error. Selection needs no separate vendor TLS contract and does not promote a definition. Reconcile all 78 source HTTP bases (77 enabled/one disabled), not only disabled examples.
- [ ] Resolve policy before effective alias safety inheritance, retaining declared/inherited source provenance. Pin `magnite` inheriting Rubicon's reclassified compiled policy and explicitly disabled `appStockSSP` keeping its own veto. Generalize unavailable classification to every absent/placeholder/unusable destination, including disabled `adxcg`/`avocet`/`pangle`; a source HTTP scheme alone is not unavailability. Reject reclassification records for the wrong identity/blob and require renewed review when source inputs change.
- [ ] Run `cargo test --locked -p auctioneer-cli --test bidders_import` and `--test bidders_source`. Include a complete reference inventory with only the current production subset and prove normal checking succeeds but `--require-complete` does not. Commit.

**Import slice history (2026-10-23):** fresh import emitted 879 bidder-owned
reference files: 259 non-executable TOML manifests, 358 verbatim metadata YAML
documents, 261 raw schemas and one shared source license. Source-ignored
spellings remained in those captures and were named in provenance notes, not
promoted settings. That layout is superseded for metadata retention only.

**Approved import contract (2026-10-24):** fresh staging must contain 521 files:
259 non-executable TOML manifests, 261 byte-identical JSON schemas and one
shared source license. Metadata source witnesses, relevant interpretations
(including missing/null distinctions and ignored spellings) and reviewed
differences stay in TOML; metadata YAML and retained witnesses for it are not
published. APS remains separately pinned and outside these staging counts.
This is the approved boundary, not a new implementation-verification claim.
Import grants no endpoint/factory/runtime approval. Byte/node/depth preflight
still protects inventory's first external YAML parse. The normal command
atomically publishes a fresh directory and never overwrites an existing
destination; check mode
verifies the importer-owned projection without modifying reviewed local code,
definitions or supplemental captures. The CI
source prerequisite now requires both `bidders_source` and `bidders_import`.
Task 2 remains incomplete until reviewed promotion, activation cohorts,
reclassifications and effective routing-policy import are implemented.

## Task 3: Qualify Schema Engine And Freeze Resource Decisions

**Files:** root/bidder manifests; `crates/auctioneer-bidder/src/schema/{mod,input}.rs`; new `tests/schema_engine_qualification.rs` and `tests/support/schema_qualification.rs`; `docs/bidders/schema-qualification.md` and `resource-contract.toml`; `test.yml` and its colocated scripts for native-job target/prefetch setup.

### Approved Constructor Initialization Increment (2026-10-21)

Scope: extend only `vendor/jsonschema-macros-core/src/codegen/emit_root.rs`
with the spec's `initialize_regexes()` lifecycle API, preserving the earlier
numeric repair as its own patch. No dependency, engine, limit or serving-path
change; constructor reachability does not discharge full Task 3 qualification.

- [x] Add executed generated-code controls in `tests/schema_initialization.rs`:
      two independent patterns skipped by required-field refusal, all constructors
      forced despite failure, cached failure, repeat/concurrent once-only behavior,
      no-pattern success and unchanged validation decisions. Observe a failing
      behavioral assertion before changing the compiler.
- [x] Hoist each regex's existing lazy storage to its private generated module;
      emit an initializer visiting every slot without short circuit, with the
      public method forwarding to it. Preserve both supported engines' builders.
- [x] Keep a separate portable initialization patch and exact source checksum.
      Extend `support/schema_vendor.rs` and portable-patch controls to reject
      independent edits/reverts of either patched source file and recreate each
      file from the original published archive.
- [x] Extend the disposable all-schema probe with a separately selectable
      constructor-initialization operation and reachable per-validator functions;
      execute all 261 native initializers and link all three WASM targets. Retain
      the existing sample/allocation modes without relabeling them cache proofs.
- [x] Run affected bidder targets under secure and insecure feature lanes,
      formatting, strict Clippy and fixture gates; independently review the
      contract and code, update qualification/ISSUES records, commit and push.

### Remaining Engine Qualification

Library-owned payload layouts: AG approved the
[bounded API design](../specs/2026-10-28-regex-resource-layout-api-design.md)
on 2026-10-28. Its
[implementation plan](2026-10-28-regex-resource-layout-api.md) tracks the separate
replayable `regex-automata` patch, exact source/feature closure, shallow native
candidate consumers and compile-only guest checks. This replaces private-layout
guesses for queried payloads without adding matcher-trait methods or changing
engines, schemas, limits, funding or serving behavior. It does not discharge
hidden-child ownership, complete heap/work, initialization overlap or target
qualification, and does not complete this task.

Constructor-allocation increment: extend the existing native allocation probe,
not the production registry or the full acceptance target.

- [x] Observe a failing `initialized-catalog` probe before implementation.
- [x] Give the existing hostname-format regex an explicit shared initializer,
      preserving its pattern, limits and length-before-match behavior. Measure it
      and all 261 generated initializers in a fresh process, first use and repeat.
- [x] Record per-owner requested-heap observations and a checked aggregate;
      then run the unchanged 323 sampled validation inputs after initialization.
      Preserve the separate cold/sample modes and independent allocator controls.
- [x] Require zero constructor-repeat allocations, nonvacuous cold format/
      aggregate allocation, unchanged outcomes and the known fractional-number
      repeat allocation. Before measurement retain the existing 12-MiB cache/data
      allowance and 20-MiB joint cache/data-plus-initialization allowance as necessary
      observation ceilings only. Never derive disjoint initialization scratch by
      subtracting final residency from an earlier peak; native requested-heap
      observations do not prove allocator overhead, concurrency, work or guest fit.
- [x] Verify both feature lanes, review the increment, update qualification and
      ISSUES records, and commit/push without changing workload or resource policy.
- [x] Add bounded launch/control-phase diagnostics and attempt controlled-load
      reproduction of the recorded allocator-control watchdog without weakening
      refusal. Fixed markers stay outside measurement; a deadline retains at most
      4 KiB of UTF-8 stderr after kill/reap/joins. All 64 fresh native control
      processes passed while a two-job workspace Clippy process was active. This
      did not reproduce the timeout: its cause remains open in
      [investigation #32](https://github.com/IABTechLab/auctioneer/issues/32), not
      replaced by a parallel-run stability claim. Investigate the retained phase
      on recurrence; do not retry measurements or increase the ten-second deadline.

Work-admission preparation, not qualification or Task 12 serving integration:

- [x] Add the scalar `SchemaWorkBudget` boundary in bidder `schema/mod.rs`, using
      exact composition-supplied limits with no defaults or workload clamping.
      Check original expiry and cost overflow/zero/single/scratch/aggregate refusal
      before the callback; debit admitted work before invocation and never refund
      invalid or panicking evaluations. Preserve exact positive-limit admission.
- [x] Exercise independently unequal limits, failed and panicking evaluations,
      exhaustion and deadline refusal in colocated unit controls. Exercise actual
      generated callbacks with explicitly synthetic costs, known validity and
      invocation counters in the existing compiler-control target.
- [x] Implement and independently verify borrowed final-Value measurement of
      exact compact JSON bytes, syntactic nodes (including keys) and container depth,
      with inclusive independent limits, bounded scalar scans, no clone/reparse or
      output buffer, and fixed errors. This is input admission only, not an engine
      or per-ABI stack bound; do not add a serving call site.
- [ ] Publish qualified source-bound cost functions and complete
      cache/work/scratch/target descriptors before wiring
      this boundary into an auction. The scalar projection accepts no authority to
      enable unqualified validation and does not authenticate evidence or police
      actual allocation. Account for retained per-shard cache populations and
      contention-driven transient caches, not only peak simultaneous searches.

- [ ] Add the exact candidate dependency without enabling runtime behavior. Inspect locked features/checksums and both macro/runtime sources; compilation or unavailable API is a real failure, not permission to use a floating version. Record generated-code bytes, compile resources and runtime-helper dependencies separately.
- [x] Before the new qualification target can run in CI, install `wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2` in the native job that executes it, and resolve/fetch the locked qualification dependencies for all four targets before offline children start. Target installation in another matrix job is not shared setup. Pin these prerequisites and their execution guards in this task, not Task 17.
- [x] Materialize the temporary harness's lockfile before locked build children run, and verify its resolved engine/helper versions/checksums against the reviewed workspace lock. Reject unexpected re-resolution; a copied workspace lock that is invalid for the temporary package must not be passed off as a successful `--locked` qualification. Bound and record the native/target prefetch separately from offline compilation.
- [x] Inventory the 261 pinned schemas structurally: drafts, actual keyword positions, formats, patterns, combinators, references and cycles. Do not mistake a property named `format` for a format keyword.
- [ ] Generate validator marker declarations and a linked `schema-validator-qualification` harness for `S` in a bounded temporary crate, not a new workspace member or a production schema cutover. Make every member of its generated cohort reachable through a parameter-dependent entrypoint; compare set equality, not only a count. Preserve the existing separate all-261 compatibility/capture harness and source census. Use the maintained macro, explicit draft/format policy and refusal of undeclared remote/filesystem references without retrieval. Exercise the supported keyword, missing/null, combinator, format and character-class cases actually required by `S` against independent reference cases.
- [ ] Derive source-hash/engine-version-bound conservative work and allocation functions for supported keyword paths, including failure paths. Use the proposed numeric ceilings above. Unknown keywords without a proof, reference amplification or unsupported regex semantics fail qualification.
- [ ] Instrument independent work/allocation controls and exact/one-over single and aggregate limits. Audit generated checks, including any lazy regex/cache initialization; no request may parse/compile a schema, and a warm request must not repeat initialization. A failed expensive input must consume its charge; an already-exhausted receipt must never call the validator. Do not compare an expectation computed from the same mutated constant.
- [ ] Retain the per-member path/blob/length witness produced by Task 1 from `git --no-replace-objects ls-tree -rl d7f8515b86258688304b0d9b6668c6a0e258bc9e -- static/bidder-params`. Sum raw blob lengths and assert the 261/164,518/3,778 literals plus the maximum member/blob from spec section 3.1; import reserialization cannot redefine them. These are source inputs, not compiled-memory estimates. Commit finite per-target delta allowances and bound derivations before runtime integration; exceeding them follows the amendment path above.
- [ ] Record per-target generated-validator code/data, lazy-cache, initialization and scratch upper bounds keyed by source/generator/helper/ABI/features/toolchain/lock identities. Missing/stale contributions refuse qualification. These records feed the later build_schema generator and Task 4 aggregator; metadata codegen does not estimate validator storage.
- [ ] Run `cargo test --locked -p auctioneer-bidder --test schema_engine_qualification` after implementing its `S`-scoped acceptance contract. The bounded child driver checks exact generated/linked cohort membership, native behavior and native/guest observations with the current production profiles. Preserve the existing all-261 compatibility driver rather than repinning its counts to `S`. Execute the four build commands below in the temporary crate, copying all profile definitions/overrides; record actual code/data sections and artifact hashes. Add bounded guest execution where supported, clearly distinguishing a schema probe from the deployed application. Required missing facts block shipped admission; provider-only facts retain resource qualification/publisher pilot ownership. `cargo check`, source length, rmeta/rlib or observations alone cannot discharge a proved-bound obligation. Commit the actual evidence or a failed report; Tasks 12/17 remain pending on unresolved `S` obligations, not on hypothetical future catalog backends.

```sh
cargo build --locked --offline --release --bin schema-validator-qualification
cargo build --locked --offline --profile wasm-release --bin schema-validator-qualification --target wasm32-unknown-unknown
cargo build --locked --offline --profile wasm-release --bin schema-validator-qualification --target wasm32-wasip1
cargo build --locked --offline --profile wasm-release --bin schema-validator-qualification --target wasm32-wasip2
```

- [ ] If a required feature/target fails, attach its reproducer and follow spec section 7.1's recovery decision: qualify a maintained alternative ahead-of-time generator or bounded upstream macro fix. A startup compiler requires a separate approved spec/plan amendment and renewed bounds, not an automatic fallback. Independent inventory/discovery work may proceed; no schema, bidder or target may be dropped and no permissive validator may be substituted.

Native preparation (2026-10-17), not Task 3 completion:

- `schema_source_inventory` reconciles all 261 committed raw inputs and their source witnesses. It records 260 Draft 4 schemas and Ogury's Draft 2020-12 schema; unresolved URI scopes/anchors remain explicit, never silently supported.
- `schema_compiler_controls` exercises actual committed schema paths with explicit draft, format and bounded linear-regex options. Initial controls exposed ignored Draft 4 UUID validation and a hostname-length incompatibility. Test-only maintained-parser/regex hooks pin the reference behavior; they are not approved production policy or allocation bounds.
- The complete `schema_engine_qualification` target intentionally remains absent. The later `schema_build_controls` increment implements all-schema links and resolver controls with CI prerequisites; independently sourced instance cases, cold/warm allocation/work proofs and target-specific cost descriptors above remain unchecked.
- The two required shared lockfile upgrades (`regex-automata` and `zmij`) are recorded in the qualification report. No schema-validation code was added to a production auction path, and native probes do not qualify WASM memory.
- Verified the 31 new controls, secure-default bidder targets (198 passed), privacy targets (407 passed), shared bidder/core/exchange/runtime targets (2,076 passed, two existing ignored), workspace Clippy, formatting and the raw-JSON gate. Exact commands and review scope are in the [preparation record](../../bidders/schema-qualification.md). No all-target engine or resource gate was discharged.

Linked compiler preparation uses `schema_build_controls`, not the full
acceptance target. It generates all 261 markers and a parameter-dependent
dispatch table in a disposable crate, materializes and reconciles its lock,
and links native plus three WASM artifacts under the complete copied profiles.
The unknown-OS target has explicit qualification-only Rust exports because
its command-line stub would otherwise let optimization erase the table.
No production registration, validator policy approval or physical-memory
readiness follows from these links; resource/instance-oracle gates stay open.
Verification on 2026-10-17: 228 secure-default bidder tests (including 30
build controls), 94 structured CI workflow tests, workspace Clippy and
format/raw-JSON checks passed. Both schema-to-callback/source mutations and
the two independently renamed WASM exports are refused. The qualification
record includes observed artifact bytes/digests, not residency bounds.

Compatibility failure recorded on 2026-10-17: the pinned candidate accepts
strings where the captured Yandex schema declares integers and Consumable
declares a number. Required-rejection probing failed on the real generated
callback; retained failed-candidate controls cover both inputs, valid numeric
controls and the maintained runtime validator's contrasting rejection. See
[the failure record](../../bidders/schema-qualification.md#disqualifying-type-bypasses).
That unpatched candidate cannot be promoted. The recovery checkbox above remains open:
qualify a maintained alternative or bounded upstream macro fix, never edit
the captured schemas or hand-roll a type prefilter. Schema integration and
Tasks 12/17 acceptance are held; independent foundations may continue.

The native allocation increment adds callback-only requested-heap observations
with independent allocator controls, all-261 empty-object dispatch and 62
named inputs across 11 validators. Fresh processes and complete repeats
separate first use, changed-input cache growth and repeated-input allocation.
Boundary strings/collections, late failures and allocating fractional-number
refusals are exercised. Secure-default bidder targets pass 230 tests, with
no new ignored tests; workspace strict Clippy passes. These sampled native
observations do not complete the work/cache/concurrency/target proof or
discharge any full qualification checkbox. That increment did not repair the
candidate compatibility failure; the acceptance holds remain in force.

Approved recovery increment (2026-10-19): AG authorized carrying the minimal,
checksum-pinned compiler repair under `vendor/`, excluded from the workspace.
That repair changes only `object_schema.rs` from the published 0.58.6 package;
the separately approved October 21 increment above also changes `emit_root.rs`.
A portable
patch and original archive preserve the upgrade/rebase path; provenance
controls verify every expanded member. The root and standalone child Cargo
manifests carry the same override, and actual Cargo metadata must resolve
the exact local compiler. The Yandex/Consumable controls now require rejection,
and synthetic integer/union controls cover inapplicable keyword branches
through all three generated validation APIs. No schema is rewritten and no
production validator or handwritten type prefilter is introduced. See the
[patch record](../../bidders/schema-qualification.md#approved-compiler-patch).
The full compatibility/work/cache/target-resource qualification and Tasks
12/17 acceptance remain pending; repairing these reproducers is not Task 3
completion.

Source-instance increment (2026-10-20): the existing linked compiler target
now drives all 90 accepted/rejected literals from three complete pinned
parameter-test tables (appnexus/openx/pubmatic), rather than recomputing their
expected booleans with Auctioneer. Capture ownership pins revision, raw-byte
integrity, exact source/case shapes, counts and ordering. An independently
supplied reference checkout additionally verifies immutable blobs and complete
literal-table byte witnesses. The raw-input probe distinguishes two JSON
refusals from 59 schema refusals and 29 accepts. Checkout-filter controls
protect the new capture without modifying the original schemas/captures.
This is native compatibility evidence for three families, not full instance,
work/cache/allocation/target qualification or live schema integration. The
Task 3 acceptance checkboxes remain open; see the
[capture contract](../../bidders/schema-qualification.md#captured-parameter-expectations).

Linked-artifact increment (2026-10-21): the existing linked control now records
function-body bytes, active/passive data payloads, defined/imported function and
memory counts, and defined initial-memory declarations through the locked WASM
parser. Recursive component accounting counts nested core modules once, not their
enclosing sections again. Exact byte admission, independent literal fixtures,
overflow, semantic-validation and outer-format controls guard the reader and its
real artifact wiring. Native code/data measurements, disjoint residency,
initialization/cache/work/scratch bounds and qualified descriptors remain open;
these complete-harness observations do not satisfy the full acceptance target or
authorize runtime validation. See the [measurement record](../../bidders/schema-qualification.md#guest-section-measurements).

Priority source-instance increment (2026-10-21): expand the same externally
labeled capture and native callback driver to all eleven available tables for
the priority families plus shipped AppNexus: 236 cases, with independently fixed
per-family verdict counts. The canonical `thetradedesk` table covers the selected
`ttd` identity; rise/rubicon/triplelift have no matching table at this pin and
receive no invented reference expectations. Immutable source blobs, table spans,
literal offsets and comment-only gaps remain part of the witness checks.
This expansion does not close the work/cache/allocation proofs, qualify the
remaining 250 schemas or authorize live validation; Task 3 remains open.

Format/pattern source-instance increment (2026-10-22): the same capture owner
and generated-callback driver add complete bidstack/feedad/visx tables, retaining
all previous 236 rows and source witnesses byte-for-byte on reconstruction.
The 300 rows across fourteen families remain within the unchanged 64-KiB capture
limit, with independently fixed 106/181/13 accept/schema/JSON counts. This adds
UUID, patterned identifier and nested SDK-property cases; it changes no source
schema, compiler patch, engine limit or serving behavior. The full Task 3 and
Task 12 gates remain open. See the [capture contract](../../bidders/schema-qualification.md#format-and-pattern-source-cases).

The source audit also records a conditional cache-population derivation for the
locked regex pool. Actual shard/concurrency ownership, per-cache capacities,
byte/work bounds and target descriptors remain unqualified. No pool policy,
workload limit or funding equation is changed by that source-only audit.

Retained-fallback increment (2026-10-23): the native qualifier now includes
initialized PikeVM/backtracker storage across a fixed cache's serial search
history, not just fresh Core construction. It covers all nine fallback vectors,
independent per-allocation minima, legal shorter spans under the visited cap,
retained capacity after shorter input and serial growth overlap. The complete
23-helper/32-/64-bit membership and hostname controls use the actual emitted
patterns. The existing allocator probe and independently reconciled per-phase
receiver exercise real engines rather than length-only usage reports.
This is a conditional requested-layout component, not private guest ABI proof,
whole-meta/schema scratch/work qualification or permission to enable serving
validation. See [component 19](2026-10-22-explicit-schema-search-cache.md#19-retained-lazy-fallback-requested-heap)
and the [scope/derivation](../../bidders/schema-qualification.md#retained-lazy-fallback-requested-heap).

Fixed-cache composition increment (2026-10-23): the qualifier now joins eager
capture/one-pass, primary forward/reverse, optional-prefix and both retained
fallback owners for one fixed native cache's serial earliest-search lifetime.
It conservatively funds the full optional-prefix peak and only one largest
remaining collection-growth overlap. All 23 emitted helpers and hostname keep
typed complete outcomes; no failed component is replaced by zero. This closes
only conditional mutable requested-heap composition, not shared programs,
whole-schema/format overlap, work/lifetime, allocator/target qualification or
Task 3 acceptance. See [component 20](2026-10-22-explicit-schema-search-cache.md#20-compose-fixed-meta-cache-requested-heap)
and its [derivation](../../bidders/schema-qualification.md#fixed-meta-cache-requested-heap-composition).

Unused-pool increment (2026-10-28): the native qualifier separately accounts
for eager pool bookkeeping still present behind the explicit-cache API. Actual
compiled constituent layouts and pinned collection rules derive 1,528 requested
bytes per singleton, including its full absent-cache slot, empty aligned shard
and fat factory capture. Discovered 23-helper/one-hostname clone/release controls
reconcile all 36,672 bytes without attributing shared programs to each clone.
The result is conditional on unused pools; it is not lifetime non-acquisition,
physical/guest, full program, constructor/work or serving qualification. Task 3
and Task 12 remain open. See [component 23](2026-10-22-explicit-schema-search-cache.md#23-unused-native-meta-pool-bookkeeping)
and the [derivation](../../bidders/schema-qualification.md#unused-native-meta-pool-bookkeeping).

Immutable-metadata increment (2026-10-28): the native qualifier accounts for
the `RegexI` and shared `RegexInfoI` Arcs, the four-slot properties vector,
both scalar property boxes and actual lazy-cell inline storage. Source-derived
layout arithmetic gives 10,176 requested heap bytes plus 576 static bytes for
all 23 emitted helpers and the one shared hostname. Public property and aligned
Arc controls corroborate requests and complete release. The shared layout-helper
extraction preserves the prior pool result. This is not complete program,
constructor/work, physical/guest or serving qualification; Task 3 and Task 12
remain open. See [component 24](2026-10-22-explicit-schema-search-cache.md#24-native-immutable-regex-metadata)
and the [derivation](../../bidders/schema-qualification.md#native-immutable-regex-metadata).

NFA/capture increment (2026-10-28): the native qualifier derives the retained
forward/reverse and optional prefix-reverse trees, including boxed transition
children and unnamed capture metadata. All 24 singletons have 53 candidate
trees; the per-pattern forward/reverse/maximum-prefix envelope totals 113,960
requested bytes. Independent prefix enumeration and order-sensitive maximum
controls prevent silent omission. This is not complete strategy/literal,
one-pass, initialization/work, physical/guest or serving qualification;
Task 3 and Task 12 remain open. See [component 25](2026-10-22-explicit-schema-search-cache.md#25-native-nfa-and-capture-metadata-trees)
and its [derivation](../../bidders/schema-qualification.md#native-nfa-and-capture-metadata-trees).

Retained one-pass increment (2026-10-28): the identical raw constructor yields
19 candidates and five rejections across the 24 regex singletons. Finalized
transition/start buffers total 14,520 requested bytes against a 22,360-byte
structural envelope, excluding the shared NFA and inline strategy storage.
Independent dimensions, request-size refusal, actual construction/release and
complete identity controls cover this component. Task 3 and Task 12 remain
open for complete strategy/literal, initialization/work, target and funded
serving qualification. See [component 26](2026-10-22-explicit-schema-search-cache.md#26-native-retained-one-pass-tables)
and its [derivation](../../bidders/schema-qualification.md#native-retained-one-pass-tables).

## Task 4: Catalog Accounting And Matched Resource Reports

**Files:** runtime `catalog_budget.rs`, `budget.rs`, `aux_work.rs`; bidder `src/catalog.rs` scalar aggregation and allocator target `tests/catalog_alloc.rs`; core `host_primitives/{gates,mod}.rs`, `configure.rs` and new `tests/{catalog_resource_profile,boot_strict_parity}.rs`; four provider envelopes; native `resource_profile.rs`; CLI `src/bidders/resources/mod.rs` and `tests/bidders_resources.rs`.

Numeric-comparison preparation (2026-10-21): the resource command adds explicit
`--catalog-baseline PATH` under `--check --all-adapters`. It verifies frozen raw
identity, matched non-code build inputs, unchanged workload and shared derivation
before checking independently bounded C/I/V/K and auxiliary/aggregate growth.
The output is arithmetic-only with qualification `not_evaluated`; native local
headroom is required and WASM absolute blockers remain. The original baseline's
lock predates the approved compiler patch and two dependency edges, so the
initial command correctly refused that mismatch. That original capture is not
reset or reinterpreted; the continuity increment below supplies a separate
matched source capture and preserves intervening auxiliary-owner growth.
Synthetic controls exercise positive assembly, not positive captured
qualification. Full descriptors, auxiliary subowners, guest proofs, live
integration and target-qualified matched acceptance remain unchecked below.
See the [comparison record](../../testing/evidence/catalog-comparison-controls-20261021.md).

Matched-lock continuity increment (2026-10-21), still arithmetic-only:

- [x] Capture the clean `7286bedf` source through the existing audit command and
      retain it separately as `catalog-baseline-patched-lock.toml`; never edit the
      original `catalog-baseline.toml` or label the new record a guest measurement.
- [x] Admit the new raw pin only with a complete typed-audit continuity check.
      Starting from the original audit, permit only recorded source/lock identity
      changes, recomputed profile digests, the 65,536-byte per-permit auxiliary
      chooser increase and its existing population-scaled component. Whole-record
      equality must then hold, including all owner/pending decisions and zero costs.
      Revalidate the new audit against current derivation and auxiliary receipts.
- [x] Carry the original capture identity and preserved chooser deltas in the
      comparison report. Do not charge them a second time: the original and new
      complete auxiliary work both fit the unchanged declared reservation. Qualified
      residency records must still use the original source basis; this arithmetic
      rebase establishes no new code/data/heap bound or readiness.
- [x] Drive byte-pin and field-by-field continuity refusals before enabling the
      loader. Require a real positive command on the matching capture ABI/features,
      exact refusal for nonmatching builds, strict-fit exit 1 with identical output,
      and no rewrite on failed comparison. Test boundary guards independently.
- [x] Complete spec/quality review, secure and linked-feature controls, strict
      lint and unchanged-capture checks. Each mode passes 138 selected resource
      unit tests and the complete 16-test command target, with none failed or
      ignored. Keep the separate roadmap edit outside this increment. Full Task 4
      and resource qualification acceptance remain open; see the [continuity verification record](../../testing/evidence/catalog-baseline-continuity-20261021.md).

Owner/parity control increment (2026-10-21), not complete Task 4:

- [x] Extend the existing strictly non-installing `catalog_resource_profile`
      with complete 11-term isolate and six-receipt auction comparisons. Count 12
      independently expected source-input controls, including coupled populations
      and both parser-dominance branches, without deriving expected deltas from
      the runtime coefficients they pin.
- [x] Count 72 config/catalog controls across auction populations 1/3/7 and
      auxiliary permits 0/1/5. Keep baked/default, backend/blob/node, C/I and V/K
      charges independent. V/K belong only to projection; fixed construction
      belongs to privacy. Synthetic baked scalars never enter the core cache.
- [x] Reject 11 independent isolate-term omissions, V and K individually moved
      into config, 128x/per-auction/per-auxiliary residency, per-auction initialization
      and a one-byte node unit. Restore the original runtime source after every
      probe; no production equation, coefficient or input limit changes.
- [x] Expand the existing isolated real CLI harness to `--all-adapters` and
      compare ten raw-cap cases against 40 fresh invocations of the same sole core
      boot probe. Distinct synthetic adapter baselines have separate literal
      expectations. Do not add another installer, production test hook or provider
      qualification claim. Accept duplicate Cargo units only for identical
      source/feature identity; reject conflicts and missing owners.
- [x] Require fixed ordered traces for four scratch-only corruption controls,
      including Cloudflare-only and Spin-only corrupt rows, then exit 2 and empty
      JSON output. A missing hook cannot substitute an unknown-hook error for
      executing the intended corruption. Compare construction and validation
      traces rather than assuming each row resolves only once.
- [x] Complete final trace-enabled verification, spec/quality review, strict
      lint, formatting and unchanged-source/capture checks. Secure and linked-feature
      core targets each pass ten pure tests plus the sole boot-probe parent; the
      complete CLI parity target passes five tests, 40 boot comparisons and four
      traced refusals. CI workflow guards pass 103 tests. Exact commands, repeated
      scopes and limitations are in the [verification record](../../testing/evidence/resource-owner-parity-controls-20261021.md).
      Qualified descriptors, remaining strict-boundary/aggregate proofs, live
      catalog integration and resource qualification readiness remain open.

Config-owner preparation (2026-10-20): the config build now emits an
allocation-free metadata storage census from the actual rendered constructor
expressions, with source-bound counts and checked target-side layout arithmetic.
The independent cold/warm allocator target walks the typed model exhaustively
and checks requested bytes, allocation counts, retained bytes and peak bytes.
This is not a qualified resident descriptor: allocator overhead, static literal
pooling/alignment, executable residency, complete qualification identities,
schema/registry costs and aggregation remain open. Production catalog costs
stay explicitly unchanged, with no live cutover or workload change. See the
[census verification record](../../testing/evidence/metadata-storage-census-20261020.md).
The full funding/qualification items below remain unchecked.

Aggregation preparation (2026-10-20): `auctioneer-bidder/src/catalog.rs` now owns
the pure scalar join. All four component owners are mandatory, including
explicit empty records. It checks independently supplied source/qualification
digests and the common target/features/toolchain/lock identity before merging
strictly ordered storage streams. Shared storage identities must carry identical
C/I/V/K charges; distinct equal-sized storage remains separately charged. The
checked merge applies no population, baked-default or auxiliary multiplier and
does not initialize metadata, indexes, bidders or caches. This is consistency
checking of scalar projections, not verification of artifact bounds or record
authenticity. No qualified production record or serving consumer is installed;
all shipped catalog descriptors remain `UNCHANGED`. Full generated-owner
qualification, precharge and the live cutover remain open. See the
[aggregation verification record](../../testing/evidence/catalog-cost-aggregation-20261020.md).

- [x] Before any new allocation-producing runtime behavior in Tasks 5-15, add mandatory scalar catalog/context descriptors to the existing runtime derivation. Keep zero-valued incremental descriptors for unchanged paths until their owning implementation lands; every behavior commit installs its nonzero verified descriptor atomically. Do not introduce an independent physical-memory qualification system.
- [x] Enforce the predeclared 16-MiB resident, 8-MiB initialization and 512-KiB validation-scratch component ceilings before the CLI exports an adapter's source row. Test independent below/at/above values and the real row consumer, preserving pending target readiness. The 1-MiB settings/secret-view bound is not a ceiling on all context `K`; its other owners need separately justified allowances. This component check does not discharge matched-baseline, per-target aggregate, auxiliary or descriptor-qualification acceptance.
- [ ] Retain the checked additive model and the complete B/G/Q/E/A/T/P/H/S source mapping from spec section 11.1. Add disjoint C and initialization I once, V and context K per auction; rederive catalog auxiliary scratch inside A using its existing auxiliary plus probe-config permit rule. No startup/steady max or subtraction of existing baseline reserves. Introduced auction charges enter the existing receipts, never a separately unfunded term.
- [ ] Pin scalar source bounds before lazy metadata/cache initialization. Core's serialized `(host_default(), account_default())` allowance remains unchanged when only metadata/validators grow; C/I are never multiplied by CONFIG_PARSE_FACTOR or auction/aux permits. Record generated executable-code/data growth with its appropriate target owner rather than assuming compile-time work eliminates guest residency.
- [x] Extract public, non-installing `resolve_budget_inputs` in core `host_primitives/gates.rs`, re-exported from `host_primitives`. Delegate to existing crate-private `configure::extraction_limits_for(Some(raw_limits))`; normalize `max_runtime_config_bytes`, `max_runtime_config_blob_bytes` and `max_config_json_nodes` to its effective backend/blob/node caps, preserving nonzero declarations and resolving each zero independently. Return `BudgetInputs` with the normalized row and checked `baked_config_bytes` counted over exactly `(auctioneer_config::host_default(), auctioneer_config::account_default())`, sharing boot's cached result. Boot and native resource export consume this facade. Its tests independently vary all three caps, count the exact tuple through a separate serializer, and prove no gates are installed.
- [ ] Make the new CLI strict resource checker use `resolve_budget_inputs` as well; do not duplicate SDK defaults outside core, expose an inaccessible crate-private helper as the CLI contract, or install gates merely to resolve inputs. This preparation does not discharge the catalog descriptor, matched-report or independent `boot_strict_parity` acceptance steps below.
- [ ] Implement `auctioneer-bidder/src/catalog.rs` as the single checked aggregator: config build_codegen owns metadata/binding bounds only; bidder build_schema owns validator code/data/lazy-cache descriptors from Task 3's qualification record. Add registry/index/bidder costs and deduplicate shared identities. Core passes the result unchanged; config never imports bidder code. Independently vary each contribution and reject missing, stale or mismatched qualification identities before allocation. Verify actual post-build code/data and allocator bounds through `catalog_alloc` and the CLI resource checker; emitted Rust source length is not a memory proof.
- [ ] Independently vary C/I/V/K, all nine existing summands, catalog size, sharing, initialization failures, auction/aux/probe populations and each info/correlation contribution. Use unequal literal terms; one I byte increases the total even under dominant auction demand and while startup has zero live auctions. Keep configured concurrency at least one, preserving the existing scalar refusal of zero. Direct term omissions, double charging and resident 128x/per-permit mutations must fail.
- [x] Pin an independently computed small arithmetic control: existing `(B,G,Q,E,A,T,P,H,S) = (1,2,3,5,7,11,13,17,19)` sums to 78; `C=23`, `I=29`, `N=2`, `V=31`, `K=37` yields total 266. Increasing I alone to 30 yields 267. These are derivation controls, not real profile values or a replacement for actual term-owner tests.
- [x] Before emitting nonzero live catalog costs, version the native resource export and qualification profile contract to carry the exact mandatory descriptor and derived terms through serialization, identity hashes and replay. Do not default missing fields, reinterpret old captures as catalog-qualified, or edit historical evidence. Add independently nonzero-field round trips and real native/CLI consumer controls before this cutover.
  - [x] Hard-cut native export, profile, observations and report to schema 2 with a required four-field object-valued `resources.catalog_costs`; leave unrelated candidate/catalog/runner/provider protocol versions unchanged. Retain runtime's serialization-free ownership via explicit export/remote DTOs.
  - [x] Rederive the exported total and four catalog terms, and verify the typed resource-serialization digest before harness binding. Exercise independently positive fields, forged terms, stale digests, missing/duplicate/unknown fields, positional arrays and old/unknown versions.
  - [x] Carry costs through profile/observations identity and report accounting. Use real CLI commands to prove nonzero costs change hashes and accounting and unmatched costs refuse. Preserve historical blobs and integrity tests; synthetic current-version fixtures are not recaptures or production migration support.
- [x] Produce four matched pre-runtime-catalog accounting baseline records from raw production declarations passed through `resolve_budget_inputs`, full baked-default allowance and exact build identity. Record raw and effective config caps and the baked tuple's digest/byte count so normalization can be audited. Record WASM absolute failures, not only old golden subtotals; do not rewrite historical captures. The frozen `catalog-baseline.toml` captures clean checkpoint `e71e40ab` after the authorized resource qualification repairs, before any live catalog growth; it does not claim that checkpoint predates all bidder catalog preparation.
- [ ] Implement `bidders resources --check --all-adapters` over component and per-target allowances. A relative pass keeps WASM readiness blocked. Native demand plus headroom must still fit; test unequal platform/configured ceilings and component breaches hidden by unrelated reductions.
- [ ] Keep `catalog_resource_profile` strictly non-installing: pure descriptor/default-byte isolation, C/I/V/K/A and all existing term sweeps, dominant auction demand, auxiliary/probe populations, unequal ceilings and under/at/over additive fit. Synthetic baked-byte values belong here, not in a shared process-global cache. A strict-only `max(startup, steady)`, omitted I/K, 128x charge or zero-baked wrapper must fail its independent control.
- [ ] Create `boot_strict_parity.rs` with the same load-bearing single-test/process-isolation header as `boot_order_gates.rs`. Its sole `#[test]` drives a bounded fixed set of fresh child invocations of that binary, one installing row per process; count every row and assert no gates are installed before each child boots. Never reset or reuse `BOOT_GATES`, `BAKED_CONFIG_BYTES`, `PROBE_EGRESS_GATE` or `OVER_CEILING_WARNED`, and do not treat a mutex or `--test-threads=1` as isolation. Exercise actual `install_boot_envelope` from raw all-zero config caps, each independently zero, and unequal explicit nonzero caps. Compare retained effective caps/baked bytes/derived terms against the real strict facade, using independent expected defaults and a separately serialized complete host/account tuple. Include nonzero C/I/V/K/A and a boot-warns/strict-refuses case. Do not call the pure derivation twice and call that consumer parity; skipping normalization in either consumer must fail the named zero-cap case.
- [ ] Add actual CLI zero-cap and explicit-cap controls in `bidders_resources`: invoke `bidders resources --check --all-adapters` with raw profile declarations, require the expected effective caps/full baked tuple charge in its report, and reject a report produced with raw zero caps or zero baked defaults. These are pure command processes, not another boot installer. Mutating the CLI to bypass the public input facade must fail this target even if the core parity target still passes.
- [ ] Run the commands below as separate invocations against current/synthetic descriptors. Require four source-bound target rows and no omitted cost component; relative WASM passes must retain their absolute blocked status. The four envelope commands are host-side profile controls, not claims of guest execution; Task 17 also requires actual target/package builds. Repeat these against implemented foundations/full-size synthetic catalogs. Initial observations are bound checks, not credentialed-provider or physical-OOM guarantees. Commit verified funding before live catalog materialization; Task 3's independent, non-promoting build/discovery preparation may proceed without claiming this acceptance. Each later allocation owner extends these tests before enabling its behavior.

```sh
cargo test --locked -p auctioneer-runtime --lib
cargo test --locked -p auctioneer-bidder --test catalog_alloc
cargo test --locked -p auctioneer-core --test catalog_resource_profile
cargo test --locked -p auctioneer-core --test boot_strict_parity
cargo test --locked -p auctioneer-cli --test bidders_resources
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters
cargo test --locked -p auctioneer-adapter-axum --bin auctioneer-adapter-axum envelope::tests
cargo test --locked -p auctioneer-adapter-cloudflare --lib envelope::tests
cargo test --locked -p auctioneer-adapter-fastly --lib envelope::tests
cargo test --locked -p auctioneer-adapter-spin --lib envelope::tests
```

Budget-input preparation (2026-10-17) preserves the existing accounting
equation and all resource constants. The public resolver is shared by boot and
the native export, but installs no gates and does not itself validate the
envelope. The secure-default core library suite passes 471 tests; the real
boot/config-node targets pass two tests, and native boot/export targets pass
59 tests, including the existing resource goldens. Catalog descriptors,
strict CLI checking and matched-profile acceptance remain pending.

Scalar catalog accounting adds mandatory `CatalogResourceCosts` to every
runtime derivation and boot entry point, retained for diagnostic re-derivation.
`C` and `I` enter the existing checked isolate sum once; `V` and `K` enter
the existing prepaid projection receipt per admitted auction. `UNCHANGED`
means no incremental catalog allocation beyond the matched baseline, not a
claim that the current catalog has no memory cost. All shipped rows still use
that explicit descriptor. Runtime sensitivity, overflow, exact-ceiling and
receipt-unwind controls exercise nonzero costs; the small arithmetic control
uses the actual sum helper. Generated-bound aggregation, expanded auxiliary
work, full matched reports and the independent strict/boot targets remain
unfinished. Native version-one output is unchanged for zero increments and
cannot represent a live nonzero catalog; the versioned contract step above is
required before introducing that behavior. This describes the initial scalar
increment; the subsequent protocol cutover below supersedes the version-one
export restriction without qualifying catalog materialization.

Verified on 2026-10-17: the feature-enabled runtime/bidder/exchange/core
all-target suites pass 2,142 tests with two existing ignored tests; CLI unit
and qualification/evidence targets pass 546 with two existing ignored tests;
native boot/export targets pass 85, and the other three provider boot targets
pass three. Workspace all-target Clippy with `insecure-bidders,conformance-suite`
and `-D warnings`, formatting and raw-fixture checks pass. These provider
controls run on the native host, not inside hosted guests. No dependency pin,
production limit or captured artifact changed. Both review stages approved
the accounting increment, not full catalog qualification or physical fit.

Resource protocol version 2 now carries the mandatory four-field catalog
descriptor through native export, CLI profile and observations, resource/full
profile identities and report accounting. The native harness validates both
the resource digest and rederived total/catalog terms before binding a run.
Its streamed object-only decoder preserves duplicate/unknown-field checks;
it does not accept Serde's positional struct arrays. Current-version synthetic
fixtures retain latency/unexpected-evidence controls, while real commands
reject untouched historical version-one captures before writing a report.
The captured artifacts and their integrity test remain unchanged. Positive
cost controls do not promote bidders, install live catalog growth, or close
the pending qualification/aggregation/resource-checker work above.

Protocol-cutover verification on 2026-10-17: the CLI unit binary plus
`qualification_cli`, `qualification_harness` and `qualification_evidence`
pass 554 tests, with two existing ignored tests. Native binary,
`boot_envelope` and `resource_profile` targets pass 84. Commands use
`cargo test --locked --offline -j 2` with each package's named targets;
CLI process/socket checks run with local host permissions and no deployed
provider access. Strict workspace all-target Clippy with
`insecure-bidders,conformance-suite` and `-D warnings`, formatting and the
169-fixture raw-JSON gate pass. Spec and quality review approved the scoped
increment after an object-only decoding repair, observed RED then GREEN.
Dependency/toolchain pins, production limits and all historical fixture and
evidence blobs are unchanged. This is protocol verification, not a new
measurement, full catalog qualification or physical-fit result.

Component-ceiling preparation (2026-10-19): the existing CLI source-row
constructor now refuses C/I/V declarations above the three predeclared
ceilings, before input derivation or export. Eleven adjacent policy tests
cover independent below/at/above values and combined boundaries; two consumer
tests exercise the real constructor and preserve all four targets' readiness
blockers with nonzero costs. The first behavioral RED had 85 passing and four
failing resource tests; GREEN has 90 passing, including the later combined
positive control. Current shipped catalog increments remain explicitly zero.
No bidder promotion, descriptor qualification or matched-baseline acceptance
is implied, and no complete-K ceiling is inferred from the narrower
settings/secret-view limit.

The full CLI unit binary passed 618 tests with two existing child-process
probes ignored; its 12 resource-command integration tests passed. A subsequent
test-literal notation-only repair satisfied the existing hexadecimal-literal
lint, without changing numbers or adding a suppression. The final resource
prefix rerun passed 90 tests; all-target CLI Clippy with `-D warnings`,
formatting, diff and the 168-fixture raw-JSON gate passed. Independent spec
and quality reviews approved this scoped step, with the latter rechecking
the notation-only change. Logs are `/private/tmp/bidder-catalog-catalog-ceilings-*.log`.
These are native CLI controls, not guest execution or target qualification.

Frozen baseline preparation (2026-10-19): the existing source command captured
and validated all four rows from clean `e71e40ab` with SDK `bc300ace`,
application-only features, the complete 1,584-byte defaults tuple and zero
incremental catalog costs. The 131,237-byte snapshot is retained separately
from the older audit at
`crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml`.
Its raw SHA-256 is
`6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39`.
A byte-pin/frozen-record test preserves its original arithmetic, ABI and
unqualified status rather than claiming it describes every future build.
The resource prefix passes 91 tests. The real checkout-filter test also
passes all eight protected fixture cases with a positive unprotected-LF
conversion control; removing the new baseline's `-text` rule first failed
on that exact file. Current-source audit replay remains stricter than frozen
evidence inspection. Matched allowance checking and proof-backed descriptors
remain unfinished, and no existing capture was replaced.
All-target CLI Clippy with `-D warnings`, formatting, diff and the 168-fixture
raw-JSON gate passed; both review stages approved this baseline-only slice.
Logs are `/private/tmp/bidder-catalog-baseline-*.log`; no guest execution is implied.

## Task 5: Generate Bidder Discovery In The Existing Module Scope

**Files:** bidder `build.rs`, `build_discovery.rs`, build dependencies, `src/bidders/mod.rs`, `url.rs`, existing implementations' `bidder.toml` and `<bidder_key>/mod.rs`.

- [x] Write parser/generator unit failures for duplicate keys, bad Rust identifiers, keywords, unknown fields/features, escaping/symlink sources, helper collisions and incomplete directory pairs.
- [x] Move only `url/mod.rs` to `url.rs`, retaining tests/API. Keep `media_type.rs` and the cfg(test) harness as ordinary modules.
- [x] Add conventional `create_bidder() -> Box<dyn Bidder>` functions and adjacent `bidder.toml` declarations for current implementations. Only `key` and optional `required_feature` are permitted; each directory name equals its declaration's `<bidder_key>`. Constructors remain infallible, synchronous, stateless and I/O-free.
- [x] Generate modules directly inside `bidders`, not an extra wrapper namespace: existing `super::url`, `super::media_type` and `crate::bidders::harness` paths must keep compiling. Generator-owned validated path attributes are allowed; declaration-provided paths are not.
- [x] Generate an all-declarations data table and feature-filtered `BidderRegistration` factory table exposed through `compiled_bidders()`. Validate all declarations first; emit rerun tracking for root/directories/source/manifests, including additions/deletions.
- [x] Run `cargo test --locked -p auctioneer-bidder --lib --test insecure_bidders_snapshot`, then the feature-enabled bidder library tests. Commit without changing the current live catalog.

Build-only preparation (2026-10-17): `build_discovery.rs` supplied the strict
fixed-root scanner, deterministic rerun-input inventory and module/table
renderer, exercised through `tests/discovery_codegen.rs`. Its original 15 tests included
actual bounded `rustc` compilation and factory execution from another working
directory, nested/sibling helpers, unknown names, zero eager construction and
feature off/on/off. That preparation did not install production build-script
wiring or APIs.

Discovery integration (2026-10-19): `build.rs` now invokes that scanner and
renderer for the fixed production root. The four existing implementations have
adjacent declarations and stateless factories; generated static slices expose
all declarations and only feature-enabled factories. The URL helper move is
byte-identical. New library tests execute the generated factories through real
request sinks and response decoders, independently distinguishing protocol and
feature behavior. The inline `tests` module is reserved alongside `harness`.

`cargo fmt` does not follow the generated module declarations: an actual
unformatted-child probe passes its check. The formatting workflow therefore
also checks every Rust file below the fixed bidder source root. A bounded
process test requires malformed unknown-bidder and nested-helper files to fail,
with formatted controls passing; semantic CI guards reject narrowing or
disabling this step. The native test job explicitly installs Rustfmt, with
missing-component controls; its POSIX formatter process test is Unix-only.
No lint suppression or raw-JSON exemption was added.

The core live registry, metadata eligibility and protocol logic are unchanged.
Task 6's metadata feature predicates, Task 7's strict join and Task 8's
reused-Cargo-target lifecycle proof remain open. These static discovery views
do not install a live catalog, qualify schema validation, supply the positive
catalog descriptor aggregation, or establish resource qualification/WASM readiness.

Verification against the discovery integration (2026-10-19, Rust 1.98.1,
native macOS host, locked/offline dependencies; logs
`/private/tmp/bidder-catalog-discovery-*.log`):

- `CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 cargo test --offline --locked -j2 -p auctioneer-bidder --lib --test discovery_codegen --test insecure_bidders_snapshot`: 128 library, 17 generator/process and three compiler feature-gate tests passed, zero failed/ignored. The socket fixture requires a local Unix socket; the sandboxed run refused its bind, and the complete suite passed outside that restriction without changing the test.
- The same command with `--features insecure-bidders`: 139 library, 17 generator/process and three feature-gate tests passed, zero failed/ignored.
- `cargo test --offline --locked -j2 -p auctioneer-core --test ci_workflows`: 103 passed, including deletion/narrowing/failure-suppression controls and Rustfmt component mutations.
- `cargo clippy --offline --locked -j2 -p auctioneer-bidder --all-targets -- -D warnings` and its `--features insecure-bidders` variant passed. `cargo clippy --offline --locked -j2 --workspace --all-targets --features insecure-bidders,conformance-suite -- -D warnings` passed, compiling native-host consumers including the CLI and all four adapter crates. No lint suppression was added.
- `cargo check --offline --locked -j2 --workspace --all-targets` passed under the workspace's default feature selection; this is compilation, not execution of every workspace test.
- `cargo check --offline --locked -j2 -p auctioneer-bidder --lib --target wasm32-unknown-unknown --target wasm32-wasip1 --target wasm32-wasip2` and the same command with `--features insecure-bidders` passed. These six guest-library checks do not link or execute guest artifacts, qualify generated schemas or prove memory fit.
- `cargo fmt --all -- --check`, the workflow's direct source-tree Rustfmt command, `git diff --check` and `bash scripts/check-raw-json-fixtures.sh` passed; raw-fixture inventory remains 168 across 50 files. The URL helper's old/new Git blob IDs are identical.
- Empty-table factory tests, reserved-`tests` discovery, the missing formatting workflow step and the missing Rustfmt prerequisite each failed before their respective fix. The separate spec and quality reviews completed; both quality findings (Rustfmt provisioning and POSIX test gating) were repaired and re-reviewed.

The existing GVL deleted-vendor diagnostics and `proc-macro-error2` future
compatibility warning remain; none was suppressed or reclassified as resolved.
No full-workspace test execution, fresh hosted CI run, Windows execution,
provider deployment, positive catalog-cost qualification or resource qualification closure is
claimed by this checkpoint. Root dependency declarations, lockfile and
reference captures are unchanged.

## Task 6: Emit Strict Data-Only Bindings And Feature Predicates

**Files:** config `build.rs`, `build_codegen.rs`, new `build_bindings.rs`, `src/types.rs`, promoted metadata/schema files; config/bidder manifests; bidder `build.rs`, `build_discovery.rs`, new `build_reconcile.rs` and `tests/factory_binding_build.rs`; config target `tests/bidder_bindings.rs`; workflow and core CI command guards.

- [ ] Add strict raw DTOs for `implementation = "<bidder_key>"` binding, parameter-schema association, alias provenance, tri-state alias default and required-feature descriptors. Keep the exact public name distinct from its implementation key: public `yahooAds` uses `implementation = "yahoo_ads"`, and independent bases may share a key. Reuse `validator::Validate` for applicable typed constraints, retaining contextual cross-field checks. Reject unknown fields and duplicate source identities before rendering.
- [ ] Add explicit direct-routing disposition and optional reviewed endpoint representation. Validate every effective `direct` endpoint unconditionally, even for opt-out identities, while `alias_only`/`unavailable` entries have no callable endpoint. Bind Task 2's source/compiled policy and AG-approved HTTPS default, preserve provenance outside the dispatch field, and emit no fake URLs. Test source-disabled, endpoint-less disabled, alias-only parent and valid endpoint-bearing alias through actual metadata builds; all require real bindings, and an unapproved HTTP or absent dispatch endpoint must fail. Keep Mocktioneer's existing `insecure-bidders` loopback exception scoped; never globally fall back or downgrade on HTTPS failure.
- [ ] Before the shipped routing cutover, bind the existing appnexus HTTP-to-HTTPS override to AG's 2026-10-28 decision and its source URL/blob/intentional transport difference; no separate vendor TLS mapping is required. Add `existing_https_override_records_source_transport_difference`, rejecting missing or wrong-identity/blob records, asserting same authority/port/path/query and bidding protocol, HTTPS routing and no unapproved HTTP dispatch. Apply the same transport policy independently to msft without treating URL/factory equality as protocol qualification. Preserve shipped defaults; metadata binding and deterministic route tests remain prerequisites, not Task 20 deferrals or completed promotion.
- [x] Generate `BidderImplementationBinding { bidder, implementation }` rows through `bidder_implementation_bindings()` for all declared metadata and live metadata only for enabled predicates. Keep exact public names in `bidder`, internal keys in `implementation`, and data-only descriptors sufficient to compare feature ownership even when a factory is disabled.
- [ ] Make build_codegen's generated resource descriptor cover only metadata/bindings it emits, with target-layout qualification. A schema path/digest must never be presented as the validator's code/data bound; the latter comes from bidder build_schema and the Task 4 aggregator.
- [x] Explicitly forward the same optional feature between bidder/config manifests. Validate matching declaration and metadata predicates before filtering. A bidder-only feature cannot silently leave active metadata with no factory.
- [ ] Do not make config depend on bidder code or run factories from codegen. Generate static data bounds and lazy metadata accessors; catalog initialization pays those bounds before materializing them.
- [ ] Preserve existing shipped eligibility/endpoint defaults, local-only schemas and insecure-feature isolation, while applying section 6.1's explicit new source/routing policy. Source schemas are required for promoted source definitions; a permissive local generic schema is not an upstream migration.
- [ ] Run `cargo test --locked -p auctioneer-config --test bidder_bindings --test codegen --test smoke`. Add this new config target to CI secure-default coverage immediately; retain the runtime-directory set-equality guard. Commit with current declarations/bindings coherent.

Binding-compiler preparation (2026-10-19), not complete Task 6 acceptance:
explicit implementation keys and optional declared-feature requirements now
produce separate active-binding and all-declaration static views. The build
validates every metadata row, including existing scalar/macro render checks,
before filtering. Root shapes are strict; TOML syntax/shape diagnostics do
not echo values. Four shipped inputs retain their existing runtime behavior;
the previously ignored `platform_id` field is removed from the baked fixture,
not from its external source capture.

The [binding controls](../../testing/evidence/metadata-binding-controls-20261019.md)
record actual generated-view, compiler, child-build and CI ownership. This
preparation neither initializes a new runtime catalog nor approves endpoint
or schema policy. Strict live joining,
schema/alias/routing DTOs, target-layout/resource descriptors and precharging
remain required. Current generic fallback and registry construction are
unchanged; no source identity is promoted by this step.

Factory-predicate reconciliation (2026-10-19): the bidder build now consumes
config's all-declarations static view as data, before rendering discovery.
It rejects a missing factory, mismatched optional predicates and absent
explicit same-feature forwarding, including inactive declarations. Several
public bases may share a factory; unused factories remain legal. Config
does not import implementations or run constructors. Colocated helper tests
and a real isolated Cargo target cover the checks; the latter executes
feature off/on/off and restores five independent source/manifest mutations
in one reused target directory. The workflow requires its exact command.
See the [factory-binding controls](../../testing/evidence/factory-binding-controls-20261019.md).
This does not install the strict target/runtime join or qualify its resource
costs. Task 7 still owns actual linked membership, cached startup failures
and removal of the existing fallback; Task 8's complete incremental
add/edit/rename/remove acceptance remains open.

The isolated driver's offline lock materialization preserves the seeded
workspace package identities and checks versions/sources/checksums before
locked builds. Final verification passed its six tests, 156/167 bidder tests
in secure/feature modes, 150 selected config tests, 606 core/workflow tests
per mode, six guest-library compilation checks and strict workspace lint.
Both review stages approved the bounded slice after the lock repair; the
linked record retains failures and exact scope, not full bidder catalog acceptance.

## Task 7: Validate And Cache The Complete Shipped Join

**Files:** bidder `src/registry/{mod,catalog}.rs`, `src/lib.rs`, `tests/registry_join.rs`; core `bidder_registry.rs`, `configure.rs`, `config/{mod,runtime,budgets}.rs`; new core `tests/bidder_registry_lifecycle.rs`.

Preparatory validation phase implemented on 2026-10-19: the non-installed
`ValidatedRegistryPlan` joins complete declarations with actual active metadata,
bindings and linked factories. Typed structural errors precede every constructor;
immutable sorted views borrow metadata and retain alias immediate/canonical
ancestry. Synthetic refusal controls include inactive features and alias graphs;
the generated-view integration target executes real bidder protocols in both
feature modes. This API does not install a serving registry, reserve storage or
change the existing core singleton. See the [verification record](../../testing/evidence/registry-join-controls-20261019.md).
The full items below deliberately remain unchecked: generated storage bounds and
precharging must precede the cached live cutover, duplicate explicit insertion
must be repaired, and every consumer must propagate the same fatal result.

Ownership improvement (2026-10-21): the current serving registry now borrows
the original generated static metadata rows instead of cloning and leaking each
one. This removes retained copies without adding storage or changing factories,
fallback, activation, cache or error handling. The real shared-registry test
checks nonempty complete membership and original pointer identity in both
feature views; the previous clone path fails it. This closes only that ownership
defect, not the unchecked serving-cutover, alias or resource-qualification work.
Both core-library feature views pass 504 tests, with no failures or ignored
tests; the old clone path fails the ownership test in each view. Independent
spec/quality reviews, native workspace all-target strict Clippy, formatting and
the unchanged raw-JSON gate pass. No guest execution or new bidder is claimed.

Fallibility increment (2026-10-28): the currently published roster now uses
`OnceLock<Result<BidderRegistry, RegistryBuildError>>`. Generated dispatch
preflights borrowed metadata/factory/binding membership before calling any
constructor; explicit duplicate insertion preserves the incumbent. Config
validation, auction/info/sync consumers and CLI push preserve typed registry
failure instead of treating it as invalid runtime data or an empty catalog.
The status default/cached-verdict paths cannot erase an established fatal
failure. Cache inspection does not initialize the registry: on a cold bound
context, the funding check still precedes construction and store reads.

This is deliberately not the full serving cutover below. The existing lazy
access boundary and registry storage representation remain; `configure_app`
does not construct bidders before a qualified reservation exists. Full-catalog
indexes, precharged startup, alias/activation lifecycle controls and process-
isolated fault injection remain bidder catalog work. No candidate is promoted. The direct
API is a hard cutoff: callers must handle the fallible result, not discard it.

Verification checkpoint (2026-10-29): default-feature configuration-schema and
core `--all-targets` passed 1,430 tests across 100 targets, with one existing
ignore. Bidder library plus `registry_join`, `registry_source_conformance` and
`test_isolation` passed 1,160 tests, with nine existing library ignores. The
separate exchange `--all-targets --no-fail-fast` sweep passed 642 tests across
36 targets, with one existing ignore. Socket/nonregular-file controls ran with
the necessary local permissions, not weakened assertions. The CLI error-class
control was observed failing before its mapper repair and passing afterward.

The exchange sweep exposed three fixture builders that silently replaced an
existing fake registration. They now construct the intended first bidder
directly, preserving the test inputs and transport settings; all ten outbound
construction controls pass. No replacement API was added. These are affected
component results, not a whole-workspace test or complete registry-cutover claim.

Final CLI `config_source::` dispatch passed nine tests with two existing
ignores. Workspace/all-target Clippy with `insecure-bidders,conformance-suite`
and `-D warnings`, formatting, whitespace and both raw-fixture gates passed
(188 raw fixtures across 54 files). The lint repair borrows the CLI mapper's
error rather than adding an allowance. Existing GVL deletion and dependency
future-incompatibility notices are not new qualification failures.

- [ ] Add duplicate/factory/binding/alias/feature failures with a constructor-call counter; every invalid join fails with zero constructors. Duplicate explicit insertion preserves the incumbent.
- [ ] Build explicit public-definition, canonical-base, source-alias and factory indexes. `base_names()` must no longer mean every registry entry once baked aliases exist.
- [ ] Borrow generated static metadata; remove production clone-and-leak and generic-fallback behavior. An explicit local generic binding remains allowed, never the default for an unknown source binding.
- [ ] Replace core's singleton with one `OnceLock<Result<BidderRegistry, RegistryBuildError>>`. Initialize through `configure_app`; direct helpers use the same precharged bounded builder and cached failure.
- [ ] Propagate registry failures through `live_names`, auction lists, usersync catalog, configuration validation and absent-store runtime defaults. Preserve a distinct fatal class rather than status's degradable bad-runtime-config class.
- [ ] Run `cargo test --locked -p auctioneer-core --test bidder_registry_lifecycle --test auction_config_lifecycle --test usersync_config_lifecycle` and bidder library tests. Process-isolate global-cache failure scenarios; no mutation of a shared production singleton across parallel tests. Commit.

## Task 8: Prove Incremental Discovery By Executing Changed Artifacts

**Files:** new bidder integration `tests/registry_build.rs`, its `tests/support/` fixture helpers; `test.yml` and its colocated scripts.

- [x] Build a bounded isolated source workspace using owned repository inputs, excluding target/output/Git/credentials. Use one temp target directory for the sequence, locked offline children and a watchdog; never mutate the user's worktree.
- [ ] Execute a synthetic unknown bidder through a real sink/registry. Add a second implementation, edit it, rename its source/declaration/metadata together, remove it, and prove the final executable no longer lists or executes it.
- [ ] Independently add a metadata-only identity bound to an existing factory. Nested source helpers compile from another cwd.
- [ ] Delete only a declaration or source and require the exact build refusal; restore it and recover without cleaning. Run feature off/on/off with matching module/factory/metadata behavior.
- [ ] Assert positive execution counts and exact artifact membership, not generated-text presence. Run `cargo test --locked -p auctioneer-bidder --test registry_build -- --test-threads=1`; bound/report child runtime and peak disk use.
- [x] Wire the real target into CI and exact command/execution guards; do not mark it ignored without an explicit required driver. Commit.

Incremental-discovery preparation (2026-10-21): `registry_build` executes
unknown synthetic implementations through the generated factory/metadata views,
the noninstalled `ValidatedRegistryPlan`, and real bounded request sinks. Its
one reused target covers source-only addition before metadata, both exact
missing-input refusals and recovery, a nested-helper edit, complete rename,
metadata removal before source removal, an independent metadata-only identity,
and feature off/on/off. Every successful stage checks complete independently
expected artifact membership, lookups, protocol bodies and positive execution
counts. Directory discovery cannot borrow metadata's invalidation in the
source-only stages. Shared build infrastructure copies seven local owner crates,
reconciles the child lock against the reviewed lock, clears inherited feature/
override controls, and reports bounded child time and sampled logical disk use.
The exact serial target is required by CI; it has no ignored tests.

The middle lifecycle items deliberately remain unchecked for full Task 8:
repeat this executable contract against Task 7's funded, installed and cached
serving registry after that cutover. The preparatory join does not exercise
installation, cached fatal failures or qualified storage, and does not remove
the current core registry's fallback. See the [dated verification
record](../../testing/evidence/registry-join-controls-20261019.md#incremental-discovery-preparation-october-21).

## Task 9: Unify Activation And Alias Provenance

**Files:** core `config/{resolve,mod,budgets}.rs`, `handlers/{auction,info_bidders}.rs`, exchange `alias.rs`; existing `usersync_catalog`, `host_alias_overlay_bounds`, `config_overlay` targets.

- [ ] Write a truth table distinguishing existing-enabled base, new opt-in base, co-promoted inheriting alias, later alias with explicit false, alias override and baked safety veto. Vary auction eligibility and sync enablement independently.
- [ ] Separate canonical-family eligibility from direct-public-identity routability. Drive the imported alias-only `teqblaze` family and endpoint-bearing `360playvid`: parent requests never dispatch, enabled aliases can dispatch, turning family activation off or safety disable on vetoes all children, and alias-specific disable leaves siblings untouched. The parent does not appear in enabled-only info/sync. Runtime aliases to non-direct bases must reject, not synthesize endpoints. Test Rubicon's approved opt-in mapping and missing regional settings/secret zero-call refusal separately from genuine safety-disabled entries.
- [ ] Key family activation by canonical public base, never shared factory: `appnexus` and `mediafuse` can be enabled/disabled independently. Assert list/detail/all status agrees with direct routing, retaining alias-only parents in unfiltered/base-only views as non-active while endpoint-bearing aliases can be active. Test `magnite` inherited versus `appStockSSP` explicit safety and reject host/request attempts to override endpoint/disposition.
- [ ] Evaluate activation over the compiled catalog, not only `disabled_from_overlay`'s explicitly configured keys. Use one request-snapshot predicate across auction/sync/info.
- [ ] Reject exact and ASCII-folded shadowing of compiled bases and baked aliases in push/load/request aliases. Runtime aliases may target canonical bases only.
- [ ] Retain alias immediate/source parent, canonical base, effective vendor, schema, sync family and requested spelling separately. Source aliases do not inherit an absent GVL ID.
- [ ] Deliberately invert existing shadowing-positive tests, including `exact_alias_override_wins_over_base_lookup` and the corresponding config harness case; do not delete them as obsolete noise.
- [ ] Run `cargo test --locked -p auctioneer-core --test usersync_catalog --test host_alias_overlay_bounds`, core info unit tests, and `cargo test --locked -p auctioneer-exchange --test config_overlay --test run_auction`. Re-enable through a fresh runtime snapshot without registry reconstruction. Commit.

## Task 10: Split Count Owners End To End

**Files:** runtime `aux_work.rs`, `budget.rs`, exports; config `usersync.rs`, manifests, `tests/{usersync_config,build_failure_harness}.rs`; core `usersync/{choose,evict,catalog}.rs`, `tests/{host_alias_overlay_bounds,usersync_routes}.rs`; existing usersync allocator tests.

- [x] Replace the shared MAX_SYNC_NAMES ownership with five independent constants in runtime aux_work.rs: `MAX_CATALOG_IDENTITIES = 512`, `MAX_SYNC_PRIORITY_ENTRIES = 256`, `MAX_SYNC_REQUESTED_NAMES = 256`, `MAX_SYNC_FILTER_NAMES_PER_LIST = 256`, `MAX_SYNC_DEBUG_ROWS = 256`. They are not aliases of one shared constant or of each other; a renamed but still shared owner does not implement this split. Wire each field of `aux_work_limits_from_envelope` to its own semantic constant.
- [x] Independently pin `(catalog_names, priority_entries, requested_names, filter_names_per_list, debug_rows)` to literal `(512,256,256,256,256)`, not those constants. Pin the interleaved MAX_SYNC_PRIORITY_GROUPS/priority_groups separately to literal 32 and retain the 64-entry runtime-overlay cap. Use unequal synthetic values to detect swapped field mappings.
- [x] Rework `usersync_routes::cookie_sync_serializes_all_256_debug_rows_without_truncation` before the compiled roster exceeds the requested-list bound. Its two `256_usize.checked_sub(bidders.len()).unwrap()` sites and `assert_eq!(bidders.len(), 256)` are caller/debug owners, not catalog owners: use `MAX_SYNC_REQUESTED_NAMES`/`MAX_SYNC_DEBUG_ROWS`, retaining an independent literal 256 guard. Bound fixture aliases by both remaining catalog capacity (`512 - complete compiled count`) and the independent overlay cap; a full 512-entry compiled catalog admits zero additional host aliases. Select only a deterministic requested subset of compiled names, leaving requested slots for those admissible aliases, then fill remaining slots with unknown names. Never start the request from every compiled identity or replace these literals with 512. Exercise small, 193-entry and full 512-entry synthetic catalogs with a 256-request/debug-row fixture and an explicit 257-name refusal. Fixture-capacity failures must identify the violated owner, not panic in unchecked subtraction; the real large catalog still reaches the actual handler.
- [x] Replace only `validate_catalog_size`'s checked total/diagnostic with the shared catalog owner. Add runtime normal/build dependencies to config; no new constants crate.
- [x] Exercise both compilation owners: config `build_failure_harness` builds baked metadata/defaults at total 511/512/513, and core `host_alias_overlay_bounds` runs real runtime host-alias validation at those totals using a synthetic compiled catalog. Vary bases, baked aliases and host aliases while keeping other limits valid (e.g. compiled 447/448/449 plus 64 host aliases); assert exact catalog refusals at 513, not an earlier overlay/name failure. The shared config library also tests checked overflow. Removing either owner's invocation must fail its own test.
- [x] Split shared validation and materialization limits. Preserve each filter, requested/priority/debug owner and 32 groups. Retain 64 per overlay, while charging/counting the merged host-default plus overlay map.
- [x] Test 255/256/257 caller entries independently from 511/512/513 catalog identities, overflow, a 512 catalog with output limit one, separate debug insertion and both filter lists.
- [x] Widen each of the four caller constants to 512 independently. Each mutation must fail **both** its independently fixed 257-entry behavioral refusal test **and** the literal shipped-tuple/profile test; run the two test owners separately to prove both fail. The other caller owners' 257-entry refusal tests must still pass, detecting shared aliases/cross-wiring. Term-removal and profile-byte checks alone do not discharge either requirement. Separately remove each accounting summand and require its owner guard to fail. Prove catalog-only growth adds 65,536 chooser bytes under unchanged coefficients.
- [x] Run `cargo test --locked -p auctioneer-runtime --lib`, `cargo test --locked -p auctioneer-config --test usersync_config --test build_failure_harness`, `cargo test --locked -p auctioneer-core --test host_alias_overlay_bounds`, and core usersync integration/allocator targets as separate invocations. The build harness must report its catalog-boundary cases executed; reuse its existing bounded child-build driver. Commit only after both build/runtime owners and both anti-widening controls pass.

Implemented and verified on 2026-10-19. The [count/storage record](../../testing/evidence/catalog-count-controls-20261019.md)
records both build-owner removal probes, runtime error-propagation removal,
four independent widening controls and six accounting-term removals. The
full-size allocator control exposed duplicated request-local strings/maps;
borrowed catalog/candidate views and indexed cookie-family projection repair
those peaks under the original coefficients. Its explicit registry fixture
is independent of the shipped roster. This completes the count-owner slice,
not Task 11's shared info/auction indexes, schema qualification, strict live
registry cutover or resource qualification readiness. No bidder is promoted by this increment.

## Task 11: Indexed Info And Sync Views

**Files:** core `handlers/info_bidders.rs`, `usersync/{catalog,choose}.rs`; privacy `sync/{mod,permission}.rs`; new core `tests/bidder_catalog_views.rs`.

Info-path repair (2026-10-30), not full Task 11 acceptance: core now retains one
ordered metadata vector beside the shared registry and reuses it for info alias
validation/list/detail/all. Exact binary-search instrumentation covers 512/0,
448/64 and 384/128 populations, including independently merged inherited/runtime
aliases. Per-probe, aggregate and ordered-render counts have independent bounds;
late/absent keys first failed against the original scans. Ordered disabled-name
cursors include the fully disabled 512-row boundary. Owning allocation controls
fit projection/encoding under the unchanged one-MiB auxiliary manifest charge,
with a positive allocator control and complete output counts. Alias shadowing,
activation and wire-byte regressions remain intact. Cross-path resolved-entry
reuse, the complete auxiliary response/resource proof and remaining sync/auction
integration below are still open; these checkboxes are not closed by the info
slice alone.

- [ ] Use immutable ordered indexes and reuse resolved alias entries across config, auction, sync and info; do not merely replace a linear search with an unmeasured map call.
- [ ] Instrument actual base probes in all three aggregate rows: compiled/host counts 512/0 (zero host-alias probes), 448/64 (at most 1,152 comparisons) and 384/128 (at most 2,304, with 64 inherited plus 64 distinct runtime aliases). Each individual base-index probe also obeys `ceil(log2(N+1))`, at most two probes per alias; rendering separately stays within `4*(N+H)`, at most 2,048. Assert actual counts/row membership, not just the loosest aggregate bound.
- [ ] Add late-key and absent-key controls against the actual index path, separately from early-key positives, through config resolution, auction, sync and info reuse. Substitute a per-alias full-catalog scan: it must fail the per-probe bound/control even if a small aggregate fixture remains below 1,152. Assert zero probes when no host aliases exist and keep the 64-entry runtime-overlay limit unchanged.
- [ ] Require base-only list/detail/all behavior from provenance, not factory membership. Keep disabled identities visible where configured; info activation and sync-only gates remain independent.
- [ ] Pass effective vendor ID explicitly to sync permission lookup. Test opposite alias/base consent in both directions, unknown vendor, shared-family suppression and no permission transfer through a sibling.
- [x] Filter disabled cooperative names before shuffle/dedup, preserve explicit-request family-attempt semantics, and add the intentional difference to audited sync fixtures. Do not change setuid's host-only ladder or callback UID trust.
- [ ] Stream borrowed full-catalog info views into a separately capped response; do not increase cookie-sync's 256-KiB output cap to accommodate info. Measure introduced scratch against the predeclared info allowance.
- [ ] Run `cargo test --locked -p auctioneer-core --test bidder_catalog_views --test usersync_catalog --test usersync_choose`, then `cargo test --locked -p auctioneer-privacy --test sync_permissions` and privacy library tests as separate invocations. Commit.

Automatic sync-selection increment (2026-10-23), not complete Task 11:

Known disabled identities are removed from both cooperative priority groups
and the catalog fallback before each shuffle and before name/family attempts.
The fallback applies this rule with an empty requested list even when
cooperative mode is false. Eligibility borrows the catalog's already resolved
sync bit, including baked base/syncer and runtime base/alias vetoes; it performs
no privacy preflight or string copying. Unknown priority names retain their
existing ordering/refusal behavior. Catalog lookup, configured-family/eviction
views, explicit requests and their denial precedence remain unchanged.

The existing raw `candidate_order` reference control delegates to the same
ordering implementation without the catalog-specific predicate. Original
external captures remain untouched. A separate eighth core design-divergence
case drives three actual catalog/chooser scenarios, including explicit disabled
first-attempt suppression. Privacy's seven design rows are unchanged.

The names/group vectors retain their original unfiltered capacity upper bounds;
filtered iterators cannot introduce a second collection or geometric growth.
No workload limit, funding coefficient, resource descriptor or serving-registry
membership changes. Five colocated unit controls vary the two runtime flags,
group order, mixed-case names, requested/cooperative/debug modes and each baked
base veto independently. Removing either automatic filter independently fails
the RNG-count control, distinguishing pre-shuffle filtering from late rejection.
The schema gate, full registry cutover, indexed catalog integration and resource qualification
readiness remain open.

Verification on Rust 1.98.1, native macOS host, locked/offline dependencies:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-core --lib --test usersync_choose --test usersync_catalog --test usersync_reference --test usersync_wire_reference --test usersync_handler_alloc --test usersync_routes --test usersync_config_lifecycle --test usersync_alias_limit --test usersync_flow --test usersync_precedence --quiet
```

- The command above passes 653 tests: 509 library plus 144 integration tests,
  zero failed/ignored. Repeating it with
  `--features insecure-bidders,conformance-suite` also passes 653. These are
  separate package runs, not workspace feature-unification evidence.
- `CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-privacy --lib --test sync_permissions --quiet`: 286 library and 23 permission tests pass.
- `CARGO_INCREMENTAL=0 cargo clippy --offline --locked -j2 --workspace --all-targets --features insecure-bidders,conformance-suite -- -D warnings`: passes. The initial lint run rejected two shadowed test bindings; renaming them added no lint exception. The changed `usersync_choose` target was subsequently rerun in both feature lanes, 11 tests each, all passing.
- `CARGO_INCREMENTAL=0 cargo check --offline --locked -j2 -p auctioneer-core --lib --features insecure-bidders,conformance-suite --target wasm32-unknown-unknown --target wasm32-wasip1 --target wasm32-wasip2`: passes. This is guest-library compilation, not guest execution or target/resource qualification.
- `CARGO_INCREMENTAL=0 cargo run --offline --locked -j2 -p auctioneer-cli --no-default-features -- reference check --root .`: verifies all ten frozen captures without writing files.
- `cargo fmt --all -- --check`, both raw-JSON fixture scripts and `git diff --check` pass. The raw inventory remains 168 fixtures across 50 files.
- The initial regression run failed the family-selection and RNG-count tests as intended. Independently removing the priority filter reports three rather than two draws; removing only the fallback filter reports two rather than one. Both mutations were restored before final verification. The explicit-request preservation controls pass unchanged.
- Separate read-only spec-compliance and code-quality reviews approved this slice with no remaining findings. Existing GVL deletion diagnostics, dependency future-incompatibility notice and fixture boot-over-ceiling warnings remain; none was suppressed or claimed repaired. Full workspace test execution, hosted CI, provider execution and full bidder catalog acceptance were not performed by this increment.

## Task 12: Enforce Parameters At The Funded Survivor Boundary

**Files:** bidder `schema/mod.rs`; exchange `parameter_validation.rs`, `run/mod.rs`, `context.rs`; core auction/context wiring; new exchange `tests/bidder_parameter_validation.rs`.

- [ ] Require Task 3's complete `S` qualification first, not universal qualification of unshipped source schemas. Create production `build_schema.rs` to discover shipped schemas/bindings and emit validator declarations/dispatch through `schema/mod.rs`. Compare the emitted/linked set with the frozen generated cohort; no source-only row becomes callable. Reuse qualified generation, inherited entry points and lazy-cache ownership; no handwritten bidder-name dispatch or runtime schema compiler. Disabled shipped definitions remain resident. Track schema add/edit/rename/delete and alias overrides and prove actual generated behavior changes.
- [ ] Integrate after participation/privacy/sampling/cap selection, final supplement/FPD merge and recorded privacy scrub, before any request body or secret lookup. Preserve existing fatal code-15 FPD identity/typed-validation precedence.
- [ ] Measure the borrowed final parameter subtree without another full-auction clone. Check original absolute deadline; precharge single/aggregate schema work and scratch before calling the boolean validation path.
- [ ] Retain valid sibling impressions/bidders and record bounded fixed code-2 failures. Structural/work/allocation exhaustion yields code 5 without starting an unfunded evaluation; deadline expiry retains the existing tmax path.
- [ ] Test direct/base/baked/runtime aliases, merged parameters, a failing expensive input, exhausted pre-call budget, independent single/aggregate/scratch limits and a denied bidder whose validator is never invoked.
- [ ] Run `cargo test --locked -p auctioneer-exchange --test bidder_parameter_validation --test privacy_policy_composition --test run_auction`; repeat the new target with insecure feature. Commit with its runtime budget wiring, not an unfunded later task.

## Task 13: Typed Host Settings And Lazy Coupled Secrets

**Files:** bidder `host_context.rs`, `request_info.rs`, trait descriptors; core `bidder_context.rs`, config DTO/resolution/auction wiring; exchange `context.rs` and request-construction loop; new core `tests/bidder_host_context.rs`.

- [x] Preserve the original root-extension shape during shared bounded ingress,
      before typed null normalization or bidder-specific transformations. Keep
      typed-only provenance explicitly unavailable and fund the fixed marker even
      when no calls survive. This shared APS prerequisite does not complete typed
      settings, secrets, APS request construction or its bidder-local null guard.
      See the [ownership and control record](../../testing/evidence/request-extension-provenance-20261023.md).

- [x] Add the approved generated non-secret settings foundation: adjacent optional integer/string declarations, strict generated object decoders, opaque redacting holder, named borrowed field-wise views, independent resolved-base binding checks before construction, and move/prune of the original extracted map. MSFT/TTD declarations do not promote definitions or complete their protocol ports. The generated typed holder supersedes the earlier optional `Any` suggestion; no downcast or second settings parser is needed.
- [x] Complete the priority protocols' additional closed `optional_secret_reference` kind and bounded credential descriptor, following the [verified service checkpoint](2026-10-26-priority-bidder-services.md#verified-service-checkpoint-2026-10-27). Keep closed kinds rather than inventing a general schema DSL.
- [x] Validate non-secret settings and secret references through existing push/load validation, retain them in the captured snapshot, and reject unknown/oversized inputs. Runtime data cannot replace endpoints, capabilities, bindings or vendor identity.
- [x] Add a provider-neutral lazy host-context source to `AuctionContext` using the existing futures dependency. Core implements it with captured request-bound EdgeZero handles, not new provider clients.
- [ ] Invoke it only for a privacy/activation/parameter-valid survivor, after an original-deadline check. Use `BoundSecretStore::get_bytes_bounded` exactly as the existing usersync key loader does; pass the absolute deadline and both byte caps, and recheck afterward.
- [ ] Read coupled credentials as one versioned bundle per admitted requested identity after resolving its implementation-bound alias/base settings, following the [approved priority service contract](2026-10-26-priority-bidder-services.md). Distinct aliases may name distinct references; do not cache by implementation. Share a redacting view across that identity's construction calls, then drop it before the next identity. Prepay the bounded sequential working bank once per auction before reads/decoding, not once per catalog entry or call.
- [ ] Test no polling for denied/empty-valid-impression bidders, missing/oversized/malformed bundles, pending-read deadline, coherent rotation, distinct alias/base binding, snapshot replacement and no secret in Debug/errors/info/debug JSON.
- [ ] Run `cargo test --locked -p auctioneer-core --test bidder_host_context` and core/exchange libraries. Missing required credentials are bidder-local code-5 refusals with zero calls, not an invalid registry or empty whole auction. Commit.

## Task 14: Correlation And Nonfatal Construction Diagnostics

**Files:** bidder `meta.rs`, `sink.rs`, `bidder.rs`, `request_info.rs`, `finalize.rs`; exchange `run/mod.rs`; runtime accounting; new exchange `tests/bidder_call_correlation.rs`.

Diagnostic implementation contract (predeclared 2026-10-23, before allocation
measurement): retain at most 32 payload-free enum rows in a fixed inline buffer.
The 33rd replaces the last row with a fixed suppression row; the wire never
grows beyond 32. Code 2 and messages of at most 64 bytes contain no request
values. A sparse observable-bidder map outlives construction, including fatal
rollback and deadline exits. Response assembly borrows these rows and existing
errors instead of copying their strings into another error vector.

The new context owner is `K_diag(B) = 4096 + B * 98304` bytes for a nonempty
participating population (zero when B is zero). B counts post-cap bidder names,
including aliases and zero-call bidders, not accepted calls. The per-bidder
reserve covers four conservative inline/map envelopes (17,408 bytes), 32
two-field JSON objects and their keys/messages (66,667 bytes at the existing
384-byte JSON-node envelope), and 4,352 bytes of sequence/name/map overhead,
rounded up from 88,427 to 98,304. The fixed reserve covers the sidecar and
serializer roots. Allocation controls must independently bound actual owner
construction and final JSON assembly across bidder/row counts and maximum
names; this declaration is not a measurement or schema qualification.

Adapters bind this scalar to `CatalogResourceCosts.context_per_auction_bytes`
in boot and inspection together. C/I/V remain unchanged. The auction checks
diagnostics plus request projection against one held Projection receipt before
construction; neither call count nor response/decode/debug funding substitutes
for it. Workload limits, ceilings and the shared additive equation do not change.

Implementation API: metadata owns an optional binary call header of at most
32 bytes and checked eight-byte rows indexed only within that call's ordered
covered-impression list. Each row has a typed media hint and a `u32` protocol
token, never a request subtree. Repeated rows, duplicate coverage, invalid
media codes, an absent ordinal or a second/oversize header return fixed typed
construction failures before body serialization. The same immutable metadata
Arc already carried by the transport owns the context. The existing
`outbound_meta_bytes` scalar funds eight bytes per maximum covered impression
plus 64 bytes per call for the context/header/row-vector overhead; provider
profiles must consume that changed scalar in the same commit. This does not
change workload limits, byte-pool capacities or the additive isolate equation.

- [x] Add a bounded typed correlation header and rows indexed by covered-impression ordinal: media hint plus a small protocol token, never retained request JSON/body. At most 8 bytes/row plus 64 bytes/call; assert actual layouts on all targets and account the maximum installed impression/call populations.
- [x] Extend the sink metadata transaction to write correlation after reservation and before acceptance. Duplicate/out-of-range coverage, oversize context or invalid media refuses only that call and rolls its reservation back.
- [x] Preserve the same immutable Arc-owned metadata through finalization, wave slots and make_bids. No lookup by impression ID alone and no re-reading the sent body.
- [x] Add a separately bounded nonfatal diagnostic channel. Semantic Err still discards accepted calls for bidder-fatal errors; partial diagnostics coexist with accepted siblings and consume their own funded retention. Zero-call aliases, both post-construction deadline exits and 32 diagnostic rows alongside 16 sink errors reach final JSON. Exact/one-short/wrong-phase and separately-fitting-but-not-combined Projection receipts guard prepayment before adapter invocation.
- [x] Test one impression split across two media-specific calls, missing response mtype, reversed completion, one failed call, exact/one-over correlation and error caps, and the old fatal rollback path.
- [x] Run `cargo test --locked -p auctioneer-exchange --test bidder_call_correlation` and bidder sink/meta/finalization tests. Commit code and updated construction/profile charges together. The diagnostic record lists the final secure/feature, profile, CLI, native export, guest-compilation and lint invocations; no guest execution or readiness claim is implied.

Correlation checkpoint (2026-10-23): the shared metadata transaction,
finalization and actual auction driver now retain call-local context. The
[control record](../../testing/evidence/call-correlation-controls-20261023.md)
separates the funded metadata increment from still-open nonfatal diagnostics,
production bidder integration and target readiness. The combined task remains
open until the separately charged diagnostic channel and its acceptance
controls are implemented. Historical resource captures remain unchanged.
The subsequent [diagnostic control record](../../testing/evidence/nonfatal-construction-diagnostics-20261023.md)
records that channel's ownership, funding and acceptance controls; it does not
close protocol migration, generated-schema qualification or physical fit.

## Task 15: Qualified Proprietary Response Decoding

**Files:** bidder `response_decode.rs`, `types.rs`, `bidder.rs`; runtime bounded JSON helpers only if required; exchange result/provenance tests; new bidder `tests/response_envelopes.rs`.

The initial standard-wrapper receipt is implemented: its first parse attempt
claims one response-owned byte atomically, before parsing or budget refusal.
Failed attempts remain spent; moving the response through `within_node_budget`
cannot renew it. The byte must fit the former native/guest inline layout by
unconditional compiler assertions. The 14 synthetic controls cover retained and
dropped trees, failed syntax/types/nodes, budget replacement and simultaneous
borrowers, while preserving the first-attempt parser and transport behavior.
This is only the one-shot foundation. The complete shared source/candidate
ledger, qualified DTOs, candidate-local typed isolation and their funding remain
pending, so the session/aggregate requirements below stay unchecked.

- [ ] Keep bodies sealed. Restrict generic envelope decoding to crate-owned qualified DTO descriptors with reviewed retained/transient bounds; do not offer public raw bytes or arbitrary unqualified Deserialize targets.
- [ ] Introduce a response-owned monotonic decode receipt shared by every session and by the standard parse wrapper. Attaching exchange limits happens before decoding and cannot renew a spent receipt. A second session does not receive a fresh budget.
- [ ] Permit at most two funded structural/typed passes across one delivered body: source structural budget N, aggregate candidate typed visits N, checked byte/work and cumulative typed-allocation allowances. Candidate subtrees consume the common receipt; repeated full parses cannot retain uncharged trees.
- [ ] Keep existing standard-parser behavior. New proprietary envelopes use an explicit 64-container guard, tested at 63/64/65. Map structural/depth/raw-value refusals to fixed code 4, syntax/typed failures to fixed code 14, both with ResponseParse provenance.
- [ ] Exercise whole-envelope typed failure, candidate-local typed failure and post-decode semantic/classification failure separately, across multiple seats/candidates with a valid sibling. Assert errors plus bids, not just Result success.
- [ ] Record per-DTO allocation qualification against installed response/construction charges. An envelope exceeding current bounds or a required non-JSON protocol needs its own reviewed parser extension before that wave; never expose raw body as a workaround.
- [ ] Run `cargo test --locked -p auctioneer-bidder --test response_envelopes`, existing response parse-factor/provenance tests and exchange debug tests. Verify request and response debug omission for asymmetric protocols independently. Commit with any changed accounting.

## Task 16: Evidence-Derived Migration Checking

**Files:** CLI `src/bidders/{check,evidence}.rs`, `tests/bidders_evidence.rs`; ledger, wave/check report schemas and source-owned fixture drivers.

Implemented execution prerequisite (2026-10-28): the reviewed
[native-control sharing plan](2026-10-28-native-cli-control-sharing.md) moves
the existing bounded process/I/O primitives into the CLI library for reuse,
including their source-hash and test-ownership guards. Baseline supervisor/I/O
controls pass 14/17; the complete CLI run passes 848 tests, and its shared
library passes 35 without application features. This does not implement
evidence commands or certify a foundation producer. Exact reviewed
operation/count requirements, promoted
wave contracts and unavailable schema/registry/decoder acceptance remain
explicitly incomplete; no successful default can replace them.

Run-ownership checkpoint (2026-10-28): the
[execution plan](2026-10-28-bidder-evidence-run-scope.md) and
[command guide](../../bidders/evidence.md) now cover actual `evidence begin`,
`produce`, `run` and consuming `check` paths. One fixed reference-integrity
operation performs fresh inspection and complete inventory comparison. The
13-test CLI target includes same-input prior-run replay, consistently repinned
binding changes, forged completion over source drift, exclusive publication,
strict records and relative-path handling. Successful preparation still yields
acceptance exit one and migrated zero, with code-owned missing requirements.
It is not a passing foundation producer, wave acceptance or full Task 16;
behavioral dispatch counts and unavailable acceptance drivers remain open.
No completion box below is checked by this preparation-only increment.

- [ ] Implement a fixed Rust evidence driver with exact operation sets, row shapes and positive/negative dispatch counts. Ledger identifiers choose supported drivers, never arbitrary commands.
- [ ] Implement the explicit producer/consumer boundary: `bidders evidence run [--wave MANIFEST] --output PATH --run-id ID` executes fixed required producers and writes run-scoped receipts; `bidders check --evidence-root PATH --run-id ID` consumes them. The driver creates a fresh exclusive output directory/manifest; an existing output or producer receipt refuses rather than reusing success. Derive the required producer set from the reviewed wave/full-catalog requirements, never from whichever receipts happen to exist. CI's expected identity is its independently supplied workflow run/attempt/commit; a local controller supplies a fresh identity/output directory before starting the driver. The checker must not learn its expected ID from the report it is checking. An inventory-only normal check without current evidence may exit 0 for honest unfinished rows, never certify migrated.
- [ ] Support distributed CI without weakening the local contract: `bidders evidence begin [--wave MANIFEST] --output PATH --run-id ID` exclusively initializes the shared manifest; `bidders evidence produce --producer ID --run-manifest PATH --output PATH --run-id ID` executes one finite reviewed producer and exclusively writes its own receipt/artifacts. It validates the manifest's run/inputs before work and cannot overwrite another producer. Local `evidence run` is the same begin/produce sequence over the whole required set, not an alternate checker. CI jobs exchange only these run-scoped artifacts; the final checker applies the same exact producer-set/refusal rules.
- [ ] Bind receipts to run/producer identity, source/schema/fixture/code/shared-dependency and effective activation/routing-policy digests, target/features and lockfile. Write success only after the actual producer completes its required operations; include bounded exact positive/negative counts and refusal status. Failed, missing, skipped, stale, foreign or unknown required evidence cannot produce migrated. No signing service or arbitrary ledger command execution is needed.
- [ ] Derive normal-run requirements from foundation controls/current shipped subset and add selected-flow requirements for a reviewed wave. `--require-complete` independently demands every required source identity/branch, regardless of a supplied run manifest's subset. Test foundation acceptance with honest unfinished inventory and refusal of that exact same shortened producer set as full completion; do not require future bidder implementations merely to pass Task 17.
- [ ] Mutate a working family to no-work stub, forge a migrated label, omit a branch/operation, change shared code/promotion policy and replay stale/failed evidence. Add `same_input_previous_run_cannot_replace_skipped_producer`: unchanged source/schema/code/lock hashes, a previous successful run receipt, but the current required producer skipped. It must refuse with exact missing/wrong-run status. Independently test unknown/failed/missing producers, wrong attempt/commit and a failed producer accompanied by an old success. Drive every control through the actual CLI, not only manifest integrity.
- [ ] Implement normal, `--wave MANIFEST` and `--require-complete` checking now, before bidder waves depend on them. Normal checking accepts honest incomplete inventory with a valid shipped subset. The full-completion command exits 1 with exact missing/unfinished sets while any required source row/branch lacks current passing evidence; Task 20's remaining catalog closure-only terminal gate must first satisfy it and Task 21 repeats it. bidder catalog/bidder expansion closure does not require full-catalog success. The checker knows evidence states, not plan task numbers. Synthetic complete fixtures prove checker semantics, not actual migration.
- [ ] A wave manifest declares required identities and source-owned operation/branch evidence. Its selected-display checkpoint may accept explicit in_progress rows with all selected-flow evidence current; the report must enumerate uncompleted source-local branches and cannot mark those identities migrated. Unknown/missing selected requirements refuse the checkpoint.
- [ ] Require current native fit/headroom and catalog-relative gates. Retain resource qualification's WASM absolute blockers and pending observations in output; they cannot be reported as qualified by completion checking.
- [ ] Run `cargo test --locked -p auctioneer-cli --test bidders_evidence`, including the actual CLI's incomplete, selected-flow and full-completion modes. Review and commit the checker before Task 17; do not publish a catalog-complete report from foundation fixtures.

## Task 17: Foundation CI And Acceptance

**Files:** `test.yml`, `format.yml`, three vendor workflows and their colocated scripts; `docs/bidders/foundation-verification.md`, architecture/registration docs.

- [ ] Verify and retain the pinned `vendor/bidder-reference` checkout/`AUCTIONEER__BIDDER__SOURCE_DIR` installed in Task 1 and native-job WASM-target/offline-prefetch prerequisites installed in Task 3. Do not introduce them for the first time here or change the separate OpenRTB round-trip pin.
- [ ] Run new CLI source/import/check tests and zero-incomplete-shipping checks. Protect exact pins, commands, features, trigger/step execution, source checkout identity and all new config targets' secure-default coverage.
- [ ] Run the following commands as separate invocations. Do not use workspace feature unification as evidence of a secure-default package run.

```sh
cargo test --locked -p auctioneer-config --all-targets
cargo test --locked -p auctioneer-bidder --all-targets
cargo test --locked -p auctioneer-exchange --all-targets
cargo test --locked -p auctioneer-core --all-targets
cargo test --locked -p auctioneer-cli --all-targets
cargo test --locked -p auctioneer-adapter-axum --all-targets
cargo test --locked --workspace --all-targets --features insecure-bidders --exclude auctioneer-config
cargo test --locked -p auctioneer-config --all-targets --features insecure-bidders
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] Preserve existing conformance/override/secure-default CI lanes, raw-fixture gates and explicit ignored-test drivers. The config build-failure harness needs its bounded shared child-build strategy; do not start duplicate unconstrained sweeps.
- [ ] Wire all required behavior/build/resource producers through the fixed evidence driver with the current workflow run/attempt/commit identity. Initialize the run once, then add a final checker job depending on every required producer, with explicit failure/missing-status collection even on upstream failure; a skipped job cannot inherit an old passing receipt. Bring vendor package producers into that dependency graph through reusable workflow calls while preserving their existing branch/PR triggers; do not pretend `needs` can reference a job in an unrelated workflow run. Include a same-input previous-run artifact substitution control; matching source hashes alone must not pass. Set `AUCTIONEER__BIDDER__EVIDENCE_ROOT` and `AUCTIONEER__BIDDER__RUN_ID` from this invocation, never a cached report.
- [ ] Execute actual adapter/shared WASM checks with conformance, not native cfg substitutes:

```sh
cargo clippy --locked -p auctioneer-adapter-cloudflare -p auctioneer-exchange -p auctioneer-privacy --all-targets --features conformance-suite --target wasm32-unknown-unknown -- -D warnings
cargo clippy --locked -p auctioneer-adapter-fastly -p auctioneer-exchange -p auctioneer-privacy --all-targets --features conformance-suite --target wasm32-wasip1 -- -D warnings
cargo clippy --locked -p auctioneer-adapter-spin -p auctioneer-exchange -p auctioneer-privacy --all-targets --features conformance-suite --target wasm32-wasip2 -- -D warnings
```

- [ ] Run the actual vendor package/size workflows on all branches, retain existing ceilings and record source/toolchain/features/package identity. Do not claim package success from clippy.
- [ ] Repeat Task 4's pure runtime/allocator/profile and isolated `boot_strict_parity` checks, CLI zero-cap controls and Task 3's complete `S`-scoped linked artifact/observation acceptance against implemented validators, registry, context and info paths. Retain existing all-source correctness/compatibility regressions separately; do not require speculative unshipped-backend proofs. `catalog_resource_profile` stays strictly non-installing: supply actual generated descriptors as data and execute initialization/context/info only in their owning integration/process targets. Boot installation stays in its fresh-child single-test contract. Exercise 512-identity synthetic catalogs, sharing and failed initialization; publish four matched reports with independent C/I/V/K/A controls and actual code/data sizes. Unavailable physical meters are not zero observations.
- [ ] Record exact test dispatch counts, resource descriptors, relative versus absolute status and unavailable meters. Full-catalog synthetic fixtures prove limit behavior; a partial shipped catalog is not 358 migrated identities. Review and commit foundation acceptance before Task 18.

## Task 18: Review And Implement Priority-Wave Contracts

**Files:** `crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml`; new source-specific plans under `docs/superpowers/plans/`; adjacent implementations/declarations/fixtures and promoted metadata/schemas.

**Checkpoint 18A: Reviewed wave contract, no bidder implementation.**

AG approved the current requirement/membership inputs covering the 14 priority
identities on 2026-10-30. The
[approval record](../../../crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority-approval.toml)
binds the exact primary and supplemental input bytes without rewriting their
frozen planning metadata. It grants no promotion, resource qualification or
onboarding acceptance. Executable review-record validation and all remaining
Checkpoint 18A implementation/evidence obligations remain required; this human
approval does not make every box below complete. The supplemental file records
APS membership, not invented operation counts or completed protocol evidence.

- [ ] Review the 14-identity priority set: msft, ttd, rise, sovrn, yahooAds, unruly, sharethrough, openx, kargo, rubicon, pubmatic, triplelift, criteo, aps. The existing 13-identity wave covers only source-backed names. Map ttd to thetradedesk; preserve yahooAds spelling and yahoo_ads internal key.
- [ ] Obtain and review APS's independent integration contract and provenance: protocol/parameter schema, supported display flow, endpoint/settings/authentication, privacy/vendor metadata, response/correlation and sync disposition. APS is absent from the pinned source; do not forge a source binding or reuse another bidder's evidence. Define its exact positive/negative producers before implementation and include its evidence in the same priority acceptance. Keep the 358-row source inventory unchanged.
- [ ] Before code, review each source family's branches, parameters, settings/authentication, endpoints, grouping, media, statuses, partial errors, correlation, response type, sync and shared dependencies. Specify the exact per-family files/test drivers in that wave plan.
- [ ] Select HTTPS for the pinned `msft`, `openx` and `sovrn` destinations under AG's 2026-10-28 decision, without separate vendor TLS-mapping evidence. Implement/bind effective metadata and source URL/blob/transport-difference records, preserving authority/port/path/query and bidding protocol, then run deterministic HTTPS/no-unapproved-HTTP route controls. Selection alone, an unavailable row or a fabricated live claim cannot satisfy the priority-flow checkpoint. Rubicon's regional endpoint/settings/credential policy remains bidder catalog work.
- [ ] Allocate the other 74 source-enabled HTTP bases to transport/provenance reconciliation in Task 20 with their owning bidder expansion or remaining catalog closure wave; full reconciliation remains remaining catalog closure work. The priority three are not the whole source HTTP population. Carry Task 6's earlier AppNexus metadata-binding/route-test prerequisite separately from its later protocol audit, and reconcile source-disabled HTTP and alias endpoints as distinct populations.
- [ ] Obtain and record review of the source-specific branch manifest, operation counts and required test-driver set; freeze its identity in the wave evidence contract before implementing bidders. This checkpoint is design acceptance only and cannot mark a flow usable or a bidder migrated. A later implementation that needs fewer/different requirements must amend and re-review the contract, not redefine its own completion gate silently.

Priority-contract preparation (2026-10-22), not Checkpoint 18A acceptance:
the [wave design](../../../crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml) records the exact 13
source-backed priority identities, canonical schemas, implementation keys, sync provenance,
38 pinned source-owner files, 113 requirements and 16 planned Rust driver
labels. The [source-specific plan](2026-10-22-priority-display-bidders.md)
maps those owners to adjacent unit tests and cross-package controls. These
are requirements, not passing fixtures or producer receipts. No foreign
program runs and no production registry, schema or endpoint changes.

The bounded `bidders check --wave PATH` preparation reports only
`wave_design_only`, pending review, zero promoted and zero migrated, and
`--require-complete` still exits 1. It reconciles metadata/schema/sync,
canonical/implementation ownership, exact owner-file membership and routing
review classifications against the pinned source. The current factory
binding capture is still pending review; this comparison is not protocol
or human approval. Strict shape/count/source refusals and CI execution
controls own this checker. The design digest is not an execution receipt.
AG approved HTTPS-default selection on 2026-10-28; serving metadata binding,
deterministic route tests, Rubicon policy/authentication, outstanding intentional
differences, fixed per-producer fixture counts and actual reviewed freeze
remain open, so all Checkpoint 18A completion boxes remain unchecked. The
transport-selection implementation now uses `routing=https_default_pending`
for the three source HTTP destinations. The authored pending design was
deliberately updated under AG's transport decision; original source captures
and the 13-row/113-operation requirements are unchanged. The new label is not
producer evidence, completed serving metadata binding or promotion.
The additional priority APS integration is not in this source-backed artifact;
its pending contract/evidence must also pass before the 14-flow checkpoint
can complete. Do not change captured source counts or declare APS implemented
merely by adding it to the priority set.

**Checkpoint 18B: Implement the approved contract and produce evidence.**

Kargo protocol-candidate preparation (2026-10-23) is scoped separately by the
[candidate plan](2026-10-23-kargo-protocol-candidate.md). Generated discovery
includes its real factory, but adjacent public definitions remain empty. Five
original captures and inline units cover its bounded passthrough, statuses and
candidate-local media classification. AG approved its single-key/ambiguity
policy only; the pending wave freeze, inherited null/absence parity gap,
schema/resource qualification, metadata promotion and composed delivery remain
open. This increment does not check off either wave checkpoint.

Triplelift protocol-candidate preparation (2026-10-23) follows its separate
[candidate plan](2026-10-23-triplelift-protocol-candidate.md). Borrowed filtered
impressions use the same ordered selection for metadata and body, with bounded
diagnostics and actual sent-ID response checks. Twelve verbatim captures retain
ten end-to-end cases and two shared-boundary controls. AG separately approved
single case-insensitive parameter/media keys and affected-object ambiguity
rejection. Source/null parity, resource qualification, promotion, gzip/sync and
composed delivery remain open; neither checkpoint is completed by this candidate.

- [ ] Write external-reference-derived positive/negative request and response cases before implementations. Use serde_json::json! in local fixtures unless exact malformed/duplicate lexical bytes are the subject.
- [ ] Port each family in a separate reviewed commit or small coherent family PR using the proven sink/schema/context/decoder paths. Existing pubmatic is audited, not auto-complete; a common endpoint is not proof of reuse.
- [ ] For msft, pin 10/11/21-impression chunking, consistent/conflicting members, impression transformation and reviewed HB-source defaults/overrides. Add ttd alias/base activation/vendor/schema cases and rubicon's coherent credential rotation.
- [ ] For every family, run its source-derived branch driver, serialized privacy/context tests, mixed good/bad bids, budget/deadline/call-cap controls and all four shipped application builds.
- [ ] Recompute migration state from current evidence. The publisher display checkpoint requires all 14 reviewed display flows, including APS, usable and configured; rows with unfinished source-local branches remain in_progress rather than being relabeled migrated. Such a partial checkpoint is not bidder catalog closure: all mandatory priority-local branches remain required at Task 19. The source-only wave command below is necessary but not sufficient: require APS's reviewed contract/evidence too.
- [ ] Run the fixed evidence driver and consuming checker below in a fresh run/output scope. `AUCTIONEER__BIDDER__RUN_ID` and `AUCTIONEER__BIDDER__EVIDENCE_ROOT` identify this invocation as established in Task 16/17; never reuse a previous run's directory or infer the expected ID from its receipts. Record missing credentials/live evidence separately; no fabricated certification. Review before promoting each runtime definition.

```sh
cargo run --locked -p auctioneer-cli -- bidders evidence run --wave crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml --output "$AUCTIONEER__BIDDER__EVIDENCE_ROOT" --run-id "$AUCTIONEER__BIDDER__RUN_ID"
cargo run --locked -p auctioneer-cli -- bidders check --wave crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml --evidence-root "$AUCTIONEER__BIDDER__EVIDENCE_ROOT" --run-id "$AUCTIONEER__BIDDER__RUN_ID"
```

## Task 19: Priority Publisher Handoff Without Premature Qualification

**Files:** new `crates/auctioneer-core/tests/bidder_onboarding.rs`, reuse `tests/support/usersync_app.rs` where applicable; `docs/bidders/{priority-onboarding.md,priority-example.toml,priority-capabilities.toml}`, the priority wave manifest and Trusted Server embedding/publisher pilot handoff references.

- [ ] Write failing integration controls in `bidder_onboarding`: `selected_display_bidders_complete_through_generated_app`, `runtime_activation_changes_on_fresh_snapshots`, `invalid_params_and_missing_secrets_preserve_valid_siblings`, and `selected_sync_stays_signed_and_browser_bounded`. Use actual generated-App request/response dispatch, real promoted implementations and source-derived parameters/responses, with test-only fanout/config/secret fixtures; do not add a production endpoint-rewrite option. This is deterministic in-process handler acceptance, not a claim of real bidder or provider networking.
- [ ] Require the selected-flow table's identity set to equal the union of the 13 names in the reviewed source-backed priority manifest and `aps` from its separately reviewed contract. Drive all 14 selected identities to nonempty expected calls/bids, inspect the actual post-scrub serialized outbound request, and exercise multiple implementations together. The permission deny rows must preserve an allowed sibling; code-2 and code-5 cases assert exact bidder attribution and zero refused-bidder calls.
- [ ] Prove runtime enable/disable/re-enable on fresh snapshots without rebuild or registry reconstruction, while baked safety flags and alias-base vetoes remain effective. Assert call-identity sets and unchanged registry identity; independently mutate activation and veto guards and require their named controls to fail.
- [ ] Drive actual sync handlers using selected identities with declared sync support and the bounded test secret store; require a verifiable signed cookie within the existing 4-KiB complete-cookie limit and unchanged response caps. Keep source-without-sync states explicit rather than inventing sync endpoints. Do not claim ownership of callback UIDs or widen cookies for the catalog.
- [ ] Document exact activation/settings/secret references in priority-example.toml and remaining per-identity shared-feature restrictions in priority-capabilities.toml. The selected-flow and capability row sets must equal the 14 requested identities; configuration keys instead follow their registry-derived canonical-base dependency closure. For example, selected `ttd` needs `thetradedesk` enabled even though that base is not an additional selected flow. Parse the actual example through runtime validation/resolution and require every selected identity effectively enabled, all required base/settings/secret references present, and no unrelated activation entries. Deleting the canonical-base enablement while keeping alias true must fail the handoff guard, with zero alias calls; do not relax the base veto or hardcode a bidder-name exception in production.
- [ ] Hand off selected-bidder deterministic evidence to Trusted Server embedding. Leave provider credentials, corrupt-config deployment injection, actual Q2-D and publisher pilot execution to publisher pilot with resource qualification's affected-target prerequisite.
- [ ] Run `cargo test --locked -p auctioneer-core --test bidder_onboarding`, then Task 18's evidence-driver/check commands as separate invocations under a fresh run scope including the onboarding and APS producers. Require all four named controls to execute, a nonzero exact fixture dispatch count covering all 14 identities, and exit 0 with any unfinished bidder-local branches still explicit. Wire this target into Task 17's preserved secure/default and feature lanes; commit the handoff only after these pass. An intermediate selected-flow checkpoint alone is not bidder catalog closure or deployment readiness.
- [ ] For bidder catalog closure under the [2026-10-26 correction](#2026-10-26-scope-correction), require current passing evidence for foundations and all 14 selected flows, including Sharethrough/Rubicon/APS, and every mandatory priority-local source/contract branch. Reconcile exact operation/fixture sets and positive/negative counts against the frozen reviewed requirements. Do not waive, transfer or relabel missing priority-local branches; keep later shared-delivery restrictions and migration state honest. A 13-row source-only wave or a partial checkpoint cannot close bidder catalog.
- [ ] Repeat Tasks 3/4/17 complete `S`-scoped schema/resource qualification, generated registration/validation, secure/feature regression and actual four-target application/package gates. Preserve existing all-261 source/compatibility tests without making universal unshipped-engine proof a prerequisite. Require shipped component bounds, four matched catalog-relative reports, native fit/headroom and full-scale synthetic foundation controls. Include composed privacy/context, original-deadline/gzip, activation/settings/secrets and applicable signed sync. Record bidder catalog closure separately from remaining catalog closure, resource qualification, Trusted Server embedding and publisher pilot. Tasks 20/21 and full-catalog `--require-complete` are not bidder catalog prerequisites; remaining-source failures stay explicit.

## bidder expansion: Post-Pilot Bidder Expansion

**Owner: bidder expansion, after the selected Trusted Server embedding/publisher pilot test path and before stored requests and accounts onward,
per AG's 2026-10-27 approval.** This does not expand initial bidder catalog or transfer its
unfinished priority-local work. Reuse the existing catalog/evidence pipeline.

- [ ] Record the selected Trusted Server/publisher pilot's passing acceptance
  identity and findings. Keep unrelated provider rows and affected resource qualification readiness
  requirements explicit; local compilation alone cannot supply this entry gate.
- [ ] Derive candidates from pinned inventory minus current verified migration
  evidence. Freeze a finite, nonempty initial queue of reviewed manifests using
  actual publisher demand and pilot findings. Specify exact identities, branch/source
  requirements, dependencies, endpoint/settings/secrets/sync decisions, required
  producers and positive/negative counts; do not invent a popularity roster.
  Require accepted new flows beyond the initial 14, not merely a replay of them.
- [ ] Assign each queued wave to bidder expansion and every residual identity/branch to
  remaining catalog closure with explicit prerequisites. Select currently supported shared flows;
  later-feature dependencies need their own approved prerequisite or stay remaining catalog closure.
  A required queued branch cannot be silently dropped or transferred to close
  bidder expansion; amend and review the frozen queue if its scope genuinely changes.
- [ ] Execute Task 20's per-family procedure for the queued set. Repeat the
  actual composed standalone and embedded acceptance for new bidders, including
  runtime activation, invalid-parameter/privacy isolation, deadlines, credentials
  and applicable browser-bounded signed sync. Preserve the priority 14 as
  regressions; passing their older evidence cannot certify the new set.
- [ ] Run current schema/resource, native fit/headroom, all four matched
  catalog-relative and target/package gates for the expanded shipped set. Use
  a fresh evidence run and each reviewed wave's consuming check. Review any
  additional real-traffic rollout separately from deterministic acceptance.
- [ ] Close bidder expansion only when every frozen queue requirement passes. Publish the
  exact expanded set, current evidence and residual ownership/readiness limits.
  Full `--require-complete` may still refuse unfinished source rows; it must not
  be weakened or required to pass for bidder expansion. Leave terminal reconciliation below
  under remaining catalog closure and do not claim catalog closure or new hosted qualification.

## Task 20: Reconcile And Migrate Remaining Families

**Owners: bidder expansion for its reviewed post-pilot queue, remaining catalog closure for residual waves and
the terminal gate.** This shared procedure does not gate bidder catalog closure or absorb
unfinished priority-local obligations. Preserve evidence-led wave selection.

**Files:** ledger/wave manifests, separately reviewed family plans, implementations/declarations/data/fixtures and capability reports.

- [ ] Derive remaining sets from pinned inventory minus verified current evidence, not a hand-maintained second central roster. Order smaller waves by demonstrated publisher requests/adoption, with exact membership and evidence reviewed before implementation; categories may reorder rather than imply guessed popularity. Group coherent protocol/shared requirements within that order. Every remaining identity stays owned through catalog reconciliation closure.
- [ ] Execute the Task 18 per-family sequence for every remaining source family, including already-shipped appnexus audit and source-disabled definitions. Alias rows get identity/vendor/metadata tests even when transform fixtures are shared.
- [ ] For each wave, reconcile AG-approved HTTPS-default selection, effective metadata/route tests and exact source URL/blob/transport-difference records for the remaining source-enabled HTTP bases, plus source-disabled HTTP and effective alias endpoints. Record effective HTTPS, an explicit scoped HTTP exception or an unavailable reason and capability status; no separate vendor TLS mapping is required for a scheme-only selection. Preserve source disables absent separate approval. At closure require disposition coverage of exactly all 78 declared source HTTP bases (77 enabled/one disabled), including the priority three and other 74 enabled bases. Honest unavailable rows can meet protocol-migration requirements with real code/evidence, but cannot be reported as usable or satisfy the priority-flow checkpoint. No omitted population or unowned future endpoint TODO may stand in for this audit.
- [ ] Keep protocol-local future video/native/AMP branches testable through injected typed context; do not ship later routes or claim cache/player behavior. Bring forward a separately reviewed bounded prerequisite if missing shared code prevents implementing/verifying a bidder.
- [ ] A required non-JSON response or unsettled authentication flow gets a reviewed parser/protocol plan and remains blocked until real code/evidence exists. Metadata-only, no-bid-only, stub or generic-fallback rows never count.
- [ ] After every wave run shared regression, schema/context/parse qualification, package builds and matched catalog-resource checks; update source-bound evidence rather than accepting old cached success.
- [ ] Preserve the complete identity/sync-family sets and opt-in policy. Full source migration is still required; source inactivity does not authorize omission.
- [ ] remaining catalog closure-only terminal gate: under a fresh full-catalog run scope, run `cargo run --locked -p auctioneer-cli -- bidders evidence run --output "$AUCTIONEER__BIDDER__EVIDENCE_ROOT" --run-id "$AUCTIONEER__BIDDER__RUN_ID"`, then `cargo run --locked -p auctioneer-cli -- bidders check --require-complete --evidence-root "$AUCTIONEER__BIDDER__EVIDENCE_ROOT" --run-id "$AUCTIONEER__BIDDER__RUN_ID"`. Require exit 0, exactly 358 migrated source identities (261 bases, 97 aliases), 261 valid base bindings, 259 reconciled source implementations, all 261 parameter schemas and zero unfinished required branches. Local generic/mocktioneer are separate. Exit 1 or any blocked/in_progress source row leaves Task 20's remaining catalog closure terminal acceptance incomplete, not the finite bidder expansion queue; do not defer the first full-set check to Task 21. Task 21 repeats it after final whole-program verification/documentation.

## Task 21: Full Catalog Closure

**Owner: remaining catalog closure, not bidder catalog, per the 2026-10-26 correction.** Repeat full-catalog
acceptance after Task 20; this is not a delayed bidder catalog completion gate.

**Files:** `docs/bidders/catalog.toml`, source-bound family/evidence reports,
`docs/roadmap.md` and `docs/architecture.md`. Track remaining completion work in
[catalog acceptance #122](https://github.com/IABTechLab/auctioneer/issues/122)
under epic #25.

- [ ] Repeat Task 20's full evidence-driver and `bidders check --require-complete --evidence-root PATH --run-id ID` commands under a new run identity/output directory: exactly 358 source rows derive migrated, 261 base bindings/259 implementations reconcile, schemas/aliases reconcile and local-only definitions remain separate. This must use real current implementations and source-owned behavioral evidence, not synthetic completion fixtures or a replay of Task 20's receipts.
- [ ] Repeat Task 19's complete 14-flow publisher handoff, including APS's separately reviewed contract and current executable evidence, as an remaining catalog closure regression gate. bidder catalog already requires these flows; this rerun does not postpone their acceptance to remaining catalog closure. Do not inflate the pinned 358-source-row count to include APS.
- [ ] Build the same complete shipped identity set for all four targets, preserving existing feature exclusions only. Repeat Task 17's acceptance commands and every family's required driver; exact operation sets and positive/negative counts must reconcile with the pinned source requirements and APS's separately reviewed contract respectively.
- [ ] Require current native fit/headroom, component bounds and all four matched catalog-relative gates. Keep resource qualification's WASM absolute blockers, unavailable physical observations and publisher pilot live evidence explicit; a catalog-complete result cannot turn these into qualification.
- [ ] Update architecture to implemented behavior and close only actually repaired ISSUES rows. Record remaining catalog closure full-catalog completion separately from bidder catalog foundation/priority closure, resource qualification readiness, Trusted Server embedding, publisher pilot live certification and stored requests through browser privacy compatibility shared features.
- [ ] Review the final source/binding/evidence/resource set differences, record exact passing test scopes and limitations, then commit and push the closure report. No required source-local branch may remain deferred while claiming remaining catalog closure full-catalog completion.

## Historical Acceptance And Self-Review Record

Before each commit, check that proposed test filters match tests and that Cargo target features do not silently skip them. A check command that only compiles the wrong path is not behavioral evidence.

This plan's non-negotiable review points are:

1. Complete reference inventory versus strict partial shipped catalog.
2. No new alias activation through an already-enabled base.
3. Single feature authority across generated modules, factories and metadata.
4. Registry failure does not become status/config degradation.
5. No schema/secret work for denied bidders or after the original deadline.
6. No fresh candidate/session decode allowance per response parse.
7. Catalog residency is once per isolate, not config-factor work per request.
8. Relative WASM resource acceptance never becomes absolute readiness.
9. Every source branch/fixture dispatch is real, counted and evidence-bound.
10. No implementation, qualification or human approval is claimed by writing this plan.

Self-review corrections incorporated in this draft: complete-inventory versus
partial-shipping separation, activation cohorts for new aliases, explicit
feature forwarding, fatal cached registry failures, generated validation rather
than guest schema compilation, additive initialization/auxiliary accounting,
source-order count guards, and evidence checking before any bidder wave.
The follow-up review also ties strict validation to boot's exact additive
derivation, separates metadata and validator descriptor owners, carries K
through proof enumeration, requires both anti-widening controls and both
catalog-validation owners, restores every alias-index control, and gives
the priority handoff and remaining-catalog tasks terminal runnable gates.
The documentation corrections precede implementation; checked Task 1 items
now record the completed source-reader and CI setup portions only.

The next review corrections distinguish source deployment defaults from
baked safety and direct routing, require real linked artifacts for the
qualification scope (now `S` under the 2026-10-29 amendment)
and early CI prerequisites, bind evidence to fresh producer runs, and make
boot/strict parity exercise raw-cap normalization in isolated processes.
The large-catalog usersync fixture retains its 256-entry caller/debug limits;
wave-contract approval is a separate checkpoint from implementation.

This records the original pre-execution review requirements, not a new gate on
completed controlled-native bidder catalog delivery. Follow the current owning plan for
remaining qualification or catalog work. Preserve failure evidence and obtain
review before changing that contract; do not weaken privacy or silently waive
the remaining full-catalog obligations.
