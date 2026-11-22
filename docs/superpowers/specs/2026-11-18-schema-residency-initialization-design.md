# Shipped Schema Residency And Initialization Design

Date: 2026-11-18. Issue: [#46](https://github.com/IABTechLab/auctioneer/issues/46).

Status: the maintainer approved this written specification at commit `ba383e2` on 2026-11-18 after advisory document review, then approved the reproducible reference-basis and downstream owner-ticket amendments in sections 7.2 and 10.1. Its implementation plan is proposed separately. No implementation, new resource bound, adapter qualification, serving activation or merge is approved by these planning decisions.

## 1. Outcome And Boundaries

Finish the common source-level residency (**C**) and initialization (**I**) contract for the discovery-selected implemented schema closure. Reuse completed component derivations; establish their correspondence and complete composition instead of repeating the full imported-catalog investigation.

Deliver one discovery-derived membership description, complete retained/constructor ownership models and source-bound C/I descriptor generation through existing owners. A descriptor is a checked derivation with explicit layout/linkage inputs, not a constant inferred from the largest measurement. Unsupported required owners refuse complete descriptor generation; they never contribute zero or disappear.

This is an Auctioneer package. Trusted Server engineers retain their host executor/service handoff. This work supports Fastly integration without claiming that the host or guest is ready.

### Included

- Hard-cut selection of implemented definitions, public aliases and their effective schemas from existing bidder discovery, with runtime enablement excluded from membership decisions.
- Immutable regex programs, generated/non-regex schema state, required shared format owners and registry/descriptor metadata attributable to this closure.
- Constructor storage and source-work bounds, already-live program overlap, repeat/failed initialization, and the initialization scheduling/sharing premises.
- Common formulas, provenance, completeness checks and descriptor correspondence for target consumers.
- Matched source-level catalog-growth observations against the reproducible Auctioneer reference in section 7.2, with data-only reconstruction stated explicitly rather than represented as historical physical measurements.
- An explicit source/owner/unit handoff to #47 and adapter qualification consumers, without installing a descriptor or changing admission.
- Independent membership, arithmetic, source/model correspondence and constructor controls in existing test owners.

### Excluded

| Outcome | Existing Owner |
| --- | --- |
| Per-validation cache/scratch/search work and evaluation stack certification | [#47](https://github.com/IABTechLab/auctioneer/issues/47) |
| Complete auxiliary response/allocator/handoff proof | [#48](https://github.com/IABTechLab/auctioneer/issues/48) |
| Actual ABI, physical allocator/host storage, linked artifact bytes, memory and application stack fit | [#72](https://github.com/IABTechLab/auctioneer/issues/72), [#73](https://github.com/IABTechLab/auctioneer/issues/73), [#74](https://github.com/IABTechLab/auctioneer/issues/74), [#75](https://github.com/IABTechLab/auctioneer/issues/75) |
| Guest serving, including the accepted common retirement of native-only publication | [#88](https://github.com/IABTechLab/auctioneer/issues/88), [#89](https://github.com/IABTechLab/auctioneer/issues/89), [#90](https://github.com/IABTechLab/auctioneer/issues/90) |
| Combined Trusted Server application and publisher acceptance | [#76](https://github.com/IABTechLab/auctioneer/issues/76), [#91](https://github.com/IABTechLab/auctioneer/issues/91) |

No new crate, dependency, matcher, vendor patch, configuration surface, catalogue or proof registry is proposed. Preserve schemas, matching options, input/workload limits, additive isolate accounting and resource ceilings. Any newly discovered need to change those is a separate reviewed decision, not an implementation shortcut. Existing native serving continues under its approved tested-reservation contract; this package does not silently replace it.

## 2. Source Baseline And Reuse

Planning baseline: Auctioneer `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`. The lockfile SHA-256 is `78118dcb53fd3f486a6032c47124ba249bd5fbf361a4a1973824e56d4f55001c`. The manifest selects patched compiler 0.61.2 and regex-automata 0.4.18, regex-syntax 0.8.11 and official EdgeZero PR #275 revision `e9ffaa8f8057144155ffe164280994445fa0f6d8`. These identify this audit, not a second source of dependency pins.

Read the [current ownership index](../../testing/resource-proof-ownership.md), [schema qualification record](../../bidders/schema-qualification.md), [shipped-scope amendment](2026-10-29-shipped-schema-qualification-design.md) and [approved shared ownership contract](2026-11-09-shared-resource-ownership-design.md) together. Historical captures retain their original bytes and scope; an old component result is reused only after checking its premises against the current source/features/generator.

| Actual Owner | Current Capability And Remaining Gap |
| --- | --- |
| [Bidder discovery](../../../crates/auctioneer-config/src/bidder_discovery.rs), [catalogue model](../../../crates/auctioneer-config/src/build_catalog.rs) | Validated bidder-owned factories/definitions/provenance already exist. Do not build another discovery service. |
| [Staging selection](../../../crates/auctioneer-config/src/schema_staging.rs), [build adapter](../../../crates/auctioneer-config/src/schema_staging_build.rs) | Still combine published roots with pinned priority/supplemental projections. Their selection authority must be removed, not renamed. |
| [Quoted validator generation](../../../crates/auctioneer-config/src/schema_staging_codegen.rs) | Existing converter and fixed boolean/cache/format options remain authoritative. It currently emits declarations, not complete C/I certificates. |
| [Actual schema owners](../../../crates/auctioneer-bidder/src/schema/staging.rs) | Generated singletons and shared hostname callback exist. Correspondence to reconstructed proof programs and constructor order must be explicit. |
| [Native admission](../../../crates/auctioneer-bidder/src/schema/native.rs) | Uses tested 16-MiB resident and 8-MiB initialization reservations, not complete bounds. Do not relabel these as certificates or copy them to a guest. |
| [Program composition](../../../crates/auctioneer-bidder/tests/schema_program_heap/mod.rs), [constructor model](../../../crates/auctioneer-bidder/tests/schema_nfa_construction/mod.rs), [syntax model](../../../crates/auctioneer-bidder/tests/schema_syntax_construction/mod.rs) | Conditional requested-storage/source-work components exist; complete generated closure, meta-constructor composition, non-regex state and target correspondence do not follow automatically. |

Existing metadata, NFA/capture, retained one-pass, scalar literal/prefilter, pool and applicable constructor components are reuse inputs. Their exclusions remain exclusions until discharged. Search-cache certificates belong to #47, except where a constructor actually creates a retained cache/program owner during initialization.

## 3. Discovery Is The Membership Authority

Derive the implemented public-definition set from the existing factory/declaration/alias models before any architecture-dependent publication filter. Require a real declared executable factory and public definition; sharing a source implementation reference does not make an imported identity an implemented alias.

For each ordinary security-feature lane, derive:

1. The implemented public definitions and aliases, including operator-disabled definitions.
2. Validated ancestry and nearest effective schema owner; aliases with an explicit schema override keep it.
3. Explicit local schema-free declarations.
4. The distinct generated validator owners, each emitted regex singleton and each required shared format/static owner.
5. The correspondence to the actual compiled lane and its existing publication rules.

A factory may own several public definitions; aliases may share a validator; one validator may own several regex singletons. These are different cardinalities. Counts are observations produced by discovery, not handwritten selection lists or frozen expected bidder names.

Keep two populations separate: the full imported catalogue is reported with explicit `not_implemented` dispositions, while the proof closure covers implemented definitions and their transitive required owners. Do not use an unimplemented skip for an implemented definition missing wiring, schema, proof or qualification. Full source/provenance controls remain intact.

Retire planned-root arguments and priority/supplemental wave selection in the active build adapter and selected-source conformance path. Do not retain a compatibility branch or an alias for the retired selection mechanism. Historical wave/capture data may remain under its existing integrity owners, but is not read to decide closure membership. Per-bidder source revisions and independently witnessed schemas remain necessary provenance, not delivery cohorts.

This selection cutover does not remove `native_only` from production publication in isolation. Reports distinguish implemented scope, compiled inspection owners and currently callable bindings. Target absence is not qualification and cannot hide a required implemented owner. The accepted publication retirement remains with the qualified common guest cutover.

## 4. Ownership And Units

Join the build-emitted membership seed to the conformance-completed expanded owner graph as specified in section 7.1. The build seed does not claim to know macro-internal regex helpers. Each completed owner has a stable structural identity, parent/reference edges, source/configuration witness and storage role. A structural identity includes its declaration and emitted ordinal/path; a pattern digest alone is not an owner identity.

| Component | Unit And Sharing Rule |
| --- | --- |
| Retained heap | Requested allocation bytes/capacity from a source bound with explicit layout premises. Arc header/padding, backing capacity and independently owned children are separate where not already included. |
| Inline/static storage | Actual compiled layouts or explicit required layout inputs, separately from heap requests. An Arc handle inside another charged object is not a second backing allocation. |
| Generated code/read-only data and registry metadata | Complete attribution inventory and a declared existing budget owner. Actual linked bytes/physical mapping are supplied and checked by #72-#75; source/token/file length is not a byte bound. |
| Initialization-only storage | Simultaneously live constructor allocations, growth copies, intermediates, cleanup and bookkeeping not already covered by retained owners. |
| Constructor work | Checked source-level visit/operation bounds with named units and exclusions, not wall-clock time, CPU instructions or the current native validation tokens. |

Deduplicate backing only with a source-backed sharing edge. Equal patterns, equal layouts, shared Rust types, aliases and cloned Arc handles do not by themselves prove that independently initialized programs share storage. Charge distinct generated singletons independently; charge the shared hostname program once per actual owning instance. Keep per-call hostname cache creation out of C unless the source retains it.

Do not multiply one catalogue's C/I by auctions, impressions, aliases or calls. Multiple independently owned compiled instances or concurrent initialization streams need their actual existing owner multiplicity, never an assumed process-wide singleton. This package introduces no new multi-instance serving lifecycle.

## 5. Retained Storage Composition

For a fixed discovered closure and explicit layout/linkage inputs, derive C from a disjoint retained-owner inventory. Separate heap requests, inline/static layouts, linked code/data attribution and metadata so target consumers can reconcile each with the existing budget exactly once.

A component certificate must identify its complete ownership subtree, already-included children, source/configuration premises, checked formula and unsupported cases. Reject duplicate ownership, a missing child, incompatible premises, unchecked overflow and an unsupported reachable strategy. A zero term is permitted only for a positively proved absent component, with its absence proof recorded; lack of a getter, zero observations or failed construction is not absence.

The existing reconstructed program bound is conditional. Before reuse, establish that the actual emitted helper uses the witnessed pattern, constructor flags, library/features and ownership topology. Preserve the distinction between a source-equivalent reconstruction and a measurement of a generated singleton. A new reachable helper/format/backend invalidates the closure certificate until its source model is covered; unrelated unreachable imported backends do not become prerequisites.

Include non-regex generated validators, required format helpers, lazy cells, retained success/failure bookkeeping and catalogue/registry descriptors. A no-regex schema is not automatically a zero-state schema. Audit the converter's emitted boolean path for any retained state; do not charge validation-temporary allocation as immutable storage or omit it from #47's handoff.

Build-host `size_of` values cannot fill a guest layout slot. Existing compiler/library-owned layout observations may supply the ABI they actually measured; require source correspondence and independently bounded capacities, not shallow layout as a whole-heap certificate. No private Rust struct mirror or new vendor layout API is authorized here.

## 6. Initialization Composition

Bind I to the actual initializer schedule and constructor ownership transitions, including the required shared format initializer. A list of successful no-pattern callback invocations is not evidence that every real singleton was constructed.

Retain already-live programs while constructing the next one. For each admitted source path, cover parser/HIR storage, compiler intermediates, both directional programs when reachable, optional retained strategy candidates, vector growth/reallocation overlap, Arc/box construction, temporary wrappers and failure cleanup. Reuse existing syntax/NFA/one-pass models only within their source premises; complete their meta-constructor overlap rather than summing peaks that each exclude the other.

The aggregate obligation is, for every successful or failed initialization prefix and every supported instant t:

```text
live_requested_schema_storage(t) <= C_requested + I_requested
```

Here C_requested covers the final retained heap capacity of all owners in the closure. I_requested is derived directly from simultaneously live construction-only owners, including extra copies or abandoned candidates; it is not an observed peak minus retained bytes. Static/code/data terms stay separately identified. An allocation omitted from I needs a demonstrated retained-owner charge that already covers it at that instant.

Use the source schedule to justify a maximum instead of a sum only when construction is serial and prior constructor scratch is released. Include all concurrent constructor instances if that is the supported schedule. Do not assume that LazyLock on one owner serializes initialization of different owners. The existing outer native OnceLock and its actual format/generated ordering are evidence to audit, not a portable concurrency guarantee for future guest boot.

Required lifecycle cases are cold success, repeated success without duplicate programs, already-live prefix plus late success, late failure/cleanup, cached failure and concurrent callers under the actual coordinator. A constructor that allocates and then fails cannot produce a zero-cost certificate. Recoverable injected failure must be observed at its intended stage; fake no-op constructors are negative controls, not proof of a production failure path. Do not claim recoverable OOM behavior for aborting allocation APIs.

Constructor work is composed with its own source units and supported grammar/configuration. Include failed prefixes and bounded repeated calls; elapsed observations do not establish that bound. Validation/search work and machine-stack certification remain separate #47/adapter obligations.

## 7. Descriptor Contract And Consumers

Extend existing staging identity/membership output with build seeds, and the existing selected-source conformance output with the completed common descriptor. Do not introduce another catalogue, certificate registry, proof crate or operator file. The completed descriptor carries:

- Discovered identity/alias/schema/helper membership and owner-sharing edges.
- Exact source/schema/generator/vendor-patch/options/feature witnesses and model identities.
- Required compiler/ABI/layout and linked-code/data inputs, each with explicit units and owner.
- Checked retained and constructor formulas, schedule premises, constructor-work units and completeness/refusal reasons.
- Explicit scope: source-level requested-storage/initialization evidence, not physical fit or serving qualification.

A source-complete descriptor is a recipe whose common derivations and required target-input slots are exhaustive. It is not an aggregate physical byte value. An unsupported common owner means source-incomplete; missing target layout/allocator/linkage inputs mean target-incomplete. Keep these distinct. A target consumer cannot instantiate costs from either incomplete state or replace required facts with defaults or zero.

Common generation must not depend on the bidder implementation crate from its own build script, import test modules into production or move native regex/compiler qualification dependencies into guest serving. Reuse current quoted generation and existing conformance consumers for formula evaluation/source correspondence. Require `quote!` and `syn::parse2` for any new Rust emission.

Existing adapter qualification owners supply actual target layouts, allocator/host facts and linked artifact identity and reconcile C/I against their accepted accounting. Existing `CatalogResourceCosts` remains the eventual numeric interface; no extra budget equation, startup/steady maximum, reserve or automatic deployment-profile change is introduced. This package must not install an unqualified number in that interface.

The controlled-native reservations remain byte-for-byte the same policy values. New descriptors do not turn them into proofs or silently reduce/increase them. Preserve current native behavior and guest refusal until their separate accepted cutovers.

### 7.1. Build Seed To Expanded Descriptor

The existing build adapter captures verified schema snapshots and emits macro-annotated declarations to OUT_DIR. Its current emitted-source and input digests already identify that boundary. Extend that seed with the declaration-to-schema mapping needed below; it remains explicitly unexpanded and cannot claim a complete helper graph or C/I bound.

Complete the graph in the existing native conformance owner using its already-declared patched compiler dependency, not by adding the compiler to auctioneer-config's build dependencies:

1. Capture the actual build output's staging identity/manifest, generated declarations and schema snapshots through the existing application/probe output path. Accept one verified build-local snapshot set; expansion must not reread the mutable bidder-source checkout. Preserve the existing private regular-file snapshot/path guarantees and require its bytes/digests to remain unchanged for the expansion. The compiler's existing path API opens that captured snapshot itself; this is not a claim of one physical filesystem read or a new in-memory compiler API. Treat Rust source as inspected data, not a handwritten source import or a production API exposing private validators.
2. Parse those captured declarations with syn. Invoke the pinned compiler's existing expansion entrypoint on each actual validator attribute and marker item, preserving its schema snapshot, draft, method/cache/limit flags and format callback paths. Reject compile_error output, unaccounted declarations, unsupported syntax or an inconsistent seed. Do not regenerate a similar declaration from pattern strings or silently substitute the existing BundleProof marker/options.
3. Derive translated patterns, actual helper identifiers/ordinals, deduplication, singleton/callback references and retained/constructor roles from that expanded syntax plus the witnessed compiler/format sources. Keep per-declaration helper namespaces distinct. The completed expanded graph in the selected-source conformance report is authoritative for these internal owners; the build seed and source-inventory pattern list are not.
4. Bind the graph to the seed identity, declaration/schema digests, compiler/patch/features and expansion/model source identities. Account for every emitted declaration and required helper exactly once. Independently check semantic options and callback identities against the approved production contract; a self-consistent but changed seed alone does not prove that contract.
5. Apply the reused source models to this graph and emit the completed descriptor in that same existing report. A source-equivalent reconstruction remains labelled as such. Physical target consumers must additionally match the source/generator/feature tuple and actual linked artifact; this host-side expansion is not guest execution or linked-layout evidence.

Any intentional source normalization for stable identities must be structural, narrowly specified and tested not to erase a semantic attribute, translated pattern, helper, constructor option or callback difference. Do not compare hashes after dropping unknown syntax.

The existing pattern-only bundle extractor may remain a component diagnostic, but cannot supply a complete C/I descriptor without the seed/expanded-graph join above. This is a one-way evidence handoff: conformance consumes actual build outputs; a passing test is not read back by build.rs to enable serving. Later target consumers instantiate only matched, complete evidence under their own acceptance gates.

### 7.2. Matched Catalog-Growth Evidence

On 2026-11-18 the maintainer approved replacing the unavailable pre-import growth reference with public Auctioneer commit [`d26fcf7f566db6d1dce2e1847d79c9f44581edb8`](https://github.com/IABTechLab/auctioneer/commit/d26fcf7f566db6d1dce2e1847d79c9f44581edb8), tree `2d54c6f53ef3d062ca9292449a4546bb76e5ddd8`. Freeze that reference, not a moving branch or implementation head. Section 7.2.1 assigns fresh-checkout acquisition to hosted setup and commit/tree verification to its offline test consumers. Failure to acquire or verify it is unmatched growth, not an empty reference.

Construct the reference seed as data through the same version-2 discovery/seed writer and expansion path as the candidate:

1. Read the pinned Git tree into an isolated, non-serving source snapshot. Record commit, tree, and every consumed regular input's repository-relative path, Git blob ID, byte length and SHA-256. Never read an untracked local checkout or execute the reference's build scripts. Preserve existing bounded, contained-path and regular-file checks.
2. Discover from that snapshot's bidder manifests, executable declaration/source-presence witnesses and Cargo feature declarations. Include every schema/reference/provenance input read by discovery and generation, including required non-cohort witness inventories and the transitive referenced schema closure. The snapshot's workspace manifest and lockfile are provenance inputs too. A missing input or schema owner refuses; no reference delivery-wave list, old seed or old report reader selects membership.
3. Emit the existing version-2 seed with the independently discovered public definitions/aliases, effective schema owners, declaration-to-schema mapping, schema bytes/digests and source witnesses. Join it to the actual generated declarations and expanded owner graph by section 7.1. Do not substitute a hand-built list, the candidate's membership, or the zero catalog fields in an old capture for the reference seed.
4. Freeze both seeds and their expected owner-set difference before observations. Run them under one measurement compiler/library/generator/options, actual ABI, features, profile and unchanged workload tuple. Record each snapshot's provenance lock separately from the measurement lock; any difference is explicit reconstruction scope, not a claim to have rebuilt that snapshot's original application. An intentional schema/program-owner delta is measured; an undeclared missing, extra or substituted owner or a changed non-catalog workload/accounting input refuses the match. Metadata/registry comparison supports only the explicitly equal-input proxy defined in section 7.2.2; unequal metadata inputs refuse rather than produce a delta.

Label the result **same-build data-only source reconstruction relative to d26fcf7**, not original pre-catalog growth, a historical executable rerun or deployed physical measurements. This comparison does not restore a legacy serving implementation or compatibility reader. It can legitimately observe no owner growth; that does not establish zero catalog storage.

Use the existing schema-source constructor owner for schema retained/initialization observations and the explicitly labelled metadata/registry equal-input proxy in section 7.2.2. Record source-generation size separately as an observation, not linked code residency. Both sides use the same actual compiler/ABI/dependency/features/profile; preserve unchanged workload inputs and identify cold/repeated/failed construction scopes. Growth and absolute component observations must remain separate from source bounds; a largest observed peak cannot set its own allowance. Existing `catalog_alloc` regression controls remain required but are not a reference-growth consumer.

Derive absolute C/I for the entire current implemented closure independently of the comparison delta, including all costs introduced before d26fcf7. Neither a zero nor negative delta discharges any absolute retained, initialization, linked or physical ownership obligation. Reconcile component deltas explicitly; unrelated savings cannot hide catalog growth. The pure `catalog_resource_profile` target consumes captured descriptors as data and remains non-installing; actual constructors stay in their existing isolated measurement owners.

The pure arithmetic consumer validates each immutable capture against its recorded reference and measurement pins: the candidate source lock and both measurement locks must equal that capture's declared measurement lock, while the reference source lock must equal its separate recorded pin. It must not bind those data fixtures to the checkout's live `Cargo.lock` or require recapture after an unrelated dependency edit. Current-build freshness belongs to the compiler/schema live-growth owner, which measures the current lock and retains all source/model compatibility and refusal checks. Preserve old capture bytes; publish a new record only as deliberate new evidence, not to make historical arithmetic green.

Preserve the frozen [original accounting capture](../../../crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml), its [patched-lock continuation](../../../crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline-patched-lock.toml), and their [continuity contract](../../testing/evidence/catalog-baseline-continuity-20261021.md) byte-for-byte as history. Their pre-import source hashes are archival, not acquisition prerequisites for this comparison; their unqualified zero C/I fields are not measured zero footprints. Existing integrity/continuity readers keep their original scope. Do not repin them to the replacement basis or disable an old lock/ABI check to consume them as current evidence.

The same approved amendment adds explicit whole-current-catalog physical/allocator bounds and actual linked code/data growth criteria to #72-#75. Those owners must obtain verifiable reference/candidate artifacts and declared comparison/continuity identities on their actual adapter ABI. This common data-only reconstruction is not a substitute for those artifacts; an unavailable or unmatched physical reference stays nonpassing. The new reference scopes growth since d26fcf7 only; each target must still account for its full current catalog, including earlier costs.

#### 7.2.1 Hosted Reference Input

The implementation extends `.github/workflows/test.yml`, not the tests' network behavior. Add a step named `Check out pinned catalog-growth reference` using the workflow's existing `actions/checkout@v7`, repository `IABTechLab/auctioneer`, exact ref `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`, `fetch-depth: 1`, `persist-credentials: false` and path `vendor/catalog-growth-reference`. Run it before test execution in `compiler-conformance` only for its `schema` lane. That existing owner executes both default/development `schema_residency` growth cases. The maintainer-approved section 7.2.2 amendment removes the three unconsumed checkouts in the `test` matrix; do not add a job, matrix, historical application build or checkout in unrelated artifact/CLI/producer lanes.

Set `AUCTIONEER__TEST__CATALOG_REFERENCE_DIR` in that existing compiler job definition to `${{ github.workspace }}/vendor/catalog-growth-reference`. It is a runtime input for real reference-growth cases only, not a build-script input, operator setting, global test initializer or proof of directory availability. Other compiler lanes must not read it; the `test` matrix does not receive it. Pure composition fixtures use explicit fixture data; `catalog_resource_profile` remains data-only and never opens this Git repository or installs gates.

Before adding the checkout, add only `/vendor/catalog-growth-reference/` to `.gitignore`, alongside the existing nested data-reference exclusions. It is not application source; its Git identity is independently bound by the growth reader. Extend CLI's existing `workflow_reference_checkouts_are_ignored_without_hiding_other_sources` control to include this newly declared repository instead of selecting only Prebid checkouts. Retain checks that tracked/untracked owned vendor sources change the application snapshot and an unknown nested repository still refuses. Do not ignore `vendor/` broadly, remove source files from the snapshot or relax `source_identity_nonfile` to accommodate the checkout.

A real reference-growth test must require that directory and verify its pinned HEAD, commit/tree and required regular-file objects before constructing a seed. Read pinned objects with the existing Git-object/source-verification pattern, including `--no-replace-objects`, not mutable checkout files. Reject partial/promisor references and missing objects; no clone, fetch, lazy fetch, private archive, current-worktree fallback or conditional skip is allowed inside a test. Network acquisition belongs only to the checkout step. The reference's build scripts are never executed. Missing/empty variable, absent directory, wrong identity and missing input must produce an executed failure/refusal with no matched-growth credit, not zero rows or zero costs.

#### 7.2.2 Maintainer-Approved Equal-Input Proxy Amendment

On 2026-11-21 the maintainer approved this narrower metadata/registry contract while addressing PR #262. It replaces the originally planned reference-side `catalog_alloc` consumer; it does not change schema/program growth measurement, resource limits, the equation or downstream physical qualification.

The conformance owner reconstructs the reference and candidate metadata inputs with the same current generator. The equality projection is every discovered bidder manifest's path/byte-length/SHA-256 witness, generated constructor and binding bytes, declaration count, and the shared current measurement generator/features/source identity. The generated constructor digest/counts must also match the compiled production descriptor. Each complete source snapshot is independently verified and frozen; historical commit/tree/input fingerprints and provenance lock remain distinct and are not required to equal the candidate or measurement lock. Only after those checks may fresh exact-test processes measure the current executable's actual `bidders()` and `configured_registry()` constructors twice. The first observation must be labelled **equal-input proxy of candidate**, not execution of reference constructors; the second is an actual-current-constructor observation. Keep both source fingerprints and process/measurement identities.

The report and evidence must state `supports_nonzero_metadata_registry_delta: false`. Unequal metadata/registry inputs refuse before observation; they cannot become a measured zero, a fallback or a supported nonzero comparison. Do not generalize the proxy from equal byte counts, matching names or one matching hash. Retain the independent source equality and compiled-constructor checks, all negative controls and fresh-process allocator positive controls.

Both observations belong to the existing `schema_source_conformance` owner. Keep the Git reader private under `tests/schema_residency/`, with ordinary module wiring and the same existing byte/deadline/refusal bounds. Do not publish a production API, execute historical code, add another target or Cargo child build, or reinterpret `catalog_alloc`'s existing regression samples as source bounds. The three non-consuming `test` lanes no longer acquire the growth reference; the compiler/schema lane remains its sole hosted acquisition owner.

**Maintaining the frozen comparison.** Keep the current reference fixed. Its equality projection covers all 262 discovered bidder manifests, including non-implemented bidders, and its reviewed schema/program owner delta is zero. A legitimate bidder change can therefore make this comparison refuse; that does not authorize skipping it or narrowing its input set.

Future bidder work belongs to the existing [#11 Demand-Led Post-Pilot Bidder Expansion](https://github.com/IABTechLab/auctioneer/issues/11) and [#25 Residual Bidder Waves And Full Catalog Closure](https://github.com/IABTechLab/auctioneer/issues/25) epics. In the affected bidder-change PR, independently inventory the changed inputs/owners and freeze the expected delta before observations. The schema/compiler maintainers responsible for [#46](https://github.com/IABTechLab/auctioneer/issues/46) review any necessary snapshot/reference update and its new memory-cost evidence as part of that same PR. No separate approval workflow is introduced. If the strict equal-input proxy cannot support the change, either explicitly review an advance to a new immutable official reference or review a nonzero comparator when it becomes necessary; neither happens automatically merely to make CI green.

An approved reference update must reconcile the Git reader's commit/tree/census, exact hosted checkout and current reference expectations together. Preserve historical captures and their recorded contexts; publish uniquely named default/development captures with their own source, lock, measurement and provenance bindings. Do not rewrite frozen arithmetic captures for an unrelated live-lock update. The update must not erase earlier costs: source proofs still cover absolute whole-current C/I, including costs predating both reference points, and #72-#75 retain their physical/allocator and actual linked-growth/continuity requirements. Resource limits, measurement premises, refusal controls and qualification gates remain unchanged.

This documents future maintenance, not standing approval to retarget the present reference or change its zero-delta expectation. Until the affected PR has an approved, supported comparison, the refusal remains a blocker for that PR. The owner links add no new issue dependency or epic blocker.

### 7.3. Coordination With #47 And Adapter Owners

Deliver an explicit producer/consumer correspondence with #47 using the same declaration/helper/source/feature identities. Assign each immutable program and initialization owner to C/I, and each per-validation cache, mutable scratch, search-time fallback and per-call hostname cache to its actual #47 V/lifetime owner. A child charged in a component cannot be charged again through another wrapper, or disappear at the boundary.

Adapter consumers receive the complete required layout/linkage/allocator inputs and schedule premises; they do not infer them from a source-complete flag. Review this interface correspondence before #46 closeout, retaining unresolved #47/target obligations on their existing issues. This requires a complete handoff, not completion of downstream validation/fit/admission or any `native_only` publication change in #46.

## 8. Implementation Ownership

| Responsibility | Existing Owner To Extend |
| --- | --- |
| Discovery, pure selection, source snapshots and manifest identity | auctioneer-config `bidder_discovery`, `schema_staging`, `schema_staging_build` |
| Quoted declarations, source snapshots and unexpanded membership/identity seeds | auctioneer-config `schema_staging_codegen`; auctioneer-bidder build adapter |
| Actual singleton/format/init correspondence | auctioneer-bidder `src/schema/staging.rs` and current generated hooks |
| Captured declaration expansion, completed owner graph/C-I descriptor, independent selection/source/component composition and observations | Existing `schema_source_conformance` target and its owner modules, including `schema_selection_source` and the patched compiler dev dependency |
| Constructor source/layout models | Existing `schema_syntax_construction`, `schema_nfa_construction`, `schema_onepass_construction` and applicable program/layout owners |
| Matched source-level growth and pure arithmetic continuity | Existing `schema_source_conformance` constructor/proxy probes and private `schema_residency/catalog_reference` reader, `catalog_resource_profile` and current source report; pinned d26fcf7 Git data supplies the reference, original captures remain historical; unchanged `catalog_alloc` retains regression scope only |
| Physical consumption and eventual activation | Existing #72-#75 and #88-#90 owners, not implemented here |

Keep pure unit tests inline. Extend existing conformance modules under normal Rust module declarations; no handwritten `#[path]` or cross-crate source inclusion. Retain independent derivations/oracles: sharing production selection output with its expected result is not a membership test. Use synthetic fake bidder declarations for infrastructure controls; real schema/provenance coverage stays with its existing owner.

## 9. Acceptance And Falsification

The implementation plan must assign each row to an existing executed test owner and name its exact assertions and CI selection. These are requirements, not claimed results.

| Surface | Required Independent Controls |
| --- | --- |
| Discovery authority | Adding a fake executable definition/alias changes membership without editing any wave list; operator enablement and native-only publication do not change the implemented proof closure. |
| Full catalogue versus implemented closure | Every inventory entry has one disposition; false skips, missing/extra/duplicate owners, broken aliases and missing declared wiring refuse. Schema-free and explicit override cases remain distinct. |
| Identity versus sharing | Equal patterns in separate singleton owners increase storage; multiple aliases of one owner do not. Removing an actual child/sharing edge fails the corresponding control. |
| C composition | Literal/independently derived totals for small graphs; vary owner count, capacity and payload/layout independently; omitted summands, duplicate charges, missing layouts and overflow fail. |
| I composition | Independently varied retained prefix, current constructor scratch and cleanup/growth overlap; cold/repeat/failed/cached/concurrent cases; deleting prefix or failure coverage fails. |
| Source correspondence | Changed seed/declaration/schema bytes, macro attributes, translated pattern, constructor option, format callback, helper namespace, generator/patch/library identity, feature lane or unsupported reachable strategy refuses the old descriptor. Build-only and pattern-only seeds cannot claim graph completeness. |
| Descriptor status | Missing common proof never becomes zero; missing target facts cannot yield instantiated costs, fit, serving permission or an overall qualified report. |
| Regression boundary | Same admitted parameter outcomes, original caps/reservations/accounting and native lifecycle; unchanged historical captures and source/provenance gates. |

Require nonzero, exact expected dispatch for filtered/parameterized controls and exact population reconciliation. Do not let zero-iteration loops, all-empty fixtures, identical independent inputs, soft non-rejection assertions or expectations calculated from the same production constant serve as guards.

Measurements corroborate source bounds with fresh constructor lifetimes and allocator-window positive controls. Record requested retained bytes and construction peaks separately; RSS, wasm section sizes, native measurements and elapsed time are not interchangeable certificates. Reuse existing native/guest probes rather than adding repeated full child builds or another harness.

## 10. Review, Validation And Completion

This written spec precedes the implementation plan. The plan must identify ordered changes, affected existing targets, mandatory CI owners, explicit red/green controls and complete hosted acceptance. No implementation starts before the spec/plan planning PR is reviewed and merged under the PRfection gates.

Honor the maintainer's instruction not to execute Cargo or CI locally. Read-only source/document checks are local; required build/test/lint/guest/producer runs use GitHub Actions at the relevant head. Keep the full existing feature/source/compiler/guest matrix. Exact result lines and source identities must come from the run that actually executed them.

At this planning baseline, GitHub reports the 25 acceptance checks successful at `d26fcf7`; its additional maintenance check is separate. That is supplied baseline evidence, not verification of this proposed document or future implementation. No new tests, descriptors or qualification runs have executed.

#46 completes only when discovery is the sole active membership authority, the common retained/initialization owner models and formulas are source-complete for the actual implemented closure, descriptor/source correspondence and refusal controls execute successfully, matched source-level catalog-growth evidence and the #47/adapter handoff are delivered, and the complete implementation matrix and review gates pass. An index of missing common proofs alone is not completion. Explicit target physical/layout/linkage inputs and #47/#48 obligations remain with their existing issues; their absence cannot be converted into a ready/fit/serving claim.

### 10.1. Issue Criteria Reconciliation

On 2026-11-18 the maintainer explicitly approved reconciling #46's issue body to this ownership split: common C/I proofs, matched catalog-growth observations and #47 coordination stay in #46; physical/linked adapter qualification stays in #72-#75 and the qualified `native_only` retirement stays in #88-#90. The subsequent approved amendment replaces the inaccessible pre-import growth source with section 7.2's pinned public reference and explicitly adds physical/allocator and actual linked catalog-growth criteria to each of #72-#75. Read back all five issue bodies before implementation; a spec-only reassignment is not a completed handoff. No integration requirement is waived. In particular, do not make #46 wait for downstream issues that already depend on it.

| Existing #46 Acceptance/Checklist Item | Required Delivery Or Downstream Owner |
| --- | --- |
| Acceptance: compiler/library layout, immutable/shared owners and constructor envelopes | Sections 4-6; plan Tasks 3-6. Common formulas require explicit target layouts, not host-to-guest substitutions. |
| Acceptance: common C/I descriptors/source proofs; unsupported owners refuse | Sections 5-7; plan Tasks 3-7. Actual target physical/ABI validation remains #72-#75. |
| Discovery: implemented definitions and bidder-owned schema/provenance, no cohort selection | Section 3; plan Tasks 1-3. |
| Discovery: common descriptors and exact full-catalog versus executable coverage | Sections 3 and 7; plan Tasks 2-4 and 7. |
| Discovery: coordinate descriptor/admission ownership with #47 and adapters | Section 7.3; plan Task 7's interface handoff. Actual validation/admission and publication changes remain #47/#88-#90. |
| Retired ledger: metadata/registry/schema sharing, aggregation and matched catalog growth | Sections 4-7.2; plan Tasks 4-8. Physical/allocator and actual linked code/data bounds/measurements are expressly #72-#75, not omitted or inferred from the common observations. |
| Retired ledger: reuse completed components; do not qualify unreachable imports | Sections 2, 3, 5 and 6; plan Tasks 1 and 4-5. |
| Integration contract: whole-catalog reporting and discovery | Section 3; plan Tasks 2 and 7. |
| Integration contract: positive implemented-identity/alias qualification on all four adapters | Mandatory downstream #72-#75/#88-#90 outcome, not #46 source-completeness evidence. |
| Integration contract: qualified common `native_only` retirement | Mandatory downstream #88-#90 common cutover; section 3 forbids isolated flag removal. |
| Shared acceptance and review/full-suite gates | Section 9-10; plan Task 9 and its validation/falsification map. |

The written-spec review is advisory document review, not a formal independent PR approval. Publication, implementation and merging require their separate user decisions. Do not close #46 on a planning merge or weaken Fastly/Trusted Server acceptance because its portable prerequisites are documented.
