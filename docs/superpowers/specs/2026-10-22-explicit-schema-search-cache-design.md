# Explicit Schema Search Caches

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Status: written contract approved by AG on 2026-10-22. The opt-in compiler API
is implemented; verification is recorded in the [plan](../plans/2026-10-22-explicit-schema-search-cache.md).
Complete resource qualification is pending. This does not approve serving integration, new workload limits,
another engine, or higher resource ceilings.

## Problem And Choice

The candidate compiler's shared regex constructors are initialized explicitly,
but boolean matching uses `regex::Regex::is_match`. Its locked implementation
borrows mutable search state from implicit pools. Cache populations depend on
thread/shard history, not just the number of simultaneous auctions.

Choose caller-owned mutable search caches. A mutex around one global cache
would serialize validation; retaining implicit pools would keep the existing
ownership proof obligation. Neither is an acceptable substitute for the
selected explicit interface. No TLS, ambient current-context variable,
per-request regex compilation, new workspace crate, or custom regex engine is
introduced.

The baseline remains `jsonschema-macros-core 0.58.6`, with the existing two
portable patches, `regex 1.12.4` and `regex-automata 0.4.18`. The latter is
already locked but not a direct emitted-code dependency. Its dependency and
resolved features must be made explicit without enabling additional engines.
Record the before/after feature graph, rather than assuming that adding its
default features preserves the current graph.

## Compiler Contract

Add an opt-in `cache = explicit` setting inside `pattern_options`, supporting
the existing linear `regex` engine, `serde_json::Value` backend and boolean-only
method selection. Reject incompatible engines, backends and enabled error
methods at compilation, not by quietly reverting to pooled matching. Existing
non-opted-in compiler behavior stays unchanged.

For an opted-in validator named `Schema`, generate:

- A same-visibility `SchemaSearchCache` type, with private fields, no `Default`
  or `Clone`, and one mutable cache per deduplicated emitted pattern helper.
- `Schema::new_search_cache() -> Option<SchemaSearchCache>`, initializing all
  shared pattern constructors, then constructing the explicit caches. A cached
  regex-construction failure returns `None`; callers cannot install that
  validator as ready. Matching neither creates a top-level bundle/meta cache
  nor acquires pooled caches; internal engine state may initialize or grow
  within independently pre-funded bounds.
- `Schema::is_valid(&serde_json::Value, &mut SchemaSearchCache) -> bool` instead
  of the one-argument boolean entry point in this opt-in mode. Internal object,
  array, branch, pattern-property and recursive helper calls carry the same
  mutable context. There is no one-argument pooled fallback on this validator.

The name-specific cache type prevents passing one validator's cache to another.
The borrow prevents simultaneous mutation; do not introduce a lock, TLS, unsafe
aliasing, or an internal clone to evade it. A schema without emitted patterns
still has a valid empty cache type and the same explicit entry signature.
Reserved generated-name collisions need a clear compile failure.

Retain the existing per-validator helper deduplication; do not infer catalog-wide
sharing from equal pattern text. Count separate statics and caches separately
unless actual shared storage is established. `initialize_regexes()` remains a
constructor operation, not a cache-population or complete-residency claim.

## Matching Compatibility

Use the locked public `regex_automata::meta::Regex` builder and `Cache` APIs,
not the private field inside `regex::Regex`. Reproduce the string builder's
`LeftmostFirst`, UTF-8 syntax and `utf8_empty(true)` configuration and its
algorithm-selection defaults. Boolean matching is
`search_half_with(cache, &Input::new(subject).earliest(true)).is_some()`, the
existing string `is_match` search shape, not full-span `search_with`.

The existing 65,536-byte `size_limit` maps to
`nfa_size_limit(Some(65_536))`; the existing `dfa_size_limit` maps to
`hybrid_cache_capacity(65_536)`, **not** the similarly named meta DFA limit.
Keep translated pattern text, drafts, Unicode/UTF-8 behavior, unanchored
matching, custom-format semantics and all workload ceilings unchanged.
No strategy is disabled to make measurements fit.

Explicit searches bypass pooled payload acquisition. Meta regex construction
still allocates pool bookkeeping. Configure its public `pool_capacity(1)` to
make that unused bookkeeping independent of host CPU count, and count it.
This is a shard-bookkeeping choice, not a one-cache allocation cap. Never call
a pooled search API, clone the shared meta regex, or count the pool as absent.

## Ownership And Funding

This API makes ownership inspectable; it does not prove that a proposed layout
fits. Before any production integration, name the actual bundle owners and
derive their simultaneous and retained multiplicities from existing admission
and execution lifetimes, including construction, cancellation and release.
Native thread histories and all four target adapters need coverage. Do not
choose one bundle merely because the qualification harness is single-threaded,
introduce a lower concurrency limit, or assume a registry-wide mutable bundle
can be borrowed concurrently.

Account immutable compiled programs and unused pool bookkeeping once per actual
shared object. Account every explicit bundle, all retained fallback-engine
state, custom-format state and construction/growth overlap at their actual
lifetimes. Keep the existing single additive catalog/resource equation and
C/I/V/K ownership definitions; do not move the same bytes into a different
term to pass a ceiling or count them twice. The qualification report must state
the selected owner layout before reporting its numeric costs.

`create_cache()` is **not** a reserve/freeze API. PikeVM/backtracker state can
initialize later, and hybrid caches can grow. Pre-funding means independently
bounded storage and work are admitted before that growth, not a claim that
every later match allocates zero bytes. If any required bound is unavailable
or exceeds the unchanged envelope, report qualification as pending/refused;
do not emit a usable cost descriptor or enable serving validation.

`Cache::memory_usage()` omits `capmatches` and inline storage. Several engine
reports use vector lengths rather than retained capacities; hybrid accounting
also approximates map storage and has saved-state/shared-allocation details.
Those reports and finite allocator samples cannot be the sole byte proof.
Derive capacities, allocation headers, fallback caches, forward/reverse hybrid
state and replacement overlap from the locked source and target layouts.

The initial hostname format hook owned a separate pooled `regex::Regex`,
**outside** the pattern-helper patch. Its bounded input length did not remove
its pool ownership obligation. The separately approved follow-up below removes
that retained search pool, not the need for complete format/cache bounds.
Other custom-format state and numeric-formatting allocations remain in the
same overall gate. No hidden owner is excused by solving pattern caches.

## Hostname Hook Follow-Up

AG approved this qualification-only change on 2026-10-22: keep the callback
signature, exact hostname pattern, byte-length guard and existing linear
meta-regex engine, with a new explicit cache scoped to each callback invocation.
Use the same string matching configuration and both 65,536-byte builder limits;
keep all existing algorithm choices. No compiler API or portable compiler patch
changes. Default and explicit generated controls use the same format hook.

Retain one shared `LazyLock<Option<meta::Regex>>`; existing format initialization
forces that constructor, including cached failure. Set `pool_capacity(1)` for
unused bookkeeping and call only explicit `search_half_with` with earliest
boolean matching. After checking the unchanged `<256` byte guard, construct the
local cache, search and drop the cache before returning. Do not clone the regex,
store a mutable cache in a static/TLS/pool or compile a regex per invocation.
Construction failure remains a false format result and failed initialization.

Fund per-call cache construction, growth, scratch and destruction, including
their overlap with the enclosing validator's retained pattern bundle. Immutable
format program/bookkeeping belong to shared residency and initialization.
The existing additive resource equation and all workload limits stay unchanged.
No reusable runtime cost descriptor or serving path is approved by this change.

Validation must retain all literal/reference hostname outcomes, observe zero
retained heap after every short/changed/repeated hostname callback, and charge
both cache construction and search in that measurement window. A 256-byte input
must reject before cache construction. Preserve the former pooled measurements
as dated history rather than silently relabeling them as the new layout.

## Verification And Upgrade

Keep this as a third, independently replayable compiler patch. Verify fresh
published archive -> existing patches -> explicit-cache patch against the
checked-in vendor tree, including every changed file and source checksum.
Update the upgrade instructions and resolved-source controls together. No
source-schema or external expected-output capture changes are permitted.

Name the source/build cohort before reporting this candidate qualified. Under
the [2026-10-29 shipped-schema amendment](2026-10-29-shipped-schema-qualification-design.md),
bidder catalog resource acceptance covers its complete target/feature-specific `S`, not
unshipped catalog backends. Keep all-source compatibility controls below.
Newly reachable strategies require qualification before their bidder expansion wave;
remaining catalog closure owns residual universal catalog resource closure. A needed bidder catalog owner cannot
be deferred, and observations cannot replace its required bound.

Required controls for that named qualification claim:

1. Real generated validators compile with the explicit API; incompatible
   settings, wrong-validator cache types and missing mutable contexts fail.
   Default compiler APIs still compile, and emitted helper/owner counts are
   reconciled, including the no-pattern case.
2. All 300 captured parameter cases and the existing type, format, Unicode,
   branching, property-name, initialization-failure and default/explicit
   differential controls retain their literal expected results. Exercise all
   relevant generated branches, not just root required-field failures.
3. Separate initialized bundles interleave and execute concurrently without
   cross-validator or cross-request state leakage. Construction failure never
   grants validity. Inspection of generated source and actual linked execution
   prove the explicit path does not call pooled matching or clone the regex.
4. Independently derived bounds cover every generated schema and reachable
   retained strategy in `S`, including custom formats and disabled shipped
   definitions. Universal all-261 resource coverage is the remaining catalog closure closure target,
   reached through the same checks on each intervening expansion wave.
   Cold, growth, warm, alternating
   validator and concurrent positive controls test the derivation; deliberately
   undercharged/missing ownership terms fail. Inputs and ownership counts vary
   independently, rather than deriving expectations from the charged constants.
5. Re-run native and all three guest linked artifacts, target layout/resource
   controls and the unchanged compatibility/portable-patch checks. Link success
   is not guest execution, a byte bound, or physical WASM memory qualification.

Source audit anchors are the locked `regex/src/builders.rs` and
`regex/string.rs::Regex::is_match`; `regex-automata/src/meta/regex.rs` public
builder/search/cache APIs, `meta/strategy.rs`, `meta/wrappers.rs`,
`util/pool.rs`, and the individual engine cache implementations. Exact archive
and existing source witnesses are recorded in
[schema qualification](../../bidders/schema-qualification.md#runtime-helper-scope).
The constructor/cache API feasibility was inspected locally on 2026-10-22;
the patch/API controls are now separate from the still-pending complete byte,
work and target qualification above.
