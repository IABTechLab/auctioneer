//! Source-derived logical dimensions, not complete resource qualification.

#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]

pub(crate) const SCHEMA_SOURCE_CONFORMANCE: bool = false;

mod schema_profile;

mod schema_bundle_heap;
mod schema_formats;

mod schema_cache_model;
use schema_cache_model as schema_cache_bounds;

mod schema_backtrack_bounds;

mod schema_onepass_bounds;

mod schema_hybrid_bounds;

mod schema_hybrid_population;

mod schema_heap_layout;

mod schema_metadata_heap;

mod schema_nfa_heap;

mod schema_pool_heap;

mod schema_meta_eager;

mod schema_fallback_heap;

mod schema_meta_heap;

mod schema_hybrid_heap;

mod schema_hybrid_graph;

mod schema_hybrid_capacity;

mod schema_hybrid_prefix;

mod schema_qualification;

const _: () = assert!(
    !SCHEMA_SOURCE_CONFORMANCE,
    "shared schema controls must be synthetic"
);

mod schema_cache_bounds_cases;
