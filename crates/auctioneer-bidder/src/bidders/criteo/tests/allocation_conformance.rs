//! Shipping browser-configuration decoding keeps its own protocol and allocation guards.

use crate::alloc_track::response as alloc_track;
use auctioneer_bidder::{
    Bidder, BidderHttpResponse, META_BYTES_CAPACITY, OutboundRequestMetaBuilder,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::{budget::MIN_RESPONSE_PARSE_FACTOR, count_json_nodes};
use serde_json::{Value, json};
use std::{iter::repeat_with, thread};

fn bidder() -> Box<dyn Bidder> {
    (auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|entry| entry.key == "criteo")
        .unwrap()
        .factory)()
}

fn request() -> BidRequest {
    serde_json::from_value(json!({"id":"r","imp":[{"id":"i","banner":{}}]})).unwrap()
}

fn sent() -> auctioneer_bidder::OutboundRequestMeta {
    let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
    let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
    builder.set_method("POST").unwrap();
    builder
        .set_url("https://ssp-bidder.criteo.com/openrtb/pbs/auction/request?profile=230")
        .unwrap();
    builder.imp_id("i").unwrap();
    builder.finish()
}

fn igi(imp: &str, config: &Value) -> Value {
    json!({"impid": imp, "igs": [{"config": config}]})
}

fn browser_response(entries: &[Value], currency: &str, bids: usize) -> Vec<u8> {
    let bid = json!({"id": "b", "impid": "i", "price": 1_i32,
        "w": 300_i32, "h": 250_i32, "adm": "creative",
        "ext": {"prebid": {"type": "banner"}}});
    serde_json::to_vec(&json!({"id": "r", "cur": currency,
        "seatbid": [{"bid": vec![bid; bids]}], "ext": {"igi": entries}}))
    .unwrap()
}

#[test]
fn only_the_first_interest_group_config_is_decoded_and_opaque_identity_stays_opaque() {
    let payload =
        json!({"bidder": "fake_forged", "adapter": "fake_forged", "nested": [1_i32, {"k": true}]});
    let mut row = igi("i", &payload);
    row["igs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"config":{"not":"first"}}));
    let decoded = bidder()
        .make_bids(
            &request(),
            &sent(),
            &BidderHttpResponse::new(200, vec![], browser_response(&[row], "USD", 0))
                .within_node_budget(512),
        )
        .unwrap();
    assert!(decoded.bids.is_empty());
    assert!(decoded.errors.is_empty());
    assert_eq!(decoded.currency, "USD");
    assert_eq!(decoded.auction_configs.len(), 1);
    assert_eq!(decoded.auction_configs[0].imp_id, "i");
    assert_eq!(decoded.auction_configs[0].config, payload);
}

#[test]
fn configs_and_currency_decode_independently_of_bid_count() {
    for currency in ["USD", "EUR", ""] {
        for count in [0_usize, 1, 3] {
            let decoded = bidder()
                .make_bids(
                    &request(),
                    &sent(),
                    &BidderHttpResponse::new(
                        200,
                        vec![],
                        browser_response(&[igi("i", &json!({"ok":true}))], currency, count),
                    )
                    .within_node_budget(512),
                )
                .unwrap();
            assert_eq!(decoded.bids.len(), count);
            assert!(decoded.errors.is_empty());
            assert_eq!(decoded.currency, currency);
            assert_eq!(decoded.auction_configs.len(), 1);
            assert_eq!(decoded.auction_configs[0].imp_id, "i");
            assert_eq!(decoded.auction_configs[0].config, json!({"ok":true}));
        }
    }
}

#[test]
fn browser_decoder_count_payload_and_nodes_fit_the_response_parse_funding_with_margin() {
    for count in [0_usize, 1, 8, 24] {
        for payload_bytes in [0_usize, 32, 512] {
            for nodes in [0_usize, 4] {
                thread::spawn(move || {
                    let bidder = bidder();
                    let request = request();
                    let sent = sent();
                    let config =
                        json!({"payload": "x".repeat(payload_bytes), "nodes": vec![0_u32; nodes]});
                    let entries = repeat_with(|| igi("i", &config))
                        .take(count)
                        .collect::<Vec<_>>();
                    let raw = browser_response(&entries, "USD", 0);
                    assert!(raw.len() <= 32 * 1024);
                    assert!(count_json_nodes(&raw, 512).unwrap() <= 512);
                    let bytes = u64::try_from(raw.len()).unwrap();
                    let response =
                        BidderHttpResponse::new(200, vec![], raw).within_node_budget(512);
                    let baseline = alloc_track::reset_peak();
                    let decoded = bidder.make_bids(&request, &sent, &response).unwrap();
                    let peak = alloc_track::peak_since(baseline);
                    assert!(peak > 0);
                    assert!(
                        peak.checked_mul(6).unwrap().div_ceil(5)
                            <= bytes * MIN_RESPONSE_PARSE_FACTOR
                    );
                    assert!(decoded.bids.is_empty());
                    assert!(decoded.errors.is_empty());
                    assert_eq!(decoded.auction_configs.len(), count);
                    for row in &decoded.auction_configs {
                        assert_eq!(row.imp_id, "i");
                        assert_eq!(row.config, config);
                    }
                })
                .join()
                .unwrap();
            }
        }
    }
}
