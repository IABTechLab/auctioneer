# Explicit Schema Search Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans task-by-task, with test-driven development and independent review.

**Goal:** Implement the approved generated cache API as a third replayable compiler patch, verify compatibility, and refuse unproved resource qualification.

**Architecture:** Preserve default generation. Explicit boolean-only generation supplies a typed public wrapper and private mutable context containing per-helper meta caches plus generated reference/format state. A structured `syn` lowering pass can thread context through the generated internal helper graph without rewriting schema semantics or introducing ambient state.

**Tech Stack:** Existing Rust workspace, patched `jsonschema-macros-core 0.58.6`, existing `syn` AST APIs, locked `regex-automata 0.4.18`, current schema/provenance/linked-artifact controls.

**Qualification scope, 2026-10-29:** remaining M7 serving/resource obligations
use the complete shipped closure `S` from the
[scope amendment](../specs/2026-10-29-m7-shipped-schema-qualification-design.md).
Preserve existing all-source compatibility/component tests and dated results.
Do not expand private-backend proofs without a demonstrated need in `S`;
future-only surfaces belong to their M7.7 wave or M20 before publication.
Required shipped bounds remain mandatory; observations cannot replace them.

AG approved the [written design](../specs/2026-10-22-explicit-schema-search-cache-design.md)
on 2026-10-22. No new engine, runtime compiler, workspace crate, deployed
dependency, schema capture, workload limit, ceiling or funding equation is
approved. Leave the unrelated user-owned `docs/roadmap.md` changes unstaged.
Commit and push verified increments without attribution trailers.

## 1. Baseline And Red Controls

**Files:** `Cargo.toml`, `crates/auctioneer-bidder/Cargo.toml`, new
`crates/auctioneer-bidder/tests/schema_cache_controls.rs`.

- [x] Record the resolved automata features before adding a direct native-only dev dependency. Keep defaults disabled and the version already locked; do not enable full-DFA features by inheriting automata defaults.
- [x] Add real macro controls and observe the existing compiler reject the unknown `cache = explicit` option. Include no-pattern/repeated-pattern schemas, nested objects/arrays, combinators, property names, recursive references, independent literal accept/reject values and default-mode counterparts.

Run `CARGO_INCREMENTAL=0 cargo test --offline -j2 -p auctioneer-bidder --test schema_cache_controls`.
Expected initial failure: unsupported cache option/API, not malformed input or
an unrelated dependency error. Zero tests is not success.

## 2. Explicit Compiler Path

**Files:** vendor `src/lib.rs`, `src/context.rs`, `src/codegen.rs`,
`src/codegen/emit_root.rs`, new `src/codegen/explicit_cache.rs`, compiler
manifests only if an existing `syn` visitor feature is needed.

- [x] Parse `pattern_options = { engine = regex, cache = explicit, ... }`. Reject duplicate/unknown values, incompatible engines/backends and enabled error methods before generation. Preserve default APIs/settings/output.
- [x] Resolve the automata crate path using the existing crate-resolution pattern. Preserve string-builder `LeftmostFirst`, UTF-8 syntax/empty matching and default limits when absent; map configured limits to `nfa_size_limit` and `hybrid_cache_capacity`. Only unused pool bookkeeping becomes `pool_capacity(1)`; algorithms stay enabled.
- [x] Emit one shared meta regex and one explicit mutable cache per existing deduplicated helper. Boolean matching uses `search_half_with` and `earliest(true)`, never pooled matching or a regex clone.
- [x] Emit same-visibility `SchemaSearchCache` with private state, no `Default`/`Clone`; `new_search_cache() -> Option<_>` forces all constructors before bundle creation. Cached constructor failure returns `None`. No-pattern validators have an empty typed bundle.
- [x] Expose only `Schema::is_valid(&Value, &mut SchemaSearchCache)` in explicit mode. Thread that context through every internal boolean helper and function-pointer signature/dispatch. Move generated recursion/cycle/URI state into that context rather than TLS; reset per-invocation state before top-level use so a prior panic cannot leak reference marks.
- [x] If using AST lowering, derive helper/state membership from generated items or compiler registries. Transform structured nodes, not strings. Preserve qualified external callbacks, handle indirect boolean dispatch explicitly, and refuse unknown shapes rather than silently omitting context.

Include the typed local `__root_key_eval`/`__root_item_eval` closures and plain
reference cycle casts. Track indirect `target` bindings by lexical scope;
a custom format may legitimately be named `target`. Preserve first-anchor
selection and copy function pointers out before mutable context dispatch.
Parse only recognized generated TLS declarations, retain their borrow
boundaries as owned fields, and reset transient state at the external entry,
never on recursive root dispatch. Static plan review found no blocking issues.

Literal behavioral control for schema pattern `[0-9]+`:

```rust
let mut cache = Example::new_search_cache().unwrap();
assert!(Example::is_valid(&serde_json::json!("prefix123suffix"), &mut cache));
assert!(!Example::is_valid(&serde_json::json!("no-match"), &mut cache));
```

## 3. Provenance And Portable Replay

**Files:** new `vendor/patches/jsonschema-macros-core-explicit-cache.patch`,
`vendor/README.md`, `tests/support/schema_vendor.rs`,
`tests/schema_build_controls.rs`, exact compiler pins and root lockfile.

- [x] Preserve original archive checksum and both earlier patch payloads. Mechanically generate the third patch against their combined output. Extend the exact vendor inventory for all changed/new members; original members remain mandatory and unknown additions/deletions/symlinks/edits still refuse.
- [x] Replay original -> type patch -> initialization patch -> explicit-cache patch and compare every changed/new file byte-for-byte. Earlier replay checks compare their historical outputs, not the extended final file. Independently alter/delete each patch member and require refusal.
- [x] Verify actual Cargo source/features, not only a pathless lock entry. Update upgrade instructions and source pins together; no captured schemas or expected outputs change.

## 4. Generated Consumer And Artifact Controls

**Files:** `tests/schema_cache_controls.rs`, `tests/support/schema_build.rs`,
`schema_probe.rs` and neighboring probe/allocation owners where needed.

- [x] Exercise default/explicit literal verdicts, visibility, deduplicated/empty bundles, independent concurrent owners, construction failure and reuse after callback panic. Inspect generated ASTs for exact helper/cache sets and absence of pooled matching/TLS; preserve qualified callback paths.
- [x] Use bounded actual Cargo/type-check controls for incompatible options, wrong-validator caches, missing mutable context and generated-name collision. Require intended diagnostics and valid sibling controls, not arbitrary compile failure.
- [x] Run all 300 externally captured parameter cases through explicit callbacks with unchanged expected outcomes and source witnesses. Link all 261 callbacks in native and three guest artifacts, retaining existing membership/section/export checks. Distinguish any default comparison artifact; do not charge both as the explicit candidate.
- [x] Measure constructor, explicit bundle creation, first-search growth, warm reuse and release separately in the native probe.
- [ ] Prove full production-owner multiplicity, internal fallback/capacity, separate hostname-format scratch, work and target qualification. Samples, `memory_usage()` and linked guest builds cannot supply those proofs or usable resource descriptors.

The measurement follow-up reuses the native counting allocator and workload
inputs, with a red control for the previously absent explicit allocation
command. Report shared constructors separately from per-schema bundles,
simultaneous independent probe owners, cold/changed/warm searches and release.
Keep input preparation, report allocation and serialization outside windows;
run the allocator's positive/negative/release/thread controls in the explicit
artifact too. Record inline owner layout and separately observed hostname
format state. This probe layout is not a production-owner selection or bound.
Independently vary input and owner populations; exercise valid and invalid
branches and require exact membership/counts rather than optional loops.
Native measurements and the locked-source audit must remain observations
until complete target-specific derivations exist.

## 5. Review And Publish

**Files:** approved spec status, `docs/bidders/schema-qualification.md`,
`ISSUES.md`, this checklist, source/patch pins and upgrade instructions.

- [x] Obtain spec-compliance and independent code-quality review; fix their findings and rerun affected controls.
- [x] Run secure/default and feature bidder controls separately, workspace Clippy, formatting and raw-fixture gates. Record exact counts, interruptions and limitations; keep serving integration closed while qualification is unproved.
- [x] Commit explicit file sets and push to the existing M7 PR. Do not stage unrelated user changes, relax watchdogs or raise limits for passing results.

## 6. Approved Hostname Cache Follow-Up

AG approved per-call format caches on 2026-10-22 under the separate follow-up
contract in the existing design. No compiler patch, new dependency, lowered
workload limit or serving integration is included.

- [x] Extend the native format measurement assertion to require zero retention
      after every cold/warm callback; observe the old pooled hook fail that assertion.
- [x] Replace only the qualification hostname hook's builder/search path with
      the same configured meta engine, shared initialization and a callback-local
      explicit cache. Preserve the pattern, byte guard, callback signature, UUID
      helper and existing semantic cases. No hidden pooled or TLS search fallback.
- [x] Add colocated configuration controls, a measured 256-byte input rejection
      with no cache allocation, exact format row membership and repeated allocation
      positive controls. Keep all construction/search/drop work inside the windows.
- [x] Run the secure/feature compiler/API controls and sequential full linked
      suite, then strict workspace lint/format/fixture gates. Record new observations
      separately from the prior pooled layout and retain source-bound capacity,
      production-owner, work and target qualification as open.
- [x] Obtain independent compliance and quality review, commit/push this hook
      increment separately and leave the unrelated roadmap edit unstaged.

Verification commands:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_cache_controls --test schema_cache_initialization --test schema_compiler_controls --test schema_source_inventory
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_build_controls -- --test-threads=1 --nocapture
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_cache_controls --test schema_cache_initialization --test schema_compiler_controls --test schema_source_inventory
CARGO_INCREMENTAL=0 cargo clippy --offline --locked -j2 --workspace --all-targets --features insecure-bidders,conformance-suite -- -D warnings
cargo fmt --all -- --check
./scripts/check-raw-json-fixtures.sh
./scripts/check-raw-json-fixtures-fixtures.sh
```

Use the existing bounded, leased Cargo harness for generated artifacts. Do not
start independent concurrent builds against its exclusive target directory.

## 7. Forward NFA And PikeVM Logical Storage

This is the next component of the already approved source-bound derivation,
not a resource descriptor or a change to generation/serving. Add the native
`tests/schema_cache_bounds.rs` integration owner and colocated unit controls in
`tests/support/schema_cache_bounds.rs`. Reuse the audited source inventory.

- [x] Write failing controls for independently varied logical dimensions and
      checked overflow before implementing their projection. With `N` NFA states,
      `S` capture slots and `P` patterns, two active sets contain `4N` sparse-set
      StateID entries and `2*(N*S + max(S,2P))` offset entries. Meta captures add
      another `S` entries. Epsilon-stack length is bounded by `max(1,B+C)`, where
      `B` sums union fanout minus one and `C` counts capture states, not unique slots.
- [x] Reconstruct the forward NFA from actual translated literals in emitted
      explicit-mode regex constructors using the locked public Thompson compiler.
      Preserve UTF-8, all captures, forward direction, no shrink, newline matching
      and the 65,536-byte NFA limit. Reject unrecognized constructor shapes instead
      of guessing. Include the separately shared hostname pattern in this audit.
- [x] Walk all 261 audited schemas. Reconcile emitted pattern slots, exercise
      literal/repeated-capture/branching controls, and pin nonvacuous helper counts.
      Treat literal-only meta fast paths as optional: these dimensions bound only
      the forward NFA/PikeVM component if instantiated, not every meta strategy.
- [x] Independently review the formulas against the locked source, run both
      feature lanes and strict lint/format/fixture gates, then record and publish
      this component without marking full qualification complete.

Logical lengths are not retained capacities, bytes, whole-meta search work,
allocator overhead, growth overlap or target fit. Source changes require a
fresh derivation; no term may be used as a production charge merely because
the component tests pass. Reverse/hybrid/backtracker/one-pass caches, format
scratch overlap and real owner multiplicities remain in the full gate.

## 8. Bounded-Backtracker Logical Storage

Continue the approved component derivation in a separate support module. This
does not enable serving or claim retained-capacity, byte or work bounds.

- [x] Add red controls for checked bitmap/stack arithmetic, independent NFA
      dimensions, 32/64-bit word rounding and full-haystack-versus-span eligibility.
      The locked meta wrapper excludes earliest searches above 128 full-haystack
      bytes. For eligible spans of `H` bytes the default visited limit is 262,144
      bytes, bitmap length is `ceil(N*(H+1)/word_bits)`, and the stack length is at
      most `max(1,(H+1)*(B+C))`. Refuse invalid shapes or overflow.
- [x] Compare bitmap lengths with real bounded-backtracker searches after
      unsuccessful searches drain the stack. Exercise changed input lengths and
      the visited-capacity boundary separately; `memory_usage()` reports lengths,
      not retained capacity. Keep full capture slots in the fallback proof.
- [x] Apply the projection to every audited emitted helper and to the actual
      shared hostname pattern. Test the 128/129-byte gate independently of the
      visited cap and include both target word widths without claiming guest runs.
- [x] Obtain independent source/compliance and quality review; verify secure
      and feature lanes, lint/format/fixture gates, document and commit/push only
      this component. Keep all remaining resource-qualification gates open.

Verification on 2026-10-22: the initial stub failed all four arithmetic,
eligibility and overflow controls with `Err(Shape)` instead of their literal
expected projections/refusals. The implemented component now has eight
colocated controls, including word-boundary rounding and real failed,
successful/reused, shortened-span and repeated-capture searches. The actual
261-schema walk still reconciles all 23 helpers in 14 schemas and adds both
word-width projections. The shared hostname pattern has its own boundary row.
Independent source/compliance and code-quality reviews were read-only and
reported no remaining must-fix issue; the source review's history-retention
caveat was added explicitly before publication.

The secure-default library and five focused targets passed 338 tests; the same
scope with `insecure-bidders` passed 349, both with zero failures or ignores.
Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
`-D warnings`, workspace and standalone-helper formatting, diff checks and both
raw-fixture gates passed (168 fixtures across 50 files). Existing GVL deletion
notices and the dependency future-incompatibility notice remain. The full
linked-artifact suite and guest execution were not repeated for this test-only
logical projection; no generated validator or serving implementation changed.
These results do not close the full section-4 resource gate.

## 9. One-Pass Capture Scratch

Continue the approved source-derived component work without changing the
generated engines, serving paths, workload or funding equation.

- [x] Add red arithmetic controls for explicit scratch slots `E=S-2P`,
      independently varying total slots and pattern count. Refuse underflow and
      overflow; a candidate above the locked 32-explicit-slot limit cannot build.
- [x] Reconstruct the public one-pass builder's meta configuration from the
      same audited forward NFA. Treat construction as a conservative candidate,
      not proof of actual meta selection. Pin real build/refusal membership and
      eager scratch lengths, including empty/partial/full capture callers, input
      changes and reuse. Keep meta captures separate.
- [x] Apply the projection to all emitted helpers and the shared hostname
      pattern. Record the no-capture/Unicode-boundary construction heuristic,
      anchored-input selection and eager-cache ownership separately; ineligible
      searches do not imply zero residency.
- [x] Obtain independent source/compliance and quality review, run both feature
      lanes and lint/format/fixture gates, update the evidence and issue status,
      and commit/push this component without closing full resource qualification.

Verification on 2026-10-22: the initial arithmetic stub failed all three
controls, and the plain one-pass builder failed the starts-per-pattern policy
control. The odd-slot control failed before its guard was added. Its sibling
overflow case now uses an even total so shape rejection cannot mask checked
multiplication. Nine colocated controls cover the projection, real builder
limits, eager/reused scratch, partial callers, an independent two-pattern NFA,
and unanchored rejection. All 23 generated candidates have literal-pinned
build/refusal identities: 19 build, four refuse as structurally non-one-pass.
The hostname has a separate fitting-slot-but-non-one-pass control.

Independent source/compliance and code-quality reviews were read-only. The
quality review found a count-only successful set, missing partial-scratch
reuse and missing real multi-pattern evidence; all three were corrected and
re-reviewed with no remaining finding. The source review confirmed the
conditional fixed-owner scope and separate HIR/selection/temporary-allocation
and retained-capacity obligations. Reviewers did not run Cargo; runtime
results below came from the main verification runs.

The secure-default library and five focused targets passed 357 tests; the same
scope with `insecure-bidders` passed 368, both with zero failures or ignores.
Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
`-D warnings`, workspace and standalone-helper formatting, diff checks and both
raw-fixture gates passed (168 fixtures across 50 files). Existing GVL deletion
notices and the dependency future-incompatibility notice remain. The full
linked-artifact suite and guest execution were not repeated for this test-only
logical component. No generated validator, serving code, workload, resource
ceiling, shared funding equation or usable resource descriptor changed.

## 10. Hybrid Directional Fixed Storage

Continue the approved source-derived component audit, without treating the
hybrid's reported-usage capacity as a physical-memory bound.

- [x] Add red checked projections for both sparse sets (`4N` StateIDs),
      six start-context classes for unanchored/anchored/per-pattern starts,
      three sentinel transition rows, the `max(1,B)` epsilon stack and encoded
      state/builder payloads. Derive the encoding header from the actual writer,
      not a possibly stale comment in the minimum-cache helper.
- [x] Reconstruct the capture-free reverse NFA with the unchanged compiler
      settings. Build public forward/reverse candidates with the locked hybrid
      settings, distinguishing shared-only reverse start configuration. Auto
      prefilters and actual optional reverse-inner prefix selection are not
      reconstructed by these candidates; keep that boundary explicit.
- [x] Cross-check fresh-cache reported usage against the fixed initialization
      layout on native and exercise both directions, caller start modes, reuse,
      growth, cache clearing/failure and independent configured-capacity refusal.
      Pin all generated helper identities and include the actual hostname pattern.
- [x] Obtain independent source/compliance and quality review; verify both
      feature lanes, lint/format/fixture gates, update evidence/issues and
      commit/push. Do not close retained-capacity, map/Arc/allocator, saved-state,
      constructor/work, optional-prefix or target/production-owner qualification.

Verification on 2026-10-22 and 2026-10-23: the initial arithmetic/reporting stub failed all
four controls. A public NFA with two match states for one pattern then proved
the encoded-payload precondition missing; it failed until the projection
rejected duplicate pattern matches. Twelve colocated hybrid controls now
exercise arithmetic, both directions and start modes, eager forward/reverse
owners, real multi-pattern starts, admission refusal, growth, clearing,
search failure and reuse after reset. A minimum-capacity recovery assumption
was corrected after the real engine legitimately gave up on a short match;
ordinary-capacity success remains independently pinned. No capacity or
give-up heuristic was raised.

Both primary directions build for all 23 literal-pinned emitted helpers;
fresh reported layouts match on native, including the separate hostname
pattern. The reverse audit independently derives its NFA rather than borrowing
forward dimensions. The [component record](../../bidders/schema-qualification.md#hybrid-directional-fixed-storage)
distinguishes report layout from physical memory and leaves exact private
selection and optional prefix NFAs unclaimed.

Independent source/compliance and quality reviews were read-only. Source
review corrected one prose ambiguity: fresh reporting excludes the initially
empty stack/builder, even though their later logical bounds are projected.
The correction was re-reviewed; neither review has a remaining must-fix
finding. Quality review noted that reset recovery completes a nonmatch, not a
post-reset positive match at the minimum capacity; documentation states this
limit. The first quality-review agent failed because of model capacity and
produced no verdict; the separate retry completed. Reviewers did not run
Cargo, and the results below come from the main verification runs.

Reproduce the focused secure-default run with:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --lib --test schema_cache_bounds --test schema_cache_controls --test schema_cache_initialization --test schema_compiler_controls --test schema_source_inventory
```

It passed 382 tests (204 library, 66 bounds, 13 API, one initialization,
74 compiler and 24 inventory), with zero failures or ignores. Adding
`--features insecure-bidders` passed 393 (215 library; the same integration
counts), also with zero failures or ignores. The twelve shared hybrid controls
run in both integration owners; these totals are executed cases, not distinct
source-test counts. Workspace and standalone-support formatting, diff checks
and both raw-fixture gates passed (168 fixtures across 50 files).

Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
`-D warnings` passed. Existing GVL deletion notices and the dependency
future-incompatibility notice remain. The full linked-artifact suite and guest
execution were not repeated for this test-only component; no generated
validator, production behavior, workload, ceiling, funding equation or usable
resource descriptor changed. Full section-4 qualification remains open.

## 11. Hybrid State Population And Encoded Payloads

Continue the approved source audit in
`tests/support/schema_hybrid_population.rs`, included by the existing bounds
and compiler integration owners. This is not a physical-capacity descriptor.

- [x] Add failing checked arithmetic controls before implementation. A resident
      state contributes at least `4d + 2w + 9` reported bytes (transition row,
      two-word state handle, minimum encoded header), even when map/scratch costs
      are omitted. Ordinary admission bounds its population by division of the
      configured capacity `C`; normal, nonrecursive recovery rebuilds three
      sentinels plus one saved and one pending state, so conditionally use
      `max(5,C/(4d+2w+9))`. Bound transition length
      by that count times `d`. For encoded payload limit `V`, resident payload sum
      is at most `max(C,27+2V)` and at most two nonresident payloads add `2V`.
      Check every sum/product, vary capacity/stride/word width/payload independently,
      and distinguish arithmetic controls from admissible DFA configurations.
- [x] Inspect actual admission, moved-builder and saved-state lifetimes in the
      locked source. Independently fund recovery using the pinned append-path
      bounds: `K=max(1,B)`, `F=16N+4s+4*max(4,2K)+2V`,
      `G=F+16d+12w+35+V`. Here `s` counts start entries, not populated states.
      Require `G<=C` before accepting actual-DFA projections; reject automatic
      capacity upgrades, and test real construction at `G-1` and `G`. Constructor
      admission and finite searches alone cannot discharge this premise. Do not
      infer physical memory from reported usage. A separate source audit records
      available local Rust-library implementation facts rather than guessed factors.
- [x] Exercise real cache growth/clearing with public state-ID identity controls
      per clearing epoch, not private layout casts or Debug-string parsing. Assert
      nonzero work and varying observed states. Apply projections to every emitted
      directional helper and hostname candidate, retaining exact membership.
- [x] Obtain source/compliance and quality reviews, rerun secure/feature tests,
      strict lint/format/fixture gates, update evidence and ISSUES, then commit/push
      this component without changing a production charge or closing full resource
      qualification. Retained-capacity, allocator, builder scratch, construction,
      work, actual owners, optional prefix storage and target fit remain open.

Verification on 2026-10-23: the population arithmetic stub failed three
controls; the later recovery stub failed four more owning controls. Eight
colocated tests now cover independent dimensions, component/final-sum overflow,
automatic capacity-upgrade refusal, inclusive recovery funding and real
per-epoch state-ID observations. The initial minimum-capacity eight-ID premise
failed (five were observed); the trace now keeps that row and requires the
larger-capacity row to exercise a strictly larger population and report overshoot.
No production capacity or clearing heuristic changed. A separate hostname
expectation initially reused the forward recovery value; the reverse result
9,477 exposed that mistake and now has its own literal expectation.

The source/compliance review found that constructor admission alone did not
prove nonrecursive recovery. The independent scratch-based gate above closes
that premise for current candidates; both directions of all 23 helpers and
hostname require an admitted projection. Review also requested a final-fold
overflow case with individually representable components; it was added.
This was a read-only review, not an independent Cargo run.

The matching source-only `rust-src` component was installed for the existing
Rust 1.98.1 compiler (`48a229ceaefd4985c50990b14116b6d856af0985`). There was no
toolchain-version, engine, dependency, generated-validator or serving change.
Audited SHA-256 witnesses, relative to that component's `library/` tree:

```text
a262b9e494236a00270be0dd0c57be5259d62bc1e538c56506f7f0ce8debccc0  alloc/src/raw_vec/mod.rs
09171c3c4c767a5c0716a85786f7670e5e149f75325fc1b27f5dbce14f16711f  alloc/src/vec/mod.rs
a0816ad0f39b947889771bf06930bf8e0c4ce794846d83863349697ec7c415ec  alloc/src/vec/spec_extend.rs
5df6230fefb967e2059a97de56d50bf2452f4805b22009f603faaa89c027a0d1  alloc/src/sync.rs
```

These are source-audit witnesses, not an automated CI source-component check.
A compiler/collection upgrade invalidates this derivation until re-audited;
full qualification must bind the eventual descriptor to its actual source and
target identities. Complete Vec/map/Arc requested-layout and allocator/physical
envelopes are still open, as are the rest of the section-4 gate.

Final verification used the same focused invocation as section 10 (library
plus `schema_cache_bounds`, `schema_cache_controls`, `schema_cache_initialization`,
`schema_compiler_controls` and `schema_source_inventory`): 398 secure-default
tests passed, and 409 passed with `insecure-bidders`, both with zero failures or
ignores. The eight shared population/recovery controls execute in both bounds
and compiler owners; totals are executions, not distinct source-test counts.
Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
`-D warnings`, workspace and standalone-support formatting, diff checks and both
raw-fixture gates passed (168 fixtures across 50 files). Existing GVL deletion
notices and the dependency future-incompatibility notice remain.

The final source/compliance recheck and separate code-quality review reported
no remaining findings after the recovery guard and overflow control landed.
Both were read-only; the main verification runs supplied all runtime results.
The full linked-artifact suite and guest execution were not repeated: no
generated validator, serving path, workload, ceiling, funding equation or usable
resource descriptor changed. No overall M7 readiness is claimed.

## 12. Requested Collection Layout And Hybrid Heap Envelope

Continue the approved source-bound derivation, without changing generation,
serving, limits, ceilings or resource descriptors. Reuse the native probe's
existing allocator rather than adding another allocator or workspace crate.

**Files:** new `tests/support/schema_heap_layout.rs` and
`schema_hybrid_heap.rs`; the existing `schema_cache_bounds` integration owner;
native `schema_allocation.rs` controls and their `schema_build_controls` driver.

- [x] Add red independent-literal controls for checked append-only Vec capacity
      and old/new overlap, state-map bucket/layout growth, and Arc-slice header and
      alignment. Parameterize word/group layout explicitly; refuse invalid shapes
      and overflow. Keep this primitive arithmetic module std-only, not coupled to
      schema generation or production funding.
- [x] Derive Vec growth from the pinned RawVec implementation and hash-table
      layout from the Rust toolchain's vendored hashbrown. A lifetime high-water
      bound survives clears; current length alone cannot replace it. State handles
      are shared Arc slices, not independently allocated vector/map payload copies.
- [x] Compose the independently admitted fixed-DFA hybrid population with
      retained transition/state/stack/builder capacities, fixed sparse/start storage,
      state-map storage, encoded Arc content plus headers/padding, and the temporary
      nine-byte sentinel builder. Fund one simultaneous old growth allocation using
      the largest applicable old-buffer bound; account for saved/pending payloads
      without pretending cache clearing frees retained collections. Apply the
      projection to both directions of all emitted helpers and hostname.
- [x] Extend the existing native allocation controls with real append-only
      vectors, state-shaped HashMaps and Arc-slice construction/cloning/release.
      Prepare inputs and report storage outside every measurement window. Check
      exact nonzero membership/counts and independent literal layout controls;
      observations are regression controls, not the all-path proof. The counter's
      realloc semantics do not measure hidden old/new physical overlap.
- [x] Run both focused feature lanes and the full serial linked-artifact suite
      because its native observation source changed. Complete independent
      source/compliance and quality reviews, strict lint, formatting and fixture
      gates; record exact scope before committing/pushing. Allocator overhead,
      fragmentation/reclamation, other meta-engine/program storage, constructor
      scratch, work, owner multiplicity and guest qualification remain open.

## 13. Complete Fixed-DFA State Certificates

Continue the approved bound derivation with a qualification-only checker in
`tests/support/schema_hybrid_graph.rs`, owned by `schema_cache_bounds` and the
hostname controls. This is not a generated/serving API, a new regex engine,
an input-sampling claim, or permission to change any engine/workload ceiling.
Keep the conditional recovery/population and heap models independently valid.
The checker uses separate exploration budgets of 2,048 discovered opaque IDs
and 8,388,608 engine API calls, including replay. These bound qualification
exploration only; they are not schema/auction workload limits or accepted
runtime cost descriptors. Exhaustion refuses rather than raising either budget.

- [x] Write failing colocated controls for a literal never-match graph, all
      start modes and 257 look-behind contexts, byte and terminal-EOI coverage,
      match-state expansion, and independently exhausted state/step budgets.
      Observe the expected failures before implementing traversal.
- [x] Use one fresh cache and only public opaque `LazyStateID` equality/hash.
      Seed `No`/`Yes` and each enabled valid per-pattern start with `None` and
      every byte as look-behind. Store a start configuration plus byte witness;
      replay it immediately before every byte or EOI edge. Never transition from
      a queued old ID, parse Debug output, cast private fields, or clone/reset the
      cache. Count replay calls against the exploration step budget.
- [x] Maintain all discovered non-sentinel states separately from byte-reached
      states awaiting expansion. EOI is terminal: count its result but enqueue it
      only if a byte witness independently reaches it. Expand matches, not only
      pre-match paths. Unknown results, any returned error, nonzero clear count,
      arithmetic failure or exhausted exploration budget refuse the certificate;
      partial exploration must never become a smaller accepted bound.
- [x] Record the pinned-source identity/closure lemma: encoded states are
      interned uniquely and stable within one no-clear epoch; all start/byte/EOI
      encodings are covered; prefilter skips and start tags add no encodings;
      clearing may reinsert an unchanged saved encoding. A completed graph with
      `R` ordinary encodings supports `R+3` indexed states for the same fixed DFA
      under section 11's independently established normal-recovery premise,
      not a whole-meta bound or an assertion that production never clears.
- [x] Apply the checker to both primary directions of all emitted helpers and
      the shared hostname. Pin exact dispatch membership, report actual completed
      counts, and retain failures as explicit unproved candidates. Do not infer
      coverage of optional reverse-prefix NFAs. Keep pending/saved payloads,
      retained collections, fallback/program owners and work proofs separate.
- [x] Run focused secure/feature lanes, independent source/compliance and
      quality reviews, strict lint/format/fixture gates, then commit and push the
      verified checker. The unchanged linked harness need not be rerun solely for
      this integration-owner-only follow-up; document the actual scope.

## 14. Bind Complete Graphs To Conditional Heap Projections

Use the completed graph only as another independently valid upper bound for
the same lifetime-fixed DFA. Keep the original recovery admission and coarse
projection available; no serving integration, production charge, workload,
engine or ceiling changes belong to this step.

**Files:** existing `tests/support/schema_hybrid_heap.rs`, the two integration
owners and this qualification record. Do not add another crate or allocator.

- [x] Write red literal arithmetic controls for independent coarse/graph state
      bounds, payload bounds, the five-state recovery floor and overflow before
      minima. For coarse `M/T/E`, graph ordinary population `R`, stride `d` and
      payload bound `V`, derive `M'=min(M,max(5,R+3))`, `T'=M'*d` and
      `E'=min(E,(M'+2)*V)`. Keep headers, map attempted occupancy `M'+1`, and old
      growth overlap as their existing separate owners.
- [x] Add a qualification-only `HybridHeap::from_complete_dfa` constructor.
      Construct recovery, graph and heap projections internally from that exact
      DFA; accept no caller-supplied graph/count receipt. Preserve unavailable
      recovery as unavailable, and return a distinct fixed typed refusal for
      graph failure. Never translate either into a zero-byte accepted cache. Add
      new-constructor controls for unavailable recovery, success and graph refusal.
- [x] Reuse the unchanged checked eight-owner heap sum. Add real-DFA positive
      and graph-refusal controls plus independent coarse-model comparisons; run
      the composed projection for every emitted direction and hostname without
      changing any bound until it fits. Require exact dispatch and non-increase
      against the coarse projection, with strict reduction on designated fixtures
      and valid equality controls, without treating a reduction as whole
      strategy/allocator/resource qualification.
- [x] Complete source/compliance and quality review; run focused secure/feature
      lanes, strict workspace lint, formatting and fixture gates. Record exact
      scope, update the still-open qualification issue, commit and push.

The source audit confirmed the pointwise minima on 2026-10-23: interning,
clearing and saved-state reinsertion preserve the fixed encoding universe;
saving clones an existing Arc, map growth moves handles, and retained
capacities depend on lifetime high-water rather than the current epoch.
Every sum/product is checked before taking its minimum. This source result
does not approve a production cost descriptor or cover optional-prefix owners.

## 15. Sufficient Reported-Capacity No-Clear Proof

This qualification-only step does not exclude whole-meta fallback, change an
engine limit, or provide a physical heap/work descriptor. It proves only that
neither prospective capacity check can force a first clear for the same fixed
DFA when its sufficient condition passes. Keep all earlier heap bounds valid.

**Files:** add `tests/support/schema_hybrid_capacity.rs`; extend the existing
graph certificate with its final **reported**, not physical, cache bytes; reuse
the same two integration owners and documentation.

- [x] Add a failing literal graph-report control and failing colocated scalar
      controls varying final report `F`, word width `w`, stride `d`, branch bound
      `B`, payload bound `V`, configured capacity `C` and ordinary count `R`
      independently. Define `S=4*max(4,2B)+2V`, `D=4d+4w+4+V`, `U=F+S+D` with
      checked arithmetic. Exercise below/at/above capacity, ID-space refusal,
      overflow, successful real constructors and unavailable/failed proof paths.
- [x] Bind the new constructor to one DFA. Validate the independently checked
      coarse heap/recovery model before observing the engine's unchecked report;
      the existing source bound must dominate those reported sums and establish
      nonwrapping accounting. Then complete the graph and derive `U` internally.
      Caller-supplied graph/report receipts are not accepted. Preserve typed
      recovery, graph, arithmetic, capacity and ID-space refusal dispositions.
- [x] Require `(R+3)*d < 0x07ff_ffff`, the pinned 32/64-bit raw-ID threshold,
      as well as `U<=C`. The exploration budget already places current candidates
      below that threshold; retain a separate arithmetic/control witness rather
      than confusing capacity fit with ID-space safety. Automatic upgrades,
      inherited/reset capacities, cloned caches and mutable configurations remain
      excluded by the existing constructor/lifetime premises.
      Transition histories must use covered starts, bytes and terminal EOI as in
      section 13; continuing from an EOI result is not part of this theorem.
- [x] Record the induction: before a hypothetical first clear, indexed
      encodings are a subset of the completed universe; its scratch-free reported
      ledger is at most `F`. `S` independently bounds scratch, even though `F`
      already includes observed scratch. `D` covers the prospective extra row,
      including the duplicate-builder check before interning. The bound is
      sufficient, not necessary, and does not turn reported bytes into allocation
      capacities, Arc headers, allocator cost or physical residency.
- [x] Evaluate all 46 emitted directions and both hostname directions, pinning
      exact membership and recording any unproved candidate without raising
      capacity or exploration limits. No-clear excludes the efficiency give-up
      branch only under this fixed-engine context. It does not establish actual
      meta strategy availability, optional-prefix coverage, bounded cumulative
      search-progress counters, no-panic behavior or whole-meta fallback exclusion.
- [x] Complete source/compliance and quality review; run focused secure/feature
      lanes, strict workspace lint, format and fixture gates. Record exact scope,
      update still-open issues, commit and push without a serving/readiness claim.

## 16. Optional Reverse-Prefix Cache Superset

AG approved the already-locked `regex-syntax 0.8.11` as a native-only test
dependency on 2026-10-23. Keep its resolved feature closure unchanged and
declare it centrally. No schema, serving engine, compiler patch, workload
limit, production funding or guest dependency changes in this component.

**Files:** root/bidder manifests and lockfile; add
`tests/support/schema_hybrid_prefix.rs`; reuse the same two integration owners,
qualification record and issue tracking. Inspect locked
`meta/reverse_inner.rs::{top_concat,flatten,InnerPrefilter::new}` and
`meta/strategy.rs::ReverseInner::new`, not regex text heuristics.

- [x] Add failing colocated controls using structured HIR: outer/nested
      captures, concat normalization that merges literals or removes concat,
      repetitions/alternations, start/end assertions, Unicode, and independent
      traversal/prefix refusal boundaries. Bound native inspection at 4,096 nodes,
      128 HIR levels and 64 candidate prefixes; exhaustion is unproved, not empty.
- [x] Parse emitted literals through the same UTF-8 syntax settings. Locate
      a top-level concat through outer captures, remove captures recursively with
      HIR smart constructors, and normalize the concat again. Enumerate every
      nonempty proper normalized-concat child slice `[..i]`, `1 <= i < len`,
      regardless of prefilter speed or
      earlier-match/start-anchor gates. This is a conservative finite superset of
      the prefix selected by the pinned strategy, not a claim it is installed.
      If normalization is no longer `HirKind::Concat`, there are no candidates.
      Nonempty describes the slice, not its match length: retain assertion-only
      and other zero-byte-matching prefixes.
- [x] Preserve the unpruned candidate API and its refusal. Add a separate typed
      strategy-admission result using the pinned single-pattern
      `props_union().look_set_prefix().contains(Look::Start)` predicate: ReverseInner
      refuses that strategy before constructing any prefix NFA/cache. Only this
      source-proven general gate may exclude an anchored pattern, never its bidder
      or format name. Test always/partially anchored alternations, multiline and
      literal carets independently. Record the unpruned hostname capacity-clear
      refusal separately from its absent optional owner; this does not affect the
      hostname's already-proved primary forward/reverse caches.
- [x] Compile each candidate HIR with the same reverse/capture-free/no-shrink
      NFA settings, newline terminator and 65,536-byte NFA limit. Use the existing
      reverse/shared-start hybrid config. Verify direction, real capture-free
      metadata, coarse checked heap/recovery admission before graph traversal or
      its unchecked engine report, complete graph, conditional heap and
      sufficient capacity proof. Preserve every typed refusal; do not enlarge caps.
- [x] Audit all 23 emitted helpers and the hostname hook, pinning exact helper
      membership and non-vacuous positive prefix cases. Record which owners have
      no possible top concat versus candidate count; do not silently skip any.
      A meta strategy retains at most one selected optional prefix cache, so its
      hypothetical contribution uses the maximum over proved alternatives, not
      their sum. Compute it only after every candidate qualifies; any refusal
      leaves the owner unproved. Primary pair and other engine/program owners
      remain separate.
- [x] Review source correspondence and test quality, rerun focused lanes,
      strict workspace lint, format and fixture gates. Because the dependency
      manifest changes, rerun the existing serial linked/provenance controls and
      verify unchanged locked engine features. Record scope, commit and push;
      optional-prefix superset coverage alone does not close whole-meta/work,
      allocator/target qualification or enable serving validation.

## 17. Explicit Quit Exclusion

This source-derived native component closes only quit-set admission. It is
not unconditional NFA-fallback exclusion, actual strategy selection, progress-
counter lifetime or a whole-engine work/memory descriptor.

**Files:** extend `tests/support/schema_hybrid_bounds.rs` and prefix qualification;
reuse the two existing integration owners and documentation. No dependency,
compiler patch, engine/input limit, funding or serving changes.

- [x] Add failing colocated controls for Unicode word assertions versus ASCII
      word assertions, forward/reverse NFAs, every explicitly configured quit byte
      including non-ASCII, and removal of a configured quit byte. Exercise real
      engine quit behavior separately from static admission.
- [x] Add a fixed typed refusal if `nfa.look_set_any().contains_word_unicode()`
      or any of the 256 `dfa.get_config().get_quit(byte)` values is true. This
      conservatively follows locked `Config::quit_set_from_nfa`; ordinary Unicode
      text/classes alone are not Unicode word assertions. Take the same immutable
      DFA used by subsequent proofs, not caller-supplied booleans/receipts.
- [x] Apply the guard independently to all 46 primary directions, both hostname
      directions and every retained optional-prefix alternative. Preserve keyed
      typed refusals and exact membership; an unsafe alternative cannot disappear
      through a partial owner maximum. Keep structural candidate/exclusion gates
      and failed graph dispositions distinct.
- [x] Review source correspondence and test quality; rerun focused lanes,
      strict workspace Clippy, format/diff and fixture gates. Record exact scope,
      commit and push. Existing public builder success plus source correspondence
      proves primary hybrid availability only if Core is constructed; no private
      strategy observation is inferred. Remaining counter-progress, eager capture/
      available one-pass storage, shared program and allocator/work/target
      obligations stay open.

## 18. Fresh Core Capture and One-Pass Ownership

This component bounds only the auxiliary requested heap of a freshly created
Core cache, before searches or reset. It does not supply primary/optional hybrid
storage, shared programs, physical allocation, work or a production descriptor.
Read both `meta/strategy.rs::Core::create_cache` and the wrapper constructors:
PikeVM and backtracker wrappers start at `None`, not at their engines' allocated
caches. Core capture slots and available one-pass scratch are eager.

**Files:** extend `schema_heap_layout.rs`; add `schema_meta_eager.rs` under the
existing test-support directory and reuse the two integration owners. Extend
the existing isolated allocation probe and its receiving guards, not another
crate, executable, dependency or serving API.

- [x] Add failing controls for fresh zero/positive resize layouts and repeated
      slot arrays. Pin minimum-capacity, 32/64-bit request limits, multiplication
      overflow and shape refusal from independent literals. These fresh histories
      do not reuse the append-growth bound or claim reset/clone admission.
- [x] Derive capture bytes from `NFA::group_info().slot_len()` and optional
      one-pass scratch from checked total-minus-implicit slots. Conservatively fund
      a fitting one-pass candidate even when meta may skip it or construction may
      fail. Do not infer private strategy selection. Sum the disjoint capture and
      one-pass terms with checked arithmetic. Exclude lazy Pike/backtracker buffers
      only from this fresh-construction component, not from whole-meta obligations.
- [x] Use same-NFA constructors for all 23 exact emitted helpers and hostname;
      retain keyed outcomes and separate 32/64-bit projections. Exercise independent
      slot/pattern/word axes, absent/zero/maximum one-pass slots, malformed shapes
      and each aggregate owner through literal controls.
- [x] Extend native allocation controls with real meta caches constructed with
      hybrid disabled only to isolate these owners. Capture-bearing and no-capture
      cases must prove positive construction and release; toggle one-pass separately
      to distinguish its term from capture storage. Measure actual standalone fresh
      Pike/backtracker caches separately to demonstrate the lazy-wrapper distinction.
      All regex compilation, report/input allocation and verification stay outside
      measured windows. Receivers pin case identity/count, independent literal byte
      expectations, and positive work so omission cannot pass as zero usage.
- [x] Review source and quality, then run focused secure/feature tests, the
      serial linked-artifact controls (probe changed), strict workspace Clippy,
      format/diff and both fixture gates. Record results and scope; commit and push
      separately. Do not infer engine fallback exclusion, lifetime/counter bounds,
      target physical storage or serving readiness from this fresh owner model.

## 19. Retained Lazy Fallback Requested Heap

Bound initialized PikeVM/backtracker caches before composing a whole meta-cache
envelope. Fresh wrapper absence does not exclude later fallback initialization.
Use the unchanged earliest-only search contract and a fixed NFA/cache history:
new, serial searches and drop, with no reset, clone or cross-engine reuse.

- [x] Add red colocated arithmetic controls for four independently resized
      sparse-set arrays, two independently resized Pike slot tables and its
      append-grown epsilon stack. Keep meta captures and one-pass scratch with
      their existing eager owner, not counted again here.
- [x] Bound backtracker bitmap and stack from the largest admitted earliest
      span: the minimum of the declared input bound, 128 bytes and the default
      visited-capacity constraint. Include retained capacities across shorter
      searches, both stacks simultaneously, and only the maximum serial growth
      overlap. Exercise independent dimensions, word widths and overflow.
- [x] Apply same-NFA constructors to all 23 emitted helpers and hostname on
      both word widths. Reconcile exact keyed membership rather than discarding
      refusals or reporting a partial maximum.
- [x] Extend the existing isolated allocator probe with real fresh/search/reuse/
      release fallback controls, positive work and independent expected results.
      Compare actual requested storage with the derived owners, not only the
      engines' length-based memory reports.
- [x] Review source correspondence and quality; run focused secure/feature
      controls, linked probes if changed, strict lint, formatting and fixture gates.
      Commit/push this bounded increment. Whole-meta composition, cumulative work,
      physical/target residency and serving qualification remain open.

### Fallback Verification (2026-10-23)

Rust 1.98.1; all Cargo commands use `CARGO_INCREMENTAL=0`,
`--offline --locked -j2`. Source controls receive the provided checkout through
`AUCTIONEER_APS_SOURCE`/`AUCTIONEER__BIDDER__SOURCE_DIR`; required loopback fixtures run
outside the socket-restricting sandbox. No live provider is contacted.

- The six colocated projection controls first compiled against a refusing
  placeholder: five failed on the missing positive/arithmetic behavior. The
  constructor control was then strengthened to unwrap both positive results,
  including a capture-free reverse NFA, instead of comparing two possible
  errors. Literal expectations are independent of the projection coefficients.
- The real allocator receiver first failed on its absent report. The quality
  review subsequently found that its positive totals could hide inconsistent
  phase data. Three new negatives failed against the summary-only receiver;
  independent phase reconstruction repaired them. The final seven receiver
  controls include six targeted refusals and a positive control, with distinct
  retained/peak caps (24/40) and a one-byte retained-cap refusal (19 versus 20).
- `cargo test -p auctioneer-bidder --test schema_cache_bounds
--test schema_compiler_controls --test schema_source_inventory
--test schema_initialization --test schema_cache_controls
--test schema_cache_initialization -- --quiet --test-threads=2` passes
  370 tests in each of secure-default and `--features insecure-bidders` modes:
  161/153/40/2/13/1 by target, zero failed/ignored.
- The complete `--test schema_build_controls -- --quiet --test-threads=1`
  passes 161 tests in each mode. Standard and explicit candidates each link
  all 261 callbacks on native and three guest targets, retain the original
  300 reference results, and execute actual compiler/API/refusal controls.
  This is not guest execution or full resource qualification.
- The first complete linked run exposed a stale explicit-inventory path from
  the preceding asset cutover (160 passed, one failed). The corrected reader
  uses bidder-owned inputs; no assertion was removed or source capture changed.
- Bidder `--lib -- --include-ignored` passes 280 secure-default and 291
  feature-enabled tests, including actual APS source-object verification.
  The discovered-source formatter exposed an existing APS-only formatting
  failure; the mechanical repair and license-comment correction preserve
  protocol behavior and captures. `cargo fmt` alone cannot check that file.
- Separate source/spec and quality reviews approved this conditional owner
  increment after the phase-reconciliation fix. Fixed-NFA/earliest/lifetime
  assumptions and the conditional private-frame ABI allowance remain explicit.
  No dependency, compiler patch, workload ceiling, funding coefficient or
  serving callback changed. Full Task 3 acceptance remains open.
- Workspace/all-target Clippy passes with `-D warnings` under defaults and
  `--features insecure-bidders,conformance-suite`. `cargo fmt --all -- --check`,
  direct Rustfmt checks of discovered bidders and the allocation probe,
  whitespace checks and both raw-fixture gates pass (170 fixtures, 51 files).
  The selected suites total 1,633 successful test executions, not distinct
  tests or a complete workspace assertion run. Existing GVL deletion notices
  and the dependency future-incompatibility warning remain visible; no lint
  suppression or raw-fixture allowance was added.

## 20. Compose Fixed Meta Cache Requested Heap

Compose the existing conditional native owner envelopes for one fixed cache's
new/serial-earliest-search/drop history. This is not a shared-program,
allocator/RSS, construction-work, target-layout or serving qualification.

- [x] Add red scalar controls with independently varied eager, forward, reverse,
      optional-prefix and fallback terms. Sum all coexisting live envelopes and add
      only the largest serial collection-growth overlap. Charge the optional
      prefix's entire peak as a conservative fixed allowance; its alternatives
      are mutually exclusive, not a sum or a reason to omit that owner.
- [x] Bind every component to the same parsed emitted pattern. Preserve the
      existing capacity/quit/recovery/graph refusals with directional diagnostics;
      never return a partial total when any required component is unproved.
- [x] Walk all 23 emitted helpers from the complete 261-schema inventory and
      the separate hostname format. Require exact keyed result membership and
      positive owners. Keep declared full-haystack lifetime bounds distinct from
      the backtracker's short-input admission rule.
- [x] Review the actual locked source and code quality, run secure/feature
      controls, strict lint/format/fixture checks, and publish this increment. Do
      not enable serving or manufacture a usable production cost descriptor.

### Composition Verification (2026-10-23)

The five scalar controls first compiled against a refusing placeholder and
failed their intended assertions. Implementing the checked join passed those
controls while the two new constructor controls remained red; the same-pattern
constructor then passed both. The eighth colocated control, added during quality
review, first failed on the generic diagnostic instead of the required stage
label. Every projection now explicitly preserves its typed forward/reverse
NFA, layout, eager, fallback or composition stage.

The source/compliance review identified a weak directional-binding oracle.
The hostname integration now pins the complete asymmetric native 64-bit result
using the prior committed forward/reverse bounds and independent addition.
Substituting the forward candidate for reverse compiled and failed the intended
exact assertion: reverse 135,569 versus 106,372, peak 1,559,762 versus 1,530,565.
The mutation was restored. An initially mistyped exact filter ran zero tests;
it was discarded, then corrected to the complete `tests::` name before the
one-test mutation run. Both independent reviewers rechecked their corrections
and found no remaining actionable issue. They inspected source, not Cargo runs.

- With Rust 1.98.1, `CARGO_INCREMENTAL=0`, `--offline --locked -j2`, bidder
  library plus six focused integration targets pass 668 secure-default and
  679 `insecure-bidders` executions, zero failures/ignores. The library counts
  are 280/291, with `--include-ignored` and actual APS source verification.
  The six targets are `schema_cache_bounds`, `schema_compiler_controls`,
  `schema_source_inventory`, `schema_initialization`, `schema_cache_controls`
  and `schema_cache_initialization`: 170/162/40/2/13/1 in either feature mode.
- After lint-only identifier/import cleanup, the two changed integration
  targets pass again in both modes: 170 plus 162, zero failures/ignores. Counts
  include reused colocated support controls; they are not distinct proofs.
- Strict workspace/all-target Clippy passes under defaults and under
  `insecure-bidders,conformance-suite`, both with `-D warnings`. No lint allow
  was added. Workspace formatting, direct discovered-bidder/probe/helper
  Rustfmt checks, whitespace checks and both raw-fixture gates pass
  (170 fixtures across 51 files). Existing GVL deletion notices and the
  dependency future-incompatibility warning remain visible.

Only qualification support, its two integration owners and documentation
changed. No compiler/probe/schema/dependency/serving input or funding/limit
changed; the previous complete linked-artifact suite was not repeated. No full
workspace assertion run, guest execution, fresh physical measurement, usable
descriptor, full M7 completion or R0 readiness is claimed. Shared programs,
constructor/work/lifetime bounds, whole-schema/format overlap, production
owner multiplicities and physical/target qualification remain open. This
increment adds no bidder protocol implementation or activation.

## Verification Record

On 2026-10-22 the initial workspace control compilation failed on the old
compiler's unknown `cache` option, as intended. The resolved automata feature
sets before and after the direct dependency were identical (25 features),
including the same engines and no full-DFA defaults.

A subsequent workspace build stopped at the unchanged expired GCL gate.
An ignored, isolated manifest under `target/cache-api-control` therefore
compiled the real integration-test sources against the same pinned compiler
and engine, without Auctioneer configuration dependencies. Its 13 API/AST controls
and one constructor/cache-lifecycle control passed. This caught and repaired
a typed-closure lowering panic before publication. These runs are not a
substitute for workspace secure/feature tests or Clippy; at that point those
were pending an authorized archive refresh.

The same isolated manifest subsequently ran all 96 linked-artifact/provenance
controls, plus the 14 API/lifecycle controls: 110 passed, zero failures or
ignores. Both default and explicit modes link all 261 schemas on native,
`wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`. Each mode retains
all 300 captured results (106 accepts, 181 schema refusals, 13 JSON refusals)
and the same extra literal numeric/format/conditional/Unicode cases. Explicit
bundles also cover 861 repeated evaluations of 287 decoded source values
across 14 schemas. Eleven actual Cargo refusals cover seven cache-ABI cases
and four incompatible modes; a valid sibling must compile first.

The first full run ended 93/2 because the sandbox denied the reference tests'
loopback listener. Repeating with loopback permission passed; neither control
was skipped or relaxed. Independent reviews found and closed a missing shared
literal-control call and missing actual-Cargo option refusals. Clippy for these
three targets passed with the workspace lint tables copied structurally into
the isolated manifest and `-D warnings`. One test-only expectation documents
Syn's intentional inheritance of unused visitor methods; no workspace lint
policy changed. Formatting and both raw-fixture gates pass (168 raw JSON
fixtures across 50 files).

These are compiler/compatibility observations, not guest execution or a byte,
work or complete owner-layout proof. Constructor/cache counters do not measure
internal engine growth. No qualification descriptor or serving callback is
enabled. The isolated runs did not replace the blocked workspace checks.

### Workspace Follow-Up

AG authorized the existing Rust CLI archive refresh on 2026-10-22. The
data-only refresh was committed separately as `c785b454`, after three archive
tests and 66 GVL-schema tests passed. It installs current spec-3 GVL v182 and
a fresh GCL without changing archive limits, the SDK pin or the toolchain.

With that gate restored, the commands above passed in the normal workspace
on Rust 1.98.1:

| Scope                                                                                   | Result                               |
| --------------------------------------------------------------------------------------- | ------------------------------------ |
| Secure-default API, lifecycle, compiler and source-inventory targets                    | 66 passed, zero failed or ignored    |
| The same four targets with `insecure-bidders`                                           | 66 passed, zero failed or ignored    |
| Sequential `schema_build_controls`, with local loopback permission                      | 96 passed, zero failed or ignored    |
| Workspace all-target Clippy with `insecure-bidders,conformance-suite` and `-D warnings` | Passed                               |
| Workspace formatting and both raw-JSON fixture gates                                    | Passed; 168 fixtures across 50 files |

The linked controls again preserved all 300 captured outcomes in each mode
and 861 repeated explicit-cache evaluations, and linked all 261 validators
on native and the three WASM targets. Existing GVL deletion notices and the
dependency future-incompatibility notice remain. This was focused testing
plus workspace linting, not a full workspace test run or guest execution.
Complete cache-owner/internal-growth/format bounds and resource qualification
remain open; no serving callback or qualification descriptor is enabled.
The cache API patch was committed and pushed as `14c51f7b`; unrelated roadmap
changes were excluded from both that increment and the archive refresh.

### Explicit Allocation Observations

This subsection records the historical pooled-format layout at `72d7dbae`.
The later per-call hook observations are recorded separately below.

The new measurement control first failed with the existing explicit probe's
`invalid probe input` refusal. The implementation shares the existing meter
and 62 workload inputs with the default probe; it does not reimplement the
allocator. The focused control then passed with all 261 schema names, exact
1/2/4-owner populations, 4,522 searches, four separately observed hostname
inputs and a zero net requested-heap delta over every bundle lifetime.
The existing positive, zero, release and cross-thread meter controls also run
inside the explicit artifact. Input preparation and report serialization stay
outside windows; the native-only modules are guarded in generated source.

On `aarch64-apple-darwin` with Rust 1.98.1, shared constructors requested
1,895,958 bytes in 5,001 calls, peaked at an incremental 497,825 bytes and
retained 173,198 bytes. The probe enum is 4,208 inline bytes per owner, reported
separately; enum containers are excluded from the measured heap. The sampled
4,522 searches requested 82,775 bytes in aggregate, not a peak or upper bound.
The hostname pool separately retained 11,208 bytes on its first search and
another 29,848 bytes on the 255-byte input. Its sampled warm repeats allocated
nothing; this does not bound unexercised inputs, thread histories or pools.

An independent locked-source audit confirmed that fallback caches can retain
PikeVM/backtracker storage alongside hybrid storage, forward/reverse caches
need distinct accounting, and `Cache::memory_usage()` omits capture/inline
storage and several capacity/overlap details. Full byte/work/target proofs
and actual production-owner selection remain open. No cost descriptor,
funding coefficient, workload limit, resource ceiling or serving path changed.

Verification of this observation increment on 2026-10-22:

- The full sequential `schema_build_controls` target passed 98 tests with
  zero failures or ignores, including both modes' four-target links,
  unchanged captured outcomes and the default allocation regressions.
- Secure-default library plus the four focused schema targets passed 270
  tests; the same scope with `insecure-bidders` passed 281. No test was ignored.
- Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
  `-D warnings` passed after replacing unchecked test sums and splitting
  assertion helpers. No lint exception or relaxed assertion was added.
- Workspace formatting, explicit formatting of the standalone probe helpers,
  and both raw-fixture gates passed (168 fixtures across 50 files).
- Temporarily removing the generated native-only guard failed its named
  structural test; restoring the guard passed. This and the original
  missing-command red reached their intended refusals; no temporary mutation
  remained during final verification.
- Independent spec-compliance and code-quality reviewers approved this
  observation scope; the latter rechecked the final assertion refactor.

These are not a full workspace test run, a physical guest-memory measurement,
a complete internal capacity bound or approval to enable serving validation.

### Per-Call Hostname Follow-Up

The zero-retention regression first failed against the old pooled hook with
11,208 live bytes after the short hostname call. Replacing only the
qualification hook's builder and search with shared meta-regex initialization
and a callback-local explicit cache passed that control. Five exact input rows
cover short, valid 255-byte, invalid-tail, overlong-label and rejected 256-byte
names. Initial and repeated calls retain zero requested heap. Every shorter
sample allocates positively inside its call; the 256-byte input allocates none.

On `aarch64-apple-darwin`, Rust 1.98.1, the shared constructor pass requests
1,894,769 bytes in 4,997 calls, peaks at 496,849 and retains 172,222. Short calls
request 12,913 bytes each; valid 255-byte calls request 67,993, peaking at
41,056. The 4,522 enclosing bundle searches request 3,851,701 aggregate bytes,
including per-call hostname scratch. Every measured bundle lifetime balances
to zero; inline probe-owner size remains 4,208 bytes. These are observations,
not limits, work bounds, RSS or guest-memory qualification. See the
[qualification record](../../bidders/schema-qualification.md#per-call-hostname-cache-observations)
for all five rows and the unchanged remaining obligations.

Verification of this follow-up on 2026-10-22:

- Sequential `schema_build_controls` passed 98 tests, zero failures or ignores,
  with loopback permission for the real fixture listener. Both generated modes
  linked all 261 callbacks on native and all three guest targets; both retained
  all 300 captured outcomes and the explicit mode's 861 repeated evaluations.
- The library and four focused targets passed 271 tests under secure defaults
  and 282 with `insecure-bidders`, zero failures or ignores. The additional
  colocated configuration control pins matching mode, UTF-8 empty handling,
  both builder limits and the unused pool capacity.
- Strict workspace all-target Clippy with `insecure-bidders,conformance-suite`
  and `-D warnings` passed. Its initial run rejected two decimal literals in the
  new test; only their notation changed to hexadecimal, with no lint allowance
  or value change. Both focused scopes and Clippy passed again afterward; the
  full linked run preceded that test-only notation correction.
- Workspace and standalone-helper formatting, `git diff --check`, and both
  raw-fixture gates passed (168 fixtures across 50 files). Existing GVL deletion
  and transitive future-incompatibility notices remain.
- Independent read-only spec-compliance and code-quality reviewers found no
  actionable issues. They did not rerun tests or approve complete byte/work
  qualification. The unrelated roadmap changes remain outside this increment.

The full workspace test suite and guest execution were not run. Source-derived
capacity/work bounds, real production-owner multiplicity and complete target
qualification remain open; resource costs and serving integration stay disabled.

### Forward NFA Component Verification

The first scalar projection stub failed three positive controls. The NFA
reconstruction and emitted-literal stubs then failed their intended positive
controls. Each dimension is varied independently with literal expected counts;
overflow controls separately reach slot multiplication, scratch addition,
final doubling and stack addition. Repeated captures deliberately have more
capture states than slots. The epsilon-branch case uses overlapping literals,
since disjoint leading literals can optimize to sparse transitions.

Independent source review confirmed the formulas and exact forward compiler
configuration. The first implementation review found that constructor-only
counts could miss extra cache slots. An added unmatched-slot control failed
against that implementation, then passed after per-owner reconciliation of
all constructors, typed fields and initializers. Empty cache owners and their
factories remain mandatory even when no regex helper is emitted. Unsupported
generated mutable state refuses rather than being omitted from this slice.
The compliance re-review and a fresh quality review found no further issues.
Both reviewers were read-only; they did not rerun Cargo or approve full resource
qualification.

Verification on 2026-10-22, Rust 1.98.1:

- `schema_cache_bounds`: 37 passed, including 12 colocated model/source-shape
  controls, 23 reused inventory controls and two integration cases. The source
  walk covers all 261 schemas, 23 helpers in 14 owners, and the independently
  literal ASCII translation of `\d+` to `[0-9]+`.
- `schema_compiler_controls`: 42 passed, including the reused component controls
  and shared hostname-pattern projection. The hostname has 265 states, eight
  capture slots, 125 pending branch frames and eight capture states; expected
  slot-array/stack counts are pinned independently of the projection function.
- Library plus the five focused schema targets passed 321 tests under secure
  defaults and 332 with `insecure-bidders`, zero failures or ignores. Counts
  include shared unit controls compiled into multiple integration binaries;
  they are executed cases, not distinct proofs.
- Sequential `schema_build_controls` passed all 98 tests with loopback access,
  including both modes' four-target links, all 300 unchanged captured outcomes
  per mode, and 861 repeated explicit-source evaluations. Moving the unchanged
  hostname literal to a shared constant preserved allocation observations.
- Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
  `-D warnings`, formatting of the workspace and standalone helpers, diff checks,
  and both raw-fixture gates passed (168 fixtures across 50 files). Initial lint
  failures were repaired without allowances or weaker assertions. Existing GVL
  deletion and transitive future-incompatibility notices remain.

This is not a full workspace test run or guest execution. The
[logical-storage derivation](../../bidders/schema-qualification.md#forward-nfa-logical-storage)
does not supply retained capacities, allocator/layout bytes, whole-meta work,
production-owner multiplicities or a usable C/I/V/K descriptor. None of those
remaining obligations, workload limits or resource ceilings changed. The
unrelated roadmap edit stays outside this commit.

### Requested Heap Component Verification

Verification on 2026-10-23, Rust 1.98.1, locked/offline dependencies and two
jobs. This remains conditional requested storage, not physical memory or
whole-meta qualification.

- Primitive stubs failed four independent-literal controls; hybrid composition
  stubs failed four. Source review identified the four sparse vectors' minimum
  capacity: two revised controls failed before adding the `max(S,16)` floor.
- Six negative receiver controls failed against empty guards before repairs,
  with positive/empty controls retained. They require actual vector length and
  positive calls/bytes, actual map repopulation, and Arc counts three then one.
  Exact duplicate-triggered map-growth membership is `{3,7,14,28,56}`. Removing
  only the map request-limit check fails the adjacent 32-bit boundary control;
  the restored guard rejects 3,489,660,944 requested bytes without overflow.
- Final library/focused run: 419 passed under secure defaults and 430 with
  `insecure-bidders`, zero failures or ignores. Targets were `schema_cache_bounds`,
  `schema_compiler_controls`, `schema_cache_controls`, `schema_initialization`
  and `schema_source_inventory`. Reused module controls execute in multiple
  owners; these totals are not counts of independent proofs.
- Final serial `schema_build_controls`: 110 passed, zero failures or ignores,
  with loopback permission. Both generated modes linked 261 callbacks on native
  and all three guest targets, retained all 300 captured outcomes per mode and
  the explicit mode's 861 repeated evaluations. Native controls exercised
  30 vector, 11 map and 13 Arc rows; constructor/search observations stayed
  unchanged. An earlier 106-test run preceded the additional ownership guards.
- Strict workspace/all-target Clippy with `insecure-bidders,conformance-suite`
  and `-D warnings`, workspace/standalone formatting, diff checks and both raw
  fixture gates passed (168 fixtures across 50 files). Lint repairs added no
  allowances; existing GVL deletion and dependency future-incompatibility
  notices remain. Source/compliance and subsequent quality reviews found and
  rechecked the fixes above; the reviewers ran no Cargo themselves.

No full workspace tests, guest execution, usable resource descriptor or serving
cutover is claimed. Source cache bounds still need whole-meta/program/format
composition, construction/work, allocator/target proofs and real owner
multiplicities. Section 13 is the reviewed next qualification-only step, not an
implemented certificate. The unrelated roadmap edit remains outside the commit.

### Complete Graph Component Verification

On 2026-10-23 the initial stub failed six of seven controls at the expected
assertions. Source investigation corrected the never-match oracle: Fail is a
retained ordinary start encoding, not the dead sentinel. The 1,799-call literal
derives from 514 starts plus 257 two-call and 257 three-call replay edges.

- All 46 exact emitted helper/direction keys and both hostname directions
  completed, with no changed cache limits or refused/partial certificates.
  Maximum helper population/API calls were 62/192,236. Hostname forward was
  251/2,217,139 and reverse 128/1,087,110; all have 771 seed configurations.
- Ten colocated controls cover actual seed identity, ordinary versus terminal
  population, byte-reached match expansion, state/step limits, clear epochs,
  engine refusal and zero budgets. Two real-engine mutations fail separate
  controls: first-discovery-only enqueueing loses a later byte witness, and
  deleting terminal accounting loses the independently constructed third
  ordinary encoding. Collapsing per-pattern IDs and look-behind bytes also
  fails the distinct-root/population controls; all mutations were restored.
- Source/compliance and subsequent quality reviews found and rechecked the
  independent dispatch, terminal accounting and seed-identity guard repairs.
  They found no checker-core defect and ran no Cargo themselves.
- Final library/focused runs passed 441 cases under secure defaults and 452
  with `insecure-bidders`, zero failures or ignores. Same five targets as the
  heap component; reused controls execute in both integration owners.
- Strict workspace/all-target Clippy with `insecure-bidders,conformance-suite`
  and `-D warnings`, formatting, diff checks and both raw-fixture gates passed
  (168 fixtures across 50 files), without new lint allowances. Existing GVL
  notices and dependency future-incompatibility notices remain.

Only the qualification checker, integration owners and documentation changed.
The linked probe source/generator/compiler/dependencies did not change, so the
earlier 110-test linked run was not repeated for this follow-up. Neither full
workspace tests nor guest execution ran. No heap projection consumes the graph
yet, and no production descriptor, serving cutover or complete byte/work/target
qualification is claimed. Section 14 is the reviewed next component. The user
roadmap edit remains excluded.

### Graph-Bound Heap Verification

Verification on 2026-10-23, Rust 1.98.1, locked/offline dependencies and two
jobs. All five constructor/minima stubs failed their intended assertions
before implementation. Six final colocated controls cover independent coarse
and graph bounds, equality, the recovery floor, overflow before minima, two
different positive NFA shapes, unavailable recovery and typed graph refusal.

The native literal never-match envelope is `916/116/1032` live/overlap/peak;
the independent Byte/End/Match NFA is `1241/116/1357`. Clamping successful graph
populations to one fails only the new positive control while the earlier five
controls stay green; the mutation was restored. The 32-bit expected branches
were independently recomputed by source review, not executed as a 32-bit
application or guest.

- Existing integration owners evaluate all 46 emitted directions and both
  hostname directions. General envelopes never increase; designated hostname
  live/peak envelopes strictly decrease. Conditional peaks are 151,821 forward
  and 114,752 reverse, with unchanged eight-owner/growth accounting.
- Final library/focused runs passed 453 secure-default and 464 feature cases,
  zero failures or ignores. Counts include reused controls in both integration
  owners; they are not distinct-proof counts. Both independent review stages
  found no remaining issues; reviewers ran no Cargo themselves.
- Strict workspace/all-target Clippy with `insecure-bidders,conformance-suite`
  and `-D warnings`, formatting, diff checks and both raw-fixture gates passed
  (168 fixtures across 50 files), without new lint allowances. Existing GVL
  deletion and dependency future-incompatibility notices remain.

Only qualifier arithmetic, its integration owners and documentation changed.
The probe source/generator/compiler/dependencies stayed unchanged; the earlier
110-test linked run was not repeated. No full workspace tests, guest execution,
physical allocator bound, whole-meta total, usable descriptor or serving
cutover is claimed. Production charges, limits and ceilings did not change.
The unrelated roadmap edit remains excluded.

### Reported-Capacity Verification

On 2026-10-23 the completed-graph report control failed with zero instead of
the independent 255-byte native oracle. All eight initial capacity stubs then
failed their intended assertions before implementation. Nine final colocated
controls exercise independent report/width/stride/stack/payload axes, exact
capacity, raw-ID exhaustion, checked arithmetic, two positive NFA shapes,
unavailable recovery, graph clearing and coarse-heap overflow before a small
completed graph. The last control also requires recovery-only admission to
succeed; bypassing the full checked envelope changes its result from arithmetic
refusal to an accepted 369-byte proof and fails the assertion. The mutation
was restored before final verification.

- Exact proved sets include all 46 emitted directions and both hostname
  directions, with no capacity or exploration changes. Helper requirements
  peak at 23,784 reported bytes; hostname forward/reverse require 38,744/24,608
  versus their unchanged 65,536-byte capacities. Both sweeps retain keyed typed
  refusals and report the cohort before asserting exact proved membership.
- Final locked/offline, two-job library/focused runs passed 471 secure-default
  and 482 `insecure-bidders` cases, zero failures or ignores. The five existing
  integration targets are `schema_cache_bounds`, `schema_compiler_controls`,
  `schema_cache_controls`, `schema_initialization` and `schema_source_inventory`.
  Shared colocated controls run in both owners; counts are not unique proofs.
- Source/compliance review found no mathematical flaw; its refusal-reporting
  finding was repaired and rechecked. Quality review found no blocking defect;
  its missing coarse-envelope control was added and mutation-verified above.
  Reviewers performed source inspection, not independent Cargo execution.
- Strict all-target workspace Clippy with `insecure-bidders,conformance-suite`
  and `-D warnings`, formatting, diff checks and both raw-fixture gates passed
  (168 fixtures across 50 files). No lint allowance was added; the existing
  GVL deletion and dependency future-incompatibility notices remain.

Only qualification helpers, their two integration owners and documentation
changed. Probe/generator/compiler/dependencies stayed unchanged, so the earlier
110-case linked-artifact run was not repeated. No full workspace tests, guest
execution, physical heap bound, whole-meta fallback exclusion, usable descriptor
or serving readiness is claimed. Production funding, limits and ceilings are
unchanged. The user-owned roadmap edit remains excluded.

### Optional-Prefix Verification

On 2026-10-23 the initial structural stubs failed eight of nine controls at
their intended assertions. Subsequent maximum/empty-owner stubs failed two
controls, and the explicit admission stub failed its anchored-disposition
control. Fifteen final colocated controls run through both existing integration
owners, alongside exact helper and hostname cohort checks.

The unpruned helper table contains 43 alternatives across 23 helpers; all pass
under existing caps. The separate original-HIR admission table is 16 anchored
exclusions, five no-concat cases and two eligible owners with five alternatives.
Hostname's unpruned cache exploration clears and remains a typed refusal.
Source inspection established the exact generic anchored predicate, whose
separate disposition excludes that optional owner before engine construction.
No name-based exception, raised cap or accepted partial bound was introduced.

- Disabling capacity-refusal propagation accepts a partial one-alternative
  2,203-byte envelope; the both-order refusal control fails. The mutation was
  restored before final runs. Independent maximum axes, explicit literal-prefix
  component binding and syntax/inspection refusal controls remain active.
- Final locked/offline, two-job library/focused runs passed 503 secure-default
  and 514 feature cases, zero failures or ignores, across the same five focused
  targets. Shared controls count twice, not as distinct independent proofs.
- The actual serial `schema_build_controls` run passed all 110 cases in
  245.69 seconds. Both modes link all 261 schemas on native and all three guest
  targets; existing 300-case compatibility/provenance and repeated explicit
  evaluation controls pass. This is linking/native execution, not guest execution.
- The only dependency addition is the approved native test edge to existing
  `regex-syntax 0.8.11`. Its ten resolved features are byte-identical before
  and after; the existing automata closure retains 25 features and no full-DFA
  build feature. The lockfile adds one direct edge, no package/version update.
- Source correspondence and independent quality reviews found no remaining
  actionable findings in this component. Reviewers inspected code and sources;
  they did not run Cargo. Strict workspace Clippy with all targets,
  `insecure-bidders,conformance-suite` and `-D warnings`, format/diff and both
  raw-fixture gates passed (168 fixtures across 50 files), without new allowances.

No serving code, schema, compiler patch, production funding, workload/engine
limit or resource ceiling changed. Complete meta availability/fallback,
counter lifetimes, shared program/constructor storage, physical allocator and
target/work descriptors remain open. No full workspace test run, guest execution,
usable descriptor, full M7 completion or R0 readiness is claimed. The separate
APS contract is source-reviewed and records AG's approved decisions; its
implementation/evidence are not supplied by these cache controls. The unrelated
roadmap edit remains excluded.

### Quit-Admission Verification

On 2026-10-23 the permissive admission stub failed the Unicode and configured-
byte controls. Before strict-constructor wiring, the Unicode fixture returned
graph-engine refusal instead of the required quit reason; before prefix wiring,
the actual prefix owner did likewise. The completed six colocated controls cover
both directions, every configured byte with an explicit 256-row count, removal
of a flag, ordinary Unicode and ASCII-boundary positive cases, and real engine
Quit with zero clears. The combined quit/overflow fixture requires the strict
view to reject before coarse projection while the original capacity-only view
still reports arithmetic refusal.

- Final locked/offline, two-job library/focused runs passed 515 secure-default
  and 526 `insecure-bidders` cases, zero failures or ignores. The five integration
  targets remain `schema_cache_bounds`, `schema_compiler_controls`,
  `schema_cache_controls`, `schema_initialization` and `schema_source_inventory`.
  Shared controls execute in both owners; these are not distinct-proof counts.
- Source/compliance review found and rechecked the missing combined-failure
  control. Independent quality review found no remaining actionable finding.
  Reviewers inspected source; they did not independently run Cargo.
- Strict all-target workspace Clippy with `insecure-bidders,conformance-suite`
  and `-D warnings`, workspace format/diff checks and both raw-fixture gates
  passed (168 fixtures across 50 files). No lint allowance was added. Existing
  GVL deletion and transitive future-incompatibility notices remain.

Probe/generator/compiler/dependencies are unchanged, so the preceding 110-case
linked-artifact run was not repeated for this component. No full workspace
tests or guest execution ran. Quit-set admission is closed for the fixed
qualified candidates, not private strategy selection, cumulative counter
lifetimes, eager capture/available one-pass residency, whole-meta work or
physical/target bounds. No serving integration, usable resource descriptor,
workload/ceiling change, full M7 completion or R0 readiness is claimed. The user
roadmap edit remains outside the commit.

### Fresh Core Auxiliary Verification

On 2026-10-23 the two fresh-vector and three auxiliary-projection controls
failed against their zero/permissive stubs. Before probe wiring, its receiving
guard failed on the absent `fresh_core.word_bytes` field. Implemented native
observations then passed all seven predeclared cases without changing expected
values. These distinguish captures, one-pass presence, maximum explicit slots,
multiple patterns and positive construction/release from the reported logical
length. Standalone PikeVM has positive fresh allocation; the standalone fresh
backtracker remains empty. Neither observation is mislabeled eager meta Pike
storage.

The same-NFA constructor control also builds a real two-pattern NFA with ten
slots. Temporarily hardcoding its pattern count to one changed the bound from
the independent 128-byte oracle to 144 and failed that control. The mutation
was restored before final tests. Exact integration membership covers 23 helpers
at each of two word widths, with maxima 40/80 bytes; hostname's conservative
56/112-byte bound includes a one-pass candidate independently known not to build.

- Final locked/offline, two-job library/focused runs passed 527 secure-default
  and 538 `insecure-bidders` cases, zero failures or ignores. Same five existing
  integration targets as section 17; reused colocated controls are executed
  cases rather than distinct proofs.
- The full serial `schema_build_controls` run passed 112 cases in 255.41
  seconds. Both generated modes link 261 schemas on native and all three guest
  targets; all 300 captured outcomes per mode and 861 repeated explicit
  evaluations pass. After lint-only field/notation and assertion-type cleanup,
  the two affected layout controls and actual native allocation target passed
  again. Probe/generator/compiler behavior and acceptance values did not change.
- Source/spec and independent quality reviews found no remaining actionable
  findings after adding the real multi-pattern constructor case. Reviewers
  inspected sources, not independent Cargo runs. Strict workspace/all-target
  Clippy with `insecure-bidders,conformance-suite` and `-D warnings`, workspace
  and standalone-probe formatting, diff checks and both raw-fixture gates
  passed (168 fixtures across 50 files). No lint allowance was introduced;
  existing GVL deletion and dependency future-incompatibility notices remain.

This component provides fresh auxiliary requested bytes, not a whole-meta,
reset/clone, physical allocator, lifetime/work or deployment descriptor. Shared
program/pool storage, primary/optional hybrid owners and reachable lazy fallback
still require composition. No full workspace test run, guest execution, serving
cutover, funding/limit/ceiling change, full M7 completion or R0 readiness is
claimed. The unrelated user roadmap edit remains excluded.

## 21. Fresh Start-Anchored Half-Search Qualification

Continue the approved source-bound ownership qualification without changing
the engine, compiler patch, schemas, input limits, funding or serving behavior.
The hostname hook constructs a fresh explicit cache, performs at most one
whole-haystack earliest half-search, and drops it. Keep component 20's broader
serial-history constructor and its published result unchanged.

**Files:** existing `tests/support/schema_hybrid_heap.rs` for fresh directional
storage; `schema_meta_heap.rs` for a distinct narrowed constructor;
`tests/schema_compiler_controls.rs` and `tests/conformance/schema.rs` for
integration/allocation controls; qualification documentation and ISSUES.

- [x] Add red inline controls for fresh hybrid construction. Derive its sparse
  sets, start table, three sentinel transitions/handles, one dead-state map
  entry, shared dead-state backing and temporary builder through the existing
  checked layout helpers. Empty stacks/builders stay empty before search.
  Keep construction growth overlap explicit; never charge reported logical
  lengths as requested capacities. Vary dimensions/word widths independently,
  preserve shape/arithmetic refusals and compare actual native construction
  and release against independent positive controls.
- [x] Add a separate qualification constructor for one fresh, start-anchored
  earliest half-search, using the real HIR `Look::Start` property rather than
  matching a caret in source text. Refuse line-only/mixed-branch/unanchored
  patterns. Require the current meta configuration, locked engine/feature
  identity, successful primary construction and non-quitting forward capacity
  proof for the forward DFA and successful reverse construction; do not merely
  assume the optimization succeeds. Refuse empty-matching patterns in this
  narrower proof, excluding the library's UTF-8 empty-match retry loop rather
  than equating one public call with one scan. The hostname pattern is nonempty.
  Establish bounded single-search progress arithmetic before admission.
- [x] Audit the locked source chain: start anchoring suppresses prefilters and
  reverse strategies; Core half-search tries its available forward hybrid;
  complete non-quitting capacity and ordinary input mode exclude its error
  path. Only after all prerequisites succeed may this constructor exclude
  initialized fallback/search-grown reverse storage. Charge eager owners,
  the complete forward peak and a separately bounded fresh reverse peak with
  checked addition. The reverse cache is constructed, not absent. Unsupported
  configuration, syntax, build, quit, capacity, graph, layout or arithmetic
  conditions refuse with typed diagnostics, never partial success.
- [x] Pin the actual hostname result from independently established component
  values, plus positive/negative synthetic anchor and failure controls. Check
  actual public meta matching, allocation/release and unchanged guards on the
  existing adversarial panel. Treat these as correspondence checks, not proof
  from finite samples. Do not apply this result to reused, reset, cloned,
  rebound, concurrent or other search APIs, or claim whole-schema scratch fits.
- [x] Obtain source/spec and quality reviews; run affected controls under
  defaults and `insecure-bidders`, strict target Clippy, formatting, whitespace
  and fixture gates. Publish exact scope/results and commit/push. No linked
  production/schema cutover or usable descriptor is authorized by this slice.

### Fresh Half-Search Verification (2026-10-27)

Rust 1.98.1, locked/offline dependencies, `CARGO_INCREMENTAL=0` and two Cargo
jobs. Source readers receive `AUCTIONEER__BIDDER__SOURCE_DIR` and `AUCTIONEER_APS_SOURCE`
through the existing local-reference bindings. Cargo invocations were serialized.

The narrowed constructor's five initial controls failed against refusing stubs
before implementation, then passed. A sixth control pins real graph/capacity
refusal rather than assuming every anchored pattern qualifies. Fresh-direction
controls independently cover word/group widths, each storage dimension,
malformed shapes, allocation/composition overflow, and actual DFA binding.
The provisional reverse oracle was corrected from source component sums:
4,821 live plus 188 overlap gives 5,009, not the earlier loose 5,181 estimate.
No implementation or threshold was changed to accommodate that correction.

Final affected-target results:

| Target | Secure Defaults | `insecure-bidders` |
| --- | ---: | ---: |
| `schema_cache_bounds` | 192 passed | 192 passed |
| `schema_compiler_controls` | 178 passed | 178 passed |
| `schema_source_conformance` | 308 passed | 308 passed |

Each lane totals 678 executions, zero failures or ignored tests. Shared inline
controls run in multiple owners; these totals are not distinct-proof counts or
a full workspace suite. Reproduce with separate secure and feature invocations:

```sh
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_cache_bounds -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_compiler_controls --test schema_source_conformance -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_cache_bounds --test schema_compiler_controls --test schema_source_conformance -- --test-threads=2
```

The first sandboxed source run passed 306 tests and failed the two linked
dispatch cases on local-listener `EPERM`. With loopback permission, both full
lanes passed unchanged; no assertion, watchdog or fixture was relaxed. Both
generated modes again link all 261 validators on native and three WASM targets,
preserve all 300 captured results per mode and run the explicit mode's 861
repeated evaluations. These are links/native execution, not guest execution.

Two separate mutations were rejected before restoration and the final runs:
omitting fresh reverse funding failed the hostname owner control; removing the
256-byte guard failed the native callback control with 38,569 allocated bytes
versus the independent zero expectation. The latter counts exactly ten callback
observations, two guard refusals and two directional construction/release
observations. Positive allocation controls prevent a missing measurement from
passing as zero usage.

Source/spec, independent code-quality and documentation reviews found no
remaining in-scope issue. Reviewers inspected source; the controller ran Cargo.
Strict Clippy passed for all three targets in both feature lanes with
`-- -D warnings`, without new lint allowances. Existing GVL-deletion build
notices and the dependency future-incompatibility notice remain visible.
Workspace/direct-helper formatting, whitespace and both raw-fixture gates
passed with the unchanged 189 fixtures across 55 files. Production code,
schemas, compiler patch, dependency/SDK pins, workload limits and funding are
unchanged. The broad serial-history model remains intact. Shared-program,
constructor/work, whole-schema and physical/target qualification still block
usable descriptors and serving integration; this is not M7 or R0 closure.

## 22. Complete Native Pattern-Bundle And Format Overlap

Continue the approved qualification without changing schemas, compiler patches,
engines, limits, funding or serving behavior. This closes cache/format overlap,
not shared-program, constructor-work, other validator scratch or physical/guest
qualification. Do not publish a usable runtime descriptor from this component.

**Files:** `tests/schema_bundle_heap/mod.rs`, the ordinary module declarations
in `schema_cache_bounds.rs`/`schema_source_conformance.rs`, the existing
`schema_build/mod.rs` and `support/schema_explicit_probe.rs`, and the source
conformance target's inline integration controls.

- [x] Observe missing-API reds for independently unequal pattern, inline and
      format terms, empty bundles and each checked-sum overflow. Implement a
      conservative sum of every retained pattern owner's complete lifecycle
      peak, actual compiled bundle inline bytes and one callback-local hostname
      peak plus that callback's actual inline `meta::Cache` size. Check the
      latter addition independently and retain an omission control. A callback
      is synchronous and releases its fresh cache; do not
      multiply it by format occurrences or omit sibling caches. UUID has no
      heap owner; unknown formats refuse instead of receiving a zero charge.
- [x] Add a missing-API red, then emit native-only per-schema cache layout
      constants with `quote!` and a bounded `cache-layouts` probe operation.
      Read actual compiled wrapper size/alignment, not the union's size or
      an independently guessed layout. Pin the report's exact inventory and
      reconcile each wrapper against the admitted homogeneous cache fields.
      Keep existing default/explicit operations and profile/lock checks intact.
- [x] Compile and audit every current schema, reconcile emitted helpers through
      the existing strict AST reader, and compose its complete bundle with all
      declared custom formats. Cover all 261 source schemas plus the separate
      synthetic profile, including zero-helper and hostname-bearing rows. Test
      schema membership and nonzero affected populations independently; do not
      replace a missing owner with an empty bundle.
- [x] Require every native combined component below the unchanged 512-KiB
      scratch ceiling, retaining inline bytes and positive controls. This is
      only a necessary cache-component condition, not complete validator fit.
      Verify both feature lanes, the existing explicit allocation controls,
      strict affected Clippy, module/fixture gates and separate source/quality
      review. Record exact scope and remaining obligations, commit and push.

### Native Bundle Component Verification (2026-10-27/28)

The unit controls first failed with the missing bundle API (`E0432`), then
passed all three independently unequal/overflow/format controls. The compiled
layout controls separately failed with the missing `native_cache_layouts`
API (`E0599`); the native-only operation then supplied actual typed wrapper
layouts for both the seven-schema synthetic profile and all 261 source schemas.
No fixture, threshold, schema, compiler patch or serving path changed.

Both complete feature lanes passed before the final lint-only extraction of
the existing dispatch-table renderer:

| Target | Secure Defaults | `insecure-bidders` |
| --- | ---: | ---: |
| `schema_build_controls` | 170 passed | 170 passed |
| `schema_cache_bounds` | 195 passed | 195 passed |
| `schema_source_conformance` | 313 passed | 313 passed |

Each lane totals 678 executions, zero failures or ignored tests; these include
shared inline controls, not 678 distinct proofs. After that pure extraction,
the complete secure lane passed again with the same counts. The feature lane
reran both owners' renderer/dispatch controls (15 each) and the three bundle
units. The complete dual-lane results are not relabeled as a second full
feature run after the extraction.

Each complete lane linked all 261 schemas on native and three WASM targets in
both default and explicit modes, retained all 300 source-captured verdicts per
mode, and executed the explicit mode's 861 repeated evaluations. These are
guest links, not guest execution or physical-memory qualification. The full
native bundle reconciliation found 23 helpers, two hostname schemas, 247
empty bundles and a maximum component of 316,705 bytes against the unchanged
524,288-byte ceiling.

Reproduce the complete lanes separately:

```sh
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_build_controls --test schema_cache_bounds --test schema_source_conformance -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_build_controls --test schema_cache_bounds --test schema_source_conformance -- --test-threads=2
```

Source/spec and independent quality review found no remaining in-scope issue.
Full workspace strict Clippy and affected-target feature Clippy passed without
new allowances. The final default
bidder library/endpoint/isolation run passed 1,100/10/16 tests; its explicit
development lane passed 1,111/10/16. Nine source-dependent checks remain ignored
in ordinary library runs; their separate final feature run passed all nine
with immutable source bindings. All 118 workflow controls passed. Both raw-fixture gates and workspace
formatting passed (188 raw fixtures across 54 files).

This closes only the conditional native cache-component overlap. Complete call
frames, other validator scratch, shared programs/pools, initialization/work,
production cache multiplicity and physical/guest bounds remain M7 qualification
work. No usable runtime descriptor, priority-flow acceptance, M7/R0 completion
or hosted qualification follows from this checkpoint.

## 23. Unused Native Meta-Pool Bookkeeping

Continue the approved explicit-ownership qualification without changing the
engine, compiler patch, dependency/features, workloads, funding or serving path.
The locked meta matcher allocates pool bookkeeping even when all searches use
caller-owned caches. This component funds that fixed requested-heap owner, not
immutable programs, complete construction scratch, physical memory or guests.

**Files:** `tests/schema_pool_heap/mod.rs`, ordinary module declarations in the
existing cache/source targets, and inline source-conformance controls.

- [x] Derive checked box, shard-vector and factory-capture terms from the locked
      `regex-automata 0.4.18` std pool and pinned native collection implementation.
      Include the full `Option<Cache>` slot despite its absent payload. Compute
      conservative field-reordering padding from actual compiled public field
      layouts; do not assume a private Rust struct has a portable mirror ABI.
      The factory captures a fat strategy Arc, not one pointer. Keep outer inline
      regex storage separate from heap allocations.
- [x] Add red/green controls with independently unequal field sizes, alignments
      and shard counts, including each checked-arithmetic/refusal boundary.
      Reconcile actual regex-clone construction and release against the three
      bookkeeping owners while the original retains shared programs. Cloning
      is a diagnostic isolation technique, not an allowed serving lifecycle.
- [x] Discover every emitted regex and the one shared hostname singleton. Require
      pool capacity one and retain the existing explicit-search/no-acquisition
      controls. Charge each singleton once, not each auction, callback or schema
      referring to the same singleton. A pooled acquisition, additional clone
      or unsupported configuration invalidates this component's premise.
- [x] Run affected controls in both feature lanes, strict lint and module/fixture
      guards, obtain source/spec and independent quality review, and record exact
      results before commit/push. Keep complete shared-program, initialization,
      work, call-frame, production multiplicity and target obligations open; do
      not publish a usable descriptor from this partial proof.

### Unused-Pool Component Verification (2026-10-28)

The initial narrow run observed seven failures at the unimplemented layout,
configuration and population paths; the independent constructor control passed.
The implemented component then passed all eight source-target controls. Native
allocation/release reconciliation derives and observes the same three requests:
1,448-byte pool box, 64-byte shard and 16-byte closure, or 1,528 bytes per
singleton. All 23 discovered helpers plus one shared hostname total 36,672
requested bytes. No threshold, engine, compiler patch or serving code changed.

| Verification | Result |
| --- | --- |
| Complete secure `schema_cache_bounds` / `schema_cache_controls` | 201 / 13 passed |
| Complete secure `schema_source_conformance` / `test_isolation` | 321 / 16 passed |
| Complete development-feature cache bounds / cache controls | 201 / 13 passed |
| Development-feature new source pool controls | 8 passed |
| Final new scalar/source controls after lint-only cleanup, each lane | 6 / 8 passed |

The complete source run links all 261 schemas on native and the three guest
targets in default and explicit modes, checks all 14 immutable parameter-source
blobs and 300 captured parameter cases per mode, and retains the existing 861
explicit repeated evaluations. Guest linking is not guest execution or physical
memory qualification. The first sandboxed source run passed 319 tests but failed
the two local-socket reference-refusal controls with `EPERM`; the unchanged full
run passed with loopback permission and pinned source checkouts. No assertion was
skipped or weakened. Cargo did not reach isolation in the failed invocation; the
successful full rerun includes all 16 isolation checks.

The complete executions preceded only inferred-integer/import/variable-name
cleanup and formatting in the new tests. Final focused controls above reran all
new assertions in both lanes; these are not relabeled complete source reruns.
Strict all-target workspace Clippy and affected development-feature Clippy pass
with `-D warnings`, without new allowances. Workspace formatting, whitespace and
both raw-fixture gates pass (188 fixtures across 54 files). Source/spec and
independent quality reviews approve this component only.

Reproduce the broad secure run with the pinned source checkout variables set;
local sandboxes must permit its loopback-only negative-reference fixtures:

```sh
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_cache_bounds --test schema_cache_controls --test schema_source_conformance --test test_isolation -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_cache_bounds --test schema_cache_controls -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance schema_pool_heap -- --test-threads=2
```

No lifetime-wide non-acquisition, complete program/constructor/work bound,
production cache multiplicity, usable descriptor or M7/R0 completion follows.

## 24. Native Immutable Regex Metadata

Continue Task 3's approved shared-program accounting. This increment covers
metadata only, without changing the engine, compiler patch, dependencies,
features, schemas, limits, funding or serving behavior. Strategy allocations,
NFA/capture graphs, one-pass tables, literal matchers and initialization peaks
remain separate owners; do not label their absence a zero contribution.

**Files:** the existing `schema_heap_layout` and `schema_pool_heap` test helpers,
new `tests/schema_metadata_heap/mod.rs`, ordinary cache/source target modules
and inline source-conformance controls.

- [x] Extract the existing conservative native field-padding rule into the
      shared layout helper, preserving every pool result. Add checked native
      Arc allocation layout using the pinned std header and actual payload
      alignment. Independently test omitted terms, padding, alignment, overflow
      and malformed shapes; do not mirror a private Rust ABI.
- [x] Derive the single-pattern metadata owner: `Arc<RegexI>`,
      `Arc<RegexInfoI>`, the properties vector's four retained slots and two
      boxed scalar `Properties` payloads. Use actual compiled public constituent
      layouts and the pinned `Properties::memory_usage()` implementation, which
      returns its private scalar payload size. Refuse unsupported pattern counts
      and configured prefilters rather than overlooking their extra ownership.
      Charge actual `LazyLock<Option<Regex>>` static storage separately from heap.
- [x] Discover all 23 emitted regexes plus the one shared hostname singleton.
      Corroborate the source-derived property construction with actual public
      clone/union allocation, retained capacity and complete release. Keep
      parsing, programs, assertion/report allocations outside measurement. Do
      not infer whole-program storage or an upper bound from an observed peak.
- [x] Execute the new and affected controls in both feature lanes, preserve the
      pool component's results, run strict lint, formatting and module/fixture
      guards, obtain source/spec and independent quality review, and record
      results before commit/push. Complete program/work/constructor/guest and
      serving qualification remain open.

### Immutable-Metadata Component Verification (2026-10-28)

The initial source-target metadata filter passed two existing cases and failed
eleven new cases against the unimplemented layout/metadata paths. The complete
implemented filter then passed 13 cases; its shared cache-target counterpart
passed ten. Counts include matching existing metadata controls, not distinct
new-proof totals. Two additional population controls subsequently separate
missing terms, inline storage, unequal nonzero boxes and heap-only overflow.

Independent native terms are `40 + 192 + 32 + 80 + 80 = 424` requested heap
bytes per regex. Actual lazy-cell inline storage is separately 24 bytes. The
23 emitted helpers plus the shared hostname total 10,176 heap bytes and 576
static bytes. Actual public property construction/release corroborates all
24 three-allocation owner sequences, totaling 4,608 property bytes. Public Arc
payload controls corroborate 24/24/40/128-byte requests, including over-aligned
payload padding and complete release. No private ABI mirror or observed peak
was substituted for the source derivation.

| Verification | Result |
| --- | --- |
| Broad secure cache bounds / cache API / source conformance / isolation | 210 / 13 / 332 / 16 passed |
| Final complete secure cache bounds / cache API / isolation | 212 / 13 / 16 passed |
| Final complete development-feature cache bounds / cache API | 212 / 13 passed |
| Final metadata-filtered source controls, each feature lane | 15 passed |
| Final development-feature source pool controls | 8 passed |

The broad source run includes all 261 validators on native and three guest
targets in default and explicit modes, all 14 immutable parameter-source blobs,
300 captured parameter verdicts per mode and 861 repeated explicit evaluations.
Guest links are not guest execution or physical-memory qualification. That
complete run preceded only two added population tests and a source-reference
comment correction. Final focused executions include every new assertion;
they are not relabeled complete 334-test source runs.

Four independently applied, compiling mutations disabled the missing-term,
inline-storage, unequal-box and heap-overflow refusals. Each produced the
intended assertion failure (two population cases passed, one failed), rather
than a compiler error. Source SHA-256 comparison confirmed byte-identical
restoration after the mutation sequence. Restored controls passed. The source
and independent quality reviews approved the component; review corrected the
fresh-push source witness to `grow_amortized/finish_grow`, without changing the
four-slot result.

The extracted common field-layout helper retains the prior 1,528-byte pool
result and 36,672-byte population. Strict all-target workspace Clippy and
affected development-feature Clippy pass with `-D warnings`, without new
allowances. Formatting, whitespace, isolation and both raw-fixture gates pass
(188 raw fixtures across 54 files). Existing GVL deletion notices and the
dependency future-incompatibility warning remain unrelated to this component.

Reproduce the complete shared controls in separate feature invocations, plus
the source component controls:

```sh
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_cache_bounds --test schema_cache_controls --test test_isolation -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_cache_bounds --test schema_cache_controls -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_source_conformance metadata -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance metadata -- --test-threads=2
```

No engine, schema, compiler patch, dependency/SDK pin, workload, ceiling,
funding coefficient or serving behavior changed. NFA/capture graphs, strategy
allocations, one-pass/literal ownership, complete initialization/work and
physical/guest qualification remain open. This is not a complete program
descriptor, full engine acceptance, priority-flow promotion or M7/R0 closure.

## 25. Native NFA And Capture-Metadata Trees

Continue the approved immutable-program ownership proof. Model retained trees
from the locked library's constructors, not its incomplete memory reports.
Preserve all engine, compiler, schema, dependency, feature, workload and funding
limits. This is not complete strategy/program, initialization/work or guest
qualification and does not enable serving validation.

**Files:** new `tests/schema_nfa_heap/mod.rs`, ordinary cache/source target
modules, existing prefix-HIR helpers if a narrow owning API is needed, and
inline source-conformance controls.

- [x] Derive the NFA's Arc payload from its actual public constituent layouts
      and source-backed scalar wrappers, using the common checked padding/header
      helpers. Include state and start vectors, every sparse/dense/union boxed
      child and the capture-metadata tree. Do not use reported lengths as proof
      of private retained capacity. State construction history and pinned native
      vector behavior must justify the retained envelopes.
- [x] Derive the capture owner's Arc, all three outer vectors and each inner
      name-index vector. Inner vectors are not shrunk. Positively verify names
      are absent before excluding map backing/name-string allocations; named
      captures receive a typed unqualified refusal. Reconcile public iterator,
      pattern, group and slot counts. Zero patterns/slots still retain an empty
      capture-metadata Arc and must never yield a zero owner.
- [x] Derive each pattern's forward plus reverse plus maximum possible
      prefix-reverse tree using the existing bounded structured-HIR candidates
      and anchoring exclusion. Keep the prefix program maximum separate from
      cache maxima. Shared NFA handles in engine wrappers do not duplicate the
      tree. Unknown shape, names, compilation, candidate coverage or arithmetic
      must refuse rather than contribute zero.
- [x] Add red/green literal owner/overflow/shape controls and real empty,
      unnamed/named and forward/reverse allocation/release corroboration.
      Reconcile all 23 emitted helpers and the shared hostname without a bidder
      roster. Run affected controls in both feature lanes, strict lint,
      formatting, module/fixture gates and independent source/quality reviews;
      record scope before commit/push. Strategy and literal-only capture owners,
      one-pass tables, constructors/work and physical/guest bounds remain open.

### NFA-Tree Component Verification (2026-10-28)

Nine initial controls failed against the unimplemented component. The native
model then covered 24 singletons, 48 forward/reverse trees and five eligible
prefix trees, with 1,385 states across those 53 separately constructed trees.
The forward, reverse and summed per-pattern maximum-prefix envelopes are
58,152, 50,572 and 5,236 bytes: 113,960 requested bytes in total. The 75,284
observed retained bytes sum every separately measured candidate, not one
simultaneous runtime owner. Each of the 53 trees released exactly its observed
retained allocation; the bound is source-derived, not inferred from that sum.
The native root Arc envelopes are 544 NFA bytes and 96 capture-metadata bytes;
even zero-pattern capture metadata retains its Arc.

Review identified two weak controls before commit. An initial maximum-to-last
mutation survived. The new literal permutation test fails on an interior
maximum (`43` instead of `101`), and passes after restoring maximum reduction.
A separate compiling mutation that reports absent prefixes now fails the
per-pattern source comparison. SHA-256 comparison confirms byte-identical
restoration. Expected prefix enumeration is independent of the subject's
disposition and pins all five eligible prefixes, not a conditional counter.

| Verification | Result |
| --- | --- |
| Final complete secure cache bounds / cache API / source / isolation | 227 / 13 / 352 / 16 passed |
| Final focused NFA cache / source controls | 15 / 18 passed |
| Final complete development-feature cache bounds / cache API | 227 / 13 passed |
| Final development-feature NFA source controls | 18 passed |

The complete source run links all 261 validators on native and three guest
targets in both default and explicit modes, checks all 14 captured source
blobs and 300 parameter cases per mode, and retains 861 repeated explicit
evaluations. No ignored test was added. Guest linkage is not guest execution
or physical-memory qualification. Source/spec and independent quality reviews
approve only this conditional native retained-tree component.

The complete executions above preceded only a lint-only success assertion
replacement and extraction of the independent HIR eligibility helper. Final
focused controls rerun every NFA assertion in both lanes; these are not
relabeled complete source reruns. The extracted helper also rejects the
false-absence mutation after refactoring. Strict all-target workspace Clippy
and affected development-feature Clippy pass with `-D warnings`, without new
allowances. Existing GVL deletion notices and dependency future-incompatibility
warnings remain unrelated.

Workspace formatting, whitespace, isolation and both raw-fixture gates pass
(188 raw fixtures across 54 files). No new source import or fixture exemption
was added.

Reproduce with the pinned source checkout variables set and loopback permission
for the full source target's unchanged negative-reference fixtures:

```sh
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_cache_bounds --test schema_cache_controls --test schema_source_conformance --test test_isolation -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_cache_bounds --test schema_cache_controls
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance nfa_heap -- --test-threads=2
```

No engine, schema, compiler patch, dependency/SDK pin, workload, ceiling,
funding coefficient or serving behavior changed. Complete strategy/literal and
one-pass ownership, initialization/work, physical/guest and production lifetime
qualification remain M7 work. No usable complete descriptor or M7/R0 readiness
claim follows.

## 26. Native Retained One-Pass Tables

Extend the existing one-pass qualifier, not the engine or compiler patch. The
locked wrapper embeds a one-pass DFA inside its strategy; only that DFA's
transition and start-ID buffers are new heap owners here. Its NFA handle shares
the forward tree already covered by component 25. Inline engine/strategy
storage and constructor peaks remain outside this component.

**Files:** `tests/schema_onepass_bounds/mod.rs` and inline source-conformance
controls. No new dependency, module wiring, schema, limit or funding change.

- [x] Derive checked retained tables `8 * S * K + 4 * (P + 1)`, including the
      dead state, power-of-two stride's match column and one aggregate start
      plus every pattern start. Justify exact finalized requests from both
      successful shrink calls in the locked constructor and pinned native
      std/Global behavior, not a generic vector-capacity assumption or peak.
- [x] Preserve the existing identical configured constructor: LeftmostFirst,
      per-pattern starts, byte classes and the one-MiB one-pass limit. Refuse
      unsupported configuration, compilation, malformed dimensions or arithmetic.
      Constructor rejection alone can produce an absent candidate; a successful
      raw candidate can conservatively be funded even if meta's heuristic would
      skip it. Derive the structural envelope with `S <= NFA states + 1` and
      the NFA byte-class stride, not a blanket one-MiB charge per regex.
- [x] Write literal red/green controls varying states, stride and patterns
      independently, including overflow and invalid-request refusals. Exercise positive,
      ambiguous, capture-free and multi-pattern constructors. Hold the original
      NFA alive during allocation/release measurements; successful construction
      retains exactly the two buffers and dropping it releases those bytes.
      Failed construction, including its dropped error, retains zero.
- [x] Discover every source regex and shared hostname. Compare independent raw
      constructor outcomes against every modeled identity without conditioning
      coverage on the model's disposition. Run affected suites in both feature
      lanes, strict lint, formatting/import/fixture guards and independent
      source/spec and quality reviews; record exact results before commit/push.
      Complete strategy/literal, initialization/work, guest and physical-memory
      qualification remain M7 work; no serving descriptor is enabled here.

### Retained One-Pass Verification (2026-10-28)

Initial projection stubs compiled and failed seven new cache-target controls
and nine source-target controls; one and three constructor-only controls
already passed. The implemented projections passed eight cache and twelve
source controls before the additional request-size refusal test. That independent
test then demonstrated two individually nonallocatable native buffers whose
products fit `usize`: both were wrongly accepted at `isize::MAX + 1` bytes.
Reusing the shared checked-layout helper fixes both without changing any
workload, constructor or ceiling.

The final nine cache controls and thirteen source controls pass. The source
cohort independently invokes the configured constructor for every discovered
identity before consulting the model: 19 successes and five constructor
rejections across all 24 singletons. Successful buffers total 14,520 requested
bytes, within the independently derived 22,360-byte candidate structural
envelope. Every observed construction/release reconciles while its original
NFA remains alive. Failed constructors drop their error inside measurement and
retain zero bytes. This does not measure a whole meta strategy or initialize
a production catalog.

| Verification | Result |
| --- | --- |
| Complete secure cache bounds / cache API / isolation | 236 / 13 / 16 passed |
| Complete development-feature cache bounds / cache API | 236 / 13 passed |
| Focused retained one-pass cache / source controls | 9 / 13 passed |
| Development-feature retained one-pass source controls | 13 passed |

The complete 352-test source suite was executed at component 25 before this
test-only extension, including all 261-validator native/guest links in both
modes and captured parameter verdicts. It is not relabeled a full 365-test
source run here. Component 26 introduces no builder, engine, schema, target,
dependency or serving change. Source/spec and independent quality reviews
approve its exact conditional retained-buffer scope, not whole-program or
M7/R0 acceptance.

The two individually valid native buffer sizes cannot overflow `usize` when
summed: each is at most `isize::MAX`. The final checked addition remains
defensive; no independently reachable addition-overflow path is claimed after
the per-buffer checks. Strict all-target workspace and development-feature
Clippy pass with `-D warnings`, without new allowances. Lint-only field naming
and checked integer expressions preserve all assertion values. Final focused
executions rerun every new control in both lanes.

Final complete cache/API/isolation runs also follow that lint-only cleanup;
their counts above are not earlier binaries. Formatting, whitespace, isolation
and both raw-fixture gates pass (188 raw fixtures across 54 files). No new
ignore, source import or fixture allowance was added. Existing GVL deletion
notices and dependency future-incompatibility warnings remain unrelated.

```sh
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_cache_bounds --test schema_cache_controls --test test_isolation -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_cache_bounds --test schema_cache_controls -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_source_conformance onepass_retained -- --test-threads=2
cargo test --offline --locked -j2 -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance onepass_retained -- --test-threads=2
```

## 27. Close Native Literal Owners

**2026-10-29 scope correction:** the
[shipped-schema amendment](../specs/2026-10-29-m7-shipped-schema-qualification-design.md)
governs remaining work here. Preserve existing code, tests and historical
evidence. Before extending private literal/backend ownership qualification,
derive the complete M7 shipped closure and demonstrate a reachable need.
Otherwise assign the extension to the first affected M7.7 wave or M20 with
that pre-publication trigger. This is not permission to defer a required M7
owner, call observations bounds, or enable unqualified schema serving.

Continue M7 Task 3 without changing matching, schemas, builder limits or serving
admission. The payload queries do not report literal children. Derive retained
storage from the pinned constructors instead of treating approximate memory
reports or an independently reconstructed prefilter as hidden-owner evidence.

- [x] Qualify Aho-Corasick DFA retained buffers from fresh nonempty literal
  lengths. The trie has at most four initial states plus one state per input
  byte. Both-start construction doubles that population and reuses four
  sentinels; the byte alphabet has at most 256 columns. Fund outer match rows
  and each row's monotonic append capacity, not only reported match lengths.
  Exact-length repeat/slice constructors fund transition, outer-row and
  pattern-length requests under the pinned std allocator contract; these
  outer vectors do not grow or inherit trie capacity before final shrinking.
- [ ] Cover shared packed patterns, Rabin-Karp buckets, and all compiled Teddy
  payload/bucket alternatives with source-bound ownership and capacity proofs.
  Count shared patterns once. Source-rejected or absent alternatives need a
  positive constructor premise, not an observation of `None` on this CPU.
- [ ] Reconcile every emitted source pattern plus hostname against the actual
  constructor policy; reject unsupported/unproved branches rather than emit a
  partial complete-program result. Preserve source/engine/feature identities.
- [x] Add the already-locked `aho-corasick = 1.1.4` as a workspace-managed,
  native-only qualification dependency with defaults disabled, solely to check
  its public layout/constructor controls. Root lock changes may add only the
  bidder dependency edge. Preserve Aho features `perf-literal,std` and the
  existing 25 regex-automata features.
- [x] Observe red/green controls, independently vary pattern count, total bytes,
  start modes and capacities, then compare real fresh constructor/release
  observations with the source bounds. Keep initialization peaks, work,
  allocator overhead and guest qualification separate until proved. Review and
  commit verified ownership before supplying a usable serving descriptor.

**Retained-buffer checkpoint, 2026-10-29:** seven focused inline controls and
the separate source-driven finite-prefix constructor check pass. The latter
inspects all 23 emitted helpers plus hostname: 17 are start-anchored, three have
no usable finite prefix and four produce eight separately constructed DFA
alternatives across the two start modes. Their retained requested bytes total
117,632 against the independent 122,784-byte component envelope. These are
alternative constructor observations, not a catalog residency charge or proof
of the original hidden matcher choice. The first four controls were observed
red before implementation; three later controls strengthen overflow, capacity
and pattern-count coverage. The already-locked dependency adds only the native
bidder test edge; it does not change package versions or enabled features.

The unchecked packed-pattern/Teddy and whole-constructor reconciliation items
remain open. Derive `S` before extending them: only a demonstrated reachable
need keeps those extensions in M7. Initialization/work, guest/physical facts and
complete serving descriptors are not established by this checkpoint.
