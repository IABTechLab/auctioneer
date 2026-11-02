#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]

pub(crate) const SCHEMA_SOURCE_CONFORMANCE: bool = false;

mod schema_profile;

mod schema_qualification;

const _: () = assert!(
    !SCHEMA_SOURCE_CONFORMANCE,
    "shared schema controls must be synthetic"
);

mod schema_source_inventory_cases;
