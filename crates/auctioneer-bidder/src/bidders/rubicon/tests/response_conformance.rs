//! Sealed Rubicon response controls with actual constructor metadata.

use auctioneer_bidder::{
    Bidder, BidderError, BidderErrorKind, BidderHttpRequest, BidderHttpResponse, BidderName,
    BidderRequestInfo, BidderResponse, CallBudget, EndpointRejectReason, EndpointValidator,
    EntryPoint, OutboundRequestSink, SinkLimits, compiled_bidders,
};
use auctioneer_config::{BidderSettings, types::MediaType};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::{bidder_credentials::BidderCredentials, bounded_json::count_json_nodes};
use serde_json::{Value, json};

pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 6;
const ENDPOINT: &str = "https://bidder.example.test/a/api/exchange";

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
    let registration = compiled_bidders()
        .iter()
        .find(|row| row.key == "rubicon")
        .unwrap();
    (registration.factory)()
}

fn context(bidder: &dyn Bidder) -> (BidRequest, BidderHttpRequest) {
    let request = serde_json::from_value(json!({"id":"r","site":{},"imp":[{
        "id":"i","native":{"ver":"1.2","request":json!({"eventtrackers":[],"plcmttype":2_i32}).to_string()},
        "ext":{"bidder":{"accountId":1001_i64,"siteId":113_932_i64,"zoneId":535_510_i64}}
    }]})).unwrap();
    let settings: BidderSettings =
        serde_json::from_value(json!({"implementation":"rubicon","values":{
            "credentials_secret":"fake_credential_bundle","external_url":"http://hosturl.com",
            "region":"us-east"
        }}))
        .unwrap();
    // These are upstream TestJsonSamples literals, not operator credentials.
    let credentials = BidderCredentials::parse(
        &serde_json::to_vec(&json!({
            "schema_version":1_u8,"username":"xuser","password":"xpass","tracker":"pbs-test-tracker"
        }))
        .unwrap(),
    )
    .unwrap();
    let mut info = BidderRequestInfo::new(
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
    info.set_settings(Some("rubicon"), Some(&settings), None)
        .unwrap();
    info.credentials = Some(&credentials);
    let budget = CallBudget::new(1);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &AllowEndpoint);
    bidder.make_requests(&request, &info, &mut sink).unwrap();
    let mut calls = sink.into_outcome().accepted;
    assert_eq!(calls.len(), 1);
    let call = calls.pop().unwrap();
    assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
    let body: Value = serde_json::from_slice(call.body()).unwrap();
    assert!(body["imp"][0].get("native").is_some());
    assert!(body["imp"][0].get("banner").is_none());
    (request, call)
}

fn native_bid(id: &str, creative: Value) -> Value {
    let mut bid = json!({"id":id,"impid":"i","price":1.25_f64,"mtype":4_i32});
    bid["adm_native"] = creative;
    bid
}

fn document() -> Value {
    json!({"id":"r","cur":"USD","bidid":"root","seatbid":[{
        "buyer":"123","seat":"rubicon","bid":[native_bid("0",json!({"property1":"creative"}))]
    }]})
}

fn response(document: &Value, nodes: u32) -> BidderHttpResponse {
    BidderHttpResponse::new(200, Vec::new(), serde_json::to_vec(document).unwrap())
        .within_node_budget(nodes)
}

fn assert_parse_error(errors: &[BidderError]) {
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.first().unwrap().code, 4);
    assert_eq!(errors.first().unwrap().kind, BidderErrorKind::ResponseParse);
}

fn assert_native(decoded: &BidderResponse, expected: &Value) {
    assert_eq!(decoded.currency, "USD");
    assert_eq!(decoded.bids.len(), 1);
    assert!(decoded.errors.is_empty());
    assert!(decoded.auction_configs.is_empty());
    let typed = decoded.bids.first().unwrap();
    assert_eq!(typed.media_type, MediaType::Native);
    assert_eq!(typed.bid.id, "root");
    assert_eq!(typed.bid.price.to_bits(), 1.25_f64.to_bits());
    assert_eq!(
        serde_json::from_str::<Value>(typed.bid.adm.as_deref().unwrap()).unwrap(),
        *expected
    );
    assert!(!typed.bid.other.contains_key("adm_native"));
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
    assert_eq!(metadata["networkId"], 123_i32);
    assert_eq!(metadata["seat"], "rubicon");
}

#[test]
fn exact_native_envelope_node_budget_admits_and_one_less_refuses_before_projection() {
    let bidder = bidder();
    let (request, call) = context(bidder.as_ref());
    let document = document();
    let bytes = serde_json::to_vec(&document).unwrap();
    // Independent count: root/scalars 7, seatbid key/array/object 3,
    // seat scalars 4, bid key/array/object 3, bid scalars 8, native member 4.
    assert_eq!(count_json_nodes(&bytes, u32::MAX).unwrap(), 29);
    let admitted = bidder
        .make_bids(&request, call.meta(), &response(&document, 29))
        .unwrap();
    assert_native(&admitted, &json!({"property1":"creative"}));
    let errors = bidder
        .make_bids(&request, call.meta(), &response(&document, 28))
        .unwrap_err();
    assert_parse_error(&errors);
}

#[test]
fn unknown_native_properties_are_charged_before_their_adm_string_is_retained() {
    let bidder = bidder();
    let (request, call) = context(bidder.as_ref());
    let mut creative = json!({"property1":"creative"});
    for index in 0_usize..64 {
        creative
            .as_object_mut()
            .unwrap()
            .insert(format!("field-{index}"), json!(true));
    }
    let mut input = document();
    input["seatbid"][0]["bid"][0]["adm_native"] = creative.clone();
    let bytes = serde_json::to_vec(&input).unwrap();
    assert_eq!(count_json_nodes(&bytes, u32::MAX).unwrap(), 157);
    let admitted = bidder
        .make_bids(&request, call.meta(), &response(&input, 256))
        .unwrap();
    assert_native(&admitted, &creative);
    let errors = bidder
        .make_bids(&request, call.meta(), &response(&input, 128))
        .unwrap_err();
    assert_parse_error(&errors);
}

#[derive(Clone, Copy, Debug)]
enum Replication {
    RootId,
    Seat,
    Combined,
}

fn replicated_document(mode: Replication, count: usize, width: usize) -> (Value, usize) {
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
    let creative = json!({"property1":"creative"});
    let per_bid = root_id
        .len()
        .checked_add(seat_id.len())
        .unwrap()
        .checked_add(creative.to_string().len())
        .unwrap();
    let bids = (0..count)
        .map(|index| {
            native_bid(
                &if root {
                    "0".to_owned()
                } else {
                    format!("b-{index}")
                },
                creative.clone(),
            )
        })
        .collect::<Vec<_>>();
    (
        json!({"id":"r","cur":"USD","bidid":root_id,"seatbid":[
            {"buyer":"123","seat":seat_id,"bid":bids},
            {"bid":[{"id":"valid","impid":"i","price":1.25_f64,"mtype":4_i32,"adm":"creative"}]}
        ]}),
        per_bid,
    )
}

#[test]
fn root_id_seat_and_native_replication_share_an_aggregate_body_sized_text_bound() {
    let bidder = bidder();
    let (request, call) = context(bidder.as_ref());
    let mut constrained = 0_usize;
    let mut cases = 0_usize;
    for mode in [
        Replication::RootId,
        Replication::Seat,
        Replication::Combined,
    ] {
        for count in [1_usize, 24] {
            let (input, per_bid) = replicated_document(mode, count, 256);
            let size = serde_json::to_vec(&input).unwrap().len();
            let accepted = count.min(size.checked_div(per_bid).unwrap());
            assert!(accepted > 0);
            let decoded = bidder
                .make_bids(&request, call.meta(), &response(&input, 2048))
                .unwrap();
            assert_eq!(
                decoded.bids.len(),
                accepted.checked_add(1).unwrap(),
                "{mode:?}"
            );
            assert_eq!(
                decoded.errors.len(),
                count.checked_sub(accepted).unwrap(),
                "{mode:?}"
            );
            assert!(decoded.errors.iter().all(|error| error.code == 4
                && error.message == "rubicon derived response exceeded limits"));
            assert_eq!(decoded.bids.last().unwrap().bid.id, "valid");
            assert!(accepted.checked_mul(per_bid).unwrap() <= size);
            if count == 1 {
                assert!(decoded.errors.is_empty());
            }
            if accepted < count {
                constrained = constrained.checked_add(1).unwrap();
            }
            cases = cases.checked_add(1).unwrap();
        }
    }
    assert_eq!(cases, 6);
    assert_eq!(constrained, 3);
}

#[test]
fn an_aggregate_native_refusal_refunds_its_root_copy_allowance_to_a_derived_sibling() {
    let bidder = bidder();
    let (request, call) = context(bidder.as_ref());
    let heavy = json!({"property1":"heavy"});
    let cheap = json!({"ok":true});
    let heavy_len = heavy.to_string().len();
    let cheap_len = cheap.to_string().len();
    let mut input = json!({"id":"r","cur":"USD","bidid":"","seatbid":[{"bid":[
        native_bid("0",heavy.clone()),native_bid("0",heavy),
        native_bid("cheap",cheap.clone())
    ]}]});
    let empty_root_bytes = serde_json::to_vec(&input).unwrap().len();
    let root_len = empty_root_bytes.checked_sub(heavy_len).unwrap();
    input["bidid"] = json!("r".repeat(root_len));
    let wire_bytes = serde_json::to_vec(&input).unwrap().len();
    let first_cost = root_len.checked_add(heavy_len).unwrap();
    assert_eq!(first_cost, empty_root_bytes);
    assert_eq!(wire_bytes.checked_sub(first_cost).unwrap(), root_len);
    assert!(cheap_len <= root_len);
    // After one heavy bid, the next root copy fits but its native markup
    // does not. Only a refunded copy allowance lets the derived sibling live.
    let decoded = bidder
        .make_bids(&request, call.meta(), &response(&input, 256))
        .unwrap();
    assert_eq!(decoded.bids.len(), 2);
    assert_eq!(decoded.errors.len(), 1);
    assert_eq!(
        decoded.errors.first().unwrap().message,
        "rubicon derived response exceeded limits"
    );
    assert_eq!(decoded.bids.first().unwrap().bid.id, "r".repeat(root_len));
    let last = decoded.bids.last().unwrap();
    assert_eq!(last.bid.id, "cheap");
    let expected_cheap = cheap.to_string();
    assert_eq!(last.bid.adm.as_deref(), Some(expected_cheap.as_str()));
}

#[test]
fn an_unattached_node_budget_cannot_project_the_same_admitted_native_value() {
    let bidder = bidder();
    let (request, call) = context(bidder.as_ref());
    let input = document();
    let raw = serde_json::to_vec(&input).unwrap();
    let unbudgeted = BidderHttpResponse::new(200, Vec::new(), raw.clone());
    let errors = bidder
        .make_bids(&request, call.meta(), &unbudgeted)
        .unwrap_err();
    assert_parse_error(&errors);
    let admitted = bidder
        .make_bids(&request, call.meta(), &response(&input, 64))
        .unwrap();
    assert_native(&admitted, &json!({"property1":"creative"}));
    let no_content = BidderHttpResponse::new(204, Vec::new(), raw);
    let empty = bidder
        .make_bids(&request, call.meta(), &no_content)
        .unwrap();
    assert!(empty.bids.is_empty());
    assert!(empty.errors.is_empty());
}

#[test]
fn invalid_standard_media_does_not_project_native_markup_or_discard_a_valid_seat() {
    let bidder = bidder();
    let (request, call) = context(bidder.as_ref());
    let input = json!({"id":"r","cur":"USD","bidid":"root","seatbid":[
        {"bid":[
            {"id":"audio","impid":"i","price":1.25_f64,"mtype":3_i32,"adm_native":{"not":"accepted"}},
            {"id":"null","impid":"i","price":1.25_f64,"mtype":null,"adm_native":{"not":"accepted"}}
        ]},
        {"buyer":"123","seat":"rubicon","bid":[native_bid("0",json!({"property1":"creative"}))]}
    ]});
    let decoded = bidder
        .make_bids(&request, call.meta(), &response(&input, 128))
        .unwrap();
    assert_eq!(decoded.errors.len(), 2);
    assert!(
        decoded
            .errors
            .iter()
            .all(|error| error.code == 4 && error.kind == BidderErrorKind::Other)
    );
    assert_eq!(decoded.bids.len(), 1);
    let bid = decoded.bids.first().unwrap();
    assert_eq!(bid.bid.id, "root");
    assert_eq!(bid.media_type, MediaType::Native);
    let expected_adm = json!({"property1":"creative"}).to_string();
    assert_eq!(bid.bid.adm.as_deref(), Some(expected_adm.as_str()));
    assert!(!bid.bid.other.contains_key("adm_native"));
}
