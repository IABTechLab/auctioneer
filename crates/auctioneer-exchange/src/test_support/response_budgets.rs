use crate::{BidderBudgets, ResolvedBudgets};
use auctioneer_bidder::BidderRegistry;
use auctioneer_config::types::HostConfig;

#[must_use]
pub fn response_budgets(registry: &BidderRegistry, host: &HostConfig) -> ResolvedBudgets {
    let limits = &host.resource_limits;
    let legacy = u64::try_from(limits.max_bidder_response_bytes).unwrap_or(u64::MAX);
    let caps = BidderBudgets {
        max_transport_bytes: if limits.max_transport_bytes == 0 {
            legacy
        } else {
            limits.max_transport_bytes
        },
        max_decoded_bytes: if limits.max_decoded_bytes == 0 {
            legacy
        } else {
            limits.max_decoded_bytes
        },
    };
    registry
        .base_names()
        .into_iter()
        .map(|name| (name, caps))
        .collect()
}
