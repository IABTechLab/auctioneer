//! Counting-allocator conformance for per-impression request construction.

use crate::alloc_track::{self, AllowAllEndpoints, CONSTRUCTION_CONTROL_BYTES};
use auctioneer_bidder::{
    BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundBuildError, OutboundRequestSink,
    SinkLimits, outbound_meta_bytes,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;
use std::{hint::black_box, thread};

fn assert_input_growth_is_bounded(observations: &[(alloc_track::Measurement, usize)]) {
    assert_eq!(observations.len(), 2);
    let (small, small_attempts) = &observations[0];
    let (large, large_attempts) = &observations[1];
    assert!(
        large.peak() <= small.peak().saturating_add(CONSTRUCTION_CONTROL_BYTES)
            && large.largest() <= small.largest().saturating_add(CONSTRUCTION_CONTROL_BYTES),
        "{observations:?}"
    );
    // A refused body refunds its slot, allowing more allocating attempts
    // than a successful body. Compare cumulative growth per observed attempt.
    assert_eq!(*small_attempts == 0, *large_attempts == 0);
    if *small_attempts != 0 {
        let actual = large.allocated().checked_mul(*small_attempts).unwrap();
        let allowed = small
            .allocated()
            .checked_add(
                CONSTRUCTION_CONTROL_BYTES
                    .checked_mul(*small_attempts)
                    .unwrap(),
            )
            .and_then(|bytes| bytes.checked_mul(*large_attempts))
            .unwrap();
        assert!(
            actual <= allowed,
            "per-attempt allocation grew: {observations:?}"
        );
    }
}

#[test]
fn single_impression_calls_keep_construction_bounded_across_input_and_call_counts() {
    use auctioneer_bidder::compiled_bidders;

    thread::spawn(|| {
        let bidder = (compiled_bidders().iter().find(|row| row.key == "yahoo_ads").unwrap().factory)();
        let info = BidderRequestInfo::new(
            BidderName::new("yahooAds"), BidderName::new("yahoo_ads"), "account", None,
            "https://bidder.example.test/auction", &[], None, None, EntryPoint::Auction, &[],
        );
        for count in [1_usize, 3] {
            for max_calls in [0_u32, 1, 4] {
                for cap in [32_usize, 4096] {
                    for in_impression in [false, true] {
                        let per_call = cap
                            .checked_add(usize::try_from(outbound_meta_bytes(u16::try_from(count).unwrap())).unwrap())
                            .and_then(|bytes| bytes.checked_add(CONSTRUCTION_CONTROL_BYTES)).unwrap();
                        let peak_bound = per_call.checked_mul(count.min(usize::try_from(max_calls).unwrap())).unwrap();
                        let cumulative_bound = per_call.checked_mul(count).unwrap();
                        let mut observations = Vec::new();
                        for width in [64_usize, 0x0004_0000] {
                            let mut document = json!({"id":"allocation", "imp":
                                (0..count).map(|index| json!({
                                    "id":format!("i-{index}"), "banner":{"w":300_i32,"h":250_i32},
                                    "ext":{"bidder":{"dcn":"publisher","pos":"slot"}}
                                })).collect::<Vec<_>>()
                            });
                            if in_impression {
                                for imp in document["imp"].as_array_mut().unwrap() {
                                    imp["payload"] = json!("x".repeat(width));
                                }
                            } else {
                                document["payload"] = json!("x".repeat(width));
                            }
                            let request: BidRequest = serde_json::from_value(document).unwrap();
                            if width == 0x0004_0000 {
                                alloc_track::start();
                                drop(black_box(request.clone()));
                                let cloned = alloc_track::finish();
                                assert!(cloned.allocated() > cumulative_bound);
                            }
                            let budget = CallBudget::new(max_calls);
                            let mut sink = OutboundRequestSink::new(
                                SinkLimits::new(cap, u16::try_from(count).unwrap()),
                                &budget, &AllowAllEndpoints,
                            );
                            alloc_track::start();
                            let semantic = bidder.make_requests(&request, &info, &mut sink);
                            let outcome = sink.into_outcome();
                            let measured = alloc_track::finish();
                            semantic.unwrap();
                            assert!(measured.peak() <= peak_bound && measured.allocated() <= cumulative_bound,
                                "count={count} calls={max_calls} cap={cap} width={width} imp={in_impression}: {measured:?}");
                            if max_calls == 0 {
                                assert_eq!(measured.allocated(), 0);
                                assert!(outcome.accepted.is_empty());
                                assert_eq!(outcome.rejected_call_count, u32::try_from(count).unwrap());
                                assert_eq!(outcome.errors, [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]);
                            } else if cap == 32 || width == 0x0004_0000 {
                                assert!(outcome.accepted.is_empty());
                                assert_eq!(outcome.errors.len(), count);
                                assert!(outcome.errors.iter().all(|error| *error == OutboundBuildError::OversizeRaw { limit: u64::try_from(cap).unwrap() }));
                                assert_eq!(budget.remaining(), max_calls);
                            } else {
                                let accepted = count.min(usize::try_from(max_calls).unwrap());
                                assert_eq!(outcome.accepted.len(), accepted);
                                for (index, call) in outcome.accepted.iter().enumerate() {
                                    assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), [format!("i-{index}")]);
                                }
                                assert_eq!(budget.remaining(), max_calls - u32::try_from(accepted).unwrap());
                            }
                            let allocating_attempts = outcome.accepted.len()
                                + outcome.errors.iter().filter(|error| !matches!(error,
                                    OutboundBuildError::CallBudgetExhausted { .. }
                                )).count();
                            observations.push((measured, allocating_attempts));
                        }
                        assert_input_growth_is_bounded(&observations);
                    }
                }
            }
        }
    }).join().unwrap();
}
