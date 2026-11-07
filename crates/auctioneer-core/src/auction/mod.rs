//! Settings-first, resource-owning in-process auction boundary.

mod operation;
mod prepare;
mod result;
mod services;

pub use crate::handlers::request_disclosure_audit::CompletionStatus;
pub use operation::PreparedAuction;
pub use prepare::{AuctionInput, prepare_auction};
pub use result::{AuctionFailure, AuctionFailureReason, CompletedAuction, CompletedResponse};
pub use services::{
    AuctionFunding, AuctionResourceInputs, AuctionResourceProfile, AuctionServices, SchemaAllowance,
};
