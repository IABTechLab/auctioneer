//! `max_bidders_per_auction` + `max_outbound_requests` enforcement.

use crate::split::PerBidderRequest;
use auctioneer_bidder::{BidderHttpRequest, BidderName};

#[derive(Debug)]
pub struct BidderCapDrop {
    pub bidder: BidderName,
    /// `N` from the warning-message format string.
    pub cap: usize,
}

#[derive(Debug)]
pub struct OutboundCapDrop {
    pub bidder: BidderName,
    /// Count of requests dropped under the bidder's call list (0 means
    /// the bidder is entirely dropped; nonzero is the truncation count
    /// to surface in the warning text).
    pub dropped_count: usize,
}

/// Truncate `per_bidder` to `cap` entries in registry order.
///
/// The slice is already sorted by `split_by_bidder` (`BTreeMap` iterates
/// in `BidderName` `Ord` order). A `cap` of 0 means "unlimited" — no
/// truncation runs in that case.
#[must_use]
pub fn apply_max_bidders(
    per_bidder: Vec<PerBidderRequest>,
    cap: usize,
) -> (Vec<PerBidderRequest>, Vec<BidderCapDrop>) {
    cap_entries(per_bidder, cap, |entry| entry.bidder)
}

/// The live path caps names before any bidder request is materialized.
pub(crate) fn apply_max_bidder_names(
    names: Vec<BidderName>,
    cap: usize,
) -> (Vec<BidderName>, Vec<BidderCapDrop>) {
    cap_entries(names, cap, |name| name)
}

fn cap_entries<T>(
    mut per_bidder: Vec<T>,
    cap: usize,
    bidder: impl Fn(T) -> BidderName,
) -> (Vec<T>, Vec<BidderCapDrop>) {
    if cap == 0 || per_bidder.len() <= cap {
        return (per_bidder, vec![]);
    }
    let dropped = per_bidder.split_off(cap);
    let drops = dropped
        .into_iter()
        .map(|entry| BidderCapDrop {
            bidder: bidder(entry),
            cap,
        })
        .collect();
    (per_bidder, drops)
}

/// Keep the first `cap` total outbound calls across all bidders in
/// per-bidder registry order. A `cap` of 0 means "unlimited".
#[must_use]
pub fn apply_max_outbound(
    pairs: Vec<(BidderName, Vec<BidderHttpRequest>)>,
    cap: usize,
) -> (
    Vec<(BidderName, Vec<BidderHttpRequest>)>,
    Vec<OutboundCapDrop>,
) {
    if cap == 0 {
        return (pairs, vec![]);
    }
    let mut kept: Vec<(BidderName, Vec<BidderHttpRequest>)> = Vec::new();
    let mut drops: Vec<OutboundCapDrop> = Vec::new();
    let mut remaining = cap;

    for (bidder, calls) in pairs {
        if remaining == 0 {
            drops.push(OutboundCapDrop {
                bidder: bidder.clone(),
                dropped_count: calls.len(),
            });
            kept.push((bidder, Vec::new()));
            continue;
        }
        let take = calls.len().min(remaining);
        if take < calls.len() {
            drops.push(OutboundCapDrop {
                bidder: bidder.clone(),
                dropped_count: calls.len().saturating_sub(take),
            });
        }
        let mut taken = calls;
        taken.truncate(take);
        remaining = remaining.saturating_sub(take);
        kept.push((bidder, taken));
    }

    (kept, drops)
}

#[cfg(test)]
mod tests {
    use super::{
        BidderHttpRequest, BidderName, PerBidderRequest, apply_max_bidder_names, apply_max_bidders,
        apply_max_outbound,
    };
    use crate::ExchangeEndpointValidator;
    use auctioneer_bidder::{
        BidderRequestInfo, CallBudget, EntryPoint, OutboundRequestSink, SinkLimits,
    };
    use auctioneer_config_data::types::BidderInfo;
    use auctioneer_openrtb::types::bid_request::BidRequest;

    fn pbr(name: &str) -> PerBidderRequest {
        PerBidderRequest {
            bidder: BidderName::new(name),
            base: BidderName::new(name),
            request: BidRequest::default(),
        }
    }

    fn call(path: &str) -> BidderHttpRequest {
        let url = format!("https://bidder.example/{path}");
        let info = BidderRequestInfo::new(
            BidderName::new("fake_fixture"),
            BidderName::new("fake_fixture"),
            "account",
            None,
            &url,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let bidder = BidderInfo {
            name: "fake_fixture".into(),
            endpoint: Some(url.clone()),
            uses_https: true,
            ..BidderInfo::default()
        };
        let validator = ExchangeEndpointValidator::new(&bidder);
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(0, 0), &budget, &validator);
        sink.push_request(
            &info,
            |meta| {
                meta.set_method("POST")?;
                meta.set_url(&url)
            },
            |_writer| Ok(()),
        )
        .expect("sink-built fixture");
        sink.into_outcome()
            .accepted
            .pop()
            .expect("accepted fixture")
    }

    #[test]
    fn apply_max_bidders_under_cap_is_noop() {
        let input = vec![pbr("fake_a"), pbr("fake_b")];
        let (kept, drops) = apply_max_bidders(input, 5);
        assert_eq!(kept.len(), 2);
        assert!(drops.is_empty());
    }

    #[test]
    fn apply_max_bidders_truncates_to_cap() {
        let input = vec![pbr("fake_a"), pbr("fake_b"), pbr("fake_c"), pbr("fake_d")];
        let (kept, drops) = apply_max_bidders(input, 2);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].bidder.as_str(), "fake_a");
        assert_eq!(kept[1].bidder.as_str(), "fake_b");
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0].bidder.as_str(), "fake_c");
        assert_eq!(drops[1].bidder.as_str(), "fake_d");
    }

    #[test]
    fn live_bidder_names_under_cap_are_unchanged() {
        let input = vec![BidderName::new("fake_a"), BidderName::new("fake_b")];
        let (kept, drops) = apply_max_bidder_names(input, 5);
        assert_eq!(kept, [BidderName::new("fake_a"), BidderName::new("fake_b")]);
        assert!(drops.is_empty());
    }

    #[test]
    fn live_bidder_names_are_capped_before_materialization() {
        let input = ["fake_a", "fake_b", "fake_c", "fake_d"]
            .into_iter()
            .map(BidderName::new)
            .collect();
        let (kept, drops) = apply_max_bidder_names(input, 2);
        assert_eq!(kept, [BidderName::new("fake_a"), BidderName::new("fake_b")]);
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0].bidder.as_str(), "fake_c");
        assert_eq!(drops[1].bidder.as_str(), "fake_d");
        assert!(drops.iter().all(|drop| drop.cap == 2));
    }

    #[test]
    fn apply_max_outbound_truncates_partial_bidder_calls() {
        let input = vec![
            (
                BidderName::new("fake_a"),
                vec![call("u1"), call("u2"), call("u3")],
            ),
            (BidderName::new("fake_b"), vec![call("u4"), call("u5")]),
        ];
        let (kept, drops) = apply_max_outbound(input, 4);
        // 'a' keeps 3, 'b' keeps 1, drop 1 from 'b'.
        assert_eq!(kept[0].1.len(), 3);
        assert_eq!(kept[1].1.len(), 1);
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].bidder.as_str(), "fake_b");
        assert_eq!(drops[0].dropped_count, 1);
    }

    #[test]
    fn apply_max_outbound_drops_entire_later_bidder_when_cap_exhausted() {
        let input = vec![
            (BidderName::new("fake_a"), vec![call("u1"), call("u2")]),
            (BidderName::new("fake_b"), vec![call("u3"), call("u4")]),
        ];
        let (kept, drops) = apply_max_outbound(input, 2);
        assert_eq!(kept[0].1.len(), 2);
        assert_eq!(kept[1].1.len(), 0);
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].bidder.as_str(), "fake_b");
        assert_eq!(drops[0].dropped_count, 2);
    }
}
