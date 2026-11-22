# Dependency Patches

## Schema Compiler

`jsonschema-macros-core/` contains the published 0.62.0 package from
[Stranger6667/jsonschema](https://github.com/Stranger6667/jsonschema), with three
separate compiler patches. It is excluded from the Auctioneer workspace and patched through
the root manifest. It generates the controlled-native validators and the
explicitly staged guest probes. This upgrade does not promote a bidder or
qualify guest serving.

The adjacent original `.crate` archive is retained for offline provenance
verification. Its SHA-256 is
`e869997df6b45ae196cfa9261861bd21f0b6cc786fc12e8e041e2f0ccf8690f7`.
Its source revision is `24280ebc025661f9743d969c7ea51122e21f205a`.
All 298 published members are preserved. The upstream MIT license, fetched
from that source revision, is added because the published package omits it.

The type-check patch changes only `src/codegen/object_schema.rs`. It no longer
omits an explicit type check merely because typed keyword dispatch was
generated. Keyword dispatch no longer takes responsibility for that type
constraint; the existing type-keyword compiler enforces it separately,
including guarded numeric unions. In the original compiler, an inapplicable string/array/object
keyword can introduce a successful arm for a type that the schema forbids.
Restoring the existing maintained type check prevents that arm from granting
validation. No schema, coercion rule, runtime compiler or Auctioneer type prefilter
is introduced. The extra check must be included in work/code-size qualification.

Version 0.62.0 separates validity checks from phase-ordered errors. Its typed
dispatch repairs some forbidden-type arms, but the unchanged numeric-union
fixture fails to compile without the independent type-check patch. The rebase
keeps the declared type check separate while preserving the new error phases.

The initialization patch changes only `src/codegen/emit_root.rs`. Each
generated validator exposes `initialize_regexes() -> bool`, forcing every
generated pattern constructor without short circuit after a failure. It shares
the existing lazy storage with validation, so repeat/concurrent calls construct
each regex once and do not retry a cached construction error. Patterns,
engines, builder limits and validation methods are unchanged. This does not
initialize custom formats, saturate search caches, establish memory/work
bounds or activate production schema validation.

The explicit-cache patch adds the opt-in boolean-only
`pattern_options = { engine = regex, cache = explicit }` API. It replaces
pooled pattern searches with the locked automata engine's explicit mutable
caches, threads the validator-specific context through generated helpers,
and moves generated reference/URI state into that context. The public cache
type has private fields and no `Default` or `Clone`; construction forces all
shared regex constructors and returns `None` on a cached constructor error.
Non-opted-in output keeps the existing ABI. The existing `syn` dependency
enables structured mutable AST traversal for this isolated lowering pass.

Version 0.62.0 also adds reference-result memoization and evaluation methods.
Explicit mode keeps the new memo results, visit counter and dynamic-scope
identities inside the validator-specific context, preserving upstream
container-identity keys and the sixteen-visit retention threshold. It does not
call the runtime's thread-local memo helpers. Its boolean-only method contract
now explicitly requires `evaluate = false`; opted-in evaluation is refused.
Memo ownership does not establish a resource or work bound for recursive schemas.

This patch changes exactly seven existing members: `Cargo.toml`,
`Cargo.toml.orig`, `src/codegen.rs`, `src/context.rs`, `src/lib.rs`, and
`src/tests.rs`, plus `src/codegen/regex.rs`. It adds one member,
`src/codegen/explicit_cache.rs`.

Version 0.61.2 introduced repetition-count matchers and collapsed search
patterns before engine construction. The rebased patch retains original-pattern
engine construction in every mode, including the default and supplied size
limits. Otherwise these substitutions bypass the constructor limits and the
initialization failure contract. The unchanged constructor controls and direct
oversized-repetition refusal tests cover this compatibility requirement; it is
not permission to substitute a matcher or relax a resource limit.

The direct automata dependency is native test-only. Its resolved engine
features and configured NFA/hybrid limits are unchanged. Explicit ownership
does not prove internal cache growth, fallback storage, work, or target fit;
the separate hostname-format pool is not changed by this patch. These remain
qualification gates, and no production validation callback is enabled.

The schema controls verify the original archive and expanded files before
constructing standalone qualification manifests. Those manifests explicitly
carry the same patch; they cannot accidentally qualify the unpatched registry
compiler. Upstream snapshots are preserved as provenance, not recaptured to
bless changed output. Auctioneer exercises the generated callbacks separately.

AG approved the type-check repair on 2026-10-19, the constructor API on
2026-10-21, and the [explicit-cache contract](../docs/superpowers/specs/2026-10-22-explicit-schema-search-cache-design.md)
on 2026-10-22. Remove the patches only
after a maintained upstream replacement passes the required rejection cases,
the complete schema compatibility set, and target/resource qualification.

## Upgrades

The portable changes are
[`patches/jsonschema-macros-core-0.62.0-type-check.patch`](patches/jsonschema-macros-core-0.62.0-type-check.patch),
[`patches/jsonschema-macros-core-0.62.0-regex-initialization.patch`](patches/jsonschema-macros-core-0.62.0-regex-initialization.patch),
and [`patches/jsonschema-macros-core-0.62.0-explicit-cache-all-modes.patch`](patches/jsonschema-macros-core-0.62.0-explicit-cache-all-modes.patch).
Their SHA-256 values match the portable compiler replay test pins:

- Type-check: `66ff2840a027d5a1b311ff68b66eeb0fd7a9220febb0e58a62109b54257632c5`.
- Regex initialization: `781eed59d8ccabdbd0f848ba34f3a434899cc0508080927294878cae056cd4d6`.
- Explicit cache: `3a2176665de3cb2a51d4992e71de5b3181c819586e17cc7de637647737589992`.

The original 0.58.6 and 0.61.2 archives, unversioned compiler patches and
versioned 0.61.2 patches remain historical artifacts. They are not the active
compiler pins. Published 0.62.0 snapshots
come unchanged from its original archive, not from recapturing patched output.

Integration tests replay them in order against fresh published source and
compare every changed or added member with the checked-in compiler tree.
The original 298 members remain mandatory. Exact inventories reject unknown
additions, deletions, symlinks, edits and partial replay; historical patch
outputs retain their own pins rather than inheriting a later file's hash.

For a new upstream version, retain the new original package and its checksum,
refresh the expanded source, then rebase all three patches in order. From the repository root:

```sh
git apply --check --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.62.0-type-check.patch
git apply --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.62.0-type-check.patch
git apply --check --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.62.0-regex-initialization.patch
git apply --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.62.0-regex-initialization.patch
git apply --check --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.62.0-explicit-cache-all-modes.patch
git apply --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.62.0-explicit-cache-all-modes.patch
```

Update the exact version, archive and patched-file pins only in that reviewed
upgrade. Run the literal type-rejection, constructor/cache lifecycle, explicit-ABI refusal,
compatibility, provenance and linked
schema controls again, followed by the outstanding resource qualification.
If upstream incorporates a fix, prove its rejection/lifecycle cases
without that patch before deleting it. A plain `cargo update` cannot silently
replace the pinned local compiler.

## Regex Layout Queries

`regex-automata/` retains all 107 published members of version 0.4.19, with
one [portable patch](patches/regex-automata-resource-layout.patch) approved by
AG on 2026-10-28. The original archive is
`regex-automata-0.4.19.crate` (SHA-256
`41d60ef07df457506a0a1a2073112eb4aae5733dbd2813c3da77d1b0fd441158`,
VCS `4226f214c56e584304520adb9080bcc4c792390e`). The patch SHA-256 is
`5d853f05430eda9b9e80fb1f359cddce730dcf86a61be32efa2605a46702e020`.

The patch changes only `src/meta/mod.rs`, `src/meta/regex.rs` and
`src/util/prefilter/mod.rs`. Read-only `resource_layout()` queries describe
the actual regex root, metadata and selected strategy payloads, and an
accessible public prefilter's matcher payload. They preserve matching,
features, representations and stored fields, with no unsafe code. Reports
exclude Arc headers, all children, capacities, caches and physical memory.
They do not identify shared allocations or expose hidden automatic prefilters.
Native candidate observations are not generated-singleton or guest evidence.
See the [approved contract](../docs/superpowers/specs/2026-10-28-regex-resource-layout-api-design.md).

The layout patch replays unchanged on 0.4.19. This upstream release fixes
matching boundary cases and changes the default meta-regex pool capacity from
native available parallelism to eight. Explicit-cache validators and Auctioneer's
custom hostname-format owner retain their configured pool capacity of one.
Stock constructors that leave the pool unset use the new default; this source
update does not recertify old pool observations. Regex-syntax 0.8.12 preserves
the previous Unicode table bytes,
repairs parser reuse and leftmost-first prefix factoring, and canonicalizes
intervals in place. Existing bounds, workloads and captures remain unchanged.

Standalone schema probes must resolve both reviewed patches. Engine controls
replay the pinned diff against the pinned archive and compare the whole tree;
manifest, lockfile and resolved-feature checks bind the source actually used.
For an engine upgrade, retain the new original package and checksum, replay or
rebase this separate patch, and rerun owner-local API tests, feature boundaries,
source controls and complete schema compatibility before updating pins. From a
fresh original tree, the patch can be replayed with:

```sh
git apply --check --directory=vendor/regex-automata vendor/patches/regex-automata-resource-layout.patch
git apply --directory=vendor/regex-automata vendor/patches/regex-automata-resource-layout.patch
```

An upstream replacement must prove these APIs and boundaries before removing
the patch. Layout queries alone do not qualify a schema engine or enable
production parameter validation.
