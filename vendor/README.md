# Dependency Patches

## Schema Compiler

`jsonschema-macros-core/` contains the published 0.61.2 package from
[Stranger6667/jsonschema](https://github.com/Stranger6667/jsonschema), with three
separate compiler patches. It is excluded from the Auctioneer workspace and patched through
the root manifest. It generates the controlled-native validators and the
explicitly staged guest probes. This upgrade does not promote a bidder or
qualify guest serving.

The adjacent original `.crate` archive is retained for offline provenance
verification. Its SHA-256 is
`75afa4b5e6986cc221f5339d7d6149360f0bf8e65bcb23617dc0f92900b576d2`.
Its source revision is `0e82636fde8b2afb5e712ede6c261166ea10fc17`.
All 280 published members are preserved. The upstream MIT license, fetched
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

This patch changes exactly seven existing members: `Cargo.toml`,
`Cargo.toml.orig`, `src/codegen.rs`, `src/context.rs`, `src/lib.rs`, and
`src/tests.rs`, plus `src/codegen/regex.rs`. It adds one member,
`src/codegen/explicit_cache.rs`.

Version 0.61.2 introduces repetition-count matchers and collapsed search
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
[`patches/jsonschema-macros-core-0.61.2-type-check.patch`](patches/jsonschema-macros-core-0.61.2-type-check.patch),
[`patches/jsonschema-macros-core-0.61.2-regex-initialization.patch`](patches/jsonschema-macros-core-0.61.2-regex-initialization.patch),
and [`patches/jsonschema-macros-core-0.61.2-explicit-cache-all-modes.patch`](patches/jsonschema-macros-core-0.61.2-explicit-cache-all-modes.patch).
Their SHA-256 values match the portable compiler replay test pins:

- Type-check: `1d1ed4cdfdd21abbd2c469bbee6f7aa3ea8079c736dd22b9abe5d08e76265358`.
- Regex initialization: `b21e13915ba100b27a8db5c1b765b5f81c302be806d763359f73e0ca5a5be798`.
- Explicit cache: `846658280e27162d5ea8334ba347eb86e0cf9ff05d22f701e632ce648868befe`.

The original 0.58.6 archive and unversioned patch files remain historical
artifacts. They are not the active compiler pins. Published 0.61.2 snapshots
come unchanged from its original archive, not from recapturing patched output.

Integration tests replay them in order against fresh published source and
compare every changed or added member with the checked-in compiler tree.
The original 280 members remain mandatory. Exact inventories reject unknown
additions, deletions, symlinks, edits and partial replay; historical patch
outputs retain their own pins rather than inheriting a later file's hash.

For a new upstream version, retain the new original package and its checksum,
refresh the expanded source, then rebase all three patches in order. From the repository root:

```sh
git apply --check --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.61.2-type-check.patch
git apply --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.61.2-type-check.patch
git apply --check --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.61.2-regex-initialization.patch
git apply --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.61.2-regex-initialization.patch
git apply --check --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.61.2-explicit-cache-all-modes.patch
git apply --directory=vendor/jsonschema-macros-core vendor/patches/jsonschema-macros-core-0.61.2-explicit-cache-all-modes.patch
```

Update the exact version, archive and patched-file pins only in that reviewed
upgrade. Run the literal type-rejection, constructor/cache lifecycle, explicit-ABI refusal,
compatibility, provenance and linked
schema controls again, followed by the outstanding resource qualification.
If upstream incorporates a fix, prove its rejection/lifecycle cases
without that patch before deleting it. A plain `cargo update` cannot silently
replace the pinned local compiler.

## Regex Layout Queries

`regex-automata/` retains all 107 published members of version 0.4.18, with
one [portable patch](patches/regex-automata-resource-layout.patch) approved by
AG on 2026-10-28. The original archive is
`regex-automata-0.4.18.crate` (SHA-256
`ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2`,
VCS `e6ad938198e68339a593f85d228f2d44cb54718f`). The patch SHA-256 is
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
