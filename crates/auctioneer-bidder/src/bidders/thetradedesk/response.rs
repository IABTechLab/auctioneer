use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors},
    status::{debug_status_error, standard_status},
};
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
use serde_json::Value;
use std::mem;

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), debug_status_error)? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    let currency = mem::take(&mut parsed.cur);
    let mut decoded = classify_response_with_errors(parsed, parse_errors, |bid| {
        let media = match bid.other.get("mtype").and_then(Value::as_u64) {
            Some(1) => MediaType::Banner,
            Some(2) => MediaType::Video,
            Some(4) => MediaType::Native,
            _ => return Err(unsupported_mtype()),
        };
        if !sent.covered_imp_ids().any(|id| id == bid.impid) {
            return Err(BidderError::bad_server_response(
                "bid response references an unsent impression",
            ));
        }
        for name in ["nurl", "burl"] {
            if bid
                .other
                .get(name)
                .is_some_and(|value| !value.is_null() && !value.is_string())
            {
                return Err(invalid_notification());
            }
        }
        classify_bid(request, bid, Some(media))
    })?;
    // Source overwrites its USD default even when cur is absent or empty.
    decoded.currency = currency;
    for typed in &mut decoded.bids {
        replace_price_macros(&mut typed.bid);
    }
    Ok(decoded)
}

fn unsupported_mtype() -> BidderError {
    BidderError::bad_server_response("unsupported thetradedesk mtype")
}

fn invalid_notification() -> BidderError {
    BidderError::bad_server_response("invalid thetradedesk notification field")
}

fn replace_price_macros(bid: &mut Bid) {
    const MACRO: &str = "${AUCTION_PRICE}";
    let price = bid.price.to_string();
    if let Some(adm) = &mut bid.adm
        && adm.contains(MACRO)
    {
        *adm = adm.replace(MACRO, &price);
    }
    for name in ["nurl", "burl"] {
        if let Some(Value::String(value)) = bid.other.get_mut(name)
            && value.contains(MACRO)
        {
            *value = value.replace(MACRO, &price);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::TheTradeDeskBidder;
    use super::super::tests::{calls, minimal, request};
    use super::{invalid_notification, unsupported_mtype};
    use crate::bidders::harness;
    use crate::{
        Bidder as _, BidderError, BidderErrorKind, BidderHttpRequest, BidderHttpResponse,
        BidderResponse,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    fn response_fixture(
        request: &BidRequest,
        document: &Value,
    ) -> (BidderHttpRequest, BidderHttpResponse) {
        let mut calls = calls(request);
        assert_eq!(calls.len(), 1);
        (
            calls.remove(0),
            harness::budgeted_response(200, serde_json::to_vec(document).unwrap()),
        )
    }

    fn decode(request: &BidRequest, document: &Value) -> Result<BidderResponse, Vec<BidderError>> {
        let (sent, response) = response_fixture(request, document);
        TheTradeDeskBidder.make_bids(request, sent.meta(), &response)
    }

    fn request_with_unsent_sibling() -> BidRequest {
        request(json!([
            {"id":"i", "banner":{}, "ext":{"bidder":{
                "publisherId":"p", "supplySourceId":"s"}}},
            {"id":"not-covered", "banner":{}},
        ]))
    }

    #[test]
    fn mtype_only_accepts_banner_video_native_and_preserves_valid_siblings() {
        let request = request(json!([{"id":"i", "banner":{}, "video":{}, "native":{},
            "audio":{}, "ext":{"bidder":{"publisherId":"p", "supplySourceId":"s"}}}]));
        for invalid in [
            Value::Null,
            json!(0_i32),
            json!(3_i32),
            json!(5_i32),
            json!(-1_i32),
            json!("1"),
            json!(1.5_f64),
            json!({}),
        ] {
            let response = decode(
                &request,
                &json!({"id":"r", "cur":"EUR", "seatbid":[
                    {"seat":"first", "bid":[
                        {"id":"bad", "impid":"i", "price":1_i32, "mtype":invalid},
                        {"id":"banner", "impid":"i", "price":1_i32, "mtype":1_i32}]},
                    {"seat":"second", "bid":[
                        {"id":"video", "impid":"i", "price":1_i32, "mtype":2_i32},
                        {"id":"native", "impid":"i", "price":1_i32, "mtype":4_i32}]},
                ]}),
            )
            .unwrap();
            assert_eq!(response.currency, "EUR");
            assert_eq!(
                response
                    .bids
                    .iter()
                    .map(|bid| bid.bid.id.as_str())
                    .collect::<Vec<_>>(),
                ["banner", "video", "native"]
            );
            assert_eq!(
                response
                    .bids
                    .iter()
                    .map(|bid| bid.media_type)
                    .collect::<Vec<_>>(),
                [MediaType::Banner, MediaType::Video, MediaType::Native],
            );
            assert_eq!(response.errors, [unsupported_mtype()]);
        }
    }

    #[test]
    fn missing_mtype_does_not_fall_back_to_prebid_type_or_single_offered_media() {
        let errors = decode(
            &minimal(),
            &json!({"id":"r", "seatbid":[{"bid":[{
                "id":"b", "impid":"i", "price":1_i32, "ext":{"prebid":{"type":"banner"}},
            }]}]}),
        )
        .unwrap_err();
        assert_eq!(errors, [unsupported_mtype()]);
    }

    #[test]
    fn shared_sent_identity_offered_media_and_conflict_checks_remain_enforced() {
        let banner_request = minimal();
        for bad in [
            json!({"id":"bad", "impid":"unsent", "price":1_i32,"mtype":1_i32}),
            json!({"id":"bad", "impid":"i", "price":1_i32,"mtype":2_i32}),
            json!({"id":"bad", "impid":"i", "price":1_i32,"mtype":1_i32,
                "ext":{"prebid":{"type":"video"}}}),
        ] {
            let response = decode(
                &banner_request,
                &json!({"id":"r", "seatbid":[{"bid":[bad,
                {"id":"good", "impid":"i", "price":1_i32, "mtype":1_i32}]}]}),
            )
            .unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].bid.id, "good");
            assert_eq!(response.errors.len(), 1);
            assert_eq!(response.errors[0].code, 4);
        }
        let request = request_with_unsent_sibling();
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/bid/s", ["i"]);
        let response = harness::budgeted_response(
            200,
            serde_json::to_vec(&json!({
                "id":"r", "seatbid":[{"bid":[
                    {"id":"bad", "impid":"not-covered", "price":1_i32, "mtype":1_i32},
                    {"id":"good", "impid":"i", "price":1_i32, "mtype":1_i32},
                ]}],
            }))
            .unwrap(),
        );
        let decoded = TheTradeDeskBidder
            .make_bids(&request, &sent, &response)
            .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(
            decoded.errors[0].message,
            "bid response references an unsent impression"
        );
    }

    #[test]
    fn currency_including_empty_and_absent_is_not_replaced_by_shared_usd_default() {
        for currency in [None, Some(""), Some("EUR"), Some("USD")] {
            let mut document = json!({"id":"r", "seatbid":[{"bid":[]}]});
            if let Some(value) = currency {
                document["cur"] = json!(value);
            }
            let response = decode(&minimal(), &document).unwrap();
            assert_eq!(response.currency, currency.unwrap_or_default());
            assert!(response.bids.is_empty());
        }
    }

    #[test]
    fn every_exact_uppercase_price_macro_is_replaced_in_all_three_channels() {
        for (price, expected) in [
            (0.0_f64, "0"),
            (1.5_f64, "1.5"),
            (0.001_f64, "0.001"),
            (999.999_f64, "999.999"),
            (5.0_f64, "5"),
            (1e-20_f64, "0.00000000000000000001"),
        ] {
            let value = "${AUCTION_PRICE}/${AUCTION_PRICE}/${auction_price}/${Auction_Price}";
            let response = decode(
                &minimal(),
                &json!({"id":"r", "cur":"", "seatbid":[{"bid":[{
                    "id":"b", "impid":"i", "price":price, "mtype":1_i32,
                    "adm":value, "nurl":value, "burl":value, "lurl":value,
                }]}]}),
            )
            .unwrap();
            let bid = &response.bids[0].bid;
            let substituted =
                format!("{expected}/{expected}/${{auction_price}}/${{Auction_Price}}");
            assert_eq!(bid.adm.as_deref(), Some(substituted.as_str()));
            assert_eq!(bid.other["nurl"], substituted);
            assert_eq!(bid.other["burl"], substituted);
            assert_eq!(bid.other["lurl"], value);
            let wire = serde_json::to_value(bid).unwrap();
            assert_eq!(wire["nurl"], substituted);
            assert_eq!(wire["burl"], substituted);
            assert_eq!(response.currency, "");
        }
    }

    #[test]
    fn notification_nulls_and_empty_values_are_preserved_but_invalid_shapes_are_bid_local() {
        for channel in ["nurl", "burl"] {
            for value in [Value::Null, json!(""), json!(false), json!({})] {
                let mut candidate = json!({"id":"candidate", "impid":"i", "price":1_i32,
                    "mtype":1_i32});
                candidate[channel] = value.clone();
                let response = decode(
                    &minimal(),
                    &json!({"id":"r", "seatbid":[{"bid":[candidate,
                    {"id":"good", "impid":"i", "price":1_i32, "mtype":1_i32}]}]}),
                )
                .unwrap();
                if value.is_null() || value.is_string() {
                    assert_eq!(response.bids.len(), 2);
                    assert_eq!(response.bids[0].bid.other[channel], value);
                    assert!(response.errors.is_empty());
                } else {
                    assert_eq!(response.bids.len(), 1);
                    assert_eq!(response.bids[0].bid.id, "good");
                    assert_eq!(response.errors, [invalid_notification()]);
                }
            }
        }
    }

    #[test]
    fn statuses_are_classified_before_the_sealed_body_is_parsed() {
        let request = minimal();
        let sent = calls(&request);
        for status in [200_u16, 204, 400, 500, 201] {
            let wire = harness::budgeted_response(status, Vec::new());
            let decoded = TheTradeDeskBidder.make_bids(&request, sent[0].meta(), &wire);
            match status {
                204 => {
                    let response = decoded.unwrap();
                    assert!(response.bids.is_empty());
                    assert_eq!(response.currency, "USD");
                    assert!(response.errors.is_empty());
                }
                200 => assert_eq!(decoded.unwrap_err()[0].kind, BidderErrorKind::ResponseParse),
                400 => assert_eq!(decoded.unwrap_err()[0].code, 2),
                _ => assert_eq!(decoded.unwrap_err()[0].code, 4),
            }
        }
    }

    #[test]
    fn malformed_typed_bid_is_a_whole_response_error_unlike_semantic_rejection() {
        for (field, value) in [
            ("price", json!("not-a-number")),
            ("adm", json!({"not":"a-string"})),
            ("impid", json!([])),
        ] {
            let mut bad = json!({"id":"bad", "impid":"i", "price":1_i32, "mtype":1_i32});
            bad[field] = value;
            let errors = decode(
                &minimal(),
                &json!({"id":"r", "seatbid":[{"bid":[
                    {"id":"good", "impid":"i", "price":1_i32, "mtype":1_i32}, bad,
                ]}]}),
            )
            .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
            assert_eq!(errors[0].message, "invalid bidder response");
        }
    }

    #[test]
    fn wrong_shaped_envelopes_are_shared_parse_errors_and_nodes_are_bounded() {
        for document in [
            Value::Null,
            json!([]),
            json!(false),
            json!({}),
            json!({"id":"r", "seatbid":{}}),
            json!({"id":"r", "seatbid":[{"bid":null}]}),
        ] {
            let errors = decode(&minimal(), &document).unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        }
        let request = minimal();
        let sent = calls(&request);
        // One object, two keys, one string, one array: five syntactic nodes.
        let bytes = serde_json::to_vec(&json!({"id":"r", "seatbid":[]})).unwrap();
        for nodes in [4_u32, 5] {
            let response =
                BidderHttpResponse::new(200, Vec::new(), bytes.clone()).within_node_budget(nodes);
            let decoded = TheTradeDeskBidder.make_bids(&request, sent[0].meta(), &response);
            if nodes == 5 {
                assert!(decoded.unwrap().bids.is_empty());
            } else {
                let errors = decoded.unwrap_err();
                assert_eq!(errors[0].code, 4);
                assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
            }
        }
    }
}
