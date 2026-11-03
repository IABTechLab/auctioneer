# bidder catalog Shipped-Schema Qualification

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-29
Status: Approved implementation in progress; no serving cutover or qualification claimed

## 1. Purpose

AG approved applying the qualification recommendation toward completing bidder catalog,
with explicit later-capability ownership for work that bidder catalog does not need. Keep
the current generated validators, engine, schemas, features, numeric limits,
privacy/security requirements and all 14 priority flows. Change the scope and
order of qualification, not the meaning of a passing resource guarantee.

This amendment governs the bidder catalog-specific qualification clauses in the
[catalog specification](2026-10-16-bidder-catalog-design.md) and
[implementation plan](../plans/2026-10-16-bidder-catalog.md). Earlier execution
records remain historical evidence, not declarations that this gate passed.
Review this written contract before changing schema serving or promotion.

## 2. Bounded Qualification Set

Call the qualification set `S`. Derive it from generated published bindings
and the frozen bidder catalog priority-wave contract, not a handwritten bidder list.
Before publishing the priority flows, `S` includes their planned shipped
bindings as well as the existing published roster. Include:

- Every distinct effective parameter schema in that union.
- Alias ancestry, inherited schemas, legitimate overrides and local references.
- Required shared format validators and their actual helper dependencies.
- Disabled-but-shipped definitions, because runtime activation must not rebuild
  or discover new unfunded state.
- Every supported feature lane, including explicitly local/insecure bindings.
  A documented no-schema local binding is not an invented imported schema.

Derive a separate `S` for each adapter/build-feature lane and require coverage
of their union. An absent Cargo feature does not create a resident owner in
that artifact; runtime disablement does not remove an owner that was compiled
and shipped. Keep declaration validation before feature filtering and retain
paired secure/development-feature acceptance.

Deduplicate shared schema/helper owners by explicit identity, not equal byte
sizes. Record source/schema digests, binding and helper membership, generator
and engine versions/features, compiler, target, build profile and lock identity.
Verify the actual generated and linked membership against `S`.

Imported source-only catalog rows do not require executable validator emission
or complete engine-resource qualification in bidder catalog. They remain inventoried and
cannot become callable through runtime configuration. Discovery must fail a
publication whose schema or resource record is missing, stale or unsupported.
If an unselected validator is nevertheless linked or initialized in a serving
artifact, account for it or remove that unintended reachability; a scope label
cannot make its cost disappear.

### Non-Serving Artifact Staging

AG approved generating and linking `S` before complete qualification so the
actual application artifacts can supply code/data and membership evidence.
This is an inspection stage, not schema admission or bidder promotion. A staged
build must refuse activation explicitly as unqualified before constructing a
registry, initializing validators, installing serving gates or exporting a
funded resource profile. Normal builds retain their existing behavior.

The staged validator table is private and unreachable from serving. Preserve
the same compiler, schemas, helper code, features, options and target/profile
identities used by qualification. Bind the table to its exact generated inputs;
do not substitute an isolated probe or source-length estimate for the resulting
application artifact. Any retention root is inspection-only and must not run
initializers or validators. Artifact inspection does not establish residency,
work or scratch bounds.

Use one internal Cargo feature to select both staging dependencies and the
unqualified compilation path. Keep those dependencies out of normal application
graphs: the unqualified runtime must not change normal transitive features such
as JSON-number parsing. There is no second environment mode selector. The test
harness's separate expected-mode value is only an independent oracle; it cannot
change the compiled mode or grant activation.

Unqualified is an explicit state with no numeric cost defaults. Staging cannot
install zero placeholders, claim a successful qualification, enable a bidder or
weaken the pre-funding requirement below. Final serving integration must repeat
the artifact correspondence and resource gates on the actual admitted build;
a staged artifact is not the final qualified shipping artifact.

Artifact inspection must check retained function references, not merely find
owner names or a digest in arbitrary file bytes. The private inspection rows
use a fixed `repr(C)` layout, bind their owner to the staged input identity,
and carry the same three typed function pointers without calling them. The
compiler checks pointer width and row layout on each target. Inspect rows only
in program data; reconcile native function addresses or WASM table entries
against actual defined code, allowing legitimate function folding. Debug/name
sections, imports, absent or extra owners, and truncated records cannot satisfy
this check. Physical row layout is not a qualified resource-cost descriptor.

The full 358-definition source inventory and 261-base-schema census remain
intact. Existing all-source correctness, provenance, compiler and compatibility
controls are not deleted, weakened or hidden behind new ignores. `S` scopes
the new serving/resource acceptance obligation; it does not rewrite that corpus.

## 3. Observed Opportunity, Not Acceptance Evidence

An initial structured scan of the 14 directly owned priority parameter files
found pattern/format keywords only in two families: six pattern occurrences
and the hostname/UUID formats. The other 12 files contained neither keyword.
This is not a generated-helper count or proof of the complete closure: the
implementation must still resolve references, aliases, existing bindings,
feature lanes and actual generated code.

The recent general literal-matcher investigation covered unselected schemas
as well. Do not expand that investigation solely to qualify a hypothetical
future catalog. First demonstrate that an owner/strategy is reachable from
`S`; count names or native payload sizes alone cannot establish that fact.
Any unresolved owner actually needed by `S` remains an bidder catalog blocker.

## 4. Evidence Strategy

Use the artifact actually built for the adapter rather than treating a
native layout reconstruction as a guest fact. Preserve three distinct claims:

| Class | Meaning | Does not establish |
| --- | --- | --- |
| Proved | A source/compiler/library-owned bound with stated premises and checked composition | Provider memory or timing facts outside those premises |
| Enforced | A limit that actually stops the corresponding operation before exceeding it | Successful completion of every otherwise valid input |
| Observed | An identified artifact/input/history produced the recorded measurement | An upper bound on untested histories or inputs |

Record these classes and exclusions in acceptance evidence. Version any strict
machine DTO whose contract changes; do not reinterpret existing scalar cost
fields, default missing facts, or rewrite frozen captures/profiles.

For the shipped set:

1. Retain independently sourced schema correctness cases and generated-code
   controls for every supported keyword, draft, reference, format and pattern
   actually required by `S`. Preserve missing/null and invalid-sibling behavior.
2. Build actual native and all three adapter artifacts with existing profiles
   and packaging gates. Verify linked validator/helper membership and inspect
   code/data sections, declared memories and artifact identities. Emitted
   source length, package size and an rlib are not resident-memory bounds.
3. Execute bounded native and local guest schema probes using existing Rust
   tooling. Measure cold initialization, repeated validation, changed inputs,
   failed validation and cache histories. Separate the probe's own parsing/
   allocator overhead and report its limits. Match its generated bindings,
   helper code, features and target to the artifact it supports.
4. Prefer compiler/library-owned layout facts and existing bounded owner APIs
   for the remaining accounting gaps. Add source-level derivations only for
   reachable gaps needed by `S`, not every backend the dependency can support.
5. Keep source-based work and scratch bounds for the admitted shipped paths,
   and actual enforcement for byte/node/depth/call/work reservations and
   original deadlines. Test the final composed auction and handler paths,
   not only isolated schema callbacks.

Local guest probes are not deployed-handler or provider-physical-memory
observations. Record unsupported execution environments as unavailable, never
as a successful zero-cost run. An unavailable fact needed by bidder catalog's shipped
admission contract blocks bidder catalog; a provider-only fact retains its resource qualification/publisher pilot owner
and blocks that target's readiness. This split must not waive an artifact build.

## 5. Safety And Accounting Stay Mandatory

Preserve `C/I/V/K/A` ownership, deduplication, checked arithmetic and existing
additive multiplicities. Keep finite descriptors available before new registry
state or lazy initialization; no zero placeholders or allocation-before-funding.
Runtime-disabled shipped definitions still contribute their retained costs.

Measurements can corroborate bounds and expose mistakes; they cannot be rounded
into purported proved descriptors. A post-call deadline check is not CPU
preemption. A Wasmtime test limit is not a production-host limit. Any proposed
runtime limiter must demonstrate its actual integration, coverage, refusal and
cleanup behavior before it can support an enforced claim.

Do not raise ceilings, shrink the frozen supported workload, alter schemas,
change the regex engine/features, omit a priority flow, or add an allow-all
fallback to make a gate pass. The existing constructor initialization,
explicit-cache and shallow layout patches remain at their reviewed scopes.
No additional nested-owner introspection patch is an automatic prerequisite:
first establish a concrete shipped-path need and review its bounded contract.

## 6. Capability Owners

| Owner | Required work | Exit or trigger |
| --- | --- | --- |
| bidder catalog | Generated shipped-set correctness; reachable owner/work/scratch bounds; native and guest artifact evidence; funded registry/schema integration; full 14-priority selected-flow acceptance | Every binding/helper in `S` is covered; all required bidder catalog controls and actual onboarding producers pass |
| resource qualification | Existing absolute-fit/headroom remediation, actual target memory ownership, missing SDK/host resource facts and affected runtime-readiness safeguards | The affected target's unchanged equation and actual required platform facts meet its existing readiness contract |
| Trusted Server embedding | Selected embedded-host receipts, resource ownership and direct-engine acceptance | Selected Trusted Server integration contract passes without an HTTP hop |
| publisher pilot | Credentialed hosted execution, actual provider meters/limits, isolated failure/recovery and limited publisher rollout | Selected actual deployments pass the retained acceptance/runbook gates |
| bidder expansion | Incremental qualification for newly shipped schemas/features, including nested literal-owner layout/cache APIs when first needed | Each frozen expansion wave proves its new reachable surface before publication; missing evidence blocks that wave |
| remaining catalog closure | Residual catalog's complete dialect/format/regex/resource reconciliation and terminal all-catalog closure | Every remaining source row and required branch passes the full completion contract |

The bidder expansion trigger is not permission to postpone a reachable bidder catalog obligation. Nor
does it require completing every future backend before a wave whose actual
schemas do not use it. Optional engine replacements or optimizations require
their own reviewed decision; none is selected by this amendment.

## 7. Finite Completion Sequence

1. Finish and verify the current registry/API repair round, including fixtures
   that previously replaced registrations, and commit it independently.
2. Generate and freeze `S` from the binding/wave union and both feature lanes.
   Produce a reachable schema/keyword/format/helper/strategy gap table. Confirm
   which existing proofs and patches apply before adding another one.
3. Implement the scoped acceptance target and artifact/probe controls. Reuse
   existing conformance infrastructure, Rust tooling and normal module trees.
   Preserve all existing source-wide controls. Close only actual bidder catalog gaps.
4. Emit and verify the nonzero descriptors, finish the funded cached registry,
   then integrate scoped parameter validation with the existing admission path.
5. Complete all 14 priority flows, actual evidence producers, activation,
   settings/credentials, aliases, privacy, sync and composed onboarding tests.
6. Run the existing CI/build/package/relative-resource and native-fit gates.
   Publish bidder catalog acceptance and the explicit remaining resource qualification/publisher pilot limitations;
   do not claim deployment readiness or full-catalog completion.

Do not declare bidder catalog complete when only this document, imported metadata, the
scoped harness or direct bidder candidates exist.

## 8. Required Negative Controls

- Adding a published schema or alias override without updating `S` refuses.
- Runtime disablement does not remove a schema from the funded set.
- A source-only schema stays uncallable; linking its state cannot omit its cost.
- A changed helper, target/profile, compiler, feature lane or lock invalidates
  the corresponding evidence rather than inheriting an old pass.
- Missing guest observations are not reported as executed or as zero bytes.
- Removing a required shipped owner/work/scratch term fails independently.
- Independent input/count/budget axes reach their actual exact/one-over guards.
- A failed/unsupported shipped schema cannot be reassigned to a later capability
  or omitted from the frozen priority wave to obtain completion.
- A new expansion schema needing an unqualified backend blocks that wave, not
  an already-qualified unrelated flow or the entire unshipped catalog.

These are required implementation controls, not claims that new tests exist.
