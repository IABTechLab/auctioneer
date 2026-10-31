//! Auction engine. `EdgeZero`-free.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub mod account;
pub mod alias;
mod bookkeeping;
pub mod budgets;
mod construction;
pub mod construction_diagnostics;
pub mod context;
pub mod debug;
pub mod error;
pub mod fanout;
pub mod limits;
pub mod plan;
mod privacy_projection;
pub mod registry_order;
pub mod response;
pub mod responsetime;
pub mod run;
pub mod scrub;
pub mod split;
pub mod targeting;
pub mod tmax;
pub mod validate_endpoint;

pub use account::{AccountSource, ResolvedAccount, UNRESOLVED_ACCOUNT_ID};
pub use alias::{AliasMap, ResolvedAliases, resolve_aliases};
pub use budgets::{
    BidderBudgets, BootEnvelope, InternalInvariantError, ResolvedBudgets, resolve_budget,
};
pub use construction::construction_costs;
pub use context::{AuctionContext, ExchangeFeatures};
pub use error::AuctionError;
pub use fanout::{
    AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, Compression, FanoutError,
    FanoutInternalError, OversizeOrigin, OversizeRequestStage, TimeoutCause, TmaxSource,
    UndecodableEncoding,
};
pub use limits::{BidderCapDrop, apply_max_bidders};
pub use plan::AuctionPlan;
pub use registry_order::participating_bidders;
pub use response::{
    BidderResultBundle, BrowserAuctionResult, ErrorEntry, NonBidRow, ResponseInput, build_response,
};
pub use responsetime::ResponseTimeStat;
pub use run::{fanout_error_codes, run_auction};
pub use split::{PerBidderRequest, SplitBidders, split_by_bidder};
pub use tmax::{TmaxResolution, resolve_tmax};
pub use validate_endpoint::ExchangeEndpointValidator;
pub use validate_endpoint::validate_endpoint;
