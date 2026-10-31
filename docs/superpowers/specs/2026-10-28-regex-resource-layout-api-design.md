# Library-Owned Regex Resource Layouts

Status: implemented and verified after AG's written approval on 2026-10-28. The
[implementation plan](../plans/2026-10-28-regex-resource-layout-api.md) tracks
execution and exact verification scope. Approval covers this API and its
replayable dependency patch, not
complete M7 qualification or serving admission.

## 1. Purpose And Choice

M7 currently derives conservative layouts for private regex metadata and lacks
the selected strategy's concrete inline layout. Mirroring private Rust structs
or treating private newtypes as their public fields would introduce unsupported
ABI assumptions. Existing approximate `memory_usage()` methods are not complete
requested-allocation bounds. See the
[current ownership derivations](../../bidders/schema-qualification.md#native-immutable-regex-metadata)
and [upstream memory-report contract](https://docs.rs/regex-automata/0.4.18/regex_automata/meta/struct.Regex.html#method.memory_usage).

Choose two shallow, library-owned layout queries. The owning library can safely
observe its actual private payloads, including a trait object's concrete size
and alignment, without exporting private representations. Use the existing
`core::alloc::Layout` type and its safe `new`/`for_value` operations, not another
layout model. `for_value` supports dynamically sized values; availability is
also confirmed in the installed Rust 1.98.1 `core/src/alloc/layout.rs`.
See the [standard API](https://doc.rust-lang.org/std/alloc/struct.Layout.html#method.for_value).

Alternatives not selected:

- Continue source-only field-padding envelopes: valid where independently
  proven, but does not supply the missing selected private strategy layout.
- Add a whole-heap graph, backend catalogue or strategy visitor: materially
  larger maintenance and verification scope, with new sharing/capacity claims.
  This increment needs none of those interfaces.

## 2. Pinned Baseline

The implementation baseline is the already-locked `regex-automata 0.4.18`:

| Identity | Value |
| --- | --- |
| Published package SHA-256 | `ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2` |
| Package VCS revision | `e6ad938198e68339a593f85d228f2d44cb54718f` |
| VCS package directory | `regex-automata` |
| Toolchain for initial qualification | Rust 1.98.1 |

These identities were checked against `Cargo.lock` and the installed package's
`.cargo_vcs_info.json`. Exact package source, not current upstream `master`, is
the implementation authority. Source anchors are `src/meta/regex.rs`,
`src/meta/mod.rs` and `src/util/prefilter/mod.rs` in that package.

Preserve the existing resolved features, matching semantics, builder options,
schema texts, workload limits, resource ceilings and additive accounting model.
Do not add an introspection feature flag, switch engines, disable optimizations,
change struct representations or add stored bookkeeping. No new workspace
crate, custom regex engine or replacement schema validator is needed.

## 3. Public API Contract

Add `meta::Regex::resource_layout(&self) -> meta::RegexResourceLayout`.
The report has private fields and these getters, each returning `Layout`:

| Getter | Exact Object Described | Excluded Children |
| --- | --- | --- |
| `value()` | Complete public `Regex` value, including its inline handles | Referenced allocations |
| `regex_payload()` | The `RegexI` value behind `imp: Arc<RegexI>` | Arc header and referenced allocations |
| `metadata_payload()` | The `RegexInfoI` value behind the metadata Arc | Arc header, property/vector backing and configured-prefilter children |
| `strategy_payload()` | Concrete value behind this regex's `Arc<dyn Strategy>` | Arc header and all referenced child allocations |

Add `util::prefilter::Prefilter::resource_layout(&self)` returning
`util::prefilter::PrefilterResourceLayout`, with these getters:

| Getter | Exact Object Described |
| --- | --- |
| `value()` | Complete public `Prefilter` value |
| `matcher_payload()` | Concrete value behind that prefilter's `Arc<dyn PrefilterI>` |

Both reports are small `Copy + Clone + Debug + Eq + PartialEq` values, contain
only layouts, and borrow or retain no regex data. They expose no pointers,
addresses, type names, pattern text, cache handles or serialization dependency.
Getters do not return an aggregate byte total.

The regex query/report is available within the existing `meta` feature boundary.
The prefilter query/report uses its existing `alloc` boundary; no-alloc builds
must still compile without pretending that a matcher exists or reporting zero
as a substitute. There is no new feature or fallback implementation.

Use `Layout::new` for statically sized owners and `Layout::for_value` on
**dereferenced payloads** for dynamic ownership. An Arc handle's layout is not
its pointee's layout. The selected strategy measurement automatically includes
its embedded wrappers, private option representations and compiler padding.
It does not identify the strategy or prove which children it retains.

The query is read-only, allocation-free, lock-free and constant-work in the
object graph: no search, cache creation/acquisition, regex clone, backend
construction, recursive traversal, name formatting or retained state. Do not
add methods to `Strategy` or `PrefilterI`; dynamic layout already exists in the
trait object's metadata, so no new trait-vtable entries are necessary. Preserve
all existing public auto traits. Add no unsafe code or representation attributes.

## 4. Accounting Boundary

These are **value/payload layouts**, not allocation layouts, recursive retained
heap, allocator usable size or physical memory. Even a valid zero-sized layout
is not evidence that an unavailable owner has no cost.

The consumer retains the separately pinned standard-library derivation of Arc
headers, alignment gaps and tail padding, with checked `Layout` arithmetic.
It then applies actual ownership and sharing rules. In particular:

- The existing lazy static cell already contains the `Regex` value. Do not add
  `value()` again on top of that complete static-cell charge.
- A strategy payload includes its embedded `Core`, engine wrappers and handles.
  Do not charge those inline fields as additional standalone allocations.
- A metadata handle inside a strategy may share the metadata Arc already
  reached from `RegexI`. Its presence is not another metadata allocation.
- A public `Prefilter` value may already be embedded in an enclosing payload.
  Its matcher Arc is a separate owner only where the actual graph has that Arc.
- Literal-only `Pre<P>` embeds its matcher directly. Building an independent
  public prefilter does not establish that `Pre<P>` has a matcher Arc to charge.

Equal layouts, equal pattern strings or equal reports never establish shared
allocation identity. The API intentionally supplies no deduplication tokens.
Source-proven ownership and the existing conservative child bounds remain
required; unknown children cannot receive a zero contribution.

## 5. First Consumer And Remaining Gate

The first consumer is the existing native schema qualification harness, not
auction serving. It may replace the private `RegexI`/`RegexInfoI` field-layout
inputs with exact payload layouts and add the missing selected strategy payload.
Keep the old conservative component results as dated history; explain changed
numbers rather than rewriting historical evidence or lowering a funding term.

Bind each observation to the actual queried object, pattern/helper identity,
constructor configuration, package/patch source, features, compiler and adapter
target. Distinguish an independently constructed candidate from a generated
validator's initialized singleton. Identical patterns/configuration alone do
not prove object identity, retained ownership or cross-object sharing.

The prefilter query covers an accessible **actual public prefilter** only.
It cannot inspect hidden automatic prefilters retained inside a strategy or
engine. `Config::get_prefilter()` exposes a configured prefilter, not every
automatic one. Reconstructing another prefilter cannot qualify that hidden
original. Do not expand this patch into a hidden-owner visitor to evade this
boundary; missing ownership remains an explicit qualification blocker.

Still required after this API:

- Literal buffers/automata, hidden prefilters and literal-only capture metadata.
- NFA/capture allocations, one-pass buffers, vector capacities and all shared
  or optional children; existing conditional component proofs still apply.
- Pool/factory/shard storage, mutable cache growth and retained fallback state.
- Initialization/transient overlap, complete work bounds, production lifetimes
  and multiplicities, allocator overhead and physical memory.
- Actual guest layouts/selected objects and adapter qualification. Native
  layout numbers or guest compilation alone cannot substitute for execution.

This patch alone cannot produce a usable complete resource descriptor, enable
schema validation, complete M7/M7-R0 or qualify a deployment. A compile-only
guest check proves API availability, not a selected strategy's observed layout.

## 6. Replayable Vendor Patch

After written approval, follow the existing compiler-patch provenance pattern:

1. Retain the exact original `.crate` and checksum, expand all original members
   into `vendor/regex-automata/`, and exclude it from workspace membership.
2. Keep one separately replayable
   `vendor/patches/regex-automata-resource-layout.patch`. The intended production
   source surface is the three owner/export files named in section 2; colocate
   unit controls in their owners. Any additional owner edit needs justification.
3. Add the root path patch without changing package versions/features. Apply
   the same patch to standalone qualification manifests, so no probe silently
   qualifies the unpatched registry implementation.
4. Verify fresh original archive plus patch equals the expanded tree, including
   exact membership, checksums, no symlinks and refusal of partial replay.
   Retain upstream license/attribution and document upgrade/rebase/removal in
   `vendor/README.md`; do not edit the installed registry cache.

Path-patching changes dependency source identity and the lockfile's corresponding
registry source/checksum entries. Review that explicit difference; do not claim
the lockfile or executable remains byte-identical. No unrelated dependency
refresh belongs in this patch. Rebind new qualification outputs to the patched
source; historical captures stay untouched.

Adding an API can change emitted code/debug/package bytes even without a new
field or vtable method. Use the actual patched artifact for subsequent size and
resource qualification; do not infer equality with an unpatched artifact.

## 7. Required Controls Before Acceptance

1. Library-local tests compare each named layout to its real private owner,
   not a Auctioneer mirror. Include concrete payloads of different sizes/alignments
   and verify dereferenced trait payloads rather than Arc handles. Typed getter
   swaps or fabricated layouts must fail; do not hardcode one native ABI as the
   portable API contract.
2. Real consumer compilation exercises both queries, private report fields and
   preserved auto traits. Build existing relevant feature modes, including
   no-alloc absence, without adding an engine feature to make the API compile.
3. Allocation controls initialize objects outside the window, repeatedly query
   them inside it, and observe zero allocations/deallocations. A nonzero
   allocation positive control proves the meter is live. Source review also
   verifies no pool/search/clone path or trait/representation change.
4. Structured report controls vary size, alignment, owner count and sharing
   independently. Reject missing/duplicate/unknown roles, wrong package/patch,
   feature/target/compiler mismatch and arithmetic overflow. A normal zero-sized
   layout must not be confused with an absent/unproved owner.
5. Re-run the existing pinned default/explicit matcher and schema cases,
   constructor/cache controls, portable compiler-patch replay and all-schema
   native/guest API builds. Do not recapture expected outcomes or change an
   existing threshold to make the patch pass. Report compile versus execution
   results separately.
6. Candidate reports cover the source-derived helper census plus the separate
   hostname owner, with explicit missing/unqualified states. They do not claim
   generated singleton observation or complete heap coverage. Preserve the
   existing NFA/one-pass/cache proofs and all missing program/work/target gates.

## 8. Review And Approval

Before patch implementation, review this API boundary and the additional
maintained dependency patch with AG. After approval, write a bounded
implementation plan with exact provenance, feature, consumer and negative-control
owners. No serving cutover, accounting coefficient or limit change is part of
that approval. An upstream equivalent can replace the patch only after the same
contract and qualification controls pass without it.
