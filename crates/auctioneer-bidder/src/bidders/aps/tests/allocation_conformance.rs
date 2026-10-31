//! Counting-allocator conformance for capped request construction.

use crate::alloc_track::{self, AllowAllEndpoints};
use serde_json::json;
use std::hint::black_box;

#[test]
fn aps_construction_does_not_copy_large_caller_trees_before_capped_serialization() {
    use auctioneer_bidder::{
        BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundRequestSink, SinkLimits,
        compiled_bidders,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::bounded_json::JsonFieldShape;

    let factory = compiled_bidders()
        .iter()
        .find(|entry| entry.key == "aps")
        .unwrap()
        .factory;
    for size in [64_usize, 8_192, 0x0004_0000] {
        for cap in [0_usize, 32, 1_024, 0x8000] {
            for debug in [false, true] {
                let request: BidRequest =
                    serde_json::from_value(json!({"id":"r", "payload":"x".repeat(size),
                    "test":i32::from(debug), "site":{"id":"s"}, "imp":[{"id":"i", "banner":{},
                        "ext":{"bidder":{"accountID":"account"}}}]}))
                    .unwrap();
                alloc_track::start();
                let clone = black_box(request.clone());
                let clone_measurement = alloc_track::finish();
                drop(black_box(clone));
                assert!(clone_measurement.peak() > 0);
                let mut info = BidderRequestInfo::new(
                    BidderName::new("aps"),
                    BidderName::new("aps"),
                    "account",
                    None,
                    "https://{{.Host}}/e/pb/bid",
                    &[],
                    Some(&["s2s.prebid.bid-na.ads.aps.amazon-adsystem.com"]),
                    None,
                    EntryPoint::Auction,
                    &[],
                );
                info.original_request_ext_shape = JsonFieldShape::Absent;
                let budget = CallBudget::new(1);
                let bidder = factory();
                let mut sink =
                    OutboundRequestSink::new(SinkLimits::new(cap, 1), &budget, &AllowAllEndpoints);
                alloc_track::start();
                let semantic = bidder.make_requests(&request, &info, &mut sink);
                let outcome = sink.into_outcome();
                let measured = alloc_track::finish();
                semantic.unwrap();
                // Independent per-call metadata/header-map constants plus
                // both body capacities; no measured value sets this bound.
                let funded = cap
                    .checked_mul(2)
                    .unwrap()
                    .checked_add(4_072 + 7_680)
                    .unwrap();
                assert!(measured.peak() > 0);
                assert!(
                    measured.peak() <= funded,
                    "size={size} cap={cap} debug={debug}: {measured:?}"
                );
                assert!(
                    measured.allocated() <= funded.checked_mul(2).unwrap(),
                    "caller-sized copies: {measured:?}"
                );
                if size == 64 && cap >= 1_024 {
                    assert_eq!(outcome.accepted.len(), 1);
                    assert!(outcome.errors.is_empty());
                }
                if size == 0x0004_0000 || cap == 0 {
                    assert!(outcome.accepted.is_empty());
                    assert_eq!(outcome.errors.len(), 1);
                    assert_eq!(budget.remaining(), 1);
                }
            }
        }
    }
}
