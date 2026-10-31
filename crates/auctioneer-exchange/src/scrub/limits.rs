//! Borrowed resource admission and actual owning layout charges.

use super::audit::DisclosureAudit;
use super::value::{AddressBuffer, Frames, Mutation};
use crate::AuctionError;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::RequestConstructionCosts;
use auctioneer_runtime::json_preflight::{MAX_CONSTRUCTION_DEPTH, preflight};
use std::mem::size_of;

/// Audit ownership, reusable mutation scratch and retained address destinations.
///
/// # Panics
/// Panics if the fixed mutation layouts cannot be summed/multiplied in `u64`.
/// Supported native and wasm32 layouts cannot reach this overflow.
#[must_use]
#[expect(
    clippy::as_conversions,
    reason = "actual layout byte sizes on supported native and wasm32 targets fit u64 in const evaluation"
)]
#[expect(
    clippy::panic,
    reason = "fixed layout overflow must refuse the descriptor rather than undercharge its actual memory"
)]
pub const fn construction_costs() -> RequestConstructionCosts {
    let Some(reusable_scratch_bytes) =
        (size_of::<Frames<'static>>() as u64).checked_add(size_of::<Mutation<'static>>() as u64)
    else {
        panic!("request disclosure scratch layout overflow");
    };
    // Device owns one typed IPv6 slot and one possible flattened shadow slot.
    let Some(retained_per_scope_bytes) = (size_of::<AddressBuffer>() as u64).checked_mul(2) else {
        panic!("request disclosure address destination layout overflow");
    };
    RequestConstructionCosts {
        fixed_per_auction_bytes: size_of::<DisclosureAudit>() as u64,
        reusable_scratch_bytes,
        retained_per_scope_bytes,
    }
}

pub(crate) fn failure() -> AuctionError {
    AuctionError::Internal("request disclosure construction limit exceeded".into())
}

/// Standalone callers still require caller-owned admission of byte/node width.
/// First prove destruction/serialization depth, then derive a checked work cap
/// from the representation size without retaining output.
pub(crate) fn request(req: &BidRequest) -> Result<(), AuctionError> {
    preflight(req, u64::MAX, MAX_CONSTRUCTION_DEPTH).map_err(|_error| failure())?;
    let bytes = auctioneer_runtime::serialized_json_bytes(req).map_err(|_error| failure())?;
    // Each syntactic key/value needs at least one byte in this representation.
    // Size counting uses checked arithmetic, including raw and duplicate fields.
    request_with_limits(req, bytes, MAX_CONSTRUCTION_DEPTH)
}

pub(crate) fn request_with_limits(
    req: &BidRequest,
    max_nodes: u64,
    max_depth: u32,
) -> Result<(), AuctionError> {
    preflight(req, max_nodes, max_depth.min(MAX_CONSTRUCTION_DEPTH)).map_err(|_error| failure())?;
    Ok(())
}
