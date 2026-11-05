//! Response-only allocation controls shared by owning bidder conformance modules.

use crate::alloc_track::response as alloc_track;
use auctioneer_bidder::{BidderHttpResponse, META_BYTES_CAPACITY, OutboundRequestMetaBuilder};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::budget::{MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR};
use serde_json::json;
use std::thread;

pub(crate) const SHIPPED_RESPONSE_NODE_CAP: u32 = 1 << 16;
pub(crate) const SHIPPED_DECODED_CAP: u64 = 256 * 1024;

pub(crate) fn response_charge(decoded: u64) -> u64 {
    let bytes = decoded
        .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
        .expect("the per-byte half does not overflow");
    let nodes = u64::from(SHIPPED_RESPONSE_NODE_CAP)
        .checked_mul(MIN_RESPONSE_NODE_PEAK_BYTES)
        .expect("the per-node half does not overflow");
    bytes
        .checked_add(nodes)
        .expect("the per-call response charge does not overflow")
}

pub(crate) fn syntactic_nodes(raw: &str) -> u64 {
    fn walk(value: &serde_json::Value) -> u64 {
        match value {
            serde_json::Value::Object(map) => map.iter().fold(1, |acc, (_key, inner)| {
                acc.saturating_add(1).saturating_add(walk(inner))
            }),
            serde_json::Value::Array(items) => items
                .iter()
                .fold(1, |acc, inner| acc.saturating_add(walk(inner))),
            serde_json::Value::Bool(_)
            | serde_json::Value::Null
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => 1,
        }
    }
    let value: serde_json::Value = serde_json::from_str(raw).expect("the fixture parses");
    walk(&value)
}

pub(crate) fn aggregate_video_response(
    factory: fn() -> Box<dyn auctioneer_bidder::Bidder>,
    key: &'static str,
    expected_video: Option<&serde_json::Value>,
) {
    for count in [1_usize, 32, 256] {
        let video_metadata = expected_video.cloned();
        thread::spawn(move || {
            let bidder = factory();
            let request: BidRequest = serde_json::from_value(json!({
                "id":"r", "imp":[{"id":"i","video":{}}],
            }))
            .unwrap();
            let bytes = serde_json::to_vec(&json!({
                "id":"r", "cur":"USD", "seatbid":[{"bid":
                    (0..count).map(|index| json!({
                        "id":format!("b{index}"), "impid":"i", "price":1_i64,
                        "mtype":2_i32, "dur":30_i64, "adm":"%3CVAST%3E+content",
                    })).collect::<Vec<_>>()
                }],
            }))
            .unwrap();
            let wire_bytes = u64::try_from(bytes.len()).unwrap();
            let response = BidderHttpResponse::new(200, vec![], bytes)
                .within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            meta.set_url("https://bidder.example.test/auction").unwrap();
            meta.imp_id("i").unwrap();
            let sent = meta.finish();
            let baseline = alloc_track::reset_peak();
            let decoded = bidder.make_bids(&request, &sent, &response).unwrap();
            let peak = alloc_track::peak_since(baseline);
            assert!(peak > 0);
            assert!(
                peak <= wire_bytes.checked_mul(MIN_RESPONSE_PARSE_FACTOR).unwrap(),
                "{key} count={count} wire={wire_bytes} peak={peak}"
            );
            assert_eq!(decoded.bids.len(), count, "{key}");
            assert!(decoded.errors.is_empty(), "{key}");
            for bid in &decoded.bids {
                assert_eq!(bid.media_type, MediaType::Video, "{key}");
                if let Some(expected_metadata) = &video_metadata {
                    let prebid = bid.bid.ext.as_ref().unwrap().prebid.as_ref().unwrap();
                    assert_eq!(prebid.other.get("video"), Some(expected_metadata));
                }
            }
        })
        .join()
        .unwrap();
    }
}
