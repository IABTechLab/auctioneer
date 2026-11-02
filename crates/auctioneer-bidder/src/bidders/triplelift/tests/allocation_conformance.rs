//! Counting-allocator conformance for filtered impression construction.

use crate::alloc_track::{
    self, AllowAllEndpoints, assert_bounded, construction_budget, size_growth_is_bounded,
};
use auctioneer_bidder::{
    BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundBuildError, OutboundRequestSink,
    SinkLimits,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;
use std::{hint::black_box, thread};

#[derive(Clone, Copy, Debug)]
enum FilteredPayload {
    Root,
    Surviving,
    Rejected,
}

fn filtered_request(count: usize, width: usize, placement: FilteredPayload) -> BidRequest {
    let mut impressions = vec![json!({"id":"invalid", "banner":{}, "ext":{"bidder":false}})];
    impressions.extend((0..count).map(|index| {
        json!({
            "id":format!("sent-{index}"), "banner":{},
            "ext":{"bidder":{"inventoryCode":"slot", "floor":1.25_f64}},
        })
    }));
    let mut document = json!({"id":"allocation", "imp":impressions});
    match placement {
        FilteredPayload::Root => document["payload"] = json!("x".repeat(width)),
        FilteredPayload::Surviving => {
            for imp in document["imp"].as_array_mut().unwrap().iter_mut().skip(1) {
                imp["payload"] = json!("x".repeat(width));
            }
        }
        FilteredPayload::Rejected => document["imp"][0]["payload"] = json!("x".repeat(width)),
    }
    serde_json::from_value(document).unwrap()
}

fn filtered_clone_probe(
    request: &BidRequest,
    placement: FilteredPayload,
) -> alloc_track::Measurement {
    alloc_track::start();
    match placement {
        FilteredPayload::Root => drop(black_box(request.clone())),
        FilteredPayload::Surviving => drop(black_box(request.imp[1].clone())),
        FilteredPayload::Rejected => drop(black_box(request.imp[0].clone())),
    }
    alloc_track::finish()
}

fn filtered_probe(
    bidder: &dyn auctioneer_bidder::Bidder,
    info: &BidderRequestInfo<'_>,
    request: &BidRequest,
    cap: usize,
    calls: u32,
    survivors: usize,
    oversized: bool,
) -> alloc_track::Measurement {
    let budget = CallBudget::new(calls);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(cap, 10), &budget, &AllowAllEndpoints);
    alloc_track::start();
    let semantic = bidder.make_requests(request, info, &mut sink);
    let outcome = sink.into_outcome();
    let measured = alloc_track::finish();
    semantic.unwrap();
    assert_bounded(&measured, cap);
    assert_eq!(outcome.diagnostics.len(), 1);
    if calls == 0 {
        assert_eq!(
            measured.allocated(),
            0,
            "no slot must mean no caller copies"
        );
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        );
    } else if cap == 32 || oversized {
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::OversizeRaw {
                limit: u64::try_from(cap).unwrap()
            }]
        );
        assert_eq!(budget.remaining(), 1);
    } else {
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(outcome.accepted[0].covered_imp_ids().count(), survivors);
        assert_eq!(budget.remaining(), 0);
    }
    measured
}

#[test]
fn filtered_impression_construction_does_not_copy_caller_trees_before_reservation() {
    use auctioneer_bidder::compiled_bidders;

    thread::spawn(|| {
        let bidder = (compiled_bidders()
            .iter()
            .find(|row| row.key == "triplelift")
            .unwrap()
            .factory)();
        let info = BidderRequestInfo::new(
            BidderName::new("triplelift-alias"),
            BidderName::new("triplelift"),
            "account",
            None,
            "https://bidder.example.test/auction?sra=1&supplier_id=20",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        for placement in [
            FilteredPayload::Root,
            FilteredPayload::Surviving,
            FilteredPayload::Rejected,
        ] {
            for count in [1_usize, 8] {
                let small = filtered_request(count, 128, placement);
                let large = filtered_request(count, 0x0004_0000, placement);
                let small_clone = filtered_clone_probe(&small, placement);
                let large_clone = filtered_clone_probe(&large, placement);
                assert!(large_clone.peak() > construction_budget(32));
                assert!(large_clone.allocated() > construction_budget(32));
                assert!(!size_growth_is_bounded(&small_clone, &large_clone));
                for cap in [32_usize, 0x4000] {
                    for calls in [0_u32, 1] {
                        let small_work =
                            filtered_probe(&*bidder, &info, &small, cap, calls, count, false);
                        let large_work = filtered_probe(
                            &*bidder,
                            &info,
                            &large,
                            cap,
                            calls,
                            count,
                            !matches!(placement, FilteredPayload::Rejected),
                        );
                        assert!(
                            size_growth_is_bounded(&small_work, &large_work),
                            "placement={placement:?} count={count} cap={cap} calls={calls}: \
                             small={small_work:?}, large={large_work:?}"
                        );
                    }
                }
            }
        }
    })
    .join()
    .unwrap();
}
