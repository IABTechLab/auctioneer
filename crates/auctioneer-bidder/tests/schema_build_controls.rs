//! Linked compiler controls, not the resource/work qualification acceptance gate.

#![cfg(all(not(target_arch = "wasm32"), unix))]
#![cfg(test)]

pub(crate) const SCHEMA_SOURCE_CONFORMANCE: bool = false;

mod schema_profile;

mod schema_build;
mod schema_qualification;
pub(crate) mod schema_reference;
mod schema_replay;
mod schema_vendor;

mod schema_heap_layout;

mod schema_cache_model;
use schema_cache_model as schema_cache_bounds;

mod schema_backtrack_bounds;

mod schema_fallback_heap;

const _: () = assert!(
    !SCHEMA_SOURCE_CONFORMANCE,
    "shared schema controls must be synthetic"
);

mod schema_build_cases;

// The source-conformance root supplies the source-only bundle proof.
fn assert_source_explicit_cache_layouts(_catalog: &schema_build::Catalog) {}
