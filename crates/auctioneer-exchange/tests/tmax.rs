//! `resolve_tmax` integration tests — four-value taxonomy.

#[cfg(test)]
mod tests {
    use auctioneer_config::types::{HostConfig, ResourceLimits};
    use auctioneer_exchange::{TmaxSource, resolve_tmax};
    use auctioneer_runtime::MonotonicInstant;
    use std::time::Duration;

    fn expect_resolved(
        resolved: Result<auctioneer_exchange::TmaxResolution, auctioneer_exchange::AuctionError>,
    ) -> auctioneer_exchange::TmaxResolution {
        resolved.expect("a bounded budget composes a representable deadline")
    }

    fn host_with_caps(default_ms: i64, max_ms: i64) -> HostConfig {
        HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: default_ms,
                max_tmax_ms: max_ms,
                max_bidder_response_bytes: 0x0004_0000,
                max_bidders_per_auction: 50,
                max_outbound_requests: 100,
                max_outbound_raw_bytes: 0x0004_0000,
                max_outbound_request_bytes: 0x0004_0000,
                max_transport_bytes: 0x0004_0000,
                max_decoded_bytes: 0x0004_0000,
                max_parsed_nodes: 0x0001_0000,
            },
            ..HostConfig::default()
        }
    }

    #[test]
    fn absent_request_tmax_uses_host_default() {
        let host = host_with_caps(750, 1500);
        let start = MonotonicInstant::now();
        let r = expect_resolved(resolve_tmax(None, &host, start));
        assert_eq!(r.bidder_visible_tmax_ms, 750);
        assert_eq!(r.deadline.source, TmaxSource::HostDefault);
        assert_eq!(
            r.deadline.deadline_at,
            start
                .checked_add(Duration::from_millis(750))
                .expect("no overflow")
        );
    }

    #[test]
    fn zero_request_tmax_uses_host_default() {
        let host = host_with_caps(750, 1500);
        let start = MonotonicInstant::now();
        let r = expect_resolved(resolve_tmax(Some(0), &host, start));
        assert_eq!(r.bidder_visible_tmax_ms, 750);
        assert_eq!(r.deadline.source, TmaxSource::HostDefault);
    }

    #[test]
    fn request_tmax_under_cap_passes_through() {
        let host = host_with_caps(750, 1500);
        let start = MonotonicInstant::now();
        let r = expect_resolved(resolve_tmax(Some(900), &host, start));
        assert_eq!(r.bidder_visible_tmax_ms, 900);
        assert_eq!(r.deadline.source, TmaxSource::Request);
    }

    #[test]
    fn request_tmax_over_cap_is_visible_to_bidder_but_deadline_clamped() {
        // Four-value taxonomy: adapter-visible TMax is NOT clamped,
        // server-side execution deadline IS clamped.
        let host = host_with_caps(750, 1500);
        let start = MonotonicInstant::now();
        let r = expect_resolved(resolve_tmax(Some(5000), &host, start));
        assert_eq!(r.bidder_visible_tmax_ms, 5000);
        assert_eq!(r.deadline.source, TmaxSource::CappedByHost);
        assert_eq!(
            r.deadline.deadline_at,
            start
                .checked_add(Duration::from_millis(1500))
                .expect("no overflow")
        );
    }

    /// The furthest instant the clock can represent. Doubling until
    /// the addition stops succeeding, then walking the remainder down,
    /// reaches it without assuming any particular backing width.
    fn saturated_instant() -> MonotonicInstant {
        let mut at = MonotonicInstant::now();
        let mut step = Duration::from_secs(1);
        loop {
            match at.checked_add(step) {
                Some(next) => {
                    at = next;
                    step = step.saturating_mul(2);
                }
                None if step == Duration::from_nanos(1) => return at,
                None => step = step.checked_div(2).unwrap_or(Duration::ZERO),
            }
        }
    }

    /// The overflow arm is an INTERNAL ERROR, never a saturation. A
    /// deadline that cannot be represented means the inputs are
    /// corrupt; substituting some far-future instant would hold every
    /// bidder call open for that substituted budget instead of failing
    /// the request.
    #[test]
    fn deadline_overflow_is_internal_error_not_saturation() {
        let host = host_with_caps(750, 1500);
        let outcome = resolve_tmax(None, &host, saturated_instant());
        let Err(err) = outcome else {
            panic!("an unrepresentable deadline must not resolve");
        };
        assert!(
            matches!(err, auctioneer_exchange::AuctionError::Internal(_)),
            "overflow is Internal (500), got {err:?}"
        );
    }
}
