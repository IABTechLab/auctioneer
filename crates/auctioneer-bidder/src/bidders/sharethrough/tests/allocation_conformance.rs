//! Native controls for split calls, borrowed prefixes, floor views and sealed responses.

use crate::alloc_track::{
    self, AllowAllEndpoints, CONSTRUCTION_CONTROL_BYTES, construction_budget,
    response as response_alloc_track,
};
use crate::response_allocation::{self, SHIPPED_RESPONSE_NODE_CAP};
use auctioneer_bidder::{
    Bidder, BidderHttpResponse, BidderName, BidderRequestInfo, CallBudget, EntryPoint,
    FloorConversion, OutboundBuildError, OutboundRequestSink, SinkLimits,
};
use auctioneer_config_data::{currency::FloorCurrencyConfig, types::MediaType};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::{Value, json};
use std::{collections::BTreeMap, hint::black_box, thread};

const ENDPOINT: &str = "https://bidder.example.test/sharethrough";

fn factory() -> fn() -> Box<dyn Bidder> {
    auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|row| row.key == "sharethrough")
        .unwrap()
        .factory
}

fn info() -> BidderRequestInfo<'static> {
    BidderRequestInfo::new(
        BidderName::new("sharethrough"),
        BidderName::new("sharethrough"),
        "account",
        None,
        ENDPOINT,
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    )
}

fn impression(id: &str) -> Value {
    json!({"id":id, "banner":{"w":300_i32,"h":250_i32},
        "video":{"mimes":["video/mp4"]}, "native":{"request":"native payload"},
        "audio":{"mimes":["audio/mp4"]}, "ext":{"bidder":{"pkey":"placement"}}})
}

#[derive(Clone, Copy, Debug)]
struct Case {
    count: usize,
    slots: u32,
    cap: usize,
    location: &'static str,
}

fn request(case: Case, width: usize) -> BidRequest {
    let mut document = json!({"id":"r", "imp":(0..case.count)
        .map(|index| impression(&format!("i-{index}"))).collect::<Vec<_>>()});
    let payload = json!("x".repeat(width));
    match case.location {
        "root" => document["payload"] = payload,
        "source" => document["source"] = json!({"ext":{"payload":payload}}),
        "prefix" => document["imp"].as_array_mut().unwrap().insert(
            0,
            json!({"id":"unsupported", "audio":{"mimes":["audio/mp4"]},
                "ext":{"bidder":{"pkey":"audio", "bcat":[payload], "badv":["prior.example"]}}}),
        ),
        location => {
            for imp in document["imp"].as_array_mut().unwrap() {
                match location {
                    "impression" => imp["payload"] = payload.clone(),
                    "placement" => imp["ext"]["bidder"]["pkey"] = payload.clone(),
                    "blocklists" => {
                        imp["ext"]["bidder"]["bcat"] = json!([payload]);
                        imp["ext"]["bidder"]["badv"] = json!(["advertiser.example"]);
                    }
                    _ => panic!("unknown allocation fixture location"),
                }
            }
        }
    }
    serde_json::from_value(document).unwrap()
}

fn measured_construction(case: Case, width: usize) -> (alloc_track::Measurement, usize) {
    let request = request(case, width);
    let total = case.count.checked_mul(3).unwrap();
    let per_call = construction_budget(case.cap);
    if width == 0x0004_0000 {
        alloc_track::start();
        drop(black_box(request.clone()));
        let copied = alloc_track::finish();
        assert!(copied.allocated() > per_call.checked_mul(total).unwrap());
    }
    let bidder = factory()();
    let info = info();
    let budget = CallBudget::new(case.slots);
    let mut sink =
        OutboundRequestSink::new(SinkLimits::new(case.cap, 1), &budget, &AllowAllEndpoints);
    alloc_track::start();
    let result = bidder.make_requests(&request, &info, &mut sink);
    let outcome = sink.into_outcome();
    let measured = alloc_track::finish();
    result.unwrap();
    let live_calls = total.min(usize::try_from(case.slots).unwrap());
    assert!(
        measured.peak() <= per_call.checked_mul(live_calls).unwrap(),
        "{case:?}: {measured:?}"
    );
    assert!(
        measured.allocated() <= per_call.checked_mul(total).unwrap(),
        "{case:?}: {measured:?}"
    );
    assert!(measured.largest() <= per_call, "{case:?}: {measured:?}");
    assert_eq!(
        outcome.diagnostics.len(),
        usize::from(case.location == "prefix")
    );
    if case.slots == 0 {
        assert_eq!(measured.allocated(), 0);
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.rejected_call_count, u32::try_from(total).unwrap());
    } else if case.cap == 32 || width == 0x0004_0000 {
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.errors.len(), total);
        assert!(outcome.errors.iter().all(|error| *error
            == OutboundBuildError::OversizeRaw {
                limit: u64::try_from(case.cap).unwrap()
            }));
        assert_eq!(budget.remaining(), case.slots);
    } else {
        assert_eq!(outcome.accepted.len(), live_calls);
        assert_eq!(
            budget.remaining(),
            case.slots
                .checked_sub(u32::try_from(live_calls).unwrap())
                .unwrap()
        );
        for (index, call) in outcome.accepted.iter().enumerate() {
            assert_eq!(
                call.covered_imp_ids().collect::<Vec<_>>(),
                [format!("i-{}", index.checked_div(3).unwrap())]
            );
            let body: Value = serde_json::from_slice(call.body()).unwrap();
            for (ordinal, name) in ["banner", "video", "native"].into_iter().enumerate() {
                assert_eq!(
                    body["imp"][0].get(name).is_some(),
                    index.checked_rem(3).unwrap() == ordinal
                );
            }
            assert!(body["imp"][0].get("audio").is_none());
            if case.location == "prefix" {
                assert_eq!(body["bcat"], json!(["x".repeat(width)]));
            }
        }
    }
    let attempts = outcome
        .accepted
        .len()
        .checked_add(
            outcome
                .errors
                .iter()
                .filter(|error| !matches!(error, OutboundBuildError::CallBudgetExhausted { .. }))
                .count(),
        )
        .unwrap();
    (measured, attempts)
}

fn assert_growth(observations: &[(alloc_track::Measurement, usize)]) {
    assert_eq!(observations.len(), 2);
    let (small, small_attempts) = &observations[0];
    let (large, large_attempts) = &observations[1];
    assert!(
        large.peak() <= small.peak().saturating_add(CONSTRUCTION_CONTROL_BYTES)
            && large.largest() <= small.largest().saturating_add(CONSTRUCTION_CONTROL_BYTES),
        "{observations:?}"
    );
    assert_eq!(*small_attempts == 0, *large_attempts == 0);
    if *small_attempts != 0 {
        // Body refusals refund slots: compare cumulative allocation per actual attempt.
        let actual = large.allocated().checked_mul(*small_attempts).unwrap();
        let allowed = small
            .allocated()
            .checked_add(
                CONSTRUCTION_CONTROL_BYTES
                    .checked_mul(*small_attempts)
                    .unwrap(),
            )
            .unwrap()
            .checked_mul(*large_attempts)
            .unwrap();
        assert!(actual <= allowed, "{observations:?}");
    }
}

#[test]
fn split_serializers_borrow_root_impression_source_parameters_and_preceding_blocklists() {
    thread::spawn(|| {
        for count in [1_usize, 3] {
            for slots in [0_u32, 1, 4] {
                for cap in [32_usize, 4096] {
                    for location in [
                        "root",
                        "impression",
                        "source",
                        "placement",
                        "blocklists",
                        "prefix",
                    ] {
                        let case = Case {
                            count,
                            slots,
                            cap,
                            location,
                        };
                        let observations =
                            [64_usize, 0x0004_0000].map(|width| measured_construction(case, width));
                        assert_growth(&observations);
                    }
                }
            }
        }
    })
    .join()
    .unwrap();
}

fn snapshot(count: usize) -> FloorCurrencyConfig {
    FloorCurrencyConfig {
        expires_at: 2000,
        usd_per_unit: (0..count)
            .map(|index| {
                let row = u8::try_from(index.checked_div(26).unwrap()).unwrap();
                let column = u8::try_from(index.checked_rem(26).unwrap()).unwrap();
                let code = String::from_utf8(vec![
                    b'Q',
                    b'A'.checked_add(row).unwrap(),
                    b'A'.checked_add(column).unwrap(),
                ])
                .unwrap();
                (code, 2.5_f64)
            })
            .collect::<BTreeMap<_, _>>(),
    }
}

#[test]
fn conversion_borrows_operator_maps_independently_of_snapshot_width() {
    thread::spawn(|| {
        for slots in [0_u32, 1] {
            let mut observations = Vec::new();
            for count in [1_usize, 128] {
                let snapshot = snapshot(count);
                let mut info = info();
                info.floor_currency = FloorConversion::new(Some(&snapshot), 1000);
                let request: BidRequest = serde_json::from_value(json!({"id":"r", "imp":[{
                    "id":"i", "banner":{}, "bidfloor":4.0_f64,"bidfloorcur":"qAa",
                    "ext":{"bidder":{"pkey":"placement"}}
                }]}))
                .unwrap();
                if count == 128 {
                    alloc_track::start();
                    drop(black_box(snapshot.usd_per_unit.clone()));
                    let copied = alloc_track::finish();
                    assert!(copied.allocated() > CONSTRUCTION_CONTROL_BYTES);
                }
                let bidder = factory()();
                let budget = CallBudget::new(slots);
                let mut sink =
                    OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &AllowAllEndpoints);
                alloc_track::start();
                let semantic = bidder.make_requests(&request, &info, &mut sink);
                let outcome = sink.into_outcome();
                let measured = alloc_track::finish();
                semantic.unwrap();
                alloc_track::assert_bounded(&measured, 4096);
                assert_eq!(outcome.accepted.len(), usize::try_from(slots).unwrap());
                if slots == 0 {
                    assert_eq!(measured.allocated(), 0);
                }
                for call in &outcome.accepted {
                    let body: Value = serde_json::from_slice(call.body()).unwrap();
                    assert_eq!(body["imp"][0]["bidfloor"], json!(10.0_f64));
                    assert_eq!(body["imp"][0]["bidfloorcur"], "USD");
                }
                observations.push(measured);
            }
            assert!(alloc_track::size_growth_is_bounded(
                &observations[0],
                &observations[1]
            ));
        }
    })
    .join()
    .unwrap();
}

#[test]
fn fatal_conversion_refunds_three_real_calls_without_copying_late_parameters() {
    thread::spawn(|| {
        let mut observations = Vec::new();
        for width in [64_usize, 0x0004_0000] {
            let request: BidRequest = serde_json::from_value(json!({"id":"r", "imp":[
                impression("good"), {"id":"foreign", "banner":{}, "bidfloor":4.0_f64,
                    "bidfloorcur":"QAA", "ext":{"bidder":{"pkey":"x".repeat(width)}}}
            ]}))
            .unwrap();
            let bidder = factory()();
            let info = info();
            let budget = CallBudget::new(3);
            let mut sink =
                OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &AllowAllEndpoints);
            alloc_track::start();
            let result = bidder.make_requests(&request, &info, &mut sink);
            let accepted_before_discard = sink.accepted_count();
            let outcome = sink.into_outcome_discarding_accepted();
            let measured = alloc_track::finish();
            let errors = result.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 999);
            assert_eq!(accepted_before_discard, 3);
            assert!(outcome.accepted.is_empty());
            assert_eq!(budget.remaining(), 3);
            assert!(measured.peak() <= construction_budget(4096).checked_mul(3).unwrap());
            assert!(measured.allocated() <= construction_budget(4096).checked_mul(3).unwrap());
            observations.push(measured);
        }
        assert!(alloc_track::size_growth_is_bounded(
            &observations[0],
            &observations[1]
        ));
    })
    .join()
    .unwrap();
}

fn response_window(count: usize, code: i32, width: usize, mixed: bool, legacy: bool) {
    let bidder = factory()();
    let request: BidRequest =
        serde_json::from_value(json!({"id":"r", "imp":[impression("i")]})).unwrap();
    let info = info();
    let budget = CallBudget::new(3);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &AllowAllEndpoints);
    bidder.make_requests(&request, &info, &mut sink).unwrap();
    let calls = sink.into_outcome().accepted;
    assert_eq!(calls.len(), 3);
    let (ordinal, media) = match code {
        1_i32 => (0_usize, MediaType::Banner),
        2_i32 => (1_usize, MediaType::Video),
        4_i32 => (2_usize, MediaType::Native),
        _ => panic!("unsupported allocation fixture media"),
    };
    let bytes = serde_json::to_vec(&json!({"id":"r", "cur":"EUR", "seatbid":[{"bid":
        (0..count).map(|index| {
            let mut bid = json!({"id":format!("b-{index}"), "impid":"i", "price":1.25_f64,
                "adm":"x".repeat(width), "mtype":if mixed && index.checked_rem(2).unwrap() == 1 { 3_i32 } else { code }});
            if legacy { bid["ext"] = json!({"prebid":{"type":"conflicting-legacy"}}); }
            bid
        }).collect::<Vec<_>>()
    }]}))
    .unwrap();
    let wire = u64::try_from(bytes.len()).unwrap();
    assert!(wire <= response_allocation::SHIPPED_DECODED_CAP);
    let response =
        BidderHttpResponse::new(200, vec![], bytes).within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
    let baseline = response_alloc_track::reset_peak();
    let decoded = bidder
        .make_bids(&request, calls[ordinal].meta(), &response)
        .unwrap();
    let peak = response_alloc_track::peak_since(baseline);
    assert!(
        peak > 0 && peak <= response_allocation::response_charge(wire),
        "count={count} code={code} width={width} mixed={mixed} legacy={legacy} peak={peak}"
    );
    assert_eq!(decoded.currency, "USD");
    let bad = if mixed {
        count.checked_div(2).unwrap()
    } else {
        0
    };
    assert_eq!(decoded.bids.len(), count.checked_sub(bad).unwrap());
    assert_eq!(decoded.errors.len(), bad);
    assert!(decoded.errors.iter().all(|error| error.code == 4));
    for (index, actual) in decoded.bids.iter().enumerate() {
        assert_eq!(actual.media_type, media);
        assert_eq!(
            actual.bid.id,
            format!(
                "b-{}",
                if mixed {
                    index.checked_mul(2).unwrap()
                } else {
                    index
                }
            )
        );
        assert_eq!(actual.bid.adm.as_deref(), Some("x".repeat(width).as_str()));
        if legacy {
            assert_eq!(
                actual
                    .bid
                    .ext
                    .as_ref()
                    .unwrap()
                    .prebid
                    .as_ref()
                    .unwrap()
                    .bid_type
                    .as_deref(),
                Some("conflicting-legacy")
            );
        }
    }
}

#[test]
fn sealed_decode_partitioning_and_standard_priority_fit_existing_response_funding() {
    for count in [1_usize, 32, 256] {
        for code in [1_i32, 2_i32, 4_i32] {
            for width in [64_usize, 512] {
                for mixed in [false, true] {
                    for legacy in [false, true] {
                        thread::spawn(move || response_window(count, code, width, mixed, legacy))
                            .join()
                            .unwrap();
                    }
                }
            }
        }
    }
}
