//! Shared policy models composed with this build's bidder-owned settings.

use auctioneer_bidder::BidderSettings;
use auctioneer_config::policy::OperatorPolicy as SharedOperatorPolicy;
pub use auctioneer_config::policy::*;

/// Shared policy composed with the discovered bidder settings aggregate.
pub type OperatorPolicy = SharedOperatorPolicy<BidderSettings>;
