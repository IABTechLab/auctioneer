//! Original MSFT sealed decode/classify/project allocation and literal output guards.

use crate::alloc_track::response as alloc_track;
use crate::response_allocation::{
    SHIPPED_DECODED_CAP, SHIPPED_RESPONSE_NODE_CAP, response_charge, syntactic_nodes,
};
use auctioneer_bidder::{
    BidderHttpResponse, BidderResponse, META_BYTES_CAPACITY, OutboundRequestMetaBuilder,
};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::budget::MIN_RESPONSE_PARSE_FACTOR;
use serde_json::json;
use std::{str::from_utf8, thread};

#[derive(Clone, Copy, Debug)]
struct MsftResponseAllocationCase {
    count: usize,
    ad_type: i64,
    priority: i64,
    duration: i64,
}

struct MsftResponseAllocationFixture {
    response: BidderHttpResponse,
    wire_bytes: u64,
    nodes: u64,
    expected: Vec<serde_json::Value>,
    errors: Vec<auctioneer_bidder::BidderError>,
}

struct MsftResponseAllocationContext {
    bidder: Box<dyn auctioneer_bidder::Bidder>,
    request: BidRequest,
    sent: auctioneer_bidder::OutboundRequestMeta,
}

fn msft_response_allocation_context() -> MsftResponseAllocationContext {
    let bidder = (auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|entry| entry.key == "msft")
        .unwrap()
        .factory)();
    // The SAME covered id offers every format. A classification refusal
    // must not substitute for the decode/classify/project allocation path.
    let request: BidRequest = serde_json::from_value(json!({
        "id":"r","imp":[{"id":"i","banner":{},"video":{},"native":{"request":"{}"}}],
    }))
    .unwrap();
    let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
    let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
    builder.set_method("POST").unwrap();
    builder
        .set_url("https://bidder.example.test/openrtb2?member_id=42")
        .unwrap();
    builder.imp_id("i").unwrap();
    let sent = builder.finish();
    assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
    MsftResponseAllocationContext {
        bidder,
        request,
        sent,
    }
}

fn msft_response_allocation_media(ad_type: i64) -> (MediaType, &'static str) {
    match ad_type {
        0 => (MediaType::Banner, "banner"),
        1 => (MediaType::Video, "video"),
        3 => (MediaType::Native, "native"),
        _ => panic!("fixture only declares source ad types 0/1/3"),
    }
}

fn msft_response_allocation_source_bid(
    case: MsftResponseAllocationCase,
    index: usize,
) -> serde_json::Value {
    let (_, media_name) = msft_response_allocation_media(case.ad_type);
    json!({
        "id":format!("b{index}"),"impid":"i","price":1.5_f64,"adm":"<a/>","crid":"creative",
        "cat":["old","categories"],
        "ext":{
            "origbidcpm":2.5_f64,"origbidcur":"EUR",
            "provider_keep":{"opaque":[1,false]},
            "appnexus":{
                "bid_ad_type":case.ad_type,"deal_priority":case.priority,
                "brand_category_id":1_i64,"brand_id":2_i64,
                "creative_info":{"video":{"duration":case.duration,"opaque":"provider-video"},
                    "opaque":true},
                "provider_opaque":{"untouched":[true,null,7]},
            },
            "prebid":{
                "type":media_name,"targeting":{"keep":"value"},"meta":{"opaque":"keep"},
                "bidid":"input-bid","dealpriority":17_i64,"dealtiersatisfied":true,
                "targetbiddercode":"fake_alias","opaque_prebid":{"keep":true},
                "video":{"duration":999_i64,"primary_category":"old","discard":true},
            },
        },
    })
}

fn msft_response_allocation_expected_bid(
    source: &serde_json::Value,
    priority: i64,
    duration: i64,
) -> serde_json::Value {
    // Independent literal projection, prepared before either measured phase.
    // Source brand category 1 is IAB20-3, not a lookup of adapter output.
    let mut expected = source.clone();
    expected["cat"] = json!(["IAB20-3"]);
    let prebid = expected["ext"]["prebid"].as_object_mut().unwrap();
    prebid.insert(
        "video".to_owned(),
        json!({"duration":duration,"primary_category":""}),
    );
    prebid.remove("dealpriority");
    if priority != 0 {
        prebid.insert("dealpriority".to_owned(), json!(priority));
    }
    expected
}

fn msft_response_allocation_fixture(
    source: &[serde_json::Value],
    expected: Vec<serde_json::Value>,
    errors: Vec<auctioneer_bidder::BidderError>,
) -> MsftResponseAllocationFixture {
    let bytes = serde_json::to_vec(&json!({
        "id":"r","cur":"USD","seatbid":[{"seat":"fake_provider","bid":source}],
    }))
    .unwrap();
    let wire_bytes = u64::try_from(bytes.len()).unwrap();
    let nodes = syntactic_nodes(from_utf8(&bytes).unwrap());
    assert!(nodes > 0 && nodes <= u64::from(SHIPPED_RESPONSE_NODE_CAP));
    assert!(wire_bytes > 0 && wire_bytes <= SHIPPED_DECODED_CAP);
    let response = BidderHttpResponse::new(200, Vec::new(), bytes)
        .within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
    assert_eq!(u64::try_from(response.body_len()).unwrap(), wire_bytes);
    assert_eq!(response.max_parsed_nodes(), SHIPPED_RESPONSE_NODE_CAP);
    MsftResponseAllocationFixture {
        response,
        wire_bytes,
        nodes,
        expected,
        errors,
    }
}

fn msft_assert_response_allocation_decoded(
    decoded: &BidderResponse,
    fixture: &MsftResponseAllocationFixture,
    case: MsftResponseAllocationCase,
) {
    assert_eq!(decoded.currency, "USD");
    assert_eq!(decoded.errors, fixture.errors);
    assert_eq!(
        decoded.bids.len(),
        fixture.expected.len(),
        "classification must not mask allocation"
    );
    let (media, _) = msft_response_allocation_media(case.ad_type);
    for (typed, expected) in decoded.bids.iter().zip(&fixture.expected) {
        assert_eq!(typed.media_type, media);
        let prebid = typed.bid.ext.as_ref().unwrap().prebid.as_ref().unwrap();
        assert_eq!(
            prebid.dealpriority,
            (case.priority != 0).then_some(case.priority)
        );
        assert!(!prebid.other.contains_key("dealpriority"));
        assert_eq!(
            prebid.other["video"],
            json!({"duration":case.duration,"primary_category":""})
        );
        assert_eq!(&serde_json::to_value(&typed.bid).unwrap(), expected);
    }
}

fn msft_response_allocation_probe(
    context: &MsftResponseAllocationContext,
    fixture: &MsftResponseAllocationFixture,
    case: MsftResponseAllocationCase,
) {
    // Fixtures, literal expectations, coverage and admission checks already
    // exist. This window includes the ENTIRE sealed decode/classify/project.
    let parse_baseline = alloc_track::reset_peak();
    let result = context
        .bidder
        .make_bids(&context.request, &context.sent, &fixture.response);
    let parse_peak = alloc_track::peak_since(parse_baseline);

    let decoded = result.unwrap();
    let byte_charge = fixture
        .wire_bytes
        .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
        .unwrap();
    assert!(parse_peak > 0);
    assert!(
        parse_peak <= byte_charge,
        "{case:?} wire={} nodes={} peak={parse_peak} byte_charge={byte_charge}",
        fixture.wire_bytes,
        fixture.nodes
    );
    assert!(parse_peak <= response_charge(fixture.wire_bytes));
    msft_assert_response_allocation_decoded(&decoded, fixture, case);
}

fn msft_response_allocation_grid_case(
    context: &MsftResponseAllocationContext,
    case: MsftResponseAllocationCase,
) {
    let source = (0..case.count)
        .map(|index| msft_response_allocation_source_bid(case, index))
        .collect::<Vec<_>>();
    let expected = source
        .iter()
        .map(|bid| msft_response_allocation_expected_bid(bid, case.priority, case.duration))
        .collect::<Vec<_>>();
    let fixture = msft_response_allocation_fixture(&source, expected, Vec::new());
    assert_eq!(fixture.expected.len(), case.count);
    msft_response_allocation_probe(context, &fixture, case);
}

#[test]
fn msft_signed_priority_video_and_category_projection_fit_unchanged_response_funding() {
    thread::spawn(|| {
        let context = msft_response_allocation_context();
        let mut cases = 0_usize;
        // Full independent Cartesian grid, no excluded admitted corners.
        let counts = [1_usize, 32, 256];
        let ad_types = [0_i64, 1, 3];
        let priorities = [i64::MIN, 0, i64::MAX];
        let durations = [-3_i64, 0, 30];
        for count in counts {
            for ad_type in ad_types {
                for priority in priorities {
                    for duration in durations {
                        msft_response_allocation_grid_case(
                            &context,
                            MsftResponseAllocationCase {
                                count,
                                ad_type,
                                priority,
                                duration,
                            },
                        );
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(
            cases,
            counts.len() * ad_types.len() * priorities.len() * durations.len()
        );
    })
    .join()
    .unwrap();
}

#[test]
fn msft_bad_provider_fields_preserve_a_good_sibling_within_response_funding() {
    // Focused malformed-provider coverage, not another multiplier on the grid.
    // Reserved typed DTO failures have a different scope and are not used here.
    thread::spawn(|| {
        let context = msft_response_allocation_context();
        let case = MsftResponseAllocationCase {
            count: 2,
            ad_type: 1,
            priority: i64::MAX,
            duration: -3,
        };
        for (field, value) in [
            ("deal_priority", json!("7")),
            ("deal_priority", json!(u64::MAX)),
            ("bid_ad_type", json!(true)),
            ("brand_id", json!({})),
            ("brand_category_id", json!([])),
            ("creative_info", json!(false)),
        ] {
            let mut bad = msft_response_allocation_source_bid(case, 0);
            bad["ext"]["appnexus"][field] = value;
            let good = msft_response_allocation_source_bid(case, 1);
            let expected =
                msft_response_allocation_expected_bid(&good, case.priority, case.duration);
            let fixture = msft_response_allocation_fixture(
                &[bad, good],
                vec![expected],
                vec![auctioneer_bidder::BidderError::failed_to_unmarshal(
                    "invalid msft bid extension",
                )],
            );
            assert_eq!(fixture.expected[0]["id"], "b1");
            msft_response_allocation_probe(&context, &fixture, case);
        }
    })
    .join()
    .unwrap();
}
