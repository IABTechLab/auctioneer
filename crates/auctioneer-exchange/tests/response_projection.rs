//! Normalized bid projection belongs to exchange; supplier decoding stays with its owner.

#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;
#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::{alloc_track, fake_bidder::FakeBidder};
    use auctioneer_bidder::{
        Bidder as _, BidderHttpResponse, BidderName, BidderResponse, META_BYTES_CAPACITY,
        OutboundRequestMetaBuilder,
    };
    use auctioneer_config_data::types::MediaType;
    use auctioneer_exchange::{
        BidderResultBundle, ResponseInput, ResponseTimeStat, build_response,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::{
        budget::{MIN_RESPONSE_PARSE_FACTOR, RESPONSE_ASSEMBLY_FACTOR},
        count_json_nodes,
    };
    use serde_json::{Value, json};
    use std::{collections::BTreeMap, thread};

    const NODE_CAP: u32 = 1 << 16;
    const DECODED_CAP: usize = 256 * 1024;

    #[derive(Clone, Copy, Debug)]
    struct Case {
        count: usize,
        media: MediaType,
        priority: i64,
        duration: i64,
    }

    fn normalized_bid(case: Case, index: usize) -> Value {
        let (mtype, format) = match case.media {
            MediaType::Banner => (1_u32, "banner"),
            MediaType::Video => (2, "video"),
            MediaType::Native => (4, "native"),
            MediaType::Audio => panic!("the independent format grid excludes audio"),
        };
        let mut bid = json!({
            "id":format!("b{index}"),"impid":"i","price":1.5_f64,"adm":"<a/>",
            "crid":"creative","mtype":mtype,"cat":["IAB20-3"],
            "ext":{
                "origbidcpm":2.5_f64,"origbidcur":"EUR",
                "fake_keep":{"opaque":[1_i32,false]},
                "fake_private":{"format_hint":mtype,"priority_hint":case.priority,
                    "category_hint":1_i64,"brand_hint":2_i64,
                    "creative":{"video":{"duration":case.duration,"opaque":"fake-video"},
                        "opaque":true},"untouched":[true,null,7_i32]},
                "prebid":{
                    "type":format,"targeting":{"keep":"value"},"meta":{"opaque":"keep"},
                    "bidid":"input-bid","dealtiersatisfied":true,
                    "targetbiddercode":"fake_alias","opaque_prebid":{"keep":true},
                    "video":{"duration":case.duration,"primary_category":""}
                }
            }
        });
        if case.priority != 0 {
            bid["ext"]["prebid"]["dealpriority"] = json!(case.priority);
        }
        bid
    }

    fn response_input(decoded: BidderResponse) -> ResponseInput {
        ResponseInput {
            construction_diagnostics: BTreeMap::new(),
            bidder_visible_tmax_ms: 100,
            auction_timestamp_ms: 0,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            prebid_errors: Vec::new(),
            prebid_warnings: Vec::new(),
            bundles: BTreeMap::from([(
                BidderName::new("fake_alias"),
                BidderResultBundle {
                    bids: decoded.bids,
                    errors: decoded
                        .errors
                        .into_iter()
                        .map(|error| auctioneer_exchange::ErrorEntry {
                            code: error.code,
                            message: error.message,
                        })
                        .collect(),
                    warnings: Vec::new(),
                    timing: ResponseTimeStat::default(),
                    dispatched: true,
                    browser_auctions: None,
                },
            )]),
        }
    }

    fn probe(case: Case, source: &[Value], expected: &[Value], error_count: usize) {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r","imp":[{"id":"i","banner":{},"video":{},"native":{"request":"{}"}}]
        }))
        .unwrap();
        let bytes = serde_json::to_vec(&json!({
            "id":"r","cur":"USD","seatbid":[{"seat":"fake_provider","bid":source}]
        }))
        .unwrap();
        assert!(!bytes.is_empty() && bytes.len() <= DECODED_CAP);
        count_json_nodes(&bytes, NODE_CAP).unwrap();
        let wire_bytes = u64::try_from(bytes.len()).unwrap();
        let response = BidderHttpResponse::new(200, Vec::new(), bytes).within_node_budget(NODE_CAP);
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
        builder.set_method("POST").unwrap();
        builder
            .set_url("https://bidder.example.test/auction")
            .unwrap();
        builder.imp_id("i").unwrap();
        let sent = builder.finish();
        assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["i"]);

        let parse_start = alloc_track::reset_peak();
        let result = FakeBidder::default().make_bids(&request, &sent, &response);
        let parse_peak = alloc_track::peak_since(parse_start);
        let decoded = result.unwrap();
        assert!(parse_peak > 0);
        assert!(
            parse_peak <= wire_bytes.checked_mul(MIN_RESPONSE_PARSE_FACTOR).unwrap(),
            "{case:?}: wire={wire_bytes} parse={parse_peak}"
        );
        assert_eq!(decoded.currency, "USD");
        assert_eq!(decoded.errors.len(), error_count);
        assert_eq!(decoded.bids.len(), expected.len());
        for (typed, literal) in decoded.bids.iter().zip(expected) {
            assert_eq!(typed.media_type, case.media);
            assert_eq!(&serde_json::to_value(&typed.bid).unwrap(), literal);
        }

        // Owner decoders independently prove their output. Here the public
        // normalized result moves into exchange without a provider-specific import.
        let input = response_input(decoded);
        let assembly_start = alloc_track::reset_peak();
        let built = build_response(input).unwrap();
        let serialized = serde_json::to_vec(&built).unwrap();
        let assembly_peak = alloc_track::peak_since(assembly_start);
        assert!(assembly_peak > 0);
        assert!(
            assembly_peak <= wire_bytes.checked_mul(RESPONSE_ASSEMBLY_FACTOR).unwrap(),
            "{case:?}: wire={wire_bytes} assembly={assembly_peak}"
        );
        let wire: Value = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(wire["cur"], "USD");
        let seats = wire["seatbid"].as_array().unwrap();
        assert_eq!(seats.len(), 1);
        assert_eq!(seats[0]["seat"], "fake_alias");
        let bids = seats[0]["bid"].as_array().unwrap();
        assert_eq!(bids, expected);
        for bid in bids {
            assert_eq!(bid["cat"], json!(["IAB20-3"]));
            assert_eq!(
                bid["ext"]["prebid"]["video"],
                json!({"duration":case.duration,"primary_category":""})
            );
            assert_eq!(
                bid["ext"]["prebid"]
                    .get("dealpriority")
                    .and_then(Value::as_i64),
                (case.priority != 0).then_some(case.priority)
            );
            assert_eq!(
                bid["ext"]["prebid"].get("dealpriority").is_none(),
                case.priority == 0
            );
        }
    }

    #[test]
    fn normalized_signed_priorities_formats_video_and_opaque_fields_fit_response_funding() {
        thread::spawn(|| {
            let mut checked = 0_usize;
            for count in [1_usize, 32, 256] {
                for media in [MediaType::Banner, MediaType::Video, MediaType::Native] {
                    for priority in [i64::MIN, 0, i64::MAX] {
                        for duration in [-3_i64, 0, 30] {
                            let case = Case {
                                count,
                                media,
                                priority,
                                duration,
                            };
                            let expected: Vec<_> = (0..case.count)
                                .map(|index| normalized_bid(case, index))
                                .collect();
                            probe(case, &expected, &expected, 0);
                            checked += 1;
                        }
                    }
                }
            }
            assert_eq!(checked, 81);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn invalid_normalized_media_does_not_hide_a_valid_sibling_or_assembly_work() {
        thread::spawn(|| {
            let case = Case {
                count: 2,
                media: MediaType::Video,
                priority: i64::MAX,
                duration: -3,
            };
            let mut checked = 0_usize;
            for invalid in [
                json!("video"),
                json!(0_u32),
                json!(5_u32),
                json!(true),
                json!(null),
                json!({}),
            ] {
                let mut bad = normalized_bid(case, 0);
                bad["mtype"] = invalid;
                let good = normalized_bid(case, 1);
                probe(case, &[bad, good.clone()], &[good], 1);
                checked += 1;
            }
            assert_eq!(checked, 6);
        })
        .join()
        .unwrap();
    }
}
