# Shared Schema Validation Bounds

Status: proposed written contract for maintainer review. The maintainer approved this design direction, not implementation, publication, admission, qualification or merge.

Issue: [#47](https://github.com/IABTechLab/auctioneer/issues/47). Parent epic: [#9, Complete Resource Qualification And Target Fit](https://github.com/IABTechLab/auctioneer/issues/9). Planning basis: merged Auctioneer `8baf0a5e71bf685d3e8ae755d4adcbe0d2329525`, including [#262](https://github.com/IABTechLab/auctioneer/pull/262). The recorded prerequisite [#33](https://github.com/IABTechLab/auctioneer/issues/33) is closed. [#46](https://github.com/IABTechLab/auctioneer/issues/46) remains open; reuse its delivered source interface without claiming its remaining certification is complete.

## 1. Objective

Complete the shared source-derived validation-storage, evaluation-work and structural-stack derivations for the discovery-selected executable schema closure. Compose the existing component proofs with the actual captured generated validators, formats and caller lifetime. Supply explicit target inputs and checked admission correspondence instead of treating tested native reservations as worst-case proof.

The work is one common proof package, not a new matcher or a target-enablement package. Native, Fastly, Cloudflare and hosted Spin bindings and physical qualification remain [#72](https://github.com/IABTechLab/auctioneer/issues/72)-[#75](https://github.com/IABTechLab/auctioneer/issues/75); qualified publication/activation remains [#88](https://github.com/IABTechLab/auctioneer/issues/88)-[#90](https://github.com/IABTechLab/auctioneer/issues/90). No reverse dependency on those issues is added to #47.

## 2. Scope And Non-Goals

Include the current per-call path: bounded borrowed-input measurement, admission/dispatch coordination, declaration-specific cache construction, generated validation and reachable runtime primitives, regex searches, hostname/UUID callbacks, failure cleanup and cache destruction. Include the sum of work across attempts and admitted evaluations, and simultaneous storage/call nesting, not just the largest observed successful call.

Preserve all current production behavior: the engine and compiler patches, schemas and formats, workload/input limits, work-token policy, reservations, original deadlines, impression-local errors and guest-publication restrictions. This package does not install a descriptor or activate a previously unqualified path. Do not implement runtime metering, cache reuse/pooling (#229), a replacement validator, a new crate, a new test binary or a second registry/configuration mechanism.

Measurements corroborate independently derived bounds; they cannot select a bound, coefficient or threshold. A missing proof, unsupported operation or incompatible source tuple refuses certification. No partial owner set, hypothetical target fact, disabled bidder or unimplemented placeholder can become a zero-cost qualification.

## 3. Existing Contracts To Reuse

| Owner | Existing Contribution | Limit To Preserve |
| --- | --- | --- |
| `schema_residency::Descriptor` | Captured declarations, expanded owner graph, source bindings and validation handoff. | Identifies obligations; supplies no numeric V/work/stack certificate. |
| `schema_cache_model`, backtrack/one-pass/hybrid models | Logical storage dimensions and conditional engine component bounds. | Logical records are not machine-stack frames; construction, eligibility and selected execution differ. |
| `schema_selected_half_search` | Fixed-cache, finite-call byte progress, source-bound strategy/capacity and requested-heap composition. | Call count is a caller premise, and byte progress is not CPU work. |
| `schema_meta_heap`, `schema_bundle_heap` | Cache children, growth overlap, wrapper inline storage and overlapping hostname cache. | Existing totals already include some fallback children; do not add them again. |
| `schema::measure_schema_input` | Borrowed value/key node counts, compact JSON byte size and container-depth admission. | Runs before work-budget admission; shape bounds do not prove evaluation cost. |
| `SchemaWorkBudget::evaluate` | Original-deadline check, checked precharge, scratch comparison and no-refund semantics. | Public scalar costs and native heuristic tokens do not authenticate a proof. |
| Existing allocation/probe owners | Isolated allocator windows, positive controls, actual wrapper layout observations and cold/repeated/changed cases. | Requested allocations, test-profile layouts and sample maxima are not physical or whole-call bounds. |

Current source files are [schema admission](../../../crates/auctioneer-bidder/src/schema/mod.rs), [native coordination](../../../crates/auctioneer-bidder/src/schema/native.rs), [input measurement](../../../crates/auctioneer-bidder/src/schema/input.rs), [format callbacks](../../../crates/auctioneer-bidder/src/schema/staging.rs), [generated dispatch](../../../crates/auctioneer-config/src/schema_staging_build.rs), [exchange admission](../../../crates/auctioneer-exchange/src/run/schema.rs) and [the source descriptor](../../../crates/auctioneer-bidder/tests/schema_residency/descriptor.rs). Read their current implementation before assigning work or lifetime.

The current checkpoint has 18 public definitions, 16 effective schema owners and five program owners. The handoff contains 51 children: three roles per program, two per declaration and two per distinct format callback. These figures are baseline controls, not a source list or a limit. Preserve full imported-source regression coverage separately from discovery-selected qualification.

## 4. One Bound Source Graph

Add a private validation-proof consumer beside the existing residency/engine proof modules in `schema_source_conformance`. It consumes the same sealed descriptor's validated expanded graph, declaration/helper identities and exact `validation_handoff` binding. It must not rediscover a similar list from bidder names, delivery cohorts, generated pattern text alone or operator-enabled definitions. Sharing an implementation does not make every imported alias executable.

Bind each handed-off child to exactly one typed contribution or a source-proven exclusion. Require complete nonempty program membership and exact child identity, source-owner, role and binding correspondence. Missing, extra, duplicate, misowned or stale-bound entries refuse before aggregation. A declaration without regexes still has generated validation work and stack obligations.

The 51 children are not a complete native-entrypoint schedule. Add separately tagged caller-context roots for input measurement and validation coordination, with witnesses for their actual call graph, including `auctioneer_runtime::serialized_json_bytes`. Derive these roots from the accepted entrypoints; do not force caller work into a regex child or alter #46's C/I ownership to hide it. Bind the additional source/library/model witnesses to the V consumer as well as the parent descriptor binding.

Identity is same-capture correspondence. The #262 seed/graph hashes can contain random snapshot roots; do not compare independently reconstructed raw hashes as reproducibility keys. Reuse one immutable bundle and its exact handoff, or establish content/semantic continuity under the [existing contract](../../testing/evidence/schema-residency-initialization-20261121.md#per-run-digests-and-source-comparison) and consume a fresh bundle. Historical captures retain their bytes and recorded locks. #47 introduces no reference reset or legacy reader.

## 5. Input And Invocation Premises

The common model takes independently supplied, checked dimensions:

- Compact serialized parameter bytes `B`, value-plus-object-key nodes `J` and container depth `D`, matching `SchemaInputStats`, including scalar and empty-container cases.
- Actual generated branch/helper structure, schema nesting and container traversals, including branch alternatives that revisit the same value.
- A derived finite number of searches per program and callback invocations per evaluation, not a count inferred from an observed workload or assumed equal to `J`.
- Caller evaluation-attempt and admitted-evaluation populations, with their actual ordering and maximum simultaneous active lifetimes.

Read the current limits from their owning code. At the basis checkpoint native input admission is 65,536 compact bytes, 4,096 nodes and depth 32; hostname rejects lengths above 255. These are existing premises, not new tunable controls or permission to clamp supported input.

Prove the relationship between admitted JSON size, decoded string/key bytes and every regex/runtime-helper haystack. Include escaped/non-ASCII strings and keys. Derive helper-call multiplicity from actual expanded control flow and bounded instance shape. Freeze the accepted operation grammar and work-unit definitions before measurements. An unknown helper, cycle, recursion edge or unbounded traversal refuses rather than inheriting an unrelated parser-depth assumption.

Preflight occurs before `SchemaWorkBudget::evaluate`. Bound its attempted traversals and refused-prefix cleanup even when no validator is admitted or aggregate work is already exhausted. Do not bound attempts by successful calls or pretend a later work debit precharged an earlier walk. Caller populations must be reconciled with existing impression/bidder limits, not independently widened or omitted.

## 6. Validation Storage And Lifetime

Model the production lifetime: a fresh declaration-specific cache is constructed inside the admitted closure, used by the generated validator, then dropped before returning. Include partial cache construction, growth/relocation, failure cleanup and final destruction. Repeated-cache observations may corroborate a component but cannot replace this fresh-per-evaluation path.

Each cache child retains its existing disjoint sub-owner attribution. Immutable regex programs, static cells and boot-time initialization stay in C/I. Mutable search storage and per-call cache construction belong to V. A fallback term already included in a broader component bound receives an explicit covered-by relation, not another summand and not an unproved zero. Excluding actual search fallback needs the complete source-bound strategy/capacity/quit proof; inactive eager cache allocation may still be live.

For one evaluation, define the requested-storage envelope as the checked maximum over source-derived lifetime states of the sum of disjoint live owners plus justified transient overlap. Include sibling caches and the enclosing generated cache while a hostname callback constructs, searches with and drops its fresh local cache. A shared callback definition is one source owner but can be invoked many times; serial callbacks permit a peak maximum only after proving non-overlap. Sum their work, not their peak storage. UUID's fixed decode buffer belongs to the call-frame model; its uppercase scan is input-dependent work.

At auction composition, storage follows the existing serial validation lifetime and its funded per-auction owner, not the number of completed calls. Work follows all applicable attempts/admitted calls and is cumulative. Any source change introducing overlapping validation or escaping/reset/cloned/rebound caches invalidates the serial/fixed-cache premise and refuses until separately reviewed. Preserve checked arithmetic and independent populations.

## 7. Evaluation Work

Define a checked source-level work ledger with explicit units and coverage, rather than one unnamed number. Cover input/serialization-sizing visits, generated branches and property/array scans, comparisons, string/Unicode operations, equality/uniqueness and reachable runtime helpers, callback work, cache construction/growth/cleanup and selected search transitions/state processing. Establish the actual primitive closure from the emitted validator and pinned dependency sources; the examples are not a whitelist that silently ignores other operations.

Use source-proven upper bounds for the worst admitted valid or invalid path, including early return, late failure, supported fallback/quit behavior and cleanup. Mutually exclusive branches may use a maximum only with a branch-exclusivity proof; loops and repeated callbacks use derived invocation counts. If a runtime primitive allocates or recurses, carry that storage/stack obligation to the corresponding lifetime model instead of considering its work count sufficient.

Byte-progress arithmetic and NFA/epsilon heap-stack dimensions remain distinct from evaluation-work units. Qualification-tool graph-exploration budgets are not serving work limits. Do not equate source record/byte visits, abstract validation operations, machine instructions, elapsed milliseconds or the native token formula. State what each work component includes and excludes, and supply a documented domination argument before relating it to an admission charge.

The existing native policy `1024 + 16 * bytes + 128 * nodes` is not the proof oracle. Neither its coefficients nor its single/auction limits may change in this package. The model must report an explicit incompatibility if the proved envelope cannot be soundly related to the unchanged policy. Do not tune units to fit, replace the workload or silently call this condition certified. Any needed production policy, compiler patch, instrumentation or engine change returns for separate maintainer design approval.

A source-level work bound is not a latency or CPU-instruction certificate. The current validator is synchronous; deadline checks before/after a call do not provide interruption inside a regex search. Preserve those checks and state the distinction, without inventing cooperative cancellation hooks.

## 8. Structural Stack And Target Inputs

Derive logical call nesting from the actual generated declarations, input traversal, callback/coordinator paths and pinned runtime-helper control flow. Distinguish schema nesting from instance container depth, repeated branch work from simultaneous call depth, and explicit regex heap-stack records from the machine stack.

Express a checked peak call-path composition in terms of required target frame/scratch facts. Add frames that can coexist on one path; take a maximum across alternatives only when their exclusivity is proven. Include called serializer/library routines, fixed UUID storage, cache construction/search/drop and declared panic/error-path alternatives. An unclassified recursive cycle, call/frame owner or nesting contribution refuses common source completion; do not guess a multiplier from input depth. A declared but unavailable target frame-byte fact instead refuses numeric/target evaluation, never becoming zero; target consumers must also match the artifact's actual panic/error-path mode.

Common #47 evidence supplies the structural bounds, source/helper identities, frame-owner requirements and composition rule. #72-#75 must supply matched actual artifact/compiler/ABI/profile/feature frame sizes, machine-stack and allocator facts and prove correspondence. Host `size_of`, a local stack sample, source AST size, a WASM section size or a logical heap-stack count does not supply these facts. No new layout API or vendor patch is preapproved here.

Keep heap/requested scratch, inline cache storage and machine stack distinct until the adapter owner maps them to its existing memory domains exactly once. In particular, Fastly's separate published stack allowance must not be folded into primary memory or treated as proof that the application fits.

## 9. Proof Status And Admission Handoff

The conformance-local common V output carries the parent/source binding, exact child/context coverage, input/lifetime premises, checked storage/work/stack formulas, source exclusions and all required target facts. No public API or production receipt is added. Reuse the existing separation between common source completeness, supplied-fact evaluation and actual qualification:

| State | Meaning | Permitted Claim |
| --- | --- | --- |
| Incomplete/refused | Any required source owner, operation, lifetime, coefficient or proof is missing/incompatible. | Diagnostic with fixed reason; no complete V or admission credit. |
| Common source complete | All reachable source obligations have supported bounded derivations and exact coverage. | Conditional common formulas and required target inputs; no physical values or serving permission. |
| Hypothetical evaluation | Synthetic facts exercise checked composition and boundary arithmetic. | Labelled test result only, never a qualification receipt. |
| Actual target bound/installed admission | Matched artifact facts and approved integration satisfy their owner gates. | Reserved to #72-#75/#88-#90, not granted by this package. |

Relate the derived common cost to `SchemaEvaluationCost`/`SchemaWorkLimits` through explicit checked premises, without replacing the existing production projection. Preserve deadline-first refusals, no invocation/debit on rejected admission, debit-before-call and no refund after false/panic. Existing behavior tests remain the authoritative arithmetic controls; add source-to-admission correspondence rather than another copy of the budget implementation.

A nonmatching current reservation, unsupported source path or unproven unit conversion leaves that admission row nonpassing. It cannot be hidden behind common-source success. Do not close #47 on an inventory, observations or a descriptor containing unresolved common work/stack obligations. Target-only facts can remain with their explicit downstream owners; missing common proofs cannot be reassigned there merely to declare this issue done.

## 10. Failure, Privacy And Integrity

Refuse exact source/binding drift, an unsupported emitted operation/attribute/format, inconsistent helper order or lifetime, missing/duplicate ownership, unavailable facts, arithmetic/counter overflow and invalid dimensions before returning a complete result. Preserve original compiler/schema/library/options/feature witnesses and full-source correctness checks. Zero is accepted only for a named source-proven absence in its exact scope, never as a missing-value default.

Keep proof/evidence output free of actual bidder-parameter values, user identifiers, raw consent or configuration secrets. Shared tests use fake owners; bidder-owned fixtures retain their independent provenance/verdicts. Do not expose private validators or raw response/request bytes, introduce cross-crate source imports, clone the caller's JSON subtree or make a serving initializer consume a test result.

## 11. Required Independent Controls

1. Complete bound handoff accepted; each independently missing, extra, duplicated, misowned or stale-bound program/declaration/callback/context contribution refused. Preserve positive before-states and exact dispatch counts. Keep the baseline 51-child census separate from new caller-context coverage.
2. Actual generated shape changes for a non-regex declaration, branch traversal, runtime primitive, helper-call multiplicity and callback source invalidate the right proof. A self-consistent changed source witness alone is not approval of new semantics.
3. Independently vary bytes, value/key nodes, depth, schema nesting/branches, search-call count and evaluation-attempt population. Include zero-regex schemas, escaped/Unicode input, repeated branches and input rejection before admission. Avoid correlated fixtures whose constants collapse distinct boundaries.
4. Preserve hostname 255/256, full-haystack/span, fallback eligibility, finite-call/counter and quit/capacity boundaries. Add fresh-wrapper creation, multiple helper caches, callback overlap, late failure and exact release controls without weakening existing broader component envelopes.
5. Independently missing target frame/layout facts and conflicting ABI/source identities refuse supplied-fact evaluation. Use small hand-written arithmetic examples and asymmetric values to distinguish sum, maximum, multiplication and overflow; do not compute expected bounds from the implementation's coefficients.
6. Retain existing admitted/refused/false/panicking work-budget controls, original-deadline behavior and impression-local error/survivor ordering. Cover preflight attempts when no validation is admitted and guard against treating an after-the-walk debit as preflight funding.
7. Corroborate the actual generated fresh-cache path with existing allocator/probe owners and positive controls. Freeze source-derived expectations before observations; retain all valid/invalid fixture verdicts and release assertions. Observations cannot establish missing CPU, ABI or physical proof.
8. Test that incomplete, hypothetical and native-only observations cannot produce usable target costs, a qualified receipt or guest/serving activation. No conditional skips, new ignores or zero-test filtered success.

The implementation plan must name the exact test owners and source mutations/control inputs for each branch. Hosted tests-first checkpoints require separate publication approval. New mutation PR batches require their own explicit approval; this design authorizes neither execution nor publication.

## 12. Execution Ownership And Delivery

Keep common models and their inline unit controls in ordinary modules of the existing `schema_source_conformance` target, with the actual-source join in that same owner. Extend the existing component modules where they already own a bound; share setup, not independent expected values. No new target, job matrix or repeated full child build is needed.

The current compiler-schema job explicitly runs and strictly lints this target because Cargo marks it `test = false`. Default source conformance retains its six separately owned heavy exclusions. Development currently selects `schema_residency::`; the implementation plan must add the V consumer's exact intended selection to the existing development owner and prove nonempty, unique, exhaustive selected dispatch and the correct filtered remainder. Merely adding a sibling module does not execute it in that lane. Preserve normal/staged checks and every existing full-source, selected-source, artifact and feature lane.

Reuse existing fresh measurement processes and actual declaration/artifact captures. Do not introduce a new network checkout or make the V owner require catalog-growth reference data. Existing linked/layout probes may supply their measured observations under their original scope; no four-adapter child-build duplication is implied.

Planning changes only this spec and its future implementation plan. Local validation is document formatting/link/consistency inspection; no local Cargo/CI execution. The future implementation must obtain hosted RED/GREEN, then pass the full unchanged acceptance matrix at its exact final head, with exact commands, exits, result lines, dispatch counts and source/features/ABI identities. Preserve historical capture bytes; publish new dated evidence for new results rather than recertifying old runs.

Deliver #47's common derivations, complete ownership/lifetime coverage, checked admission correspondence/refusals and explicit #72-#75 target-input handoff. Review and acceptance must distinguish common completion from missing target facts and from any unresolved common-policy incompatibility. A planning PR does not close #47 or authorize production cutover. Implementation, remote publication, qualification and merge require their respective approvals.
