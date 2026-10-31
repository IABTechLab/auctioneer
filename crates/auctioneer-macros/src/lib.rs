//! Compile-time transformations of Rust declarations for Auctioneer.
//!
//! This is the workspace's procedural-macro boundary. Each API has its own
//! reviewed input, expansion and diagnostic contract.
//!
//! Keep expansion-helper unit tests beside their implementation. Integration
//! tests under `tests/` exercise exported macros through the Rust compiler.
//! Filesystem discovery, provenance checks and build identity remain build-script
//! responsibilities rather than implicit side effects of a macro expansion.

mod field_metadata;

/// Describe the potential wire fields of an owned, nongeneric named-field struct.
///
/// Field eligibility uses compiler-resolved `FieldMetadata`, not type syntax. Supported
/// Serde options are field `rename`, `flatten`, `default`, `skip_serializing_if`
/// and container `default`. Other Serde options are rejected, not ignored.
/// `#[openrtb_metadata(name = "EID", crate_path = crate::field_metadata)]`
/// overrides the object name and metadata interface path independently.
#[proc_macro_derive(OpenRtbFieldMetadata, attributes(openrtb_metadata, serde))]
pub fn derive_openrtb_field_metadata(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let declaration = syn::parse_macro_input!(input as syn::DeriveInput);
    field_metadata::expand(&declaration)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
