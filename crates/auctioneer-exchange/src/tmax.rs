//! Effective-tmax + auction-deadline resolution.
//!
//! The four-value taxonomy distinguishes the adapter-visible `TMax`
//! (what the bidder sees in `BidRequest.TMax`) from the server-side
//! execution deadline (when the exchange stops waiting for responses).
//! Only the deadline is clamped to the host's `max_tmax_ms`; the
//! adapter-visible `TMax` passes through verbatim so the bidder can
//! decide its own internal latency budget.

use crate::error::AuctionError;
use crate::fanout::{AuctionDeadline, TmaxSource};
use auctioneer_config::types::HostConfig;
use auctioneer_runtime::MonotonicInstant;
use std::time::Duration;

pub struct TmaxResolution {
    /// What the bidder will see in the outbound `BidRequest.TMax`
    /// (adapter-visible `TMax`). NOT clamped to `max_tmax_ms`.
    pub bidder_visible_tmax_ms: u32,
    /// Whole-auction execution deadline (`request_start + effective_budget`).
    pub deadline: AuctionDeadline,
}

/// Resolve the four-value tmax taxonomy from the request, host caps,
/// and the anchored `request_start` instant.
///
/// # Errors
/// [`AuctionError::Internal`] when `request_start + effective_budget`
/// is not a representable instant. That is corrupt input, not a long
/// auction: `ResourceLimitsRaw::validate()` bounds `max_tmax_ms` at
/// build time, so the effective budget is always small enough for the
/// addition to succeed on a real clock. Saturating to some far-future
/// instant instead would hold every bidder call open for the length of
/// that substituted budget rather than failing the request.
pub fn resolve_tmax(
    req_tmax_ms: Option<u64>,
    host: &HostConfig,
    request_start: MonotonicInstant,
) -> Result<TmaxResolution, AuctionError> {
    // `default_tmax_ms` / `max_tmax_ms` are validated `> 0` by the
    // build-time `ResourceLimitsRaw::validate()` derive in auctioneer-config,
    // so the i64 -> u64 conversion is infallible in practice. The
    // `.max(0)` + `try_from(...).unwrap_or(0)` form is the safe
    // fallback if a future config bug slips a negative value past the
    // codegen validator.
    let default_ms = u64::try_from(host.resource_limits.default_tmax_ms.max(0)).unwrap_or(0);
    let max_ms = u64::try_from(host.resource_limits.max_tmax_ms.max(0)).unwrap_or(0);

    let (bidder_visible_ms, effective_ms, source) = match req_tmax_ms {
        None | Some(0) => {
            // Effective deadline MUST NOT exceed `max_tmax_ms` even
            // when the request omits `tmax`. A pushed config with
            // `default_tmax_ms > max_tmax_ms` would otherwise silently
            // run past the supposed hard cap. The adapter-visible
            // `tmax` passes through verbatim — it advertises the
            // bidder's slice, not the exchange's execution budget.
            let effective = default_ms.min(max_ms);
            (default_ms, effective, TmaxSource::HostDefault)
        }
        Some(req) if req > max_ms => (req, max_ms, TmaxSource::CappedByHost),
        Some(req) => (req, req, TmaxSource::Request),
    };

    let deadline_at = request_start
        .checked_add(Duration::from_millis(effective_ms))
        .ok_or_else(|| {
            AuctionError::Internal(format!(
                "auction deadline is not representable: request_start + {effective_ms}ms overflows the monotonic clock"
            ))
        })?;
    Ok(TmaxResolution {
        bidder_visible_tmax_ms: u32::try_from(bidder_visible_ms.min(u64::from(u32::MAX)))
            .unwrap_or(u32::MAX),
        deadline: AuctionDeadline {
            deadline_at,
            source,
        },
    })
}
