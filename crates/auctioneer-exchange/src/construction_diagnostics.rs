//! Auction-owned bounds for nonfatal construction rows and their wire projection.

use auctioneer_runtime::CatalogResourceCosts;

const DIAGNOSTIC_ROOT_BYTES: u64 = 4_096;
const DIAGNOSTIC_BIDDER_BYTES: u64 = 98_304;

/// Context storage is funded even when a participating bidder produces no calls.
#[must_use]
pub fn construction_diagnostic_costs(bidder_names: u32) -> CatalogResourceCosts {
    let context = if bidder_names == 0 {
        0
    } else {
        // A u32 population cannot overflow this u64 expression.
        u64::from(bidder_names)
            .saturating_mul(DIAGNOSTIC_BIDDER_BYTES)
            .saturating_add(DIAGNOSTIC_ROOT_BYTES)
    };
    CatalogResourceCosts {
        context_per_auction_bytes: context,
        ..CatalogResourceCosts::UNCHANGED
    }
}

#[cfg(test)]
mod tests {
    use super::construction_diagnostic_costs;
    use auctioneer_runtime::CatalogResourceCosts;

    #[test]
    fn diagnostic_cost_counts_bidder_names_without_a_call_factor() {
        for (names, expected) in [(0, 0), (1, 102_400), (16, 1_576_960), (50, 4_919_296)] {
            assert_eq!(
                construction_diagnostic_costs(names),
                CatalogResourceCosts {
                    resident_bytes: 0,
                    initialization_bytes: 0,
                    schema_validation_per_auction_bytes: 0,
                    context_per_auction_bytes: expected,
                }
            );
        }
        assert_eq!(
            construction_diagnostic_costs(u32::MAX).context_per_auction_bytes,
            422_212_464_971_776,
        );
    }
}
