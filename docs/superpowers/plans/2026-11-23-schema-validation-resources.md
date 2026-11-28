# Shared Schema Validation Bounds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL after the amendment approval gate: Use superpowers:subagent-driven-development or superpowers:executing-plans for the ordered implementation tasks. Unit tests remain inline; conformance stays in the existing targets.

**Maintainer amendment, 2026-11-28:** Pause implementation. This existing plan and the [existing specification](../specs/2026-11-22-schema-validation-resources-design.md) are the only current contract for #47 / PR #268. Source-prove Auctioneer-owned code and composition; consume reviewed, identity-bound third-party premises instead of deriving their internals. Commit the documentation amendment first, obtain REVIEWER planning review and explicit maintainer approval of the numeric premise packet, then resume implementation. No reviewed numeric engine constants exist yet.

**Goal:** Close the five common derivations under that explicit trust boundary, keep `CommonSourceComplete` as the final gate, and hard-cut replaced derivers/report paths without compatibility routes.

**Architecture:** Reuse the sealed residency handoff, existing generated-body `syn` recognizer, own-code/caller source proofs, component bounds, canonical report and hosted owners. Compose source-derived call/input/lifetime populations with reviewed dependency effects. Keep hypothetical data, premise approval, common completeness and actual target qualification distinct.

**Tech Stack:** Existing Rust workspace, `syn`/`quote`, checked typed arithmetic, `serde_json`, existing provenance/component and isolated allocation/artifact owners. No new dependency, crate, runtime setting, job, target, source checkout or capture/build plan.

## Contract And Execution Gate

**Maintainer amendment, 2026-11-28:** Issue [#47](https://github.com/IABTechLab/auctioneer/issues/47), epic [#9](https://github.com/IABTechLab/auctioneer/issues/9), merged planning PR #267, implementation PR #268. The original basis `8baf0a5e71bf685d3e8ae755d4adcbe0d2329525` and prior checkpoint evidence remain history, not an instruction to continue retired derivations. Read [development ownership](../../testing/development.md) and [crate rules](../../../crates/AGENTS.md).

This amendment overrides the prior tasks that required private regex/std/serializer/formatter cost parsing. Preserve the existing componentwise shape decision, input/workload/reservation/token/engine/compiler limits and physical/target ownership. Unknown own-code behavior or a missing/unapproved common numeric premise remains `Incomplete`; do not move it to #72-#75. Every dependency stack boundary uses a named #72-#75 whole-operation target slot, not an alternative private-frame derivation. Declared target layout/allocator/clock/entropy facts remain target requirements, not zero defaults.

All commands below are future commands with expected, not executed, results. No local Cargo/compiler/test/Clippy/proof/mutation/CI execution is authorized. Pinned rustfmt remains editing only. Existing GitHub owners run RED/GREEN and the full unchanged matrix after the relevant publication approval. Keep this PR draft until all five derivations close; do not weaken any final assertion or infer implementation approval from the documentation commit.

## File And Test Ownership

**Maintainer amendment, 2026-11-28:** `T` below means `crates/auctioneer-bidder/tests/`. Keep one owner per contribution. Add no second specification or test target.

| Path | Current responsibility / amended disposition |
| --- | --- |
| `T/schema_validation/{mod,report}.rs` | Canonical consumer/status/report, actual captured-source integration and unchanged completion/retirement gates. |
| `T/schema_validation/handoff/{mod.rs,source/mod.rs}` | Keep same-capture handoff and own-code source windows; delete internal-cost-only vendor/std rosters and consumers when their premise replacements land. |
| `T/schema_validation/public_names.rs` | Keep exact executable-name/alias/schema-free disposition. |
| `T/schema_validation/operations/{mod,model,lower,resolve,source_syntax}.rs` | Keep the generated/own-code recognizer and Recipe. Map known dependency calls to approved premises; do not interpret their private bodies. |
| `T/schema_validation/operations/premises.rs` if a focused child module is needed | One private reviewed-packet/identity/applicability owner with inline controls, not another source deriver, registry or runtime config. |
| `T/schema_validation/work/{mod,context,caller,partition}.rs` | Own-code operation/caller/attempt bounds, provenance, search multiplicities and checked premise composition. |
| `T/schema_validation/work/prefix/mod.rs` | Retain own input-prefix/Counter protocol and refusal proofs; replace dependency costs with premises. |
| `T/schema_validation/work/prefix/scalar/{mod,fixture}.rs` | Retire third-party source interpretation. Preserve/move the own `json_size.rs`/Counter checks and necessary independent inputs to their sole retained owner. |
| `T/schema_validation/work/entropy.rs` | Replace private constructor/std/platform interpretation with reviewed seed multiplicity/service records and source-derived own-call composition. |
| `T/schema_validation/work/epsilon_closure/**` | Delete out-of-scope private engine/std cost parsers, event tables, import/prelude/derive machinery and their obsolete callers. No alternate path remains. |
| `T/schema_validation/storage/{mod,source}.rs` | Source-proved own wrapper/partial/callback/release lifetimes and disjoint composition of existing reviewed component bounds. |
| `T/schema_validation/stack/{mod,source}.rs` | Own logical call/depth/cleanup composition; explicit required #72-#75 whole-operation stack slots for every dependency boundary. |
| `T/schema_validation/admission.rs` | Source-and-premise affine coefficients/Q and unchanged policy correspondence; no new budget. |
| `T/schema_validation/observations.rs` | Preserve existing independent verdict/release/allocation corroboration; it cannot supply constants or completeness. |
| `T/schema_residency/{descriptor,expanded,growth}.rs` | Keep the sealed C/I view and same immutable current capture; do not duplicate or retire C/I. |
| Existing `schema_cache_model`, `schema_meta_heap`, `schema_bundle_heap`, `schema_hybrid_capacity`, `schema_backtrack_bounds`, `schema_onepass_bounds` | Reuse reviewed storage/component rules and retain all their independent assertions; no new engine-total heap request. |
| `T/schema_source_conformance.rs`, `T/native_schema_resources.rs` | Keep actual report/application/allocator joins, process isolation and final assertions in their existing owners. |
| Existing workflow/scripts and documentation owners | Preserve every job, target, feature lane, six heavy exclusions and list/dispatch checks; only migrate retired control names to their actual premise replacements. |

Use ordinary modules, inline unit tests, `quote!`/`syn` and valid `json!` fixtures. No handwritten source imports, public test-only serving API or duplicated expected-value helpers. Retain historical evidence/capture bytes.

## Task 0: Commit And Review This Amendment Before Code

**Maintainer amendment, 2026-11-28:** This is the immediate task. No implementation deletion or premise fallback is authorized before its gate.

- [ ] Preserve uncommitted implementation work outside the proposed commit and verify only the two existing Markdown documents differ. Do not commit an abandoned internal-cost extension alongside this amendment.
- [ ] Amend the spec/plan in place with the dated note in every changed section. The numeric review table is spec §7.1; std/other boundaries are §7.2; the five completion criteria are §9. No values are guessed or inferred from O(m*n).
- [ ] Record the maintainer decision on #47 and update PR #268 Scope. State actual implementation deletions as zero at this checkpoint, the documentation delta, and the separately labelled retirement candidate census below.
- [ ] Commit/push only these two documents as the amendment commit in the same PR. Ask REVIEWER to review the planning change, including each `Incomplete` numeric row, unchanged obligations and the hard-cut inventory. Keep #47 open and #268 draft.
- [ ] REVIEWER challenges the proposed engine P1/P2 inequalities, certificate-based exclusions, whole-cache rather than auxiliary-only envelopes, seed/probe bounds and remaining std/serializer/UUID gaps. The maintainer approves the exact reviewed numeric packet before implementation resumes. A proposed inequality is not approval; missing map intermediate-work, serializer or UUID work premises keep the packet `Incomplete`. No missing primitive or unsigned review field can be filled at implementer discretion or selected from an observation.

### Retirement Census At The Amendment Basis

**Maintainer amendment, 2026-11-28:** Read-only census at `678c575cc3929298287f9e48ca8a7e322a39c03a`; these are gross source lines, not already removed lines or a promised net reduction.

| Candidate area | Files | Current lines | Required treatment after approval |
| --- | --- | --- | --- |
| `work/epsilon_closure/**` | 7 | 6,807 | Remove the private engine/std derivation implementation and obsolete controls/imports; preserve required component/regression ownership outside this retired path. |
| `work/prefix/scalar/{mod,fixture}.rs` | 2 | 2,974 | Retire dependency interpretation; retain/move our Counter/input/source invariants and their independent assertions. |
| `work/entropy.rs` | 1 | 2,436 | Replace internal-cost/seed-source interpretation with the reviewed premise and own-call/service composition. |
| Core candidate subtotal | 10 | 12,217 | Gross replacement area, not an executed deletion or net forecast. |
| Mixed `work/prefix/mod.rs` and three `handoff/source` files | 4 | 3,332 | Keep own-code proofs/window and replace only internal-deriver rosters, imports, controls and calls. |

After implementation, report actual `git diff --numstat`, deleted paths, replacement additions and assertion-ownership mapping. Never count this whole mixed area as deleted or claim all candidate lines are unnecessary own-code coverage.

## Task 1: Freeze Retained Obligations And Premise Identity

**Maintainer amendment, 2026-11-28:** Retain the earlier baseline/evidence in its dated record; do not copy its private dependency derivations into the new contract.

- [ ] Inventory every active generated/own-code root, dependency boundary, component owner, old internal deriver/report caller and required hosted case. Derive membership from discovery, not bidder names/cohorts. Keep the recorded 18 public names, 16 schema owners, five programs and 51 children as independent baselines, not configuration limits.
- [ ] Preserve exact binding to the one live immutable capture and the independent public-name/alias/schema-free census. Keep distinct caller-context roots and all own-source deadline, borrowed-parameter, cap, error/survivor and cleanup obligations.
- [ ] Bind the engine packet to version 0.4.19, the full reviewed patch digest from the existing verifier, vendor/archive identity, resolved features, generated/compiler options, existence/earliest mode and exact caps. Bind std/other records to their existing compiler/lock/feature/ABI/service identity owners. Do not add a resolver, checkout or capture.
- [ ] Classify each old assertion as retained unchanged, moved unchanged to its own-code owner, or exclusively testing a deleted internal deriver and replaced by a specific premise refusal control. Keep the coverage map reviewable; no silent test or assertion deletion, alias, ignored case or old-source fallback.

### Retained Canonical-Report Retirement Inventory

**Maintainer amendment, 2026-11-28:** Keep this prior inventory explicit. Confirm every actual caller at cutover and add any newly found one; the dependency-premise amendment does not retire these obligations.

| Superseded boundary | Canonical replacement / deletion step | Required control or retained owner |
| --- | --- | --- |
| `tests::assert_selected_cache_heap`, its caller in `selected_schema_probe_links_and_executes_the_source_bound_cohort`, field `native_cache_heap_candidate`, kind `conditional_native_schema_cache_heap_candidate` | Task 8 adopts the canonical `schema_validation` report and separately labelled observations, then deletes the former assembly/function/field/kind from active output. | Canonical positive; old/hybrid field-set refusals; active-symbol AST inventory finds no old function, call or writer. Archival bytes stay unchanged. |
| `schema_validation::admission::check_current_report_fields` and its three-field `conditional_source_shape` format | Task 8 deletes the alternate checker/accepted format. Preserve its named retirement controls and independent arithmetic through the canonical checker, not a translator or alias. | Canonical positive; superseded-kind, retired-candidate, hybrid and heuristic-fallback refusals; no remaining checker declaration/reference. Pure hypothetical arithmetic remains non-qualifying. |
| `tests::selected_cache_bundle`, called by the superseded assembly | Task 5 makes the common storage owner compose the existing components; Task 8 deletes this duplicate assembler. | Preserve independent lifetime arithmetic and actual helper/cache correspondence; no second assembly is called. |
| Manifest-only enumeration in `selected_half_search_certificates_cover_five_actual_owners_in_both_lanes` | Task 8 obtains qualification-facing identities from the same descriptor graph. | Preserve low-level search-certificate/replay controls, exact nonempty graph equality and independent omitted/misowned-program refusal. |
| Other `selected_regex_patterns` callers | Retain component/layout/observation callers under their existing non-qualification scope; remove only the replaced qualification-facing call. | Those callers cannot return a common descriptor/cost and are not globally deleted as legacy. |
| Current Selected Validation-Cache Heap guidance in `docs/bidders/schema-qualification.md` and Schema V row in `docs/testing/resource-proof-ownership.md` | Task 10 migrates current guidance to the canonical report and its limits. Date historical descriptions rather than present a deleted output as current. | No active documentation dependency on the retired kind; original evidence bytes remain archival and non-qualifying. |
| Native heuristic/reservation route and exchange `mandatory`/`funded_legacy` dispatch | No serving replacement in #47. #72-#75/#88-#90 own atomic qualified cutover/deletion, not an in-process compatibility bridge here. | No second serving route, fallback, activation receipt/flag or conversion; downstream adoption proves old-route deletion. |

## Task 2: Keep Handoff And Own-Code Recognition

**Maintainer amendment, 2026-11-28:** Do not redesign the working handoff or source recognizer.

- [ ] Reuse `ValidationView`, the exact handoff children, captured `Recipe`, independent public-name census and current-only descriptor path. Preserve missing/extra/duplicate/misowned/stale source and alias controls and outer closing checks, including returned interpretation errors.
- [ ] Keep source proofs for generated validator bodies; `schema/{native,mod,input}.rs`; `auctioneer-runtime/src/json_size.rs`; own format/dispatch glue; and the exchange caller. Keep recursion/depth, input provenance and unchanged borrowing proofs. Use `syn`, not raw-string Rust matching or file-size costs.
- [ ] Map only identified dependency calls to the approved operation/profile. Reject unrecognized own code, changed call arguments/search mode, unresolved scope or a missing applicable premise. A generic "third party" label cannot bless an unknown call.
- [ ] Narrow source rosters/consumers only where the internal derivation is removed. Continue complete before/after/compiler/seed checks for the retained own-code window; replace dependency body parsing with the existing version/patch/features/cap premise identity checks. Do not retain the abandoned ten-runtime-file availability requirement as another internal-cost path.

## Task 3: Freeze Premises And Publish Tests-First Controls

**Maintainer amendment, 2026-11-28:** Begin only after Task 0's planning review and maintainer numeric approval. The spec's `Incomplete` rows are a review gate, not executable placeholders.

- [ ] Bind every descriptor program to its exact existing half-search/capacity/ID certificate and source/cap/options identities. Freeze certified-direction P1 work `u*(1+2*S_p+E_p)`, `u=n+2`, `E_p=256*S_p+R_p`, and P2 constructor/drop proposals `2*F_p` / `2*L_p` only after review and maintainer approval. F must cover the complete fresh cache, not only `eager_bytes`; using the whole-history L for F is conservative. Retain the reviewed component owners and their original parameters rather than create another heap model.
- [ ] Represent clear/rebuild, give-up/fallback, PikeVM, backtracker, one-pass and prefilter search as certificate/applicability exclusions, not missing numeric factors or feature disables. Preserve the inactive owners' construction/storage/drop. Bind the ReverseAnchored no-prefilter fact to the actual ordinary half-search, anchored primary reverse route and no-prefilter reverse builder. A new/missing/refused certificate, changed strategy, incomplete/quit graph, reset/clone/rebind or new reachable matcher refuses the packet, including zero invocations; no fallback coefficients or deriver are selected.
- [ ] Use the fewest existing units: search in `search_transitions`, constructor/drop in `construction_visits`/`cleanup_visits`, and separately owned state-map probes in existing `map_visits`. Keep our fourteen-unit source ledger and explicit covered-by mapping; do not spread one opaque effect across all fields, double-charge covered allocator events or infer unproved zeroes. Distinguish final drop from intermediate search-time cleanup.
- [ ] Reuse reviewed component storage envelopes from the spec. Add a row only for a specific uncovered component/transient. Keep actual target layout/physical/allocator facts separate. A hybrid/pool cap is not an engine-wide or process-wide heap total.
- [ ] Review the declaration-level seed proposal: at most two `RandomState::new` entries per complete fresh certified meta cache, including the inactive primary direction; source-prove our constructor/callback/partial-prefix populations separately. Preserve the whole E service, E/H separation, transient overlap and thread/process retention. No warm-thread or portable finite OS-retry bound is assumed.
- [ ] Review probe-only `2*u*K_p`, using the existing owning state-map envelope's bucket bound, not just graph entries. Expose an already justified logical dimension through that owner if needed; do not copy its layout derivation. Resolve cumulative growth/rehash, key hashing/comparison, relocation, intermediate cleanup and search-time allocation requests in an explicitly owned premise before approval. A peak-byte or probe-only form cannot silently stand in for all these effects. Constructor/final-drop allocator request counts are covered by P2, not added again.
- [ ] Resolve the named `serializer_work_contract_missing` and `uuid_work_contract_missing` gaps through justified numeric per-input premises and review, not dependency-source interpretation. Preserve the actual Value-only Counter path, reached versus admitted serialization dimensions and numeric formatting coverage. Preserve our n-byte uppercase scan, at most one UUID parser call and 16-byte output buffer; invalid-length input is still passed to the parser, so 36 normal bytes cannot bound every call. Functional output/call dimensions are not a whole-library work certificate. Unsupported factors stay `Incomplete` under the maintainer's rule; do not resume implementation until the complete numeric packet is approved.
- [ ] In existing inline test owners, start with a complete synthetic positive packet. Independently alter premise identity, patch/features/review binding, each cap/search mode and each required constant. Test missing/duplicate/extra rows, zero-workload refusal, covered-by duplication, unit mismatch and overflow. Keep synthetic facts unable to approve the actual capture.
- [ ] Publish the authorized tests-first checkpoint in #268. Obtain completed hosted assertion RED, not compile/setup failure. Then implement the sole packet/applicability/refusal path and obtain GREEN for these controls. No new job, target or mutation PR batch; no implementation-selected or sample-fitted values.

## Task 4: Compose Own-Code Work And All Attempts

**Maintainer amendment, 2026-11-28:** Replace internal engine/std event interpretation, not our operation/caller derivations.

- [ ] Keep the canonical generated-body Recipe, formal/actual bindings, disjoint input-tree partitions, object key/value distinctions, branch/loop/search/callback multiplicities and independent source-mutation/overflow controls. Hard-cut the superseded coarse counter when the canonical replacement is adopted; no compatibility fallback.
- [ ] Derive own input-walk and Counter protocol work, cumulative raw-string/key precheck bounds, rejected prefixes, traversal depth and all attempted populations from our source. Serializer internals use their approved premise. Never charge a rejected raw operand as if the precheck admitted it, or silently change B/J/D.
- [ ] Compose source-bound search n/call counts with the approved active-direction numeric contribution and separate map effects. Apply the complete non-quitting certificate before using excluded-path contributions; any unmatched fallback refuses, not becomes an unchecked maximum or missing zero. Construction/drop populations follow actual generated cache and callback calls, not a workload observation.
- [ ] Derive `P_i(A; existing caps)` for every reached attempt, including unavailability, input rejection, deadline and exhausted budget, plus local returned-error/partial/unwind cleanup outside V. Preserve actual zero debit before admission; no later parse-budget funding is inferred.
- [ ] Source-prove the existing expiry invocation prefixes and compute seed invocations from own construction/callback populations times the reviewed dependency multiplicity. Compose H/E bodies separately with their reviewed identity/retained-state slots; do not parse std/platform implementations or assume warm threads.
- [ ] Obtain hosted RED/GREEN for independent B/J/D/branch/search/attempt axes, zero-admission work, late failure/cleanup and overflow. Preserve the caller/source and independent arithmetic oracles; coefficients must precede new corroborating observations.

### Retained Own-Code Cases And Independent Oracles

**Maintainer amendment, 2026-11-28:** These prior prescriptions remain required even where their implementation is unfinished. Keep their exact case names in the existing required development roster, execute every retained assertion, and use the same original complete `Recipe` and shared input/search provenance/counting kernel. This section does not retain dependency-body interpretation or grant conditional DATA sealed source authority.

- [ ] Preserve `work::tests::{dimensions_vary_independently,expiry_prefixes_bound_real_call_sites,attempts_before_admission_are_charged,branches_and_callbacks_have_source_derived_multiplicity,late_failure_includes_cleanup,overflow_refuses}` and `work::prefix::tests::{rejected_preflight_prefixes_remain_charged,keys_and_string_prechecks_bound_partial_prefixes,iterator_population_and_recursion_axes_remain_distinct,counter_protocol_source_drift_and_overflow_refuse}`. Retain literal own-code node/key/byte/depth/string/Counter controls, independent source-AST mutations and reached/refused preflight prefixes. Raw-plus-quotes covers operands reaching serialization, including a failed final byte charge, not rejected precheck operands. Serializer/navigation/formatter internals now require reviewed premises; do not delete the surrounding Counter/input invariants with the retired scalar interpreter. Retain the independent local-work example: three attempts at five units and one admitted body at eleven total **26**, before H/E bodies.
- [ ] Preserve `work::partition::tests::{disjoint_children_across_helpers_have_linear_coefficients,ancestor_rescans_require_source_bound_cap_lifting,changed_iteration_and_call_bindings_are_refused}`. Borrowed child traversal permits disjoint J; ancestor rescans require checked cap lifting. Keep fixture cap seven separate from observed J=4, independently change the cap to eleven, retain literal coefficients and zero-attempt drift/overflow refusals. Prior hosted RED `102bf2f` and GREEN `66540da` establish only the tested unary boundary. Extend the canonical kernel, not a compatibility counter; retire `recipe_counts` atomically when complete actual generated integration replaces it.
- [ ] Preserve `work::partition::tests::{parameterized_calls_preserve_instance_and_cache_bindings,object_entry_values_partition_across_parameterized_helpers,object_ancestor_rescans_require_key_funded_cap_lifting,cache_and_object_binder_drift_are_independently_refused}` and the original three cases. One call is one charge despite multiple argument bindings; keys never become Value children. Keep object witness J=11 versus cap17, and ancestor witness J=7 versus independent caps13/19 with literal leaf/callback node coefficients `(6,7)` and `(9,10)`. Reject independent cache/formal/actual/key/value/lexical/iterator drift at zero and one attempts.
- [ ] Preserve `work::partition::coupled_tests::input_tests::{coupled_input_keeps_factory_and_validation_populations_separate,object_pass_locals_require_exact_source_bindings,coupled_input_source_checks_and_overflow_survive_zero_validations}`. Obtain the validation root from the original checked factory/owner foundation, never a cloned/filtered Recipe or erased initializer map. With F=3 and V=2 retain five root entries, six two-program readiness probes and three constructors per program; only input traversal scales with V. Preserve the seven traversal and three factory contracts, exact emitted Bool-local bindings, V=0/V=1 drift refusals and per-invocation overflow before zero multiplication. Buffer results through the same whole-roster source closing checks/census without another capture/build.
- [ ] Keep the emitted protocols needed after whole-roster RED `f3af018`: scalar Value payloads preserve node provenance and exact selector/type/lexical bindings; literal `Or` alternatives bind nothing; borrowed key strings retain initializer lineage and remain distinct from Values; labelled `Iterator::next/for` breaks/exhaustion and `Map::values` preserve their element/cardinality contracts. The String-plus-cache helper checks formal/actual/cache/static-program and immutable Input binding. `Option::is_some_and` executes at most once per enclosing occurrence and creates no input-member axis. Preserve all thirteen earlier partition cases and their independent before-states; these syntax protocols are own generated-code recognition, not third-party cost derivation.
- [ ] Preserve `input_tests::{emitted_protocols_keep_scalar_and_optional_callback_coefficients,emitted_protocol_bindings_refuse_at_zero_validations}`: non-binding wildcard type unions and key-only `Map::keys` never manufacture Value children. Keep the two-entry J=5 witness with independent caps7/11; direct String/sentinel and key-only node coefficient one; ancestor-rescan String coefficients three/five and iterator-callback coefficients four/six. Optional callbacks are not JSON iterators or zero-cost by assertion. Independently corrupt cache field, earliest flag, String formal, regex binder and label at zero/one validations. Every still-unproved own primitive/control/storage/stack/cleanup duty remains required; only dependency effects change to reviewed premises.
- [ ] Preserve `input_tests::erased_wrappers_change_only_expression_addresses`: match canonical `Paren`/`Group` descent only for expression slots with the same function/prefix followed by zero address components; never relax formal/local/iterator/Option binder anchors. Retain parenthesized positives and all thirteen earlier oracles. An emitted Scalar cast visits exactly its one checked Scalar operand once and yields a temporary Scalar; independently refuse a nonzero child suffix at zero/one validation. Width, scalar semantics, source/control/frame and ordinary-work requirements remain explicit.
- [ ] Preserve `input_tests::search_tests::{search_sites_bind_programs_and_independent_populations,repeated_helpers_and_distinct_sites_preserve_search_multiplicity,search_refusals_and_overflow_survive_zero_workload}`. Share the input kernel, retain independent factory F and validation V, cache-field/program ownership, unused zero-search programs and a no-regex positive. Direct/rescanned caps7/11 keep literal search coefficients one/three/five and V=2 ceilings **14/22/42/110**; changing only F from three to nine cannot change search counts. Repeated helper calls accumulate at one site while eight distinct sites remain eight rows. Independently change engine entry name and primitive role at V=0/V=1; check per-program and aggregate overflow before zero multiplication, with input-only arithmetic still passing where only search aggregation overflows. Remove its tests-first missing-search result when complete, not keep it as fallback; buffer actual whole-roster results through existing closing checks.
- [ ] Preserve `work::caller::tests::{original_impressions_follow_both_core_admission_representations,bidder_cap_records_clamp_then_nonzero_effective_cap,scope_gates_do_not_discharge_retention_or_cleanup,missing_and_rehashed_caller_sources_refuse_at_zero_attempts}`. Derive both core-admission representations and host-resolution/clamp/run expressions, including existing `max(cap,1)` and scoped-node construction. Vary policy/resource operands independently; prove the original scoped-parameter borrow through schema admission rather than guess a policy number. Missing/rehashed own sources refuse at zero attempts too; identity JSON does not manufacture a sealed caller proof.

## Task 5: Close Storage And Lifetime Composition

**Maintainer amendment, 2026-11-28:** Source-prove our schedule; trust the exact reviewed component/premise effects at the dependency boundary.

- [ ] Keep generated wrapper construction/field ordering, all completed-sibling/partial prefixes, use-before-drop, synchronous hostname overlap, normal/returned-error/unwind release and serial non-escape/reset/clone/rebind controls from the existing source recognizer.
- [ ] Consume existing per-program component envelopes through descriptor-owned identity and original parameter/layout interfaces, not a copied engine-total formula. Cover growth/relocation, inactive eager/reverse caches, callback temporaries and supported failure/drop. An uncovered component is an explicit missing-premise refusal.
- [ ] Preserve explicit fallback covered-by attribution. Keep immutable program/C/I costs separate. Compute checked peak maxima over sums of disjoint simultaneous owners; work remains cumulative across serial callbacks and evaluations.
- [ ] Compose entropy transient overlap and host thread/process retention once under bound populations/domains. Do not multiply retained state by seed calls or treat pool capacity as a process/thread limit.
- [ ] Retain independent literal storage examples, all existing broad component and actual allocator release/positive controls, source-lifetime drift and arithmetic-overflow checks. Native observations do not approve a premise or establish physical fit.

## Task 6: Close Own Stack Composition And Name Target Slots

**Maintainer amendment, 2026-11-28:** Do not derive private engine/std frame trees or treat unclassified own-code edges as target facts.

- [ ] Retain the Recipe/source-derived own call paths, argument-before-callee order, schema/instance depth distinction, callback overlap, normal/error/partial/unwind cleanup and checked simultaneous-versus-exclusive arithmetic.
- [ ] At **every** engine, std/RandomState/HashMap, allocator, serializer/formatter and UUID/SIMD construct/search/callback/drop boundary, declare the required #72-#75 **whole-operation stack slot**. There is no numeric internal-stack alternative to design or derive. Name the operation/phase, premise identity, compiler/artifact/profile/features/ABI/panic tuple, normal/error/unwind coverage and overlap with our source-proved frames. Missing numeric target facts refuse actual evaluation, never become zero; missing own paths or unnamed dependency slots prevent common composition.
- [ ] Keep UUID inline-buffer and host expiry/entropy temporary/retained/frame requirements separate from heap-stack records and allocator physical mapping. Retain allocation success and terminal OOM-diagnostics exclusion without erasing ordinary cleanup.
- [ ] Preserve independent path sum/max/depth/overflow, missing-fact and ABI/panic controls. Unknown own-code recursion or an unnamed dependency boundary prevents common completeness; a declared unavailable target-byte slot is separately target-incomplete.

## Task 7: Close Shape Correspondence Without Policy Changes

**Maintainer amendment, 2026-11-28:** Keep the original equations and numerical-policy boundary.

- [ ] Source-bind the unchanged `T(B,J) = 1024 + 16*B + 128*J`. Compose own source counts and approved numeric premise terms into each component/declaration `a + b*B + c*J`, including explicit existing-cap lifting and coverage witnesses. Missing/unapproved K/A/D values refuse; no defaults or measured coefficients.
- [ ] Compute `Q_i,k = max(ceil(a/1024),ceil(b/16),ceil(c/128))` and checked declaration maxima, not chosen weights. Preserve independent coefficient/ratio boundaries, overflow and all existing no-invoke/no-debit/debit-first/no-refund behavior tests.
- [ ] Keep `P_i + sum(V_i,k) + N_clock*H_i + N_seed*E_i <= P_i + Q_i*sum(T) + N_clock*H_i + N_seed*E_i`, with disjoint local/service attribution. Preserve `numeric_policy_adequacy = not_evaluated`, unestablished attempted-work funding, all token/scratch limits and the separate #72-#75 acceptance gates.

## Task 8: Hard-Cut Derivers And Complete The Canonical Report

**Maintainer amendment, 2026-11-28:** Deletions and replacement land in this PR, not a second implementation or deferred cleanup.

- [ ] Delete the inventoried out-of-scope epsilon/private helper/prelude/derive machinery and vendor/std parsing in prefix/entropy. Remove old imports, readers/rosters, event tables and active callers. Retain/move own-source proofs and map retired internal controls to the actual premise controls; no alias, disabled alternate, fallback or translator.
- [ ] Keep the handoff join, public-name/alias disposition, generated recognizer, canonical report and retirement of its prior native/selected candidate, plus existing observations and development dispatch. Do not recreate a legacy reader/writer to reject it.
- [ ] The canonical report must expose the own-source/reviewed-dependency trust boundary and exact approved premise identity. Require all five derivations, complete child/context coverage, and accepted numeric common premises before `CommonSourceComplete`. All hypothetical/native-only data remain non-qualifying.
- [ ] Preserve the final `CommonSourceComplete` assertions in the report case, current-body case and every actual application attachment. Do not turn them into `Incomplete` expectations, delete them or hide them behind a condition. A status-only change does not close a proof.
- [ ] Integrate immediately within the existing actual capture/application and isolated allocator lifetimes. No additional artifact builds, captures, public serving API or source imports. Keep qualification/physical-fit/serving permission false and target/policy/funding requirements explicit.

## Task 9: Preserve Existing Hosted Execution Ownership

**Maintainer amendment, 2026-11-28:** No workflow scheduling, job, target, feature-mode, timeout or six-heavy-exclusion change is authorized by this amendment.

- [ ] Keep the explicit source-conformance/compiler-control and strict-lint owners; `test = false` still requires their existing targeted commands. Preserve development's complete `--list` census, separate residency/validation selections and exact normal/staged coverage, with zero V growth frames versus one residency growth frame.
- [ ] For deleted internal-case names only, migrate the minimum required roster to the corresponding real premise/refusal controls and publish the mapping. Preserve nonempty/unique/listed-selected-executed equality, exact filtered remainder, one result summary, no ignored/failed case, and every existing checker refusal assertion. Do not retain old-case aliases or weaken the census.
- [ ] Run authorized hosted tests-first RED, focused GREEN and the complete unchanged acceptance matrix at the fixing head. A compile error is not assertion RED. A skipped/success-gated stage, zero-test result or external service failure is not a pass. Do not remove checks or change assertions/limits to obtain GREEN.

## Task 10: Record Completion And Target Handoff

**Maintainer amendment, 2026-11-28:** Use the following checklist rather than treating premise identity or this amendment as delivery.

| Derivation | Remaining closure requirement |
| --- | --- |
| Storage | Own fresh/partial/callback/release schedule plus complete reviewed component coverage and disjoint/covered-by/entropy lifetime composition. |
| Work | Own source/Recipe/caller counts plus approved numeric engine/std/other effects and complete failure/cleanup/service partition. |
| Structural stack | Complete own paths/depth/cleanup plus explicit required #72-#75 whole-operation dependency stack slots for every call phase. |
| Shape correspondence | Affine source-and-premise coefficients, unchanged policy tuple and calculated checked Q maxima, not weights or samples. |
| Attempted work | All reached preflight/refusal/coordination/cleanup populations and applicable premise terms, including zero admitted debit, separate from H/E and funding. |

- [ ] Record all premise review references, numeric values, version/patch/features/caps binding, units, component coverage, own source proofs, exclusions and required target facts. Keep missing common rows `Incomplete` and actual target policy/funding/allocator/host facts explicitly nonpassing where absent.
- [ ] Record exact hosted commits, commands, exits, result lines, dispatch populations and final full matrix. Preserve earlier evidence bytes; append new dated results through the existing evidence owner, not another spec or invented historical recertification.
- [ ] Publish actual deleted files/lines, replacement additions/net delta and retained-assertion/roster ownership mapping on #47 and PR Scope. Until the deletion commit, report zero implementation lines removed and identify the candidate census as planned only.
- [ ] Provide #72-#75 the exact artifact/layout/stack/allocator/clock/entropy and numerical policy/funding requirements, retaining Fastly's separate stack domain. Qualified activation and atomic native-heuristic retirement remain with #88-#90; no route, config or workload change is authorized here.
- [ ] Only when all five common derivations and mandatory hosted checks pass, remove draft status and request implementation review. #47 remains open until its reviewed delivery; merge requires separate current-head approval/authorization. The amended planning review is not implementation approval or a CI waiver.

## Future Verification Commands And Expected Evidence

**Maintainer amendment, 2026-11-28:** Existing hosted owners remain authoritative. These commands identify the existing scope, not completed execution for this documentation-only commit and not permission to run Cargo locally.

```bash
cargo test --locked -p auctioneer-bidder --test schema_source_conformance schema_validation:: -- --test-threads=1
cargo test --locked -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance schema_validation:: -- --test-threads=1
cargo clippy --locked -p auctioneer-bidder --test schema_source_conformance -- -D warnings
cargo clippy --locked -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance -- -D warnings
cargo test --locked -p auctioneer-bidder --test native_schema_resources
cargo test --locked -p auctioneer-bidder --test test_isolation
cargo test --locked -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core --all-targets
bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh schema
bash .github/workflows/scripts/test/verify-schema-metadata-development.sh
```

Expected assertion RED is a completed hosted failure on missing behavior, not a compile/setup error. Expected GREEN is exit 0 with all exact intended cases executed, no failure/ignore and unchanged full owners. Capture real results; no predicted pass counts, observation-fitted constants, test-result cache or self-referential expectations.

## Planning Review And Publication

**Maintainer amendment, 2026-11-28:** Commit 1 modifies only these two existing documents in PR #268. Verify whitespace, relative links, source/config/patch references, spec/plan consistency and the retired-versus-retained inventory with read-only document inspection. Product suites are not local validation for a documentation-only amendment; GitHub's existing checks may still run and their existing implementation failures remain real failures.

Record the decision on #47 and request REVIEWER planning review of the amendment and the deliberately `Incomplete` numeric rows. Require explicit maintainer approval of the reviewed numeric table before implementation resumes. Keep #268 draft; do not create another spec/plan/PR, resolve threads, close #47, waive checks, or merge this incomplete implementation. Report the five remaining closure items and an assumption-qualified work estimate, with numeric-premise approval latency separate from engineering effort.
