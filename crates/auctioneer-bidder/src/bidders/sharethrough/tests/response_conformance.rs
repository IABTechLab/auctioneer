//! Sealed-response controls using metadata from the actual split constructor.

use auctioneer_bidder::{
    Bidder, BidderError, BidderErrorKind, BidderHttpRequest, BidderHttpResponse, BidderName,
    BidderRequestInfo, BidderResponse, CallBudget, EndpointRejectReason, EndpointValidator,
    EntryPoint, OutboundRequestSink, SinkLimits,
};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::{Value, json};

pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 7;
const ENDPOINT: &str = "https://bidder.example.test/sharethrough";

struct AllowEndpoint;

impl EndpointValidator for AllowEndpoint {
    fn validate(
        &self,
        _url: &url::Url,
        _info: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        Ok(())
    }
}

fn bidder() -> Box<dyn Bidder> {
    let registration = auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|row| row.key == "sharethrough")
        .unwrap();
    (registration.factory)()
}

fn context(bidder: &dyn Bidder) -> (BidRequest, Vec<BidderHttpRequest>) {
    let request = serde_json::from_value(json!({"id":"r", "imp":[{
        "id":"i", "banner":{}, "video":{"mimes":["video/mp4"]},
        "native":{"request":"native payload"}, "audio":{"mimes":["audio/mp4"]},
        "ext":{"bidder":{"pkey":"placement"}}
    }]}))
    .unwrap();
    let info = BidderRequestInfo::new(
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
    );
    let budget = CallBudget::new(3);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &AllowEndpoint);
    bidder.make_requests(&request, &info, &mut sink).unwrap();
    let calls = sink.into_outcome().accepted;
    assert_eq!(calls.len(), 3);
    for call in &calls {
        assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
    }
    (request, calls)
}

fn document(extra_fields: Value) -> Value {
    let Value::Object(fields) = extra_fields else {
        panic!("bid fields must be an object");
    };
    let mut bid = json!({"id":"good", "impid":"i", "price":1.25_f64,"mtype":1_i32});
    bid.as_object_mut().unwrap().extend(fields);
    json!({"id":"r", "cur":"EUR", "seatbid":[{"bid":[bid]}]})
}

fn response(document: &Value, nodes: u32) -> BidderHttpResponse {
    BidderHttpResponse::new(200, vec![], serde_json::to_vec(document).unwrap())
        .within_node_budget(nodes)
}

fn assert_single(decoded: &BidderResponse, media: MediaType) {
    assert_eq!(decoded.currency, "USD");
    assert_eq!(decoded.bids.len(), 1);
    assert_eq!(decoded.bids[0].bid.id, "good");
    assert_eq!(decoded.bids[0].bid.price.to_bits(), 1.25_f64.to_bits());
    assert_eq!(decoded.bids[0].media_type, media);
    assert!(decoded.errors.is_empty());
    assert!(decoded.auction_configs.is_empty());
}

fn assert_parse_error(errors: &[BidderError], code: u32) {
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, code);
    assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
}

#[test]
fn flattened_unknown_field_flood_has_an_independent_admitted_positive_control() {
    let bidder = bidder();
    let (request, calls) = context(bidder.as_ref());
    let mut document = document(json!({}));
    for index in 0_usize..128 {
        document["seatbid"][0]["bid"][0]
            .as_object_mut()
            .unwrap()
            .insert(format!("unknown_{index}"), json!(index));
    }
    let wide = bidder
        .make_bids(&request, calls[0].meta(), &response(&document, 8192))
        .unwrap();
    assert_single(&wide, MediaType::Banner);
    assert_eq!(wide.bids[0].bid.other["unknown_127"], json!(127_i32));
    let errors = bidder
        .make_bids(&request, calls[0].meta(), &response(&document, 64))
        .unwrap_err();
    assert_parse_error(&errors, 4);
}

#[test]
fn deep_unknown_extension_chain_cannot_bypass_the_node_budget() {
    let bidder = bidder();
    let (request, calls) = context(bidder.as_ref());
    let mut chain = json!(true);
    for _ in 0_usize..100 {
        chain = json!({"next":chain});
    }
    let document = document(json!({"chain":chain}));
    assert!(serde_json::to_vec(&document).unwrap().len() < 2048);
    let wide = bidder
        .make_bids(&request, calls[0].meta(), &response(&document, 8192))
        .unwrap();
    assert_single(&wide, MediaType::Banner);
    let errors = bidder
        .make_bids(&request, calls[0].meta(), &response(&document, 64))
        .unwrap_err();
    assert_parse_error(&errors, 4);
}

#[test]
fn no_node_allowance_refuses_the_same_body_that_an_admitted_call_decodes() {
    let bidder = bidder();
    let (request, calls) = context(bidder.as_ref());
    let document = document(json!({}));
    let bytes = serde_json::to_vec(&document).unwrap();
    let unbudgeted = BidderHttpResponse::new(200, vec![], bytes);
    let errors = bidder
        .make_bids(&request, calls[0].meta(), &unbudgeted)
        .unwrap_err();
    assert_parse_error(&errors, 4);
    let admitted = bidder
        .make_bids(&request, calls[0].meta(), &response(&document, 64))
        .unwrap();
    assert_single(&admitted, MediaType::Banner);
}

#[test]
fn exact_same_id_split_calls_validate_media_independently_without_erasing_legacy_data() {
    let bidder = bidder();
    let (request, calls) = context(bidder.as_ref());
    for ((call, code), media) in calls.iter().zip([1_i32, 2_i32, 4_i32]).zip([
        MediaType::Banner,
        MediaType::Video,
        MediaType::Native,
    ]) {
        let document = document(json!({"mtype":code,"ext":{"prebid":{"type":"legacy-other"}}}));
        let admitted = bidder
            .make_bids(&request, call.meta(), &response(&document, 128))
            .unwrap();
        assert_single(&admitted, media);
        assert_eq!(
            admitted.bids[0]
                .bid
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .bid_type
                .as_deref(),
            Some("legacy-other")
        );
        let mut wrong = document;
        wrong["seatbid"][0]["bid"][0]["mtype"] = json!(if code == 1_i32 { 2_i32 } else { 1_i32 });
        let errors = bidder
            .make_bids(&request, call.meta(), &response(&wrong, 128))
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::Other);
    }
}

#[test]
fn mixed_invalid_standard_media_preserves_a_valid_sibling_from_another_seat() {
    let bidder = bidder();
    let (request, calls) = context(bidder.as_ref());
    let document = json!({"id":"r","cur":"EUR","seatbid":[{"bid":[
        {"id":"null","impid":"i","price":1.0_f64,"mtype":null,"ext":{"prebid":{"type":"banner"}}},
        {"id":"audio","impid":"i","price":1.0_f64,"mtype":3_i32},
        {"id":"unsent","impid":"other","price":1.0_f64,"mtype":1_i32}
    ]},{"bid":[{"id":"good","impid":"i","price":1.25_f64,"mtype":1_i32}]}]});
    let decoded = bidder
        .make_bids(&request, calls[0].meta(), &response(&document, 512))
        .unwrap();
    assert_eq!(decoded.bids.len(), 1);
    assert_eq!(decoded.bids[0].bid.id, "good");
    assert_eq!(decoded.currency, "USD");
    assert_eq!(decoded.errors.len(), 3);
    assert!(
        decoded
            .errors
            .iter()
            .all(|error| error.code == 4 && error.kind == BidderErrorKind::Other)
    );
}

#[test]
fn status_handling_precedes_sealed_parsing_but_success_still_requires_it() {
    let bidder = bidder();
    let (request, calls) = context(bidder.as_ref());
    for status in [0_u16, 200, 204, 400, 500] {
        let response = BidderHttpResponse::new(status, vec![], b"private malformed body".to_vec())
            .within_node_budget(0);
        let result = bidder.make_bids(&request, calls[0].meta(), &response);
        if status == 204 {
            let decoded = result.unwrap();
            assert_eq!(decoded.currency, "USD");
            assert!(decoded.bids.is_empty());
            assert!(decoded.errors.is_empty());
        } else {
            let errors = result.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
            assert_eq!(
                errors[0].kind,
                if status == 200 {
                    BidderErrorKind::ResponseParse
                } else {
                    BidderErrorKind::Other
                }
            );
            assert!(!errors[0].message.contains("private malformed body"));
        }
    }
    let malformed =
        BidderHttpResponse::new(200, vec![], b"not JSON".to_vec()).within_node_budget(64);
    let errors = bidder
        .make_bids(&request, calls[0].meta(), &malformed)
        .unwrap_err();
    assert_parse_error(&errors, 14);
}

#[test]
fn eager_typed_extension_decode_remains_a_named_whole_response_boundary() {
    let bidder = bidder();
    let (request, calls) = context(bidder.as_ref());
    let document = json!({"id":"r", "seatbid":[{"bid":[
        {"id":"good","impid":"i","price":1.25_f64,"mtype":1_i32},
        {"id":"bad-type-shape","impid":"i","price":1.0_f64,"mtype":1_i32,
            "ext":{"prebid":{"type":7_i32}}}
    ]}]});
    let errors = bidder
        .make_bids(&request, calls[0].meta(), &response(&document, 512))
        .unwrap_err();
    assert_parse_error(&errors, 14);
}
