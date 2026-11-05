//! Bounded constructor and sealed-response allocation controls for the discovered factory.

use crate::alloc_track::{
    self, AllowAllEndpoints, construction_budget, response as response_alloc_track,
    size_growth_is_bounded,
};
use crate::response_allocation::{
    SHIPPED_DECODED_CAP, SHIPPED_RESPONSE_NODE_CAP, response_charge, syntactic_nodes,
};
use auctioneer_bidder::BidderSettings;
use auctioneer_bidder::{
    Bidder, BidderHttpResponse, BidderName, BidderRequestInfo, BidderResponse, CallBudget,
    EntryPoint, OutboundBuildError, OutboundRequestSink, SinkLimits, compiled_bidders,
};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::bidder_credentials::BidderCredentials;
use serde_json::{Value, json};
use std::{hint::black_box, str::from_utf8, thread};

const ENDPOINT: &str = "https://bidder.example.test/a/api/exchange";
const LARGE: usize = 32 * 1024;

fn factory() -> fn() -> Box<dyn Bidder> {
    compiled_bidders()
        .iter()
        .find(|row| row.key == "rubicon")
        .unwrap()
        .factory
}

fn settings() -> BidderSettings {
    serde_json::from_value(json!({"implementation":"rubicon","values":{
        "credentials_secret":"fake_credential_bundle","external_url":"http://hosturl.com",
        "region":"us-east"
    }}))
    .unwrap()
}

fn credentials() -> BidderCredentials {
    // Literal credentials belong to upstream TestJsonSamples, not an operator.
    BidderCredentials::parse(
        &serde_json::to_vec(&json!({
            "schema_version":1_u8,"username":"xuser","password":"xpass","tracker":"pbs-test-tracker"
        }))
        .unwrap(),
    )
    .unwrap()
}

fn info<'a>(
    settings: &'a BidderSettings,
    credentials: Option<&'a BidderCredentials>,
) -> BidderRequestInfo<'a> {
    let mut context = BidderRequestInfo::new(
        BidderName::new("rubicon"),
        BidderName::new("rubicon"),
        "fake_account",
        None,
        ENDPOINT,
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    );
    context
        .set_settings(Some("rubicon"), Some(settings), None)
        .unwrap();
    context.credentials = credentials;
    context
}

fn banner(id: &str) -> Value {
    json!({"id":id,"banner":{"format":[{"w":300_i32,"h":250_i32}]},
        "ext":{"bidder":{"accountId":1001_i64,"siteId":113_932_i64,"zoneId":535_510_i64}}})
}

fn native_payload(width: usize) -> Value {
    json!({"eventtrackers":[],"plcmttype":2_i32,"context":1_i32,
        "payload": "x".repeat(width),
        "assets":[{"id":1_i32,"title":{"len":25_i32}}]})
}

#[derive(Clone, Copy, Debug)]
enum Location {
    Root,
    Impression,
    Native,
    Target,
}

fn request(location: Location, width: usize) -> BidRequest {
    let mut document = json!({"id":"r","site":{},"imp":[banner("i")]});
    match location {
        Location::Root => document["payload"] = json!("x".repeat(width)),
        Location::Impression => document["imp"][0]["payload"] = json!("x".repeat(width)),
        Location::Native => {
            document["imp"][0].as_object_mut().unwrap().remove("banner");
            document["imp"][0]["native"] = json!({
                "ver":"1.2","request":native_payload(width).to_string()
            });
        }
        Location::Target => {
            document["imp"][0]["ext"]["bidder"]["inventory"] =
                json!({"payload":["x".repeat(width)]});
        }
    }
    serde_json::from_value(document).unwrap()
}

fn constructor_window(
    location: Location,
    width: usize,
    cap: usize,
    slots: u32,
) -> alloc_track::Measurement {
    let input = request(location, width);
    let before = serde_json::to_value(&input).unwrap();
    let allowance = construction_budget(cap);
    if width == LARGE {
        alloc_track::start();
        drop(black_box(input.clone()));
        let copied = alloc_track::finish();
        assert!(
            copied.allocated() > allowance,
            "clone control: {location:?} {copied:?}"
        );
    }
    let bidder = factory()();
    let host = settings();
    let bundle = credentials();
    let context = info(&host, Some(&bundle));
    let budget = CallBudget::new(slots);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(cap, 1), &budget, &AllowAllEndpoints);
    alloc_track::start();
    let result = bidder.make_requests(&input, &context, &mut sink);
    let outcome = sink.into_outcome();
    let measured = alloc_track::finish();
    result.unwrap();
    alloc_track::assert_bounded(&measured, cap);
    assert!(outcome.diagnostics.is_empty());
    if slots == 0 {
        assert_eq!(measured.allocated(), 0);
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.rejected_call_count, 1);
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        );
    } else if cap <= 48 || width == LARGE {
        assert!(outcome.accepted.is_empty());
        assert_eq!(budget.remaining(), slots);
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::OversizeRaw {
                limit: u64::try_from(cap).unwrap()
            }]
        );
    } else {
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(budget.remaining(), slots.checked_sub(1).unwrap());
        let call = outcome.accepted.first().unwrap();
        assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
        assert!(
            call.headers()
                .any(|pair| pair == ("Authorization", "Basic eHVzZXI6eHBhc3M="))
        );
        let body: Value = serde_json::from_slice(call.body()).unwrap();
        let payload = json!("x".repeat(width));
        match location {
            Location::Root => assert_eq!(body["payload"], payload),
            Location::Impression => assert_eq!(body["imp"][0]["payload"], payload),
            Location::Native => {
                assert_eq!(
                    body["imp"][0]["native"]["request_native"],
                    native_payload(width)
                );
                assert!(body["imp"][0].get("banner").is_none());
            }
            Location::Target => assert_eq!(
                body["imp"][0]["ext"]["rp"]["target"]["payload"],
                json!([payload])
            ),
        }
    }
    assert_eq!(serde_json::to_value(&input).unwrap(), before);
    measured
}

#[test]
fn constructor_borrows_root_impression_native_and_target_across_caps_and_slots() {
    thread::spawn(|| {
        let mut windows = 0_usize;
        for location in [
            Location::Root,
            Location::Impression,
            Location::Native,
            Location::Target,
        ] {
            for cap in [0_usize, 48, 4096] {
                for slots in [0_u32, 1] {
                    let small = constructor_window(location, 32, cap, slots);
                    let large = constructor_window(location, LARGE, cap, slots);
                    assert!(
                        size_growth_is_bounded(&small, &large),
                        "{location:?} cap={cap} slots={slots}: {small:?} / {large:?}"
                    );
                    windows = windows.checked_add(2).unwrap();
                }
            }
        }
        assert_eq!(windows, 48);
    })
    .join()
    .unwrap();
}

fn split_request() -> BidRequest {
    serde_json::from_value(json!({"id":"r","site":{},"imp":(0_usize..3).map(|index| {
        let mut imp = banner(&format!("i-{index}"));
        imp["ext"]["bidder"]["bidonmultiformat"] = json!(true);
        imp["video"] = json!({"mimes":["video/mp4"],"protocols":[2_i32],"maxduration":30_i32,"linearity":1_i32});
        imp["native"] = json!({"ver":"1.2","request":native_payload(8).to_string()});
        imp
    }).collect::<Vec<_>>()})).unwrap()
}

#[test]
fn split_call_slots_and_zero_body_cap_preserve_exact_attempts_and_refunds() {
    thread::spawn(|| {
        let bidder = factory()();
        let input = split_request();
        let host = settings();
        let bundle = credentials();
        let context = info(&host, Some(&bundle));
        for cap in [0_usize, 4096] {
            for slots in [0_u32, 1, 4] {
                let budget = CallBudget::new(slots);
                let mut sink =
                    OutboundRequestSink::new(SinkLimits::new(cap, 3), &budget, &AllowAllEndpoints);
                alloc_track::start();
                let result = bidder.make_requests(&input, &context, &mut sink);
                let outcome = sink.into_outcome();
                let measured = alloc_track::finish();
                result.unwrap();
                let per_call = construction_budget(cap);
                let live_slots = usize::try_from(slots).unwrap();
                assert!(
                    measured.peak() <= per_call.checked_mul(live_slots).unwrap(),
                    "{measured:?}"
                );
                assert!(
                    measured.allocated() <= per_call.checked_mul(9).unwrap(),
                    "{measured:?}"
                );
                assert!(measured.largest() <= per_call, "{measured:?}");
                assert!(outcome.diagnostics.is_empty());
                if slots == 0 {
                    assert_eq!(measured.allocated(), 0);
                    assert!(outcome.accepted.is_empty());
                    assert_eq!(outcome.rejected_call_count, 9);
                } else if cap == 0 {
                    assert!(outcome.accepted.is_empty());
                    assert_eq!(outcome.errors.len(), 9);
                    assert!(outcome.errors.iter().all(|error| matches!(
                        error,
                        OutboundBuildError::OversizeRaw { limit: 0 }
                    )));
                    assert_eq!(budget.remaining(), slots);
                } else {
                    assert_eq!(outcome.accepted.len(), live_slots);
                    assert_eq!(budget.remaining(), 0);
                    assert_eq!(
                        outcome.rejected_call_count,
                        9_u32.checked_sub(slots).unwrap()
                    );
                    for (index, call) in outcome.accepted.iter().enumerate() {
                        assert_eq!(
                            call.covered_imp_ids().collect::<Vec<_>>(),
                            [format!("i-{}", index.checked_div(3).unwrap())]
                        );
                        let body: Value = serde_json::from_slice(call.body()).unwrap();
                        let selected = index.checked_rem(3).unwrap();
                        for (ordinal, name) in ["banner", "video", "native"].into_iter().enumerate()
                        {
                            assert_eq!(body["imp"][0].get(name).is_some(), selected == ordinal);
                        }
                    }
                }
            }
        }
    })
    .join()
    .unwrap();
}

#[test]
fn native_credential_preflight_borrows_nested_values_and_escaped_unknown_keys() {
    thread::spawn(|| {
        let bidder = factory()();
        let host = settings();
        let context = info(&host, None);
        for width in [32_usize, LARGE] {
            let mut payload = native_payload(width);
            payload
                .as_object_mut()
                .unwrap()
                .insert(format!("escaped{}key", "\"".repeat(width)), json!(true));
            let mut input = request(Location::Native, width);
            input
                .imp
                .first_mut()
                .unwrap()
                .native
                .as_mut()
                .unwrap()
                .request = Some(payload.to_string());
            alloc_track::start();
            let key = bidder.credential_key(&input, &context);
            let measured = alloc_track::finish();
            assert_eq!(key.unwrap(), Some("fake_credential_bundle"));
            assert_eq!(measured.allocated(), 0, "width={width}: {measured:?}");
            assert_eq!(measured.peak(), 0);
            assert!(context.credentials.is_none());
        }
    })
    .join()
    .unwrap();
}

#[derive(Clone, Copy, Debug)]
enum Replication {
    Native,
    RootId,
    Seat,
    Combined,
}

fn assert_replicated_bids(
    decoded: &BidderResponse,
    mode: Replication,
    accepted: usize,
    root_id: &str,
    seat_id: &str,
    expected_adm: &str,
) {
    let native = matches!(mode, Replication::Native | Replication::Combined);
    let root = matches!(mode, Replication::RootId | Replication::Combined);
    let seat = matches!(mode, Replication::Seat | Replication::Combined);
    for (index, typed) in decoded.bids.iter().take(accepted).enumerate() {
        assert_eq!(typed.media_type, MediaType::Native);
        assert_eq!(
            typed.bid.id,
            if root {
                root_id.to_owned()
            } else {
                format!("b-{index}")
            }
        );
        assert_eq!(
            typed.bid.adm.as_deref(),
            Some(if native { expected_adm } else { "creative" })
        );
        assert!(!typed.bid.other.contains_key("adm_native"));
        if seat {
            let metadata = typed
                .bid
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .meta
                .as_ref()
                .unwrap();
            assert_eq!(metadata["seat"], seat_id);
        }
    }
}

fn response_window(mode: Replication, count: usize, width: usize) -> bool {
    let bidder = factory()();
    let input = request(Location::Native, 0);
    let host = settings();
    let bundle = credentials();
    let context = info(&host, Some(&bundle));
    let budget = CallBudget::new(1);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &AllowAllEndpoints);
    bidder.make_requests(&input, &context, &mut sink).unwrap();
    let calls = sink.into_outcome().accepted;
    assert_eq!(calls.len(), 1);
    let call = calls.first().unwrap();
    let native = matches!(mode, Replication::Native | Replication::Combined);
    let root = matches!(mode, Replication::RootId | Replication::Combined);
    let seat = matches!(mode, Replication::Seat | Replication::Combined);
    let root_id = if root {
        "r".repeat(width)
    } else {
        String::new()
    };
    let seat_id = if seat {
        "s".repeat(width)
    } else {
        String::new()
    };
    let creative = json!({"property1":"x".repeat(width)});
    let expected_adm = creative.to_string();
    let document = json!({"id":"r","cur":"USD","bidid":root_id,"seatbid":[
        {"seat":seat_id,"buyer":"123","bid":(0..count).map(|index| {
            let mut row=json!({"id":if root {"0".to_owned()} else {format!("b-{index}")},
                "impid":"i","price":1.25_f64,"mtype":4_i32});
            if native {row["adm_native"]=creative.clone();} else {row["adm"]=json!("creative");}
            row
        }).collect::<Vec<_>>()},
        {"bid":[{"id":"valid","impid":"i","price":1.25_f64,"mtype":4_i32,"adm":"creative"}]}
    ]});
    let bytes = serde_json::to_vec(&document).unwrap();
    let length = bytes.len();
    assert!(u64::try_from(length).unwrap() <= SHIPPED_DECODED_CAP);
    let nodes = syntactic_nodes(from_utf8(&bytes).unwrap());
    assert!(nodes <= u64::from(SHIPPED_RESPONSE_NODE_CAP));
    let per_bid = root_id
        .len()
        .checked_add(seat_id.len())
        .unwrap()
        .checked_add(if native { expected_adm.len() } else { 0 })
        .unwrap();
    assert!(per_bid > 0);
    let accepted = count.min(length.checked_div(per_bid).unwrap());
    assert!(accepted > 0);
    let response = BidderHttpResponse::new(200, Vec::new(), bytes)
        .within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
    let baseline = response_alloc_track::reset_peak();
    let result = bidder.make_bids(&input, call.meta(), &response);
    let peak = response_alloc_track::peak_since(baseline);
    let decoded = result.unwrap();
    assert!(peak > 0);
    assert!(
        peak <= response_charge(u64::try_from(length).unwrap()),
        "{mode:?} count={count} width={width} bytes={length} nodes={nodes} peak={peak}"
    );
    assert_eq!(decoded.currency, "USD");
    assert_eq!(decoded.bids.len(), accepted.checked_add(1).unwrap());
    assert_eq!(decoded.errors.len(), count.checked_sub(accepted).unwrap());
    assert!(decoded.errors.iter().all(|error| error.code == 4));
    assert!(decoded.auction_configs.is_empty());
    assert_replicated_bids(&decoded, mode, accepted, &root_id, &seat_id, &expected_adm);
    assert_eq!(decoded.bids.last().unwrap().bid.id, "valid");
    assert!(accepted.checked_mul(per_bid).unwrap() <= length);
    accepted < count
}

#[test]
fn native_markup_root_ids_and_seats_share_one_funded_response_expansion_bound() {
    thread::spawn(|| {
        let mut windows = 0_usize;
        let mut limited = 0_usize;
        for mode in [
            Replication::Native,
            Replication::RootId,
            Replication::Seat,
            Replication::Combined,
        ] {
            for count in [1_usize, 8, 32] {
                for width in [16_usize, 256] {
                    if response_window(mode, count, width) {
                        limited = limited.checked_add(1).unwrap();
                    }
                    windows = windows.checked_add(1).unwrap();
                }
            }
        }
        assert_eq!(windows, 24);
        assert!(limited > 0);
    })
    .join()
    .unwrap();
}
