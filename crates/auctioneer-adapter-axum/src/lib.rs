//! Pure access to the same source-owned envelope used by the native binaries.

pub mod envelope;
pub mod timer;

use auctioneer_config_data::types::BidderInfo;

/// Linked policy features, including dependency feature unification.
#[must_use]
pub fn build_features() -> Vec<&'static str> {
    let endpoint = "http://127.0.0.1:8999";
    let info = BidderInfo {
        endpoint: Some(endpoint.to_owned()),
        ..BidderInfo::default()
    };
    let mut features = if auctioneer_exchange::validate_endpoint(endpoint, &info).is_ok() {
        vec!["insecure_bidders"]
    } else {
        Vec::new()
    };
    if auctioneer_core::CONFORMANCE_SUITE_ENABLED {
        features.push("conformance_suite");
    }
    if auctioneer_config_data::TEST_HARNESS_OVERRIDE_ENABLED {
        features.push("test_harness_override");
    }
    features
}
