//! Native allocation conformance for the capped request and sealed response paths.

use crate::alloc_track::{
    self, AllowAllEndpoints, assert_bounded, construction_budget, size_growth_is_bounded,
};
use crate::response_allocation::{self, SHIPPED_RESPONSE_NODE_CAP};
use auctioneer_bidder::{
    Bidder, BidderHttpResponse, BidderName, BidderRequestInfo, CallBudget, EntryPoint,
    META_BYTES_CAPACITY, OutboundBuildError, OutboundRequestMetaBuilder, OutboundRequestSink,
    SinkLimits,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;
use std::{hint::black_box, thread};

fn factory() -> fn() -> Box<dyn Bidder> {
    auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|row| row.key == "openx")
        .unwrap()
        .factory
}

#[test]
fn capped_aggregate_construction_borrows_independent_root_and_parameter_trees() {
    let create = factory();
    thread::spawn(move || {
        let bidder = create();
        let info = BidderRequestInfo::new(
            BidderName::new("fake_alias"),
            BidderName::new("openx"),
            "fake_account",
            None,
            "https://bidder.example.test/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        for count in [1_usize, 8, 100] {
            for in_parameters in [false, true] {
                for cap in [32_usize, 0x4000] {
                    for calls in [0_u32, 1] {
                        let mut observations = Vec::new();
                        for width in [64_usize, 0x0004_0000] {
                            let mut document = json!({
                                "id":"fake_request", "imp":(0..count).map(|index| json!({
                                    "id":format!("fake_imp_{index}"), "banner":{},
                                    "ext":{"bidder":{"unit":"123", "platform":"fake_platform"}}
                                })).collect::<Vec<_>>()
                            });
                            if in_parameters {
                                document["imp"][0]["ext"]["bidder"]["customParams"] =
                                    json!({"fake_payload":"x".repeat(width)});
                            } else {
                                document["fake_payload"] = json!("x".repeat(width));
                            }
                            let request: BidRequest = serde_json::from_value(document).unwrap();
                            if width == 0x0004_0000 {
                                alloc_track::start();
                                drop(black_box(request.clone()));
                                let copied = alloc_track::finish();
                                assert!(copied.allocated() > construction_budget(cap));
                            }
                            let budget = CallBudget::new(calls);
                            let mut sink = OutboundRequestSink::new(
                                SinkLimits::new(cap, 100),
                                &budget,
                                &AllowAllEndpoints,
                            );
                            alloc_track::start();
                            let result = bidder.make_requests(&request, &info, &mut sink);
                            let outcome = sink.into_outcome();
                            let measured = alloc_track::finish();
                            result.unwrap();
                            assert_bounded(&measured, cap);
                            assert!(outcome.diagnostics.is_empty());
                            if calls == 0 {
                                assert_eq!(measured.allocated(), 0);
                                assert!(outcome.accepted.is_empty());
                                assert_eq!(outcome.errors, [
                                    OutboundBuildError::CallBudgetExhausted { max_calls: 0 }
                                ]);
                            } else if cap == 32 || width == 0x0004_0000 {
                                assert!(outcome.accepted.is_empty());
                                assert_eq!(outcome.errors, [OutboundBuildError::OversizeRaw {
                                    limit: u64::try_from(cap).unwrap()
                                }]);
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
                            "count={count} params={in_parameters} cap={cap} calls={calls}: {observations:?}");
                    }
                }
            }
        }
    }).join().unwrap();
}

#[test]
fn rewarded_video_rewrite_does_not_clone_unknown_video_fields_before_the_cap() {
    let create = factory();
    thread::spawn(move || {
        let bidder = create();
        let info = BidderRequestInfo::new(
            BidderName::new("fake_alias"),
            BidderName::new("openx"),
            "fake_account",
            None,
            "https://bidder.example.test/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        for rewarded in [0_i32, 1_i32] {
            for calls in [0_u32, 1] {
                for cap in [32_usize, 1024] {
                    let mut observations = Vec::new();
                    for width in [64_usize, 0x0004_0000] {
                        let request: BidRequest = serde_json::from_value(json!({
                            "id":"fake_request", "imp":[{
                                "id":"fake_imp", "rwdd":rewarded,
                                "video":{"mimes":["video/mp4"], "fake_payload":"x".repeat(width),
                                    "ext":{"fake_old":true}},
                                "ext":{"bidder":{"unit":"123", "platform":"fake_platform"}}
                            }]
                        }))
                        .unwrap();
                        if width == 0x0004_0000 {
                            alloc_track::start();
                            drop(black_box(request.imp[0].video.clone()));
                            let copied = alloc_track::finish();
                            assert!(copied.peak() > construction_budget(cap));
                        }
                        let budget = CallBudget::new(calls);
                        let mut sink = OutboundRequestSink::new(
                            SinkLimits::new(cap, 1),
                            &budget,
                            &AllowAllEndpoints,
                        );
                        alloc_track::start();
                        let result = bidder.make_requests(&request, &info, &mut sink);
                        let outcome = sink.into_outcome();
                        let measured = alloc_track::finish();
                        result.unwrap();
                        assert_bounded(&measured, cap);
                        assert!(outcome.diagnostics.is_empty());
                        if calls == 0 {
                            assert_eq!(measured.allocated(), 0);
                            assert!(outcome.accepted.is_empty());
                            assert_eq!(budget.remaining(), 0);
                            assert_eq!(
                                outcome.errors,
                                [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
                            );
                        } else if cap == 32 || width == 0x0004_0000 {
                            assert!(outcome.accepted.is_empty());
                            assert_eq!(budget.remaining(), 1);
                            assert_eq!(
                                outcome.errors,
                                [OutboundBuildError::OversizeRaw {
                                    limit: u64::try_from(cap).unwrap()
                                }]
                            );
                        } else {
                            assert!(outcome.errors.is_empty());
                            assert_eq!(budget.remaining(), 0);
                            assert_eq!(outcome.accepted.len(), 1);
                            let body: serde_json::Value =
                                serde_json::from_slice(outcome.accepted[0].body()).unwrap();
                            assert_eq!(body["imp"][0]["video"]["fake_payload"], "x".repeat(width));
                            assert_eq!(
                                body["imp"][0]["video"].get("ext"),
                                (rewarded == 1_i32)
                                    .then(|| json!({"rewarded":1_i32}))
                                    .as_ref()
                            );
                        }
                        observations.push(measured);
                    }
                    assert!(
                        size_growth_is_bounded(&observations[0], &observations[1]),
                        "rewarded={rewarded} calls={calls} cap={cap}: {observations:?}"
                    );
                }
            }
        }
    })
    .join()
    .unwrap();
}

#[test]
fn video_metadata_decode_fits_the_existing_response_charge() {
    response_allocation::aggregate_video_response(
        factory(),
        "openx",
        Some(&json!({"duration":30_i64, "primary_category":""})),
    );
}

#[test]
fn browser_configuration_decode_and_move_fit_the_existing_response_charge() {
    let create = factory();
    for count in [1_usize, 32, 128] {
        for width in [64_usize, 1024] {
            thread::spawn(move || {
                let bidder = create();
                let request: BidRequest = serde_json::from_value(json!({
                    "id":"fake_request", "imp":[{"id":"fake_imp", "banner":{}}]
                }))
                .unwrap();
                let bytes = serde_json::to_vec(&json!({
                    "id":"fake_response", "seatbid":[], "ext":{
                        "fledge_auction_configs":{"fake_imp":{
                            "payload":(0..count).map(|index| json!({
                                "index":index, "value":"x".repeat(width)
                            })).collect::<Vec<_>>()
                        }}
                    }
                }))
                .unwrap();
                let wire_bytes = u64::try_from(bytes.len()).unwrap();
                assert!(wire_bytes <= response_allocation::SHIPPED_DECODED_CAP);
                let response = BidderHttpResponse::new(200, vec![], bytes)
                    .within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
                let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
                let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
                meta.set_method("POST").unwrap();
                meta.set_url("https://bidder.example.test/auction").unwrap();
                meta.imp_id("fake_imp").unwrap();
                let sent = meta.finish();
                let baseline = alloc_track::response::reset_peak();
                let decoded = bidder.make_bids(&request, &sent, &response).unwrap();
                let peak = alloc_track::response::peak_since(baseline);
                assert!(peak > 0);
                assert!(
                    peak <= response_allocation::response_charge(wire_bytes),
                    "count={count} width={width} wire={wire_bytes} peak={peak}"
                );
                assert!(decoded.bids.is_empty());
                assert!(decoded.errors.is_empty());
                assert_eq!(decoded.auction_configs.len(), 1);
                assert_eq!(decoded.auction_configs[0].imp_id, "fake_imp");
                let payload = decoded.auction_configs[0].config["payload"]
                    .as_array()
                    .unwrap();
                assert_eq!(payload.len(), count);
                for (index, row) in payload.iter().enumerate() {
                    assert_eq!(row["index"], index);
                    assert_eq!(row["value"], "x".repeat(width));
                }
            })
            .join()
            .unwrap();
        }
    }
}
