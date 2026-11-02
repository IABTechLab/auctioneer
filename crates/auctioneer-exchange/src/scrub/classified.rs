//! Compiled-rule disclosure over a bidder-owned request.

use super::decision::Disclosure;
use super::value::{Mutation, TransformScratch};
use super::{DisclosureAudit, ScrubDecisions, fields, limits};
use crate::AuctionError;
use auctioneer_config::types::IpKeepBits;
use auctioneer_openrtb::types::bid_request::BidRequest;

/// Apply already-resolved disclosure restrictions in place.
///
/// # Errors
/// Returns a static internal error when the borrowed preflight refuses the scope.
pub fn apply_classified(
    req: &mut BidRequest,
    decisions: ScrubDecisions,
    keep_bits: IpKeepBits,
    audit: &mut DisclosureAudit,
) -> Result<(), AuctionError> {
    scope(req, decisions, keep_bits, audit, None)
}

/// Live construction supplies its already-funded normalized/projection allowance.
pub(crate) fn apply_classified_with_limits(
    req: &mut BidRequest,
    decisions: ScrubDecisions,
    keep_bits: IpKeepBits,
    audit: &mut DisclosureAudit,
    max_nodes: u64,
    max_depth: u32,
) -> Result<(), AuctionError> {
    scope(
        req,
        decisions,
        keep_bits,
        audit,
        Some((max_nodes, max_depth)),
    )
}

fn scope(
    req: &mut BidRequest,
    decisions: ScrubDecisions,
    keep_bits: IpKeepBits,
    audit: &mut DisclosureAudit,
    allowance: Option<(u64, u32)>,
) -> Result<(), AuctionError> {
    audit.start_scope();
    let admitted = match allowance {
        Some((nodes, depth)) => limits::request_with_limits(req, nodes, depth),
        None => limits::request(req),
    };
    let result = admitted.and_then(|()| mutate(req, decisions, keep_bits, audit));
    if result.is_err() {
        audit.fail_scope();
    }
    result
}

fn mutate(
    req: &mut BidRequest,
    decisions: ScrubDecisions,
    keep_bits: IpKeepBits,
    audit: &mut DisclosureAudit,
) -> Result<(), AuctionError> {
    let disclosure = Disclosure::resolve(decisions, keep_bits);
    if disclosure.unrestricted() {
        return Ok(());
    }
    let mut mutation = Mutation {
        disclosure,
        transforms: TransformScratch::default(),
        audit,
    };
    if disclosure.protected() {
        if let Some(user) = req.user.as_mut() {
            fields::user(user, &mut mutation)?;
        }
        if let Some(device) = req.device.as_mut() {
            fields::device(device, &mut mutation)?;
        }
    }
    if disclosure.deny_tid {
        fields::tid(req, mutation.audit);
    }
    Ok(())
}
