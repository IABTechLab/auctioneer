// Debug projection evidence for default-active factories using their owner parameters.

use super::*;
use auctioneer_bidder::test_support::serving_catalog::default_active_publications;

#[test]
fn compiled_reference_bidders_project_both_sides_without_changing_ordinary_output() {
    for bidder in default_active_publications() {
        assert_debug_capture_preserves_ordinary_output(
            bidder.name(),
            bidder.params,
            bidder.factory,
        );
    }
}
