//! Sealed-response budget controls parameterized by an owning factory and status contract.

#![cfg(test)]

use auctioneer_bidder::{
    Bidder, BidderError, BidderHttpResponse, BidderResponse, META_BYTES_CAPACITY,
    OutboundRequestMeta, OutboundRequestMetaBuilder,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;

const BAD_SERVER_RESPONSE: u32 = 4;
const FAILED_TO_UNMARSHAL: u32 = 14;
const TIGHT_BUDGET: u32 = 64;
const WIDE_BUDGET: u32 = 8_192;

#[derive(Clone, Copy)]
pub(crate) enum NoContentPolicy {
    SkipBody,
    ParseBody,
}

fn request() -> BidRequest {
    let mut impressions = vec![json!({"id": "i", "banner": {}})];
    impressions.extend((0_u32..8).map(|index| json!({"id": format!("i{index}"), "banner": {}})));
    serde_json::from_value(json!({"id": "request", "imp": impressions})).unwrap()
}

fn sent(endpoint: &str) -> OutboundRequestMeta {
    let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
    let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 100);
    builder.set_method("POST").unwrap();
    builder.set_url(endpoint).unwrap();
    builder.imp_id("i").unwrap();
    for index in 0_u32..8 {
        builder.imp_id(&format!("i{index}")).unwrap();
    }
    builder.finish()
}

fn response(status: u16, body: Vec<u8>, budget: u32) -> BidderHttpResponse {
    BidderHttpResponse::new(status, vec![], body).within_node_budget(budget)
}

fn structurally_large(bid_count: usize, width: usize) -> Vec<u8> {
    let bids = (0..bid_count)
        .map(|index| {
            let mut bid =
                json!({"id": format!("b{index}"), "impid": format!("i{index}"), "price": 1.5_f64, "mtype": 1_i32});
            for key in 0..width {
                bid.as_object_mut()
                    .unwrap()
                    .insert(format!("k{key}"), json!(key));
            }
            bid
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(
        &json!({"id": "r-1", "seatbid": [{"seat": "fake_s", "bid": bids}], "cur": "USD"}),
    )
    .unwrap()
}

fn deeply_nested(depth: usize) -> Vec<u8> {
    let mut chain = json!(1_i32);
    for _ in 0..depth {
        chain = json!({"a": chain});
    }
    serde_json::to_vec(&json!({
        "id": "r-1", "cur": "USD", "seatbid": [{"seat": "fake_s", "bid": [{
            "id": "b", "impid": "i", "price": 1.5_f64, "mtype": 1_i32, "deep": chain,
        }]}],
    }))
    .unwrap()
}

fn codes(errors: &[BidderError]) -> Vec<u32> {
    errors.iter().map(|error| error.code).collect()
}

fn assert_valid(decoded: &BidderResponse, expected_bids: usize) {
    assert_eq!(decoded.bids.len(), expected_bids);
    assert_eq!(decoded.currency, "USD");
    assert!(decoded.errors.is_empty());
    assert!(decoded.auction_configs.is_empty());
}

pub(crate) fn flattened_flood(bidder: &dyn Bidder, endpoint: &str) {
    let request = request();
    let sent = sent(endpoint);
    let body = structurally_large(8, 40);
    let wide = bidder
        .make_bids(&request, &sent, &response(200, body.clone(), WIDE_BUDGET))
        .unwrap();
    assert_valid(&wide, 8);
    for (index, bid) in wide.bids.iter().enumerate() {
        assert_eq!(bid.bid.id, format!("b{index}"));
        assert_eq!(bid.bid.impid, format!("i{index}"));
        assert_eq!(bid.bid.other["k39"], json!(39_i32));
    }
    let errors = bidder
        .make_bids(&request, &sent, &response(200, body, TIGHT_BUDGET))
        .unwrap_err();
    assert_eq!(codes(&errors), [BAD_SERVER_RESPONSE]);
}

pub(crate) fn nesting_chain(bidder: &dyn Bidder, endpoint: &str) {
    nesting_chain_at_depth(bidder, endpoint, 100);
}

pub(crate) fn candidate_nesting_chain(bidder: &dyn Bidder, endpoint: &str) {
    // Five envelope containers plus 59 chain objects reach the source limit exactly.
    let admitted = deeply_nested(59);
    assert_eq!(
        auctioneer_runtime::count_json_nodes(&admitted, WIDE_BUDGET).unwrap(),
        141
    );
    nesting_chain_at_depth(bidder, endpoint, 59);
    for depth in [60_usize, 100] {
        let body = deeply_nested(depth);
        assert!(body.len() < 1_024);
        let errors = bidder
            .make_bids(
                &request(),
                &sent(endpoint),
                &response(200, body, WIDE_BUDGET),
            )
            .unwrap_err();
        assert_eq!(codes(&errors), [BAD_SERVER_RESPONSE], "depth {depth}");
    }
}

fn nesting_chain_at_depth(bidder: &dyn Bidder, endpoint: &str, depth: usize) {
    let request = request();
    let sent = sent(endpoint);
    let body = deeply_nested(depth);
    assert!(
        body.len() < 1_024,
        "a byte cap must admit this nesting fixture; got {} bytes",
        body.len()
    );
    let wide = bidder
        .make_bids(&request, &sent, &response(200, body.clone(), WIDE_BUDGET))
        .unwrap();
    assert_valid(&wide, 1);
    assert_eq!(wide.bids[0].bid.id, "b");
    assert_eq!(wide.bids[0].bid.impid, "i");
    let mut chain = &wide.bids[0].bid.other["deep"];
    for _ in 0..depth {
        chain = &chain["a"];
    }
    assert_eq!(chain, &json!(1_i32));
    let errors = bidder
        .make_bids(&request, &sent, &response(200, body, TIGHT_BUDGET))
        .unwrap_err();
    assert_eq!(codes(&errors), [BAD_SERVER_RESPONSE]);
}

pub(crate) fn unbudgeted_response(bidder: &dyn Bidder, endpoint: &str) {
    let request = request();
    let sent = sent(endpoint);
    let body = structurally_large(1, 0);
    let unbudgeted = BidderHttpResponse::new(200, vec![], body.clone());
    assert_eq!(unbudgeted.max_parsed_nodes(), 0);
    let errors = bidder.make_bids(&request, &sent, &unbudgeted).unwrap_err();
    assert_eq!(codes(&errors), [BAD_SERVER_RESPONSE]);
    let budgeted = bidder
        .make_bids(&request, &sent, &response(200, body, TIGHT_BUDGET))
        .unwrap();
    assert_valid(&budgeted, 1);
}

pub(crate) fn within_budget(bidder: &dyn Bidder, endpoint: &str) {
    let decoded = bidder
        .make_bids(
            &request(),
            &sent(endpoint),
            &response(200, structurally_large(1, 0), TIGHT_BUDGET),
        )
        .unwrap();
    assert_valid(&decoded, 1);
    assert_eq!(decoded.bids[0].bid.id, "b0");
    assert_eq!(decoded.bids[0].bid.impid, "i0");
    assert_eq!(decoded.bids[0].bid.price.to_bits(), 1.5_f64.to_bits());
}

pub(crate) fn malformed_body(bidder: &dyn Bidder, endpoint: &str) {
    // Deliberately malformed bytes distinguish lexical failure (14) from structural refusal (4).
    let malformed = response(200, b"{not json".to_vec(), TIGHT_BUDGET);
    let errors = bidder
        .make_bids(&BidRequest::default(), &sent(endpoint), &malformed)
        .unwrap_err();
    assert_eq!(codes(&errors), [FAILED_TO_UNMARSHAL]);
}

pub(crate) fn early_status(bidder: &dyn Bidder, endpoint: &str, no_content: NoContentPolicy) {
    let empty_request = BidRequest::default();
    let sent = sent(endpoint);
    for (label, status) in [("204 no content", 204_u16), ("empty body", 200)] {
        let decoded = bidder
            .make_bids(&empty_request, &sent, &response(status, vec![], 0))
            .unwrap_or_else(|errors| panic!("{label} must return before parsing: {errors:?}"));
        assert_valid(&decoded, 0);
    }
    // Invalid bytes plus a zero node budget ensure status refusal precedes either parser error.
    let errors = bidder
        .make_bids(&empty_request, &sent, &response(500, b"oops".to_vec(), 0))
        .unwrap_err();
    assert_eq!(codes(&errors), [BAD_SERVER_RESPONSE]);
    assert_eq!(errors[0].message, "status=500");

    let request = self::request();
    let body = structurally_large(1, 0);
    let unbudgeted = bidder.make_bids(&request, &sent, &response(204, body.clone(), 0));
    let budgeted = bidder
        .make_bids(&request, &sent, &response(204, body, TIGHT_BUDGET))
        .unwrap();
    match no_content {
        NoContentPolicy::SkipBody => {
            assert_valid(&unbudgeted.unwrap(), 0);
            assert_valid(&budgeted, 0);
        }
        NoContentPolicy::ParseBody => {
            assert_eq!(codes(&unbudgeted.unwrap_err()), [BAD_SERVER_RESPONSE]);
            assert_valid(&budgeted, 1);
        }
    }
}

macro_rules! response_budget_tests {
    ($factory:path, $endpoint:literal, $no_content:expr) => {
        pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 6;

        #[test]
        fn flattened_catch_all_flood_is_refused() {
            $crate::response_budget::flattened_flood($factory().as_ref(), $endpoint);
        }

        #[test]
        fn nesting_chain_under_the_catch_all_is_refused() {
            $crate::response_budget::nesting_chain($factory().as_ref(), $endpoint);
        }

        #[test]
        fn an_unbudgeted_response_refuses_every_body() {
            $crate::response_budget::unbudgeted_response($factory().as_ref(), $endpoint);
        }

        #[test]
        fn a_within_budget_response_still_decodes() {
            $crate::response_budget::within_budget($factory().as_ref(), $endpoint);
        }

        #[test]
        fn a_malformed_body_still_reports_failed_to_unmarshal() {
            $crate::response_budget::malformed_body($factory().as_ref(), $endpoint);
        }

        #[test]
        fn the_pre_parse_early_exits_are_unchanged() {
            $crate::response_budget::early_status($factory().as_ref(), $endpoint, $no_content);
        }
    };
}

pub(crate) use response_budget_tests;
