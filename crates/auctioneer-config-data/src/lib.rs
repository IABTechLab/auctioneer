//! Host / account / bidder configuration.
//!
//! The `data/` table (bidder TOMLs, host/account defaults, EEA country list,
//! the GVL/GCL archive) is baked at build time into `OUT_DIR/generated.rs`
//! plus `OUT_DIR/gvl_archive.rs` by `build.rs` and re-exported through this
//! crate's public surface.

pub use auctioneer_config::currency;
pub mod disclosure;
mod gvl_archive;

use auctioneer_config::disclosure_types;
pub use auctioneer_config::{activity, bidder_storage, limits, types, usersync};

pub use auctioneer_config::currency::FloorCurrencyConfig;
pub use gvl_archive::BakedGvlEntry;
// A plain re-export of the sibling crate — no `include!`, no `#[path]`,
// no duplicated source file. Keeps `auctioneer_config_data::gvl_schema::…` call
// paths resolving for runtime callers and tests alike.
pub use auctioneer_gvl_schema as gvl_schema;

/// Whether this linked build permits development-only configuration overrides.
pub const TEST_HARNESS_OVERRIDE_ENABLED: bool = cfg!(feature = "test-harness-override");

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
include!(concat!(env!("OUT_DIR"), "/gvl_archive.rs"));

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::{hint::black_box, ptr};

    #[test]
    fn default_accessors_share_the_product_instances() {
        assert!(ptr::eq(
            super::host_default(),
            super::product_host_default()
        ));
        assert!(ptr::eq(
            super::account_default(),
            super::product_account_default()
        ));
    }

    #[test]
    fn product_host_defaults_preserve_resources_without_an_operator_service() {
        let host = super::product_host_default();
        assert!(host.user_sync.is_none());
        assert!(host.aliases.is_empty());
        assert!(host.gdpr.non_standard_publishers.is_empty());
        assert_eq!(host.gdpr.host_vendor_id, Some(0));
        assert!(host.host_cookie.domain.is_none());
        assert!(host.host_cookie.optout_cookie.is_none());
        assert!(host.status_response.is_empty());
        assert_eq!(
            serde_json::to_value(&host.resource_limits).unwrap(),
            json!({
                "default_tmax_ms": 1000_i64,
                "max_tmax_ms": 2000_i64,
                "max_bidder_response_bytes": 0x0004_0000_usize,
                "max_bidders_per_auction": 50_usize,
                "max_outbound_requests": 100_usize,
                "max_outbound_raw_bytes": 0x0004_0000_u64,
                "max_outbound_request_bytes": 0x0004_0000_u64,
                "max_transport_bytes": 0x0004_0000_u64,
                "max_decoded_bytes": 0x0004_0000_u64,
                "max_parsed_nodes": 0x0001_0000_u32
            })
        );
    }

    #[test]
    fn product_account_defaults_preserve_neutral_policy() {
        let account = super::product_account_default();
        assert!(!account.disabled);
        assert!(!account.debug_allow);
        assert_eq!(account.bid_rounding, "down");
        assert_eq!(account.targeting_prefix, "hb");
        assert!(account.truncate_target_attribute.is_none());
        assert!(account.gdpr.purposes.is_empty());
        assert!(account.gdpr.special_features.is_empty());
        assert!(account.gdpr.basic_enforcement_vendors.is_empty());
        assert_eq!(account.privacy.ipv4.anon_keep_bits, 24);
        assert_eq!(account.privacy.ipv6.anon_keep_bits, 56);
        assert!(account.cookie_sync.default_limit.is_none());
        assert!(account.cookie_sync.max_limit.is_none());
        assert!(account.cookie_sync.priority_groups.is_none());
    }

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
