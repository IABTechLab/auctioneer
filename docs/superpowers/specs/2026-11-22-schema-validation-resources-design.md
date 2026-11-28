# Shared Schema Validation Bounds

**Maintainer amendment, 2026-11-28:** This is the single current contract for #47 and PR #268. Pause derivation implementation. Prove Auctioneer-owned code from source and treat third-party internals as explicitly reviewed premises. This amendment and its numeric premise table require planning review by REVIEWER and maintainer approval before implementation resumes. No reviewed numeric engine constants currently exist; the `Incomplete` entries below are deliberate review questions, not zeroes or implementation discretion. The 2026-11-23 componentwise correspondence decision remains in force under this amended boundary.

Implementation plan: [Shared Schema Validation Bounds](../plans/2026-11-23-schema-validation-resources.md). Issue: [#47](https://github.com/IABTechLab/auctioneer/issues/47), under [#9](https://github.com/IABTechLab/auctioneer/issues/9). Original planning basis: `8baf0a5e71bf685d3e8ae755d4adcbe0d2329525`, including [#262](https://github.com/IABTechLab/auctioneer/pull/262); planning PR #267 is merged. This amendment stays inside implementation PR #268, not a second specification or planning PR. Reuse #46's delivered source interface without claiming its remaining certification is complete.

## 1. Objective

**Maintainer amendment, 2026-11-28:** Complete the common validation-storage, work, structural-stack, shape-correspondence and attempted-work derivations by composing source-proved Auctioneer behavior with version-bound, reviewed dependency premises. `CommonSourceComplete` means complete derivation under that stated trust boundary, not a source proof of dependency internals.

Keep one canonical proof/report owner. Native, Fastly, Cloudflare and hosted Spin artifact binding, physical qualification, numerical policy adequacy and funding remain with [#72](https://github.com/IABTechLab/auctioneer/issues/72)-[#75](https://github.com/IABTechLab/auctioneer/issues/75); qualified activation remains with [#88](https://github.com/IABTechLab/auctioneer/issues/88)-[#90](https://github.com/IABTechLab/auctioneer/issues/90). Those issues do not become reverse prerequisites for completing the common derivation. A missing common derivation or unapproved required numeric premise still blocks #47.

## 2. Scope And Non-Goals

**Maintainer amendment, 2026-11-28:** Retain the existing `syn` recognizer and source proofs for the captured generated validator bodies; `schema/native.rs`, `schema/mod.rs`, `schema/input.rs`; `auctioneer-runtime/src/json_size.rs`; Auctioneer's format/dispatch glue; the exchange caller; and the composition of storage, work, stack and attempts. Preserve the real borrowed-parameter, cap, deadline, invocation, failure, ownership and cleanup relationships in our code. Unknown Auctioneer syntax or an unmapped dependency call remains a refusal.

Do not derive the internal costs, private call graphs or implementation details of regex-automata, std, serde/serde_json, numeric formatters or UUID/SIMD helpers from their source. Their reachable operations need reviewed premise records with explicit identities, units, bounds, coverage and omissions. Inspecting declarations, public documentation, configuration wiring and existing provenance for premise applicability is permitted; building another interpreter for their private bodies is not.

Do not change serving behavior, schemas, formats, input/workload limits, reservations, the work-token policy, original deadlines, engine/compiler patches, dependencies, resource ceilings or the additive memory equation. Do not add a configuration path, crate, test target, CI job or alternate serving route. This is not runtime metering, pooling/reuse, a new matcher or target enablement. Existing observations are corroboration only; no constant may be selected or increased to fit them.

### 2.1. Reviewed Premise Records

**Maintainer amendment, 2026-11-28:** A premise is a reviewed assumption, not a source-derived theorem. Each record must name its operation/profile, public contract, supported input dimensions, existing work unit, numeric coefficients or an expressly permitted required target fact, storage ownership, failure/unwind coverage, review reference and approval, and a frozen identity digest. A hash authenticates the reviewed record; it does not prove the assumption. Synthetic records exercise arithmetic only and cannot authorize the actual capture.

The regex identity includes vendored version **0.4.19**, the full existing patch digest **`5d853f05430eda9b9e80fb1f359cddce730dcf86a61be32efa2605a46702e020`**, the existing verified vendor-tree/archive identity, resolved features, and the exact settings in §5. Bind the compiler patch/options and captured generated call shapes too. Std records bind the captured compiler release/commit, applicable feature/ABI and allocator/service identity; other dependency records bind their locked version/checksum/features through the existing identity owners. Reuse those owners, not another source checkout, capture or package resolver.

Any changed version, patch, feature, cap, call mode, units, coefficient or review identity refuses and requires premise re-review. Missing or unapproved constants refuse even for zero invocations. Do not regenerate a premise from changed source, substitute a source deriver, infer zero, accept an alias to an old packet or reuse a historical approval. An explicit source-proved exclusion is different from an omitted row. Freeze the approved numeric packet before any new corroborating observations; historical measurements do not justify its values.

## 3. Existing Contracts To Reuse

**Maintainer amendment, 2026-11-28:** Keep these owners and their existing independent assertions. Reuse the already reviewed component bounds as the storage premise; do not re-prove their dependency internals or manufacture a replacement total-heap number.

| Owner | Contribution | Boundary |
| --- | --- | --- |
| `schema_residency::descriptor::ActualSourceDescriptor` | Sealed captured graph, programs, exact handoff and binding. | Identity/coverage is not itself a work or stack bound. |
| `schema_cache_model`, `schema_meta_heap`, `schema_bundle_heap` | Existing cache dimensions, disjoint component envelopes, growth overlap, wrapper and hostname composition. | Preserve their parameters, broad envelopes and covered-by attribution. |
| `schema_hybrid_capacity`, `schema_backtrack_bounds`, `schema_onepass_bounds` | Existing capacity and component bounds. | Requested storage is not physical memory or a work coefficient. |
| `schema_selected_half_search` and existing literal-owner controls | Existing finite-call/counter, eligibility and component coverage checks. | Retain them; do not expand them into private engine cost derivations. |
| `schema::measure_schema_input`, `serialized_json_bytes` | Auctioneer's source-proved input walk, Counter protocol and preflight prefixes. | Serializer internals use a reviewed dependency premise. |
| `SchemaWorkBudget::evaluate` | Existing deadline, checked debit, scratch and no-refund behavior. | Do not copy the budget or redefine token units. |
| Existing isolated allocation/artifact owners | Independent verdict, release, provenance and corroboration controls. | Samples and native layouts cannot approve premises or qualify guests. |

Read [native coordination](../../../crates/auctioneer-bidder/src/schema/native.rs), [schema admission](../../../crates/auctioneer-bidder/src/schema/mod.rs), [input measurement](../../../crates/auctioneer-bidder/src/schema/input.rs), [Counter sizing](../../../crates/auctioneer-runtime/src/json_size.rs), [format glue](../../../crates/auctioneer-bidder/src/schema/staging.rs), [dispatch generation](../../../crates/auctioneer-config/src/schema_staging_build.rs) and [exchange admission](../../../crates/auctioneer-exchange/src/run/schema.rs) before composing their effects.

The recorded baseline is 18 public definitions, 16 effective schema owners, five program owners and 51 handoff children. These remain independent coverage controls, not an allowlist, workload limit or substitute for discovery. Preserve imported-source regressions separately.

## 4. One Bound Source Graph

**Maintainer amendment, 2026-11-28:** Keep the existing handoff join, captured generated-body recognizer, public-name/alias disposition and caller-context roots. Consume the same immutable seed/graph/descriptor lifetime, not a rebuilt lookalike. Each handed-off child must have exactly one source-derived contribution, applicable approved premise contribution or source-proved exclusion. Missing, extra, duplicate, misowned or stale-bound contributions refuse.

Prove every executable public name's disposition against independent discovery and actual generated `native_owner` dispatch. Shared aliases share program residency but their invocations remain separate attempts/admitted evaluations. An existing `Schema::LocalSchemaFree` disposition has a positive source-proved exclusion; missing schema-bearing names and zero-regex declarations are not free. Preserve actual public coordinator calls and their independent valid/invalid verdicts without exposing private validators.

Retain existing nested-literal owner and changed-generated-shape controls. A dependency call outside the approved premise's scope refuses; it cannot inherit a nearby row or a generic free-helper marker. Keep separately tagged input-measurement and validation-coordination roots, including `serialized_json_bytes`. Bind own-code source/caller witnesses and the reviewed premise packet to the same common output.

Same-capture identity remains mandatory. Random snapshot roots can affect seed/graph hashes; do not compare independently reconstructed raw hashes as reproducibility keys. Preserve the [existing continuity contract](../../testing/evidence/schema-residency-initialization-20261121.md#per-run-digests-and-source-comparison), historical bytes and current-only reconstruction. No reference reset or legacy reader is introduced.

### 4.1. Hard-Cut Retirement

**Maintainer amendment, 2026-11-28:** In this same PR, delete the out-of-scope vendor/std internal-cost derivations under `tests/schema_validation/work/epsilon_closure/**`, the dependency-parsing portions of `work/prefix/**`, and `work/entropy.rs`. Replace them with premise records, applicability checks and identity/cap/missing-constant refusals. Remove their imports, source rosters, private event tables, core/prelude/derive parsers, conditional helper-cost entrypoints and obsolete callers. No fallback, alias, disabled alternate deriver or source-to-premise translator remains.

Preserve Auctioneer input-prefix/Counter proofs currently mixed into those modules; move or keep their single owner instead of deleting their assertions. Internal-deriver controls that cease to have an implementation must be explicitly inventoried and replaced by premise identity/refusal coverage at cutover, not silently dropped to obtain green. All retained own-code, behavioral, independent component, alias, lifetime, dispatch, release and final-completion assertions remain unchanged. Publish the deletion/migration inventory and actual line delta. Historical evidence remains archival, not an active alternative implementation.

Keep the canonical report and the already required retirement of the old selected/native candidate report. Old/hybrid envelopes, cohort-selected qualification and heuristic fallback refuse. Existing component bounds and independent non-qualification regressions remain reusable; #46's C/I producer is not a legacy path to remove. Later qualified serving adoption must still atomically replace its superseded native heuristic route under #72-#75/#88-#90; no in-process rollback route or compatibility switch is authorized here.

## 5. Input And Invocation Premises

**Maintainer amendment, 2026-11-28:** Source-prove our input dimensions, actual generated traversals, helper/search/callback multiplicities, attempt populations, original deadlines and live-owner schedules. Dependency premises supply internal effects, not guessed caller counts. Keep `B` as compact JSON bytes, `J` as values plus keys, `D` as container depth and `n` as a particular decoded search haystack length. Prove their relationship, including escaped/non-ASCII input, rejected preflight operands, empty containers and unchanged borrowing from the admitted scoped request.

| Existing setting | Frozen value | Owning code / applicability |
| --- | --- | --- |
| Native `B/J/D` | **65,536 / 4,096 / 32** | `schema/native.rs` and `schema/input.rs`; no new clamp. |
| Hostname length | **at most 255 bytes** | `schema/staging.rs` rejects length 256 before search. |
| `nfa_size_limit` | **65,536** | [`Config::nfa_size_limit`](../../../vendor/regex-automata/src/meta/regex.rs#L2944), emitted by `explicit_cache.rs` from `schema_staging_codegen.rs`; hostname sets it directly. |
| `hybrid_cache_capacity` | **65,536** | [`Config::hybrid_cache_capacity`](../../../vendor/regex-automata/src/meta/regex.rs#L3018), same generated/hostname paths. |
| `pool_capacity` | **1** | [`Config::pool_capacity`](../../../vendor/regex-automata/src/meta/regex.rs#L3175), same paths; validation uses explicit fresh caches, not pooled search. |
| Match/search mode | **LeftmostFirst; `earliest(true)`; existence result** | [`Config::match_kind`](../../../vendor/regex-automata/src/meta/regex.rs#L2650), [`Input::earliest`](../../../vendor/regex-automata/src/util/search.rs#L394); generated `search_half_with(...).is_some()` and hostname glue. |

The [compiler patch](../../../vendor/jsonschema-macros-core/src/codegen/explicit_cache.rs) obtains `size_limit`/`dfa_size_limit` from [Auctioneer's generator](../../../crates/auctioneer-config/src/schema_staging_codegen.rs). Prove the actual supplied values and emitted calls, not the compiler's larger default values. A changed cap or non-earliest/iteration/capture API refuses this premise without changing runtime behavior.

Preflight remains outside admitted work. Bound every reached attempt and refused prefix, even with no debit or successful validation. Reconcile populations against actual impression/bidder/resource limits. Preserve the expiry service's existing whole caller binding through `dyn Clock`, `RequestClock` and `MonotonicClock`, shorter failure prefixes and maximum-three-call ordinary path. Its target facts remain separate; no fixture `|| false` or arbitrary callback supplies them.

## 6. Validation Storage And Lifetime

**Maintainer amendment, 2026-11-28:** Source-prove our fresh-per-admitted-call construction, generated wrapper/field ordering, partial-prefix ownership, validation, callback overlap and release before return. Compose the existing reviewed component bounds per program, plus the bound hybrid-cap identity, instead of deriving private cache constructors/destructors from source. Keep immutable programs and initialization in C/I, not a second V charge.

The storage premise is the existing `schema_cache_model`, `meta_heap`, `bundle_heap`, `hybrid_capacity`, `backtrack_bounds` and `onepass_bounds` contributions under their original parameters and caveats. `hybrid_cache_capacity` limits the hybrid-cache domain, not total meta-cache/allocator/stack residency. `pool_capacity = 1` is not a global bound on validating threads or all explicit caches. Keep eager inactive siblings, reverse storage, fallback, growth/relocation, partial construction and ordinary error/unwind coverage. Add a reviewed row only for a concretely identified cache component or transient not covered by those owners; do not request or invent an engine-wide heap total. An uncovered component refuses with `missing_component_storage_premise`.

Take the checked maximum of sums of disjoint simultaneously live owners plus justified overlap. A hostname cache overlaps the live generated wrapper and sibling caches. Serial callbacks/evaluations permit storage maxima only after our source proves non-overlap; their work still accumulates. Preserve fallback covered-by relationships, exact normal/failure release and the UUID decode buffer's inline/stack ownership. Escaping, cloned, reset, rebound or concurrent caches outside this schedule refuse.

RandomState/hash-seed, HashMap growth and allocation effects use §7.2's reviewed premises. Keep transient entropy overlap at its real constructor sites; thread/process retained state is charged once under bound populations and existing domains, never once per seed/cache call. Keep the existing whole `std::hash::RandomState::new` service entry and separate required host facts; no std/platform-body parser or assumed warm-thread/capped-retry guarantee is retained.

## 7. Evaluation Work

**Maintainer amendment, 2026-11-28:** Keep the fourteen existing work units for Auctioneer's source-derived operations. Charge opaque search effects to the existing `search_transitions` unit and fresh-cache construction/drop effects to `construction_visits`/`cleanup_visits`; do not spread one opaque cost across all fourteen fields. Record other internal activity as covered by the reviewed row, not as source-proved zero work. Any separately charged dependency effect needs an explicit non-overlapping owner so growth, entropy, allocation and fallback are not double-counted.

For our code, retain checked sequence, exclusive-branch, traversal, callback, scalar and cleanup derivations using the existing recognizer/Recipe. At an identified dependency boundary, compose the approved premise with source-proved dimensions and call/lifetime counts. Unknown Auctioneer calls, unapproved premises and missing numeric coefficients refuse. Source identity alone, a lookup label, a sample maximum or the asymptotic class does not supply a number.

The engine documents [worst-case O(m*n) searches](../../../vendor/regex-automata/src/lib.rs#L5), with regex/haystack dimensions, and distinguishes [iteration's slower case](../../../vendor/regex-automata/src/lib.rs#L282). Our source proves earliest existence searches, not find-every-match iteration. This supports the intended linear-in-`n` family for a fixed admitted program; it supplies neither a hidden constant nor a conversion into our units. Do not silently equate regex length, state count, encoded NFA bytes and work. Use the existing reviewed NFA/component dimensions to establish the relationship to `N = 65,536`; a missing relationship is `nfa_complexity_relation_not_reviewed`, not one state per byte by assertion.

### 7.1. Numeric Engine Premise Table For Planning Review

**Maintainer amendment, 2026-11-28:** No reviewed constants exist. Draft each row below for schema/compiler-maintainer and REVIEWER challenge, then obtain explicit maintainer approval of the exact numeric packet before implementation resumes. `K_*`, `A_*` and `D_*` are required slots, not accepted symbolic substitutes for numeric coefficients. Each eventual numeric upper bound must cover its whole stated contribution, including zero-byte overhead via `n + 1`, under `n <= B`, the existing NFA relation and caps `N = C = 65,536`, `P = 1`. Values must be sound, need not be tight and cannot be fitted to observations. If justification needs renewed line-by-line internal derivation, retain the named `Incomplete` row and raise it in review instead.

All rows bind to the full §2.1 regex version/patch/features/caps identity. The configuration enables the following possible meta-engine paths; applicability must cover actual captured programs and callbacks, not a bidder/cohort allowlist. Use a source-proved call/API exclusion or reviewed applicability fact for any omitted path; do not assume a helper is absent because one workload did not exercise it. Complete-DFA construction is not enabled by the pinned feature set and cannot be silently added to this packet.

| Path / effect | Existing unit and proposed bound form | Public contract / cap basis | Numeric review status |
| --- | --- | --- | --- |
| Lazy DFA / hybrid search, forward or reverse as applicable | `search_transitions <= K_hybrid * (n + 1)` | Earliest-search O(m*n); [hybrid capacity API](../../../vendor/regex-automata/src/meta/regex.rs#L2987), `C = 65,536`; no constant factor is published. | **Incomplete: `numeric_hybrid_factor_unreviewed`**. |
| Lazy-DFA clearing/rebuild contribution across the whole call | `search_transitions <= K_clear * (n + 1)` | Same contract and capacity; includes all clears/rebuilds, not one clear or a sample count. | **Incomplete: `numeric_clear_factor_unreviewed`**. |
| Give-up/restart/fallback coordination contribution across the whole call | `search_transitions <= K_giveup * (n + 1)` | Meta-engine contract includes restarting with a different engine; must cover failure prefixes and restart multiplicity. | **Incomplete: `numeric_giveup_factor_unreviewed`**. |
| PikeVM fallback | `search_transitions <= K_pikevm * (n + 1)` | Documented engine family; state/edge complexity must be related to the reviewed NFA cap/dimensions. | **Incomplete: `numeric_pikevm_factor_unreviewed`**. |
| Bounded backtracker, including an attempt that gives way to PikeVM | `search_transitions <= K_backtrack * (n + 1)` | Documented bounded-backtracking O(m*n); preserve its existing component/eligibility bounds. | **Incomplete: `numeric_backtrack_factor_unreviewed`**. |
| One-pass path where eligible, including eager cache ownership | `search_transitions <= K_onepass * (n + 1)` | Pinned one-pass feature/API and component bounds; no assumption of universal eligibility. | **Incomplete: `numeric_onepass_factor_unreviewed`**. |
| Reachable prefilter / scalar matcher | `search_transitions <= K_prefilter * (n + 1)` | Existing literal-owner applicability and engine contract; retain new-nested-owner refusal. | **Incomplete: `numeric_prefilter_factor_unreviewed`**. |
| Fresh cache construction, once per program/actual construction invocation | `construction_visits <= A_construct(N,C,P,component dimensions)`; freeze its resulting numeric ceiling | Exact config/cap profile and existing reviewed component bounds; includes eager/partial prefixes and local allocation/growth work, excluding the separately attributed entropy body. | **Incomplete: `numeric_cache_construction_bound_unreviewed`**. |
| Cache drop after construction, search or supported failure/unwind | `cleanup_visits <= D_drop(N,C,P,B_max,component dimensions)`; freeze its resulting numeric ceiling | Existing component envelopes include search-grown fallback owners; the hybrid cap alone does not bound all owners released. | **Incomplete: `numeric_cache_drop_bound_unreviewed`**. |

Compose the whole-call search contribution conservatively by summing approved path contributions, each of which already covers its within-call multiplicity. Do not use a maximum to erase a failed hybrid prefix followed by backtracker/PikeVM work. A tighter exclusive-path composition requires an explicitly reviewed premise; it is not inferred from O(m*n). Construction/drop remain separate invocation terms. Engine-internal stack uses §8, not another source deriver. Do not add a numeric total-heap row: use §6's component composition.

### 7.2. Std And Other Dependency Premises

**Maintainer amendment, 2026-11-28:** These records use the same identity, review, freeze and refusal rules. They are not permission for a general free-library-call escape hatch.

| Boundary | Required stated bound / existing unit | Planning status and disjointness |
| --- | --- | --- |
| Whole `std::hash::RandomState::new` and hash-seed entropy | A reviewed seed-entry ceiling per engine construction profile; whole-service `E_i` work, transient storage, stack and thread/process retention facts supplied by the actual host binding. | **Incomplete: `seed_multiplicity_premise_unreviewed`** until that numeric ceiling is approved. Preserve the existing target-fact treatment of `E_i`: no portable finite OS retry constant or warm-thread zero is claimed. |
| HashMap growth, probing, relocation and drop | Explicit reviewed work/request-count bounds and the existing component/layout growth/storage envelope, in construction/cleanup units or covered by the applicable engine-search row. | **Incomplete: `map_effect_premise_unreviewed`**. State the covered-by owner; do not additionally bill a map effect already included in the whole engine row. |
| Allocator allocation/deallocation calls | **One request event per call** in construction/cleanup units by the existing unit definition; requested bytes and relocation overlap come from the existing reviewed component rules. Actual allocator stack/physical mapping/allocation success remain matched target facts. | Request-event counting is not an allocator CPU/latency bound. Per-cache request populations must be present in the reviewed construction/drop bounds; **Incomplete: `allocation_population_premise_unreviewed`** where absent. |
| Serialization, number/string formatting and UUID/SIMD internals | Reviewed functional interface, per-input work, temporary storage and stack applicability for each reachable call; map onto existing units and keep our Counter, length/precheck and callback schedules source-proved. | **Incomplete: `dependency_primitive_premise_unreviewed`** for any required missing row. Do not resume serde/std/formatter/UUID source-cost interpretation or invent a formatter maximum from a buffer declaration. |

Retain separate expiry `H_i` and entropy `E_i` service identities and lifetime facts. Source-prove our invocations; use reviewed engine/seed multiplicities at the dependency boundary rather than parsing its internal constructors. Thread/process populations and retained state are not per-call multiples. A target-only service/stack/layout fact may remain unevaluated only where this contract explicitly declares that slot; a missing common numeric search/construction/drop bound cannot be relabelled as a target fact.

### 7.3. Shape Correspondence And Attempts

**Maintainer amendment, 2026-11-28:** Keep the equations and production debit unchanged. `T(B,J) = 1024 + 16*B + 128*J`. Compose our source counts and approved numeric premise contributions to derive each component/declaration bound `V_i,k(B,J) <= a_i,k + b_i,k*B + c_i,k*J`. Show every fixed-cap lifting, decoded-byte relation, call count and covered-by attribution. A unit not separately charged under the premise mapping needs an explicit coverage/exclusion witness, not an absent-value default.

Derive `Q_i,k = max(ceil(a_i,k/1024), ceil(b_i,k/16), ceil(c_i,k/128))` using checked arithmetic; the declaration maximum gives `Q_i`. A missing or unapproved required numeric premise prevents coefficients/Q and common completion. Ratios need not be one; they are not new token weights, machine instructions or a CPU/latency certificate.

Keep the separate all-attempt envelope `P_i(A; existing caps)` for preflight, coordination and refused/failure-prefix cleanup, including zero admitted debit. The unchanged whole expression is `P_i + sum(V_i,k) + N_clock*H_i + N_seed*E_i <= P_i + Q_i*sum(T) + N_clock*H_i + N_seed*E_i`. Count each local stage and service body once. The admitted debit limit does not fund `P_i`, expiry or entropy. Keep `numeric_policy_adequacy = not_evaluated` and attempted-work funding `not_established` for #72-#75; looseness is not permission to tune any existing limit or policy.

Validation remains synchronous: deadline checks before/after a call do not interrupt a search. Source-level work units and reviewed dependency premises are not CPU instructions, a latency certificate or a new cooperative-cancellation guarantee.

## 8. Structural Stack And Target Inputs

**Maintainer amendment, 2026-11-28:** Source-prove the logical call nesting, input recursion and error/cleanup paths of our generated bodies, input walk, coordinator and exchange caller. Sum simultaneous frames and take maxima only for proved alternatives/serial lifetimes. Keep instance depth, schema nesting and heap-stack records distinct.

At each dependency construction/search/callback/drop boundary, use either a numeric reviewed internal-stack premise or an explicit #72-#75 required whole-operation stack fact. Bind that slot to the exact premise, operation, compiler/artifact/profile/features/ABI and panic mode. Missing target bytes refuse numeric target evaluation, not become zero; an unknown own-code edge or an unspecified dependency-stack slot still blocks common composition. Do not enumerate private engine/std frame trees or smuggle unproved common work into a target frame label.

Preserve the approved allocation-success premise and terminal allocation-failure/fatal-diagnostics exclusion. Normal returns, returned errors, partial construction and supported ordinary unwind/cleanup remain covered by our schedule and the reviewed dependency effects. No bounded-abort assertion, allocator change or new fatal-handler proof is introduced. Requested bytes, native `size_of`, samples, source AST size and WASM section lengths do not establish physical fit, actual frame bytes or allocation success. Map memory domains once; Fastly's separate stack allowance stays separate.

## 9. Proof Status And Admission Handoff

**Maintainer amendment, 2026-11-28:** Keep one strict canonical report, the existing status names and all final `CommonSourceComplete` assertions. Add the explicit trust boundary and frozen premise/review identity to its current contract; do not add a legacy reader or alternate writer. `Incomplete` remains a diagnostic, never delivery of #47.

| Derivation | What closes it under this boundary | What does not close it |
| --- | --- | --- |
| Storage | Source-proved fresh/partial/callback/release schedule and checked disjoint composition of every existing reviewed component/premise, with fallback covered once and entropy retention/overlap correctly owned. | Cache-cap value alone, missing component coverage, a native observation or an unjoined inventory. |
| Work | Complete own-code operation/caller graph plus approved numeric engine/std/other-operation premise contributions, source-bound inputs/multiplicities, normal/error/cleanup and disjoint service-body terms. | O(m*n) without numeric rows, guessed constants, missing primitive premises or zero defaults. |
| Structural stack | Complete own-code call/depth/cleanup composition with every dependency internal-stack boundary explicitly represented as an approved bound or declared target slot. | Unknown own-code cycles, unnamed dependency stack, heap records or a native stack sample. |
| Shape correspondence | Checked source-and-premise-derived affine coefficients in the retained units, exact unchanged `(1024,16,128)` policy correspondence and computed component/declaration Q maxima. | A symbolic unfilled K/A/D slot, chosen weights, sample-fit coefficients or actual-policy certification. |
| Attempted work | Source-proved complete attempt/refusal populations and borrowed-input/preflight/coordination/cleanup prefixes composed with their applicable approved premises, separate from admitted debit and H/E bodies. | Success/debit counts alone, omitted refused work or a guessed later parse-budget charge. |

All five must close before #268 leaves draft or #47 is complete. The report must still deny resource qualification, physical fit and serving permission and state unestablished numerical policy/funding. Hypothetical evaluation remains test-only. Actual artifact, allocator, host-service, physical/stack and policy/funding acceptance stays with #72-#75 before #88-#90 activation; no missing common requirement is silently reassigned there.

## 10. Failure, Privacy And Integrity

**Maintainer amendment, 2026-11-28:** Refuse source/handoff/premise identity drift, changed caps, unsupported own-code syntax/calls, missing or unapproved constants, incompatible units, incomplete ownership/lifetimes and checked-arithmetic overflow. A self-consistent rehash or changed approval label cannot bless a different packet. Apply these checks before multiplying by zero workloads. Exact zero requires the approved mapping or a source-proved exclusion.

Keep parameters, identifiers, consent and secrets out of proof output. Shared tests use fake owners; bidder-owned provenance and independent verdicts remain. No private validator exposure, cloned caller subtree, cross-crate source import, serving initializer consuming a test result, runtime knob or qualification permission is added.

## 11. Required Independent Controls

**Maintainer amendment, 2026-11-28:** Preserve all retained own-code, behavioral, independent component, handoff/alias, Counter/prefix, lifetime/release, deadline/admission, canonical-retirement, dispatch and final-completion assertions. Replace only controls belonging to deleted internal derivers, with an explicit coverage mapping, not weakened expected results.

1. Retain exact same-capture handoff and public-name/alias/schema-free coverage, positive before-states, all 51 child roles and distinct caller-context roots.
2. Retain generated-body/source mutations for branches, loops, callback identity, traversal origin, parameter borrowing, multiplicity and own-code cleanup. A newly unmapped dependency boundary refuses.
3. Independently reject a changed premise version/patch/features/review identity, a changed cap/search mode and each missing numeric constant. Start with a complete synthetic positive packet and vary one field at a time; reject zero-workload bypass, duplicate/missing/extra rows, source-deriver fallback and old/hybrid packets. Synthetic constants cannot qualify the actual capture.
4. Preserve independent byte/node/depth/branch/search/attempt axes and coefficient under/at/over/overflow controls. Exercise composed numeric premise terms independently; do not calculate expectations from the constants being pinned.
5. Preserve hostname 255/256, eager sibling construction, actual fallback/component coverage, wrapper/callback overlap, partial/normal/unwind release and serial versus cumulative composition. Do not narrow existing component envelopes to observations.
6. Retain expiry and entropy identity, invocation-prefix, transient/retained/process/thread and allocation-success/fatal-exclusion controls. Missing target facts remain required at zero calls; source/refusal failures cannot become empty facts.
7. Preserve actual public coordinator verdicts, isolated allocator positive/release controls and original deadline, refusal, false/panic/no-refund and survivor ordering. No measurement may create or tune a premise.
8. Preserve the three final report/current-body/application `CommonSourceComplete` gates and strict rejection of hypothetical/native-only/retired/hybrid qualification. Do not change them to expect `Incomplete`.
9. Keep exhaustive development-lane list/selection/execution/filter checks, normal/staged cases, six independently owned heavy exclusions and every existing feature/adapter/CI owner. Retired internal-control names must be replaced through an explicit coverage/roster migration, not aliases, ignores or conditional skips.

No extra hosted mutation PR batch is authorized. Existing data/source-AST negative controls and the approved hosted tests-first process remain the verification mechanism.

## 12. Execution Ownership And Delivery

**Maintainer amendment, 2026-11-28:** Commit 1 contains only this spec and the existing plan amendment in PR #268. Record the maintainer decision on #47, update the PR Scope with the amendment and actual versus planned deletion line deltas, and ask REVIEWER to review this as a planning change. Keep the implementation PR draft. Resume no derivations or deletions until REVIEWER has reviewed the amended contract/table and the maintainer has approved the exact numeric premises. No second spec/plan PR or implementation approval is implied by this document edit.

Commit 2 onward performs the hard-cut deletions and premise records with tests-first identity/cap/missing-constant controls, then hosted assertion RED/GREEN and the unchanged full matrix. Keep unit tests inline in the owning ordinary module tree and existing targets/process isolation. Reuse handoff, source captures, metadata/provenance and component owners; add no job, target or build/capture plan. Local validation for this amendment is document/link/consistency inspection only, not Cargo/CI execution.

Report actual files/lines removed, net additions/replacements, remaining work for each of the five derivations and a qualified estimate. Until implementation deletions occur, report **zero implementation lines removed**, not the candidate inventory as an executed deletion. Historical evidence and captures keep their bytes; add new dated execution evidence only when it exists. Full hosted acceptance and separate current-head review remain mandatory before completion/merge; no source-proof, premise, target or CI failure is waived by this amendment.
