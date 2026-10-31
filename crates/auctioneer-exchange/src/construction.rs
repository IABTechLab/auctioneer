//! Complete exchange-owned auction-construction costs, independent of bidders.

use crate::scrub;
use auctioneer_runtime::RequestConstructionCosts;
use auctioneer_runtime::bidder_credentials::BIDDER_CREDENTIAL_WORKING_BYTES;

const COSTS: RequestConstructionCosts = {
    let scrub = scrub::construction_costs();
    RequestConstructionCosts {
        reusable_scratch_bytes: scrub.reusable_scratch_bytes + BIDDER_CREDENTIAL_WORKING_BYTES,
        ..scrub
    }
};

/// Reusable construction ownership passed unchanged into boot and runtime checks.
#[must_use]
pub const fn construction_costs() -> RequestConstructionCosts {
    COSTS
}

#[cfg(test)]
mod tests {
    use super::construction_costs;
    use crate::scrub;

    #[test]
    fn credentials_have_one_independent_sequential_scratch_bank() {
        let before = scrub::construction_costs();
        let complete = construction_costs();
        assert_eq!(
            complete.fixed_per_auction_bytes,
            before.fixed_per_auction_bytes
        );
        assert_eq!(
            complete.retained_per_scope_bytes,
            before.retained_per_scope_bytes
        );
        assert_eq!(
            complete
                .reusable_scratch_bytes
                .checked_sub(before.reusable_scratch_bytes),
            Some(128 * 1024)
        );
    }
}
