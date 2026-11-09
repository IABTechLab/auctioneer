//! Settings-first, resource-owning in-process auction boundary.

pub mod audit;
mod operation;
mod prepare;
mod result;
mod services;

pub use audit::CompletionStatus;
pub use operation::PreparedAuction;
pub use prepare::{AuctionInput, prepare_auction};
pub(crate) use prepare::{MAX_AUCTION_BODY, disabled_from_overlay};
pub use result::{AuctionFailure, AuctionFailureReason, CompletedAuction, CompletedResponse};
pub use services::{
    AuctionFunding, AuctionResourceInputs, AuctionResourceProfile, AuctionServices, SchemaAllowance,
};
