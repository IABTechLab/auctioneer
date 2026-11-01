//! Sealed-decoder conformance for this implementation's registered factory.

use crate::response_budget;
use auctioneer_bidder::{
    Bidder, BidderHttpResponse, META_BYTES_CAPACITY, OutboundRequestMetaBuilder, compiled_bidders,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;

pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 6;

fn bidder() -> Box<dyn Bidder> {
    let registration = compiled_bidders()
        .iter()
        .find(|row| row.key == "pubmatic")
        .expect("the owning factory must be linked under its declared feature");
    (registration.factory)()
}

const ENDPOINT: &str = "https://hbopenbid.pubmatic.com/translator?source=prebid-server";

#[test]
fn flattened_catch_all_flood_is_refused() {
    response_budget::flattened_flood(bidder().as_ref(), ENDPOINT);
}

#[test]
fn nesting_chain_under_the_catch_all_is_refused() {
    response_budget::candidate_nesting_chain(bidder().as_ref(), ENDPOINT);
}

#[test]
fn an_unbudgeted_response_refuses_every_body() {
    response_budget::unbudgeted_response(bidder().as_ref(), ENDPOINT);
}

#[test]
fn a_within_budget_response_still_decodes() {
    response_budget::within_budget(bidder().as_ref(), ENDPOINT);
}

#[test]
fn a_malformed_body_still_reports_failed_to_unmarshal() {
    response_budget::malformed_body(bidder().as_ref(), ENDPOINT);
}

#[test]
fn only_no_content_status_can_bypass_the_parse_budget() {
    let bidder = bidder();
    let request = BidRequest::default();
    let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
    let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
    builder.set_method("POST").unwrap();
    builder.set_url(ENDPOINT).unwrap();
    let sent = builder.finish();
    for status in [200_u16, 204, 400, 500] {
        let response =
            BidderHttpResponse::new(status, Vec::new(), serde_json::to_vec(&json!({})).unwrap())
                .within_node_budget(0);
        let result = bidder.make_bids(&request, &sent, &response);
        if status == 204 {
            let decoded = result.unwrap();
            assert!(decoded.bids.is_empty());
            assert!(decoded.errors.is_empty());
            assert!(decoded.auction_configs.is_empty());
        } else {
            let errors = result.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
        }
    }
}
