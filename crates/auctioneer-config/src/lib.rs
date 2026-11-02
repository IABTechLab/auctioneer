//! Host / account / bidder configuration.
//!
//! The `data/` table (bidder TOMLs, host/account defaults, EEA country list,
//! the GVL/GCL archive) is baked at build time into `OUT_DIR/generated.rs`
//! plus `OUT_DIR/gvl_archive.rs` by `build.rs` and re-exported through this
//! crate's public surface.

pub mod bidder_endpoint;
pub mod bidder_settings;
pub mod currency;
pub mod disclosure;
mod gvl_archive;

use auctioneer_config_schema::disclosure_types;
pub use auctioneer_config_schema::{activity, bidder_storage, limits, types, usersync};

pub use bidder_settings::BidderSettings;
pub use currency::FloorCurrencyConfig;
pub use gvl_archive::BakedGvlEntry;
// A plain re-export of the sibling crate — no `include!`, no `#[path]`,
// no duplicated source file. Keeps `auctioneer_config::gvl_schema::…` call
// paths resolving for runtime callers and tests alike.
pub use auctioneer_gvl_schema as gvl_schema;

/// Whether this linked build permits development-only configuration overrides.
pub const TEST_HARNESS_OVERRIDE_ENABLED: bool = cfg!(feature = "test-harness-override");

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
include!(concat!(env!("OUT_DIR"), "/gvl_archive.rs"));

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    #[cfg(not(feature = "test-harness-override"))]
    #[test]
    fn production_build_disallows_harness_overrides() {
        assert!(!black_box(super::TEST_HARNESS_OVERRIDE_ENABLED));
    }

    #[cfg(feature = "test-harness-override")]
    #[test]
    fn explicit_harness_build_reports_override_support() {
        assert!(black_box(super::TEST_HARNESS_OVERRIDE_ENABLED));
    }
}
