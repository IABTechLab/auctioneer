//! Counting-allocator conformance for aggregate request construction.

use crate::alloc_track::{
    self, AllowAllEndpoints, assert_bounded, construction_budget, size_growth_is_bounded,
};
use crate::response_allocation::aggregate_video_response;
use auctioneer_bidder::{
    BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundBuildError, OutboundRequestSink,
    SinkLimits,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;
use std::{hint::black_box, thread};

#[test]
fn aggregate_protocol_decoding_and_video_metadata_fit_the_existing_response_charge() {
    let registration = auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|entry| entry.key == "sovrn")
        .unwrap();
    aggregate_video_response(registration.factory, "sovrn", None);
}

#[test]
fn aggregate_protocol_construction_does_not_allocate_in_proportion_to_caller_trees() {
    use auctioneer_bidder::compiled_bidders;

    thread::spawn(|| {
        let key = "sovrn";
            let bidder = (compiled_bidders().iter().find(|row| row.key == key).unwrap().factory)();
            let info = BidderRequestInfo::new(
                BidderName::new(key), BidderName::new(key), "account", None,
                "https://bidder.example.test/auction?existing=1", &[], None, None,
                EntryPoint::Auction, &[],
            );
            for count in [1_usize, 8] {
                for in_impression in [false, true] {
                    for cap in [32_usize, 0x4000] {
                        for calls in [0_u32, 1] {
                            let mut observations = Vec::new();
                            for width in [64_usize, 0x0004_0000] {
                                let mut document = json!({"id":"allocation", "imp":
                                    (0..count).map(|index| json!({
                                        "id":format!("i-{index}"), "banner":{},
                                        "ext":{"bidder":{"org":"publisher","tagid":"tag","siteid":123_i64}},
                                    })).collect::<Vec<_>>()
                                });
                                if in_impression {
                                    document["imp"][0]["payload"] = json!("x".repeat(width));
                                } else {
                                    document["payload"] = json!("x".repeat(width));
                                }
                                let request: BidRequest = serde_json::from_value(document).unwrap();
                                if width == 0x0004_0000 {
                                    alloc_track::start();
                                    drop(black_box(request.clone()));
                                    let clone = alloc_track::finish();
                                    assert!(clone.allocated() > construction_budget(cap));
                                }
                                let budget = CallBudget::new(calls);
                                let mut sink = OutboundRequestSink::new(
                                    SinkLimits::new(cap, 8), &budget, &AllowAllEndpoints,
                                );
                                alloc_track::start();
                                let semantic = bidder.make_requests(&request, &info, &mut sink);
                                let outcome = sink.into_outcome();
                                let measured = alloc_track::finish();
                                semantic.unwrap();
                                assert_bounded(&measured, cap);
                                assert!(outcome.diagnostics.is_empty());
                                if calls == 0 {
                                    assert_eq!(measured.allocated(), 0, "{key}: exhausted slot");
                                    assert_eq!(outcome.errors, [
                                        OutboundBuildError::CallBudgetExhausted { max_calls: 0 },
                                    ]);
                                    assert!(outcome.accepted.is_empty());
                                } else if cap == 32 || width == 0x0004_0000 {
                                    assert!(outcome.accepted.is_empty());
                                    assert_eq!(outcome.errors, [
                                        OutboundBuildError::OversizeRaw { limit: u64::try_from(cap).unwrap() },
                                    ]);
                                    assert_eq!(budget.remaining(), 1);
                                } else {
                                    assert!(outcome.errors.is_empty());
                                    assert_eq!(outcome.accepted.len(), 1);
                                    assert_eq!(outcome.accepted[0].covered_imp_ids().count(), count);
                                    assert_eq!(budget.remaining(), 0);
                                }
                                observations.push(measured);
                            }
                            assert!(size_growth_is_bounded(&observations[0], &observations[1]),
                                "{key} count={count} cap={cap} calls={calls} imp={in_impression}: {observations:?}");
                        }
                    }
                }
            }
    }).join().unwrap();
}

#[test]
fn assembled_cookie_headers_share_funded_metadata_without_an_intermediate_string() {
    use auctioneer_bidder::compiled_bidders;

    thread::spawn(|| {
        let bidder = (compiled_bidders()
            .iter()
            .find(|row| row.key == "sovrn")
            .unwrap()
            .factory)();
        let info = BidderRequestInfo::new(
            BidderName::new("sovrn"),
            BidderName::new("sovrn"),
            "account",
            None,
            "https://bidder.example.test/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        for cap in [32_usize, 0x4000] {
            let mut measurements = Vec::new();
            for width in [64_usize, 3000, 3300, 100_000] {
                let uid = "x".repeat(width);
                let request: BidRequest = serde_json::from_value(json!({
                    "id":"r", "imp":[{"id":"i","banner":{},"ext":{"bidder":{"tagid":"slot"}}}],
                    "user":{"buyeruid":uid},
                }))
                .unwrap();
                let budget = CallBudget::new(1);
                let mut sink =
                    OutboundRequestSink::new(SinkLimits::new(cap, 1), &budget, &AllowAllEndpoints);
                alloc_track::start();
                let semantic = bidder.make_requests(&request, &info, &mut sink);
                let outcome = sink.into_outcome();
                let measurement = alloc_track::finish();
                semantic.unwrap();
                assert_bounded(&measurement, cap);
                assert!(outcome.diagnostics.is_empty());
                if width >= 3300 {
                    assert!(outcome.accepted.is_empty());
                    assert_eq!(
                        outcome.errors,
                        [OutboundBuildError::MetaOverCap { limit: 3328 },]
                    );
                    assert_eq!(budget.remaining(), 1);
                } else if cap == 32 {
                    assert!(outcome.accepted.is_empty());
                    assert_eq!(
                        outcome.errors,
                        [OutboundBuildError::OversizeRaw { limit: 32 },]
                    );
                    assert_eq!(budget.remaining(), 1);
                } else {
                    assert!(outcome.errors.is_empty());
                    assert_eq!(outcome.accepted.len(), 1);
                    assert_eq!(budget.remaining(), 0);
                    let cookie = format!("ljt_reader={uid}");
                    assert_eq!(
                        outcome.accepted[0]
                            .headers()
                            .find(|(name, _)| *name == "Cookie"),
                        Some(("Cookie", cookie.as_str())),
                    );
                }
                measurements.push(measurement);
            }
            for measurement in measurements.iter().skip(1) {
                assert!(size_growth_is_bounded(&measurements[0], measurement));
            }
        }
    })
    .join()
    .unwrap();
}
