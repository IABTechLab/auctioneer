//! Targeting key construction + winner selection. See spec Appendix E.

pub mod keys;
pub mod price_bucket;
pub mod winner;

pub use keys::{TargetingInput, TargetingKeys, build_keys};
pub use price_bucket::{
    BidRounding, PriceGranularity, PriceGranularityScope, Range, parse_price_granularity,
    price_bucket,
};
pub use winner::{Winner, is_new_winning_bid, pick_winner};
