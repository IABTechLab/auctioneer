# Shipped Schema Residency And Initialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement the approved plan task-by-task. Steps use checkbox syntax. This document is a planning proposal, not implementation authorization.

**Goal:** Complete #46's discovery-derived common retained-storage and initialization descriptors for the implemented schema closure, reusing the current component proofs without claiming adapter fit.

**Architecture:** Existing bidder discovery emits an unexpanded membership/source seed. The existing native conformance owner captures the actual build declarations and snapshots, invokes the existing patched compiler, and completes the owner graph and checked C/I derivation. Target layout/physical/linkage evidence and serving activation retain their existing issue owners; native reservations and isolate accounting do not change.

**Tech Stack:** Existing Rust packages and workspace dependencies, serde, quote/syn, the pinned schema compiler/regex libraries and existing compiler-layout/allocator/application probes. No new dependency, crate, vendor patch, matcher, operator configuration or test executable.

**Contract:** [Approved specification](../specs/2026-11-18-schema-residency-initialization-design.md), approved by the maintainer after advisory review at specification commit `ba383e2`, with the section 7.2/10.1 reference-basis and owner-ticket amendments approved on 2026-11-18. Refs [#46](https://github.com/IABTechLab/auctioneer/issues/46). Source baseline and exclusions are in spec sections 1-2.

---

## Execution And Evidence Rules

- [ ] Merge the current-head-reviewed spec/plan PR into the default branch before implementation. Create a separate implementation worktree/PR; do not execute this plan in the planning branch.
- [ ] Refresh the source/dependency/feature baseline before coding. Compare actual relevant source changes with the approved spec; material premise changes return to review rather than being hidden by updating a hash.
- [ ] Run Cargo/build/test/lint/guest/producer checks only on GitHub Actions, following the maintainer's no-local-CI instruction. Local read-only source/document inventories and Git diff checks remain permitted.
- [ ] Obtain authorization for implementation publication, tests-first hosted checkpoints and any disposable mutation PRs before those remote writes. The planning-publication approval does not authorize that batch or a merge.
- [ ] Record behavioral RED and GREEN evidence at exact commits. New tests may initially use a clearly labelled compiling conformance scaffold returning an incomplete/refusal state; a missing function/import, compiler error or zero selected tests is not the intended behavioral RED.
- [ ] Preserve the complete existing feature/compiler/producer/guest matrix. Group coherent changes into reviewable commits and reuse existing child builds; do not introduce a full-matrix rerun per tiny plan step or remove any existing required owner.
- [ ] Treat the successful 25-check baseline at `d26fcf7` as historical supplied evidence only. No future result/count in this plan is an executed observation.

Every task below follows tests first, recorded hosted RED, minimal implementation, hosted GREEN and a focused commit. Keep positive and negative controls together so an implementation returning refusal for everything cannot pass.

## Historical Implementation Checkpoint: 2026-11-21

This historical checkpoint does not replace the final delivery record or GitHub review state. See the [dated evidence](../../testing/evidence/schema-residency-initialization-20261121.md) for exact source/capture identities, the per-task RED/GREEN map and current verified implementation checkpoints. Earlier failed runs remain history, not current-state claims.

| Task | Delivered / Executed Scope At 081 | Remaining Acceptance At That Checkpoint |
| --- | --- | --- |
| 2-3 | Discovery/version-2 capture and expanded-source controls execute in the 081 default source suite. | Final-head four-application/source/producer owners and complete matrix. |
| 4-7 | Default captured recipes are SourceComplete / TargetIncomplete; retained/constructor/fact/handoff controls and reconstructed lifecycle observations pass in the 806-case suite. | Do not credit uninspected native lifecycle, ABI or separately owned probes from this filtered result; final-head acceptance remains required. |
| 8 | Captured-source behavioral RED and default matched reconstruction GREEN are recorded; production metadata/registry observations and hypothetical C/I controls pass; raw hosted capture is frozen with its digest. | Verify the captured-data consumer's integration controls against unchanged production arithmetic; record existing catalog/continuity owners and development capture. No separate additional RED is required for each consumer test. |
| 9 | Existing CI wiring and default dispatch/capture are present. | Eight strict-lint errors at 081, hosted formatting, complete final-head matrix, both producers, issue-criteria readback and advisory/independent review. |

The plan ends at Task 9. Final completion/approval/merge obligations are also in specification section 10, not a separate Task 10. Acceptance and issue closure are recorded by PR #262's review/merge, not mirrored as mutable status here. Missing physical/allocator/linked target facts remain #72-#75 inputs, not zero costs or new prerequisites reversing the approved dependency direction. No fit/serving claim is made.

## Maintainer-Approved Review Amendment: 2026-11-21

Specification section 7.2.2 replaces the originally planned `catalog_alloc` reference-growth consumer with an explicitly closed equal-input metadata/registry proxy in the existing `schema_source_conformance` owner. Full schema/program growth remains measured. Exact constructor/binding bytes, manifests, counts, measurement tuples and compiled source identity must match before either fresh process starts. Report the reference as `equal-input-proxy-of-candidate`, the candidate as `actual-current-constructor`, and `supports_nonzero_metadata_registry_delta: false`; unequal inputs refuse. These are observations, not common C/I or physical bounds.

Ordered repair/verification steps for that approved amendment:

1. Retain the tests-first handoff/evidence-name checkpoint and record the intended hosted failures, not a compile/lint/setup error.
2. Keep the offline Git reader private in `tests/schema_residency/catalog_reference.rs`, preserving its input caps, no-network rules and complete refusal controls. Remove both production exports and update source-witness paths.
3. Make the proxy scope/roles and unequal-input refusal explicit in the producer and its independent assertions. Rename the plan-task evidence field to `matched_reconstruction_complete`; preserve old captures as history rather than rewriting them.
4. Remove the three unused `test`-matrix reference checkouts and its unused reference variable. Keep the compiler/schema checkout, both feature-lane growth cases, exact dispatch and all other jobs unchanged. `catalog_alloc` retains its existing regression scope.
5. Freeze both post-repair default/development frames with separate source/run/job/length/digest provenance. Record each reference source lock separately from the current measurement lock. Update the pure data-only consumer without adding Git, constructor or gate installation to it.
6. Reconcile current documentation and per-task executed evidence; record the #47/#72-#75 interface correspondence. Run the unchanged complete hosted matrix and request independent re-review in the original threads. No thread resolution or merge is authorized by this amendment.

## Files And Responsibility Map

Existing paths unless explicitly marked new:

| Responsibility | Files |
| --- | --- |
| Discovery/declaration source | `crates/auctioneer-config/src/bidder_discovery.rs`, `src/build_catalog.rs` |
| Pure selection, input-kind and identity hard cut | `crates/auctioneer-config/src/schema_staging.rs` |
| Source projection, snapshots, generated seed and emitted Rust | `crates/auctioneer-config/src/schema_staging_build.rs`, `src/schema_staging_codegen.rs`; `crates/auctioneer-bidder/build.rs` |
| Actual generated format/program/native lifecycle | `crates/auctioneer-bidder/src/schema/{staging,native}.rs` |
| Independent discovery oracle/current source report | `crates/auctioneer-bidder/tests/schema_selection_source/mod.rs`, `schema_selection/mod.rs`, `schema_source_conformance.rs` |
| Actual OUT_DIR capture and artifact comparison | `crates/auctioneer-bidder/tests/schema_build/application.rs`; existing `schema_build` helpers |
| Expanded owner graph and common descriptor composition | New ordinary modules `crates/auctioneer-bidder/tests/schema_residency/{mod,expanded,initialization}.rs`, declared by the existing `schema_source_conformance` target |
| Reused retained models/explicit layout inputs | `crates/auctioneer-bidder/tests/schema_{program_heap,metadata_heap,nfa_heap,onepass_bounds,literal_heap,pool_heap,heap_layout}/mod.rs` and `schema_resource_layout/mod.rs` |
| Reused constructor/work models | `crates/auctioneer-bidder/tests/schema_{syntax_construction,nfa_construction,onepass_construction}/mod.rs` |
| Matched growth and non-installing arithmetic comparison | Existing `schema_source_conformance` modules `schema_residency/{catalog_reference,growth,metadata_registry,observations}.rs`, `crates/auctioneer-core/tests/catalog_resource_profile.rs`; unchanged `catalog_alloc` regression controls and frozen `crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline{,-patched-lock}.toml` retain their original scope |
| Current evidence consumers, only if their admitted report shape changes | `crates/auctioneer-cli/src/bidders/evidence/conformance/{mod,runner}.rs`, `crates/auctioneer-cli/tests/bidders_evidence.rs` |
| Existing hosted owners | `.github/workflows/test.yml`, `.github/workflows/scripts/test/{verify-schema-source,verify-schema-compiler-conformance,verify-schema-metadata-secure,verify-schema-metadata-development}.sh` |
| Nested reference input versus application source identity | `.gitignore`, `crates/auctioneer-cli/src/resource_identity/mod.rs` and its existing reference-checkout fixture; read the actual build owner `crates/auctioneer-cli/src/build.rs` |
| Current source/proof documentation | `docs/testing/resource-proof-ownership.md`, `docs/bidders/schema-qualification.md` |

Paths following a crate-local `src/` or test-module name above are relative to that table row's named crate. Read `AGENTS.md` and `crates/AGENTS.md` before any implementation. New source files remain small owner modules, not new Cargo targets or a second library. Unit tests stay inline; normal `mod` declarations only. Do not introduce handwritten source imports, a build-script dependency cycle or a dependency on test modules from shipping code.

## Task 1: Freeze The Actual Change And Verification Inventory

**Files:** Read the file map, both approved documents, `Cargo.toml`, `Cargo.lock`, `.tool-versions` and current test workflows. No production edit yet.

- [ ] Inventory every active use of `PriorityWave`, `SupplementalWave`, `SupplementalInventory`, `planned_roots`, `published_roots_all_features` and the selector's planned-root arguments. Include the mandatory input-kind list in `schema_staging::identify`, source conformance, artifact `check_cohort` and omission controls. Do not assume the producer's first reader is the complete migration set.
- [ ] Read back #46 and #72-#75 against the maintainer-approved criteria map and reference amendment in spec sections 7.2/10.1. Verify that each adapter issue explicitly requires whole-current-catalog physical/allocator bounds and actual linked code/data growth evidence; a promise in #46 alone does not transfer acceptance. Preserve the downstream qualification gates and existing dependency direction; do not close #46 against contradictory unchecked criteria or expand it to execute downstream issues.
- [ ] Separate source/provenance inventory inputs from cohort membership inputs. Record how every discovered implemented definition resolves its effective schema/witness; keep independent revision evidence without selecting by a wave file.
- [ ] Inventory native tested reservations, guest refusal branches and current accounting owners. Reuse their behavioral controls as invariants; do not add Rust snapshots of source text or workflow YAML.
- [ ] Record hosted test listings and the exact current selections described in the validation map below. In particular, `schema_source_conformance` has `test = false` and does not execute in broad all-target runs.
- [ ] Record the current source/model premises for each required regex/format/registry owner. A missing common model is assigned to Tasks 4-5; a missing physical target fact remains an explicit #72-#75 input, not an invented zero.
- [ ] Freeze the intended commit/checkpoint boundaries and focused selectors before implementation. Preserve all original assertion populations even if retired-cohort test names or argument shapes need correction.

**Expected result:** An implementation inventory with no duplicate cycle or new tracking issue. Not a completed C/I certificate.

## Task 2: Replace Cohort Authority With Discovery

**Files:** Modify `bidder_discovery.rs`, `schema_staging.rs`, `schema_staging_build.rs`, `schema_selection_source/mod.rs`, `schema_selection/mod.rs`, `schema_build/application.rs` and root conformance consumers as needed by the inventory.

- [ ] Add synthetic controls with literal expected public-definition/schema-owner sets. Include: a newly declared executable bidder; its inherited alias; an overriding alias; a schema-free local bidder; an operator-disabled definition; an implemented native-only definition; a feature-gated implementation; and a source-only reference identity sharing an implementation reference.
- [ ] Prove that the operator flag/native publication restriction does not remove implemented proof membership, while an actually absent security feature does. Prove no source-only identity becomes implemented merely by sharing a reference/factory name.
- [ ] Add missing-factory/wiring/schema, alias-cycle, duplicate, extra-owner and false-skip controls. Assert exact before-state membership and nonzero row counts, not only absence after filtering.
- [ ] Execute the behavioral RED through the current selector/build path. For tests initially calling its old signature, later change only the call adaptation, not the expected sets or failure assertions.
- [ ] Derive implemented roots from discovered executable declarations before the architecture publication filter. Replace the public/planned union with one implemented-root input; remove the planned-root API and active wave parser/constants rather than leaving an optional compatibility path.
- [ ] Keep provenance verification from bidder-owned manifests and existing source witnesses. Remove mandatory wave input kinds and wave-derived identity fields; use source-witness identities only where actual provenance requires them.
- [ ] Hard-cut the active seed/source-report format to version 2. Replace cohort root fields with explicit implemented roots, effective owners and current callable-lane correspondence. Reject old active versions/fields; do not silently deserialize them into empty/default lists. Preserve frozen historical evidence bytes.
- [ ] Update all current shape/identity/omission consumers together, including `schema_staging::identify`, artifact checks and the two CLI-driven source-membership tests. Keep the producer's source-only scope: those two tests do not suddenly certify C/I, semantics or serving.
- [ ] Retain an independent real-catalog oracle that reads validated factory/definition/witness declarations and follows alias ownership without consuming production-selected roots or calling the production selector for its expected result. Synthetic cases use literal expected sets. Do not build another generic discovery framework.
- [ ] Record hosted GREEN and commit the coherent selection/consumer cutover. Native serving publication, reservation values and guest refusals must remain unchanged.

**Expected result:** Every inventory row has exactly one disposition; implemented definitions cannot be hidden behind a wave or an unimplemented skip. The current source report is discovery-based, but not yet a complete C/I report.

## Task 3: Capture And Expand The Real Build Seed

**Files:** Modify `schema_staging_build.rs`, `schema_staging_codegen.rs`, `schema_build/application.rs`; create `schema_residency/mod.rs` and `expanded.rs`; declare them normally in `schema_source_conformance.rs`.

- [ ] Add failing seed controls for declaration/schema mapping, output identity, snapshot substitution, unknown syntax, wrong callback/options and missing/duplicate macro declarations. Include a fully accepted positive seed so blanket refusal fails.
- [ ] Extend the version-2 build seed with the exact declaration-to-schema map and its witnessed source identities. Keep the seed explicitly unexpanded/source-incomplete: `quote_validators` emits macro declarations, not internal helpers.
- [ ] Reuse `schema_build::application::verify_staging`, which already reads the real `schema-staging.rs`, verifies the emitted-source digest and checks snapshots. Return a private conformance-owned captured seed alongside its existing evidence; do not rebuild the app, expose validators publicly or reconstruct the source from a schema list.
- [ ] Preserve bounded regular-file/contained-path handling. Capture one accepted private snapshot set; the compiler may reopen those same verified snapshots through its existing path API. Never expand by rereading the bidder checkout or claim one physical I/O read.
- [ ] Parse captured declarations with syn. For each actual validator attribute and marker, invoke the existing patched `jsonschema_macros_core::expand` dev dependency with its actual tokens; strip only the invoking attribute from the item as required. Reject multiple/unknown validator attributes, compile_error output and unaccounted declarations.
- [ ] Collect the expanded graph with existing AST helpers where applicable. Identify each helper by its real declaration namespace plus emitted identifier/ordinal; retain translated pattern, constructor flags, callback path, singleton/lazy cell and initialization references. Reject unknown emitted shapes rather than skipping them.
- [ ] Verify the semantic option/callback tuple independently against the production contract. Keep equal patterns in different namespaces distinct and compiler-proved intra-declaration deduplication explicit.
- [ ] Bind the graph to the actual seed/source/schema/expansion/library/feature tuple. Add one-field-at-a-time stale/changed controls, including escaped string values in patterns and a changed translated pattern; normalization cannot erase semantic differences.
- [ ] Execute hosted RED/GREEN and commit. Leave the existing pattern-only BundleProof extractor as a component diagnostic where still useful, but prohibit it from claiming complete graph/descriptor evidence.

**Expected result:** The existing conformance report can demonstrate an exact seed-to-expanded-owner join. The build seed alone still cannot certify hidden owners, guest layouts or serving.

## Task 4: Compose Retained Storage Without Assuming Host Layouts

**Files:** `schema_residency/mod.rs`; reused retained/layout models in the file map; source correspondence in `schema_source_conformance.rs`.

- [ ] Add literal small-graph C controls first: two independently compiled equal-pattern owners count twice; aliases referencing one owner count once; one shared hostname program counts once; missing children, duplicate charge ownership, wrong sharing edges and overflow refuse.
- [ ] Separate structural owner/count/formula information from ABI layout values. Factor arithmetic from native-only model entrypoints into functions taking explicit verified layout facts; keep existing native entrypoints as adapters to the same formulas, not duplicate implementations. Missing target/private payload facts remain required inputs, not host `size_of` substitutions.
- [ ] Reuse metadata/root Arc, NFA/capture trees, one-pass tables, scalar prefilter/literal and unused-pool components only after checking their current pattern/configuration/source premises. Preserve their existing independent native literal expectations and source witnesses.
- [ ] Bind reconstructed models to the actual expanded graph. A component claims only its declared subtree; reject overlapping child coverage, an unaccounted allocation, changed engine topology or an unsupported reachable strategy.
- [ ] Audit non-regex generated validators, callback/lazy-cell storage, retained failure state and registry/descriptor metadata. Record positively proved absence where applicable; no-pattern/no-observation alone is not absence.
- [ ] Keep linked code/data attribution as named target-input groups mapped to existing budget owners. The required attribution set is source-complete only when every generated/static/registry owner is covered. Do not sum token/file lengths or install a physical value.
- [ ] Check arithmetic with owner count, vector capacity, payload/layout size and alias count varied independently. Explicitly remove each tested charge from a candidate graph/report and require the intended completeness/total assertion to fail; a non-rejection-only check is insufficient.
- [ ] Execute hosted RED/GREEN, preserving existing component controls, and commit.

**Expected result:** A retained-owner recipe is complete or explicitly refused. Native requested-layout observations remain observations at their own ABI; target costs/physical qualification remain unavailable.

## Task 5: Complete Constructor Composition And Failure Prefixes

**Files:** Create `schema_residency/initialization.rs`; extend `schema_syntax_construction/mod.rs`, `schema_nfa_construction/mod.rs`, `schema_onepass_construction/mod.rs` and applicable retained models. Read the pinned `vendor/regex-automata/src/meta/{regex,strategy}.rs`, `src/nfa/thompson/compiler.rs` and `src/dfa/onepass.rs`; do not patch them.

- [ ] Write a phase ledger from the actual constructor source before composing its numeric model. Use the table below, adding a source-backed phase if inspection finds another reachable owner. Each phase records incoming live retained owners, constructor-only owners, release/transfer points, failure cleanup and source-work terms.
- [ ] Reuse existing `SyntaxConstructionCertificate`, `NfaConstructionCertificate::from_hir_with_layouts` and `OnePassConstructionCertificate::from_nfa_with_layouts` premises and owner rows. Adapt explicit layout inputs as needed; do not reuse their exclusions as if they were complete meta-constructor bounds.
- [ ] Derive construction-only charges directly from lifetime/ownership transitions. Avoid subtracting a measured retained value or adding whole component peaks that each include the same HIR/NFA/backing allocation.
- [ ] Compose serial scratch with a maximum only after proving the actual release boundaries. Include old/new vector backing, shrink/remap copies, abandoned candidate storage and source-reachable failure cleanup. Any unbounded common phase refuses the descriptor and keeps #46 incomplete.
- [ ] Compose constructor work separately with declared source-visit units and exclusions. Preserve separate counters when units differ; do not reinterpret these as runtime validation tokens, CPU instructions or a wall-time bound.
- [ ] Add cold/repeat/prefix/late-failure controls with literal independent lifetime traces. Independently vary previous retained storage, current scratch, failed-prefix storage, growth and instance count; exact peak assertions prevent a slack upper-bound inequality from hiding a deleted prefix.
- [ ] Exercise only source-supported grammar/strategy paths in the real selected graph. Positively prove unreachable optional backends instead of expanding the proof to every imported regex engine.
- [ ] Record hosted RED/GREEN and commit the complete constructor composition, not merely a list of missing models.

| Phase | Required Composition |
| --- | --- |
| Parse and HIR translation | Parser/AST/translation/drop owners and growth; HIR that survives into later compiler phases is carried forward. |
| Metadata and automatic prefilter | Metadata/Arc/property construction, literal extraction and only the selected reachable prefilter candidates; prove absence before excluding a branch. |
| Forward NFA and wrappers | Syntax/HIR overlap, NFA builder/conversion backing, Arc transfer, Pike/backtracker wrapper construction and failure cleanup. Search-time lazy caches remain #47 unless actually initialized here. |
| One-pass candidate | Live forward NFA plus construction tables/queues/maps/shrink overlap; candidate failure and retained tables have distinct ownership. |
| Reverse NFA and hybrid/directional wrappers | Live forward/one-pass owners, reverse construction and configured wrapper initialization; exclude a backend only with its actual source/configuration proof. |
| Final strategy/lazy-cell publication | Existing metadata/NFAs/prefilter sharing, final Arc/wrapper bookkeeping, obsolete temporary release and cached success/failure storage. |

Use this independent arithmetic fixture in the new pure conformance controls:

```text
Final retained owners: fake_first = 3 bytes, fake_second = 7 bytes.
C_requested = 10.
Success traces: first construction peak 3 + 5 = 8;
                second peak 3 + 7 + 11 = 21.
Late-failure trace: 3 retained + 6 partial-current + 13 scratch = 22.
I_requested = max(5, 11, 13) = 13; C_requested + I_requested = 23.
```

These are synthetic owner units, not real engine reservations. Assert each trace's exact peak and each typed total independently. Removing the failed-prefix contribution must yield an incorrect I, and removing the earlier retained owner must yield an incorrect trace/C. Vary those inputs separately; do not rely on the final inequality alone. Actual source capacities/counts, not this illustrative fixture, determine the real descriptor.

## Task 6: Bind Actual Initialization And Lifecycle Observations

**Files:** `crates/auctioneer-bidder/src/schema/{native,staging}.rs`; existing generated initialization in `schema_staging_build.rs`; `schema_residency/initialization.rs`; existing meter/build helpers.

- [ ] Verify the actual ordering: required shared formats, generated owner hooks, nested LazyLock cells and the outer native OnceLock. Distinguish callback visits from first program construction; no-pattern callbacks cannot stand in for lazy program coverage.
- [ ] Add original-funding/native-boundary assertions before any cold construction. Underfunded or staging-only calls must still refuse before touching the process-global initialization cell.
- [ ] Use fresh isolated processes/instances for cold/cached-failure cases and retain existing allocation-window positive controls. A production-used private coordinator helper may accept fresh cells/constructor callbacks for inline lifecycle tests if necessary; it must preserve the current production sequence/cache semantics and expose no public failure API.
- [ ] Use real bounded constructor calls and retained allocations in positive lifecycle controls. Prove a recoverable failed construction allocates/releases its prefix where such a source path exists. A model-only injected failure/no-op does not establish that production cleanup path, and an aborting allocator is not a recoverable OOM seam.
- [ ] Cover repeated success, actual already-live owner prefix, late failure/cleanup, repeated cached failure and concurrent callers under the actual coordinator. Reconcile real singleton initialization witnesses with the expanded graph.
- [ ] If a specific recoverable failure is unreachable under the fixed emitted source/options, document that exact reachability argument and retain source-level failure-prefix proof plus a genuinely allocating supported library failure control. Do not claim the admitted production path executed that failure. Any proposed relaxation of the spec's required lifecycle acceptance returns to review.
- [ ] Observe constructor peaks and retained bytes separately; compare with independently derived source bounds, not an expectation computed from the same observed peak. Repeat success must not construct another retained program. Native requested allocations do not supply guest physical facts.
- [ ] Execute hosted RED/GREEN, current native reservation/funding controls and existing source/application probes; commit without changing policy values or guest availability.

**Expected result:** The descriptor's schedule/sharing premises correspond to actual generated/native construction. No new portable concurrency guarantee is inferred from one native OnceLock.

## Task 7: Complete The Source Descriptor And Refusal Contract

**Files:** `schema_residency/mod.rs`, current `schema_selection_source/mod.rs` report and `schema_build/application.rs`; current evidence readers only when the version-2 migration inventory requires them.

- [ ] Implement one strict report producer/consumer contract with these distinct states: build seed only; source-incomplete with named common owners; source-complete recipe with exhaustive target input slots; requested-storage evaluation for a matching supplied ABI. None is a physical/serving-qualified state.
- [ ] Attach discovered membership, expanded owner graph, sharing/subtree coverage, component/formula identities, constructor schedule/work units, target layout/linkage requirements and complete source/options/features witnesses. Keep partial rows for diagnosis, but refuse complete totals when any required common owner is missing.
- [ ] Require exact set equality and nonzero execution across every discovered implemented identity/alias and required owner; report full-catalog not-implemented rows separately. Reject duplicate/extra/stale rows and attempts to skip an implemented but unproved identity.
- [ ] Add table-driven negative controls independently changing each identity/option/layout requirement/status. Include build-only seeds claiming graph completeness, wrong schema version, incomplete states claiming a numeric cost, and a source-only report claiming fit/serving qualification.
- [ ] Keep missing ABI/layout/linkage/allocator facts distinct from a missing common model. Neither can become a default/zero cost. A complete recipe must enumerate all target facts it needs; an unnamed catch-all missing-evidence bucket cannot discharge a common owner.
- [ ] Deliver spec section 7.3's reviewed interface handoff to #47 and #72-#75. Reconcile immutable/program/init C/I owners, per-validation caches/scratch, search-time fallback and per-call hostname cache by the same source/helper identities and explicit units/lifetimes. Test a missing or duplicate child assignment; do not install admission or wait for downstream qualification as part of this handoff.
- [ ] Keep `CatalogResourceCosts`, production adapter profiles, native reservation constants, workload/headroom and runtime budget arithmetic unchanged. Do not introduce a new CLI resource command or make build.rs consume a passing conformance result.
- [ ] Reconcile unchanged admitted schema outcomes and source/provenance controls. Record all source-completeness failures honestly; do not close #46 if only diagnostic rows or an inventory of remaining common proofs were delivered.
- [ ] Record hosted GREEN and commit the complete descriptor integration.

## Task 8: Measure Matched Catalog Growth Without Rewriting History

**Files:** Existing constructor/source-report owner with ordinary `schema_residency/{catalog_reference,growth,metadata_registry,observations}.rs` modules, version-2 seed writer from Tasks 2-3, pure `catalog_resource_profile.rs` and current proof documentation. `catalog_alloc.rs` remains an unchanged regression owner, not a reference reader. Reference data comes from the pinned official Git tree below. Read, but do not rewrite, the two frozen baseline captures and `docs/testing/evidence/catalog-baseline-continuity-20261021.md`; their existing consumer at `crates/auctioneer-cli/src/bidders/resources/records/catalog_continuity.rs` retains its historical scope.

- [ ] Freeze official Auctioneer commit `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`, tree `2d54c6f53ef3d062ca9292449a4546bb76e5ddd8`, as the maintainer-approved replacement reference. Task 9's hosted `Check out pinned catalog-growth reference` step supplies `AUCTIONEER__TEST__CATALOG_REFERENCE_DIR`; require it at runtime in real growth cases. Verify that directory's pinned HEAD, commit/tree and every consumed regular-file Git object using `--no-replace-objects` before seed construction. Tests do not acquire references over the network or read mutable checkout files; reject partial/promisor repositories and never clone/fetch/lazy-fetch or fall back to the current worktree, private archive or empty seed.
- [ ] Read the pinned tree as isolated data, not executable historical code. Record every consumed path/blob/length/SHA-256; include bidder manifests, executable declaration/source-presence and Cargo feature witnesses, workspace manifest/lock, non-cohort provenance inventories and all referenced schema inputs. Use the Tasks 2-3 discovery/version-2 writer and actual declaration expansion to construct the reference seed; require exact definition/alias/effective-schema/expanded-owner correspondence. No old wave selector, version-1 reader or reference build script is executed. Missing input, unknown model shape or source substitution refuses rather than synthesizing a reference.
- [ ] Freeze candidate input identities and the declared reference-to-candidate owner-set delta before observing either side. Use the same measurement compiler/library/generator/options/ABI/features/profile and unchanged workload. Record each source snapshot's lock separately from the measurement lock and label the comparison `same-build data-only source reconstruction relative to d26fcf7`. It does not recreate historical physical evidence or measure cumulative pre-catalog growth.
- [ ] For future bidder changes affecting manifests or schema/program owners, follow section 7.2.2's maintenance procedure in the affected bidder-change PR under [#11](https://github.com/IABTechLab/auctioneer/issues/11)/[#25](https://github.com/IABTechLab/auctioneer/issues/25), with the #46 schema/compiler maintainers. Inventory differences independently and freeze the expected delta before observations. Review any necessary reference/capture update and new memory-cost evidence within that PR; only design a nonzero comparator when needed. No separate approval workflow, automatic retarget, weakened comparison or current pin/delta change is authorized.
- [ ] Add controls independently changing reference commit/tree/input bytes, features, workload and a non-catalog term; each incompatible change refuses. A declared added schema/program owner must produce the independent expected delta; an undeclared missing/extra/substituted owner refuses. Unequal metadata/registry inputs, including an added metadata owner, always refuse under section 7.2.2. Include an accepted identical-seed comparison with zero growth but positive absolute C/I, proving that growth is not the absolute cost. Preserve both historical capture digests and their original lock/ABI/source checks.
- [ ] Add offline reference-input controls for a missing/empty variable, absent/wrong directory, wrong HEAD/tree, partial/promisor repository and missing required object, plus an accepted complete pinned reference. Check the intended reason and no matched-growth credit; no ignored/conditional/zero-test success. Keep pure fixture cases independent of the variable. The reader performs local Git-object operations only and must not attempt a network recovery.
- [ ] Reuse fresh constructor lifetimes and requested-allocation windows in the existing source-conformance owner. Capture the explicit equal-input metadata/registry proxy and schema retained/cold/repeat/failed initialization separately; record generated-source size as source-generation observation only. Require exact equality with compiled metadata inputs and refuse any unequal metadata/registry comparison. Preserve allocator positive controls and independent scope assertions, not a threshold chosen after measuring.
- [ ] Reconcile per-component matched deltas without offsetting catalog growth with unrelated savings. Derive absolute C/I over the entire current closure independently, including costs introduced before d26fcf7; zero/negative delta never removes an owner or charge. The old pre-import captures remain history, not a dependency on unavailable source or a zero-storage model. An unverifiable seed or comparison emits unmatched-growth refusal and keeps this criterion incomplete.
- [ ] Feed captured descriptors/observations as data to the pure `catalog_resource_profile` arithmetic checks; it must not install gates, initialize global programs or execute constructors. Label hypothetical/source-scoped arithmetic separately from usable target costs and reject any attempt to instantiate physical `CatalogResourceCosts` from incomplete source-only evidence. Observe constructors only in their existing isolated owners and reuse existing child builds.
- [ ] Check the immutable arithmetic inputs against their declared recorded reference/measurement pins, not the current workspace lock. Independently vary two recorded measurement contexts across both feature lanes and reject each altered source/measurement lock plus a coherently altered measurement pair, preserving validated before-states, all 54 arithmetic cases and both no-receipt refusals. Leave current-lock freshness and source/model compatibility to the existing compiler/schema live-growth owner; an unrelated dependency update does not require rewriting these fixtures.
- [ ] Publish new dated evidence binding both source snapshots/seeds, the declared owner delta, measurement identities, absolute and delta observations, cold/repeat/failure scope and nonphysical limitations. Read back the explicit physical/allocator and actual linked code/data growth criteria in #72-#75. Those owners require matched verifiable reference/candidate artifacts or explicit continuity; this data-only reconstruction cannot discharge their physical rows. All full-current-catalog target costs remain due, regardless of growth since d26fcf7.
- [ ] Record hosted RED/GREEN in the existing catalog/source/core owners and commit. Do not mark #46 complete without both the common descriptors and this matched-growth evidence.

## Task 9: Preserve Once-Owned CI And Publish Honest Evidence

**Files:** Existing workflow/scripts in the map, `.gitignore` and CLI `src/resource_identity/mod.rs`; `docs/testing/resource-proof-ownership.md`, `docs/bidders/schema-qualification.md`; current CLI conformance readers/tests if required.

- [ ] Keep the two existing CLI source-membership test names stable or update their exact dispatch list and every owning assertion together. Their producer still credits source membership only; it cannot borrow a separate deep-conformance pass to claim C/I qualification.
- [ ] Before adding the reference checkout, add the exact anchored `/vendor/catalog-growth-reference/` ignore entry. Extend `resource_identity::tests::workflow_reference_checkouts_are_ignored_without_hiding_other_sources` beyond its current Prebid-only repository filter to exercise the newly declared pathed data checkout too. Derive its path from the existing workflow declaration, require a nonempty/distinct checkout set, and retain the real temporary nested-repository controls: reference content changes leave the application snapshot unchanged, tracked/untracked owned vendor sources still change it, and `vendor/unknown-reference` still refuses with `source_identity_nonfile`. No broad vendor exclusion or source-snapshot bypass; the growth reader separately validates the ignored reference's pinned objects. This inline CLI case remains executed by the existing CLI suite, with no new workflow-body snapshot gate or test target.
- [ ] In `.github/workflows/test.yml`, add `AUCTIONEER__TEST__CATALOG_REFERENCE_DIR: ${{ github.workspace }}/vendor/catalog-growth-reference` to the existing `compiler-conformance` job environment only. Add `Check out pinned catalog-growth reference` before consuming tests, using `actions/checkout@v7` with `repository: IABTechLab/auctioneer`, `ref: d26fcf7f566db6d1dce2e1847d79c9f44581edb8`, `path: vendor/catalog-growth-reference`, `fetch-depth: 1`, `persist-credentials: false` and condition exactly `matrix.lane == 'schema'`. Remove the unused `test`-matrix checkout and reference variable under the approved amendment. Keep the PR's normal checkout unchanged. The reference is input data, never another compiled application.
- [ ] Verify the acquisition-to-consumer join: compiler `schema` runs default growth cases through `verify-schema-compiler-conformance.sh schema`, and its existing development-metadata step runs the insecure prefix required below. Both receive the pinned directory before execution. Secure and both workspace lanes continue running unchanged `catalog_alloc` regression controls without a reference checkout. Do not make production/build scripts, pure core arithmetic, selected artifact probes or CLI source-membership producers require this reference, and do not infer availability merely from the environment string. Preserve all existing jobs, exact dispatch checks and once-owned child builds.
- [ ] If an affected bidder-change PR approves a future reference update under section 7.2.2, reconcile reader commit/tree/census, exact CI checkout and current reference expectations together; retain refusal/dispatch controls and publish uniquely named captures in both feature lanes. Preserve old captures/contexts and absolute whole-current source C/I, including pre-pin costs; #72-#75 linked-growth/continuity obligations and all limits remain unchanged. Do not rewrite historical arithmetic captures for unrelated live-lock changes.
- [ ] Default-feature pure `schema_residency` controls execute in the existing compiler-schema job's explicit `schema_source_conformance` invocation. Add the insecure-feature filtered invocation to the existing `verify-schema-metadata-development.sh` step; no new job/matrix is needed. Run it with `AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT` unset, reusing that step's already-built normal insecure test binary, and retain both existing staged metadata checks unchanged.
- [ ] In that development step, list the intended `schema_residency` cases under the exact insecure feature arguments and require a nonempty, duplicate-free set. List all tests for the same binary/feature graph, derive the exact unselected count, and validate the run using `check-libtest-result.sh` with one report, that filtered count and the selected positive pass count; require each selected test's exact successful result once. A zero-test filter, missing result, ignore, failure, wrong feature graph or substituted selection refuses. Do not use `verify-schema-source.sh` for this prefix unchanged: it adds `--exact` to filters other than `schema_selection`.
- [ ] Extend the four existing actual-application tests to capture/re-expand their already-built declarations; reuse the same product/snapshot lifetime rather than adding another app build. Native/guest source/layout probes retain their existing explicit owners.
- [ ] Keep the existing `schema_selection` dispatch guard in secure/default/insecure lanes. Before changing its expected 34 count, list the exact intended old/new names under each feature graph on CI and reconcile every case. Update all three workflow call sites if the intended set changes; do not set the count to an incomplete passing run's observed value.
- [ ] Preserve the compiler script's six intentional heavy exclusions and their separate mandatory owners, plus full strict lint. In addition to the newly required insecure-prefix invocation above, adapt existing exact selectors only where a renamed case requires it; no extra matrix, workflow-body Rust gate, new target, timeout increase or cached-test result.
- [ ] Reuse existing hosted negative controls and independent fixtures. Any additional source-code mutation executions require an approved hosted batch; retain exact mutated/restored commits, intended test names and actual failure output. Unrun controls remain unrun, not inferred kills.
- [ ] Run the complete hosted matrix at the final implementation head and inspect full logs/result lines, not only green badges. Record exact toolchain/dependency/features/source/artifact identities and any ignores/refusals.
- [ ] Update current proof documentation with the delivered common source scope and remaining physical/#47/#48/guest/host boundaries. Add new dated evidence where appropriate; do not repin or rewrite historical captures, thresholds or old qualified outputs.
- [ ] Complete advisory self-review and independent PR review, preserving original threads for REVIEWER. Merge/close #46 only after current-head full CI, accepted findings, eligible approval/explicit account exception and maintainer merge authorization.

## Validation Map

Commands below describe **future hosted execution**, not local commands run for this plan. Tool/source setup comes from current workflow/development owners. An implementation handoff records each actual command, commit, actor, exit code and verbatim result lines.

| Owner | Required Checks |
| --- | --- |
| config build support | Inline discovery/staging/codegen/identity cases, including version-2 migration and source-witness completeness. |
| compiler conformance schema job | Default-feature expanded-graph/C/I/formula/refusal cases, existing component models and strict explicit-target Clippy. Real growth cases receive Task 9's pinned reference checkout through `AUCTIONEER__TEST__CATALOG_REFERENCE_DIR`; pure fixtures do not need it. |
| hosted reference setup in `compiler-conformance` schema only | `Check out pinned catalog-growth reference` supplies the exact official commit at `vendor/catalog-growth-reference` before both growth consumers. Tests verify HEAD/tree/objects offline and fail on absent/unmatched inputs; they never fetch their own source. No new job/matrix or non-consuming checkout. |
| existing CLI source-identity unit owner | Extended nested-reference fixture exercises the new checkout's exact ignore entry without hiding tracked/untracked application vendor sources or accepting an unknown nested repository. Source snapshot integrity remains unchanged; this is not a replacement workflow-content gate. |
| existing development metadata step, `verify-schema-metadata-development.sh` | New insecure-feature `schema_residency` prefix run with exact positive dispatch/result-set verification; both original normal/staged metadata cases remain mandatory. This is a planned extension, not a run already present at the baseline. |
| schema source selected-schema/private-layout jobs | Existing real source probes and constructor/layout correspondence, without promoting those observations to target fit. |
| four application-artifact jobs | Same actual built outputs, captured declarations/snapshots, source identity and layout/artifact controls across both security feature lanes. |
| secure/default/insecure source-selection steps | Exact intended membership/omission population and no hidden implementation skip. |
| CLI/evidence-producer owners | Original two real source-membership dispatches and both mandatory fresh evidence producers; no new qualification credit. |
| existing catalog/source/core owners | `catalog_alloc` retains unchanged regression controls; source growth alone gets the reference in compiler schema/default+development. Metadata/registry comparison is the explicit equal-input proxy, with unsupported nonzero deltas refusing. Matched observations retain original baseline bytes. Pure `catalog_resource_profile` consumes captured data, not Git/environment/network input, and remains non-installing. |
| full matrix | Existing workspace/secure/insecure/conformance, compiler, producer, guest, provider packaging/size and strict format/lint jobs, all required. |

Focused Cargo commands for existing hosted owners and the explicitly planned development-step extension:

```sh
cargo test --locked -p auctioneer-config --features build-support --lib
cargo test --locked -p auctioneer-bidder --lib
cargo test --locked -p auctioneer-bidder --test native_schema_resources
cargo test --locked -p auctioneer-bidder --test schema_source_conformance schema_selection -- --test-threads=1
cargo test --locked -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance schema_selection -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test schema_source_conformance schema_residency -- --test-threads=1
cargo test --locked -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance schema_residency -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test catalog_alloc
cargo test --locked -p auctioneer-core --test catalog_resource_profile
cargo test --locked -p auctioneer-cli --test bidders_evidence -- \
  --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification \
  --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
```

List and count the `schema_residency` cases on the exact hosted feature graph before crediting either selector; a planned command is not executed evidence. Real reference-growth cases in `schema_source_conformance` require Task 9's hosted checkout and runtime directory variable; unchanged `catalog_alloc` regression cases do not. A focused command does not acquire the reference itself or become a local-execution instruction. Other focused targets supplement, not replace, full CI. The focused CLI command excludes only the two heavy producers that execute through the existing mandatory `.github/workflows/scripts/test/verify-evidence-producers.sh` owner, with exact-test/nonzero dispatch validation and fresh processes. Do not repeat those child builds in each feedback lane or omit their once-owned final runs.

The final acceptance invocations are the existing complete workflows:

- `.github/workflows/test.yml`, including `verify-schema-compiler-conformance.sh schema`, the extended existing development metadata step, schema-source probe matrix, four application artifact owners, explicit source-selection checks and both evidence producers. Source scripts remain colocated with this workflow.
- `.github/workflows/format.yml`, with every existing strict feature/lint lane.
- `.github/workflows/ci-fastly.yml`, `ci-cloudflare.yml` and `ci-spin.yml`, preserving their current executed checks and packaging/size gates.

For each current-head run, retrieve jobs and complete relevant logs using the GitHub run/job IDs returned for that head. Record real nonzero dispatched counts and refusal classifications. Successful compilation, a shallow layout report, pending/ignored jobs or another artifact's measurements cannot discharge source-complete/target-fit acceptance.

## Falsification Checklist

- [ ] Added implemented identity/alias without a wave edit is included; reference-only identity is not falsely implemented.
- [ ] Removing a required member/helper, altering a witness or treating a false skip as valid fails exact coverage.
- [ ] Replacing captured attributes/options/callbacks with a similar BundleProof expansion fails correspondence.
- [ ] Equal-pattern independent singleton ownership and shared aliases produce different, literal expected C totals.
- [ ] Removing a retained/temporary/growth/failure contribution fails its independent total or exact lifetime trace.
- [ ] Supplying host layouts for an unmatched guest/source tuple or missing a required slot refuses.
- [ ] Source-only/incomplete evidence cannot become a numeric physical cost, fit result or serving permission.
- [ ] Native tested reservations, original funding/refusal semantics and actual schema outcomes remain unchanged.
- [ ] The insecure-feature controls actually execute under their owned step; zero or substituted dispatch fails.
- [ ] A changed/missing reference, unmatched ABI/features/source, undeclared owner delta or hidden non-catalog change cannot claim matched catalog growth; an accepted zero-growth comparison still has positive independently accounted absolute storage.
- [ ] Hosted setup supplies the reference to every real growth owner; missing/unmatched inputs fail executed checks, with no network fetch, current-checkout fallback, ignore or zero-row success in tests.

These controls need actual executed evidence. A self-referential constant comparison, a zero-test filter, an all-refusing scaffold or a comment asserting that a branch is unreachable does not check a box.

## Planning Handoff

- [x] Maintainer approved the written spec at `ba383e2`; no implementation approval was implied.
- [ ] Advisory whole-plan/spec review of the latest revision completes and substantive findings are addressed. Earlier approval does not cover later revisions.
- [x] Local document/source/path/whitespace checks complete, with no local Cargo/CI execution.
- [ ] Commit/push only the spec and this plan under the maintainer's planning-publication authorization; open one non-draft `PRfection phase: plan` PR with `Refs #46`.
- [ ] Read back its head/base/open/non-draft state, document links and full current-head check state. Clearly distinguish pending CI from passing baseline evidence.
- [ ] Hand off to independent REVIEWER and the maintainer. Do not implement, resolve reviewer threads, close #46 or merge on the strength of this planning document.
