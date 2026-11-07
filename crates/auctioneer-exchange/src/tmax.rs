//! Effective-tmax + auction-deadline resolution.
//!
//! The four-value taxonomy distinguishes the adapter-visible `TMax`
//! (what the bidder sees in `BidRequest.TMax`) from the server-side
//! execution deadline (when the exchange stops waiting for responses).
//! Only the deadline is clamped to the host's `max_tmax_ms` and optional
//! absolute outer-host deadline; the
//! adapter-visible `TMax` passes through verbatim so the bidder can
//! decide its own internal latency budget.

use crate::error::AuctionError;
use crate::fanout::{AuctionDeadline, TmaxSource};
use auctioneer_config_data::types::HostConfig;
use auctioneer_runtime::MonotonicInstant;
use std::time::Duration;

pub struct TmaxResolution {
    /// What the bidder will see in the outbound `BidRequest.TMax`
    /// (adapter-visible `TMax`). NOT clamped to `max_tmax_ms`.
    pub bidder_visible_tmax_ms: u32,
    /// Whole-auction execution deadline, including any absolute outer cap.
    pub deadline: AuctionDeadline,
}

impl TmaxResolution {
    /// Intersect an absolute outer cap without changing bidder-visible tmax or
    /// its source. The original request-start anchor is never refreshed.
    #[must_use]
    pub fn with_outer_deadline(mut self, outer_deadline: Option<MonotonicInstant>) -> Self {
        if let Some(cap) = outer_deadline {
            self.deadline.deadline_at = self.deadline.deadline_at.min(cap);
        }
        self
    }
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
    // build-time `ResourceLimitsRaw::validate()` derive in auctioneer-config-data,
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

#[cfg(test)]
mod tests {
    use super::resolve_tmax;
    use crate::fanout::TmaxSource;
    use auctioneer_runtime::{Clock as _, FakeClock};
    use std::time::Duration;

    #[test]
    fn host_outer_deadline_caps_execution_without_changing_tmax_or_source() {
        let mut host = auctioneer_config_data::host_default().clone();
        host.resource_limits.default_tmax_ms = 80;
        host.resource_limits.max_tmax_ms = 100;
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let start = clock.now_monotonic();
        for (request, visible, source, original_ms) in [
            (None, 80, TmaxSource::HostDefault, 80),
            (Some(0), 80, TmaxSource::HostDefault, 80),
            (Some(90), 90, TmaxSource::Request, 90),
            (Some(250), 250, TmaxSource::CappedByHost, 100),
        ] {
            for outer_ms in [None, Some(40), Some(80), Some(100), Some(120)] {
                let outer =
                    outer_ms.map(|ms| start.checked_add(Duration::from_millis(ms)).unwrap());
                let resolution = resolve_tmax(request, &host, start)
                    .unwrap()
                    .with_outer_deadline(outer);
                let expected_ms = outer_ms.map_or(original_ms, |ms| ms.min(original_ms));
                assert_eq!(
                    resolution.deadline.deadline_at,
                    start
                        .checked_add(Duration::from_millis(expected_ms))
                        .unwrap()
                );
                assert_eq!(resolution.bidder_visible_tmax_ms, visible);
                assert_eq!(resolution.deadline.source, source);
            }
        }
    }

    #[test]
    fn host_outer_deadline_uses_original_start_and_expires_at_the_boundary() {
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let start = clock.now_monotonic();
        let mut host = auctioneer_config_data::host_default().clone();
        host.resource_limits.max_tmax_ms = 100;
        let cap = start.checked_add(Duration::from_millis(50)).unwrap();
        for elapsed in [49, 50, 51] {
            let now = start.checked_add(Duration::from_millis(elapsed)).unwrap();
            let resolution = resolve_tmax(Some(250), &host, start)
                .unwrap()
                .with_outer_deadline(Some(cap));
            assert_eq!(resolution.deadline.deadline_at, cap);
            assert_eq!(now < resolution.deadline.deadline_at, elapsed < 50);
            assert_eq!(resolution.bidder_visible_tmax_ms, 250);
            assert_eq!(resolution.deadline.source, TmaxSource::CappedByHost);
        }
    }
}
