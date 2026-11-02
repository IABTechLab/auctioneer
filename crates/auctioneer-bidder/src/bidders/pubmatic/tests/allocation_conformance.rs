//! Borrowed request construction must fit the existing sink construction charge.

use crate::alloc_track::{
    self, AllowAllEndpoints, assert_bounded, construction_budget, size_growth_is_bounded,
};
use crate::response_allocation::{
    SHIPPED_DECODED_CAP, SHIPPED_RESPONSE_NODE_CAP, aggregate_video_response, response_charge,
};
use auctioneer_bidder::{
    Bidder, BidderHttpResponse, BidderName, BidderRequestInfo, CallBudget, EntryPoint,
    META_BYTES_CAPACITY, OutboundBuildError, OutboundRequestMetaBuilder, OutboundRequestSink,
    SinkLimits, compiled_bidders,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::{Map, Value, json};
use std::{hint::black_box, thread};

fn bidder() -> Box<dyn Bidder> {
    (compiled_bidders()
        .iter()
        .find(|entry| entry.key == "pubmatic")
        .unwrap()
        .factory)()
}

fn info() -> BidderRequestInfo<'static> {
    BidderRequestInfo::new(
        BidderName::new("pubmatic-alias"),
        BidderName::new("pubmatic"),
        "account",
        None,
        "https://hbopenbid.pubmatic.com/translator?source=prebid-server",
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    )
}

fn document(survivors: usize) -> Value {
    let mut impressions = vec![json!({"id":"invalid", "banner":{}, "ext":{"bidder":false}})];
    impressions.extend((0..survivors).map(|index| {
        json!({
            "id":format!("sent-{index}"),"banner":{"format":[{"w":300_i32,"h":250_i32}]},
            "ext":{"bidder":{"publisherId":" p "}},
        })
    }));
    json!({
        "id":"allocation","imp":impressions,"site":{"publisher":{"name":"retained"}},
        "ext":{"prebid":{"bidderparams":{"wrapper":{"profile":1_i32,"version":2_i32}}}},
    })
}

#[derive(Clone, Copy, Debug)]
enum Payload {
    Root,
    Survivor,
    Rejected,
    Keyword,
    FirstPartyString,
    FirstPartyArray,
    Skadn,
    Acat,
    Alternate,
    SdkSource,
    SdkIgnoredEscapedKey,
}

fn payload_request(survivors: usize, width: usize, placement: Payload) -> BidRequest {
    let mut document = document(survivors);
    let payload = "\"\\\n".repeat(width);
    match placement {
        Payload::Root => document["payload"] = json!({"nested":payload}),
        Payload::Rejected => document["imp"][0]["payload"] = json!({"nested":payload}),
        Payload::Acat => {
            document["ext"]["prebid"]["bidderparams"]["acat"] = json!([payload]);
        }
        Payload::Alternate => {
            document["ext"]["prebid"]["alternatebiddercodes"] = json!({
                "enabled":true,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":[payload]}},
            });
        }
        Payload::SdkSource => {
            document["app"] = json!({"id":"app","ext":{
                "prebid":{"source":payload,"version":"1.0"},"source":"flat","version":"2.0",
            }});
        }
        Payload::SdkIgnoredEscapedKey => {
            let key = "\"".repeat(width);
            document["app"] = json!({"id":"app","ext":{
                key:{"ignored":[true,null]},"source":"sdk","version":"1.0",
            }});
        }
        Payload::Survivor
        | Payload::Keyword
        | Payload::FirstPartyString
        | Payload::FirstPartyArray
        | Payload::Skadn => {
            for imp in document["imp"].as_array_mut().unwrap().iter_mut().skip(1) {
                match placement {
                    Payload::Survivor => imp["payload"] = json!({"nested":payload}),
                    Payload::Keyword => {
                        imp["ext"]["bidder"]["keywords"] =
                            json!([{"key":"large","value":[payload,"tail"]}]);
                    }
                    Payload::FirstPartyString => imp["ext"]["data"] = json!({"large":payload}),
                    Payload::FirstPartyArray => {
                        imp["ext"]["data"] = json!({"large":[payload," tail "]});
                    }
                    Payload::Skadn => imp["ext"]["skadn"] = json!({"large":payload}),
                    Payload::Root
                    | Payload::Rejected
                    | Payload::Acat
                    | Payload::Alternate
                    | Payload::SdkSource
                    | Payload::SdkIgnoredEscapedKey => {
                        panic!("root payload used for an impression: {placement:?}")
                    }
                }
            }
        }
    }
    serde_json::from_value(document).unwrap()
}

fn clone_control(request: &BidRequest) -> alloc_track::Measurement {
    alloc_track::start();
    drop(black_box(request.clone()));
    alloc_track::finish()
}

fn probe(
    bidder: &dyn Bidder,
    request: &BidRequest,
    cap: usize,
    calls: u32,
    survivors: usize,
    oversized: bool,
) -> alloc_track::Measurement {
    let info = info();
    let budget = CallBudget::new(calls);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(cap, 10), &budget, &AllowAllEndpoints);
    alloc_track::start();
    let semantic = bidder.make_requests(request, &info, &mut sink);
    let outcome = sink.into_outcome();
    let measured = alloc_track::finish();
    semantic.unwrap();
    assert_bounded(&measured, cap);
    assert_eq!(outcome.diagnostics.len(), 1);
    if calls == 0 {
        assert_eq!(
            measured.allocated(),
            0,
            "exhaustion must precede caller copies"
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
fn request_payload_growth_never_creates_uncapped_caller_tree_or_joined_string_copies() {
    thread::spawn(|| {
        let bidder = bidder();
        for placement in [
            Payload::Root, Payload::Survivor, Payload::Rejected, Payload::Keyword,
            Payload::FirstPartyString, Payload::FirstPartyArray, Payload::Skadn, Payload::Acat,
            Payload::Alternate, Payload::SdkSource, Payload::SdkIgnoredEscapedKey,
        ] {
            for survivors in [1_usize, 8] {
                let small = payload_request(survivors, 128, placement);
                let large = payload_request(survivors, 0x0004_0000, placement);
                let small_copy = clone_control(&small);
                let large_copy = clone_control(&large);
                assert!(large_copy.peak() > construction_budget(32));
                assert!(large_copy.allocated() > construction_budget(32));
                assert!(!size_growth_is_bounded(&small_copy, &large_copy));
                for cap in [32_usize, 0x4000] {
                    for calls in [0_u32, 1] {
                        let small_work = probe(&*bidder, &small, cap, calls, survivors, false);
                        let large_work = probe(&*bidder, &large, cap, calls, survivors,
                            !matches!(placement, Payload::Rejected));
                        assert!(size_growth_is_bounded(&small_work, &large_work),
                            "placement={placement:?} survivors={survivors} cap={cap} calls={calls}: small={small_work:?}, large={large_work:?}");
                    }
                }
            }
        }
    }).join().unwrap();
}

#[derive(Clone, Copy, Debug)]
enum Collection {
    Keywords,
    RepeatedKeyword,
    FirstParty,
    FirstPartyArray,
    Acat,
    Alternate,
    SdkIgnored,
}

fn collection_request(entries: usize, placement: Collection) -> BidRequest {
    let mut document = document(1);
    match placement {
        Collection::Keywords | Collection::RepeatedKeyword => {
            let keywords: Vec<_> = (0..entries)
                .map(|index| {
                    let key = if matches!(placement, Collection::RepeatedKeyword) {
                        "same".to_owned()
                    } else {
                        format!("key-{index}")
                    };
                    json!({"key":key,"value":["value"]})
                })
                .collect();
            document["imp"][1]["ext"]["bidder"]["keywords"] = json!(keywords);
        }
        Collection::FirstParty => {
            let mut data = json!({});
            for index in 0..entries {
                data[format!("key-{index}")] = json!("value");
            }
            document["imp"][1]["ext"]["data"] = data;
        }
        Collection::FirstPartyArray => {
            document["imp"][1]["ext"]["data"] = json!({"key":vec![" value ";entries]});
        }
        Collection::Acat => {
            document["ext"]["prebid"]["bidderparams"]["acat"] = json!(vec![" category "; entries]);
        }
        Collection::Alternate => {
            document["ext"]["prebid"]["alternatebiddercodes"] = json!({
                "enabled":true,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":vec!["bidder";entries]}},
            });
        }
        Collection::SdkIgnored => {
            let mut extension = json!({"source":"sdk","version":"1.0"});
            for index in 0..entries {
                extension[format!("ignored-{index}")] = json!({"payload":[true,null]});
            }
            document["app"] = json!({"id":"app","ext":extension});
        }
    }
    serde_json::from_value(document).unwrap()
}

#[test]
fn request_collection_growth_does_not_retain_parameter_maps_or_prepared_impression_vectors() {
    thread::spawn(|| {
        let bidder = bidder();
        for placement in [Collection::Keywords, Collection::RepeatedKeyword, Collection::FirstParty,
            Collection::FirstPartyArray, Collection::Acat, Collection::Alternate, Collection::SdkIgnored] {
            let small = collection_request(8, placement);
            let large = collection_request(8192, placement);
            let small_copy = clone_control(&small);
            let large_copy = clone_control(&large);
            assert!(large_copy.allocated() > construction_budget(32));
            assert!(!size_growth_is_bounded(&small_copy, &large_copy));
            for cap in [32_usize, 0x4000] {
                for calls in [0_u32, 1] {
                    let small_work = probe(&*bidder, &small, cap, calls, 1, false);
                    let large_work = probe(&*bidder, &large, cap, calls, 1,
                        !matches!(placement, Collection::RepeatedKeyword));
                    assert!(size_growth_is_bounded(&small_work, &large_work),
                        "placement={placement:?} cap={cap} calls={calls}: small={small_work:?}, large={large_work:?}");
                }
            }
        }
    }).join().unwrap();
}

#[test]
fn request_fatal_root_errors_remain_fixed_size_and_precede_impression_diagnostics() {
    thread::spawn(|| {
        let bidder = bidder();
        for width in [128_usize, 0x0004_0000] {
            let mut document = document(1);
            document["ext"]["prebid"]["bidderparams"]["acat"] =
                json!([{"secret":"x".repeat(width)}]);
            let request: BidRequest = serde_json::from_value(document).unwrap();
            let info = info();
            let budget = CallBudget::new(1);
            let mut sink =
                OutboundRequestSink::new(SinkLimits::new(32, 10), &budget, &AllowAllEndpoints);
            alloc_track::start();
            let semantic = bidder.make_requests(&request, &info, &mut sink);
            let outcome = sink.into_outcome();
            let measured = alloc_track::finish();
            assert_bounded(&measured, 32);
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].message, "invalid pubmatic request extension");
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.diagnostics.len(), 0);
            assert_eq!(budget.remaining(), 1);
        }
    })
    .join()
    .unwrap();
}

#[test]
fn aggregate_video_metadata_stays_within_the_existing_response_parse_charge() {
    aggregate_video_response(
        bidder,
        "pubmatic",
        Some(&json!({"duration":0_i32,"primary_category":""})),
    );
}

#[test]
fn native_unwrapping_and_browser_config_moves_fit_the_existing_response_charge() {
    for count in [1_usize, 16] {
        for nodes in [1_usize, 128] {
            for bytes in [0_usize, 1024] {
                thread::spawn(move || {
                    let bidder = bidder();
                    let ids: Vec<_> = (0..count).map(|index| format!("fake_i{index}")).collect();
                    let request: BidRequest = serde_json::from_value(json!({
                        "id":"r", "imp":ids.iter().map(|id| json!({"id":id,"native":{}})).collect::<Vec<_>>(),
                    })).unwrap();
                    let creative = json!({"native":{
                        "assets":vec![json!({"id":1_i32,"title":{"text":"title"}});nodes],
                        "link":{"url":"https://fake_bidder.test/","payload":"\\".repeat(bytes)},
                    }}).to_string();
                    let configs: Map<_,_> = ids.iter().map(|id| (id.clone(), json!({"keep":true}))).collect();
                    let body = serde_json::to_vec(&json!({
                        "id":"r", "seatbid":[{"bid":ids.iter().map(|id|json!({
                            "id":id,"impid":id,"price":1.25_f64,"mtype":4_i32,"adm":creative,
                        })).collect::<Vec<_>>()}], "ext":{"fledge_auction_configs":configs},
                    })).unwrap();
                    let body_bytes = u64::try_from(body.len()).unwrap();
                    assert!(body_bytes <= SHIPPED_DECODED_CAP);
                    let response = BidderHttpResponse::new(200, Vec::new(), body).within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
                    let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
                    let mut builder = OutboundRequestMetaBuilder::new(&mut pool, u16::try_from(count).unwrap());
                    builder.set_method("POST").unwrap();
                    builder.set_url("https://bidder.example.test/").unwrap();
                    for id in &ids { builder.imp_id(id).unwrap(); }
                    let sent = builder.finish();
                    let baseline = alloc_track::response::reset_peak();
                    let decoded = bidder.make_bids(&request, &sent, &response).unwrap();
                    let peak = alloc_track::response::peak_since(baseline);
                    assert!(peak > 0 && peak <= response_charge(body_bytes), "count={count} nodes={nodes} bytes={bytes} peak={peak}");
                    assert_eq!(decoded.bids.len(), count);
                    assert!(decoded.errors.is_empty());
                    assert_eq!(decoded.auction_configs.len(), count);
                    let original: Value = serde_json::from_str(&creative).unwrap();
                    for bid in &decoded.bids {
                        assert_eq!(serde_json::from_str::<Value>(bid.bid.adm.as_ref().unwrap()).unwrap(), original["native"]);
                    }
                    for config in &decoded.auction_configs {
                        assert!(ids.contains(&config.imp_id));
                        assert_eq!(config.config, json!({"keep":true}));
                    }
                }).join().unwrap();
            }
        }
    }
}
