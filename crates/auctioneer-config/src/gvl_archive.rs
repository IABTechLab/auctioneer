//! DTO for the baked GVL archive `auctioneer-privacy` consumes. The
//! entries themselves are emitted by `build.rs::emit_gvl_archive` into
//! `OUT_DIR/gvl_archive.rs` (one `include_bytes!` per manifest entry)
//! and surfaced through `auctioneer_config::gvl_archive()`.

/// One bundled `(spec, vendor_list_version)` GVL blob, validated at
/// build time. Includes the manifest metadata so `/_auctioneer/health/gvl`
/// can report versions and dates without a second file read.
#[derive(Debug)]
pub struct BakedGvlEntry {
    pub spec: u32,
    pub vendor_list_version: u32,
    pub tcf_policy_version: u32,
    /// Raw bytes, embedded via `include_bytes!`. The runtime lowers
    /// them lazily via `Gvl::load_from_json_bytes` inside
    /// `GvlStore::get` — byte slices, so boot performs no UTF-8
    /// revalidation of the whole archive.
    pub raw: &'static [u8],
    /// SHA-256 from the manifest, verified at build time.
    pub sha256: &'static str,
    /// `last_updated` copied from the archive JSON (RFC 3339 UTC).
    pub last_updated: &'static str,
}
