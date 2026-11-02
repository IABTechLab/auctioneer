use super::request::unique_field;
use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors},
    status::standard_status,
};
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
use serde_json::Value;

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        let message =
            format!("Unexpected status code: {status}. Run with request.debug = 1 for more info");
        if status == 400 {
            BidderError::bad_input(message)
        } else {
            BidderError::unknown(message)
        }
    })? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    // This protocol deliberately ignores the valid wire currency.
    parsed.cur.clear();
    classify_response_with_errors(parsed, parse_errors, |bid| {
        if !sent.covered_imp_ids().any(|id| id == bid.impid) {
            return Err(BidderError::bad_server_response(
                "bid response references an unsent impression",
            ));
        }
        classify_bid(request, bid, Some(media_hint(bid)?))
    })
}

fn media_hint(bid: &Bid) -> Result<MediaType, BidderError> {
    let ambiguous =
        |()| BidderError::bad_server_response("triplelift bid contains ambiguous media keys");
    let namespace = match &bid.ext {
        None => None,
        Some(ext) => unique_field(&ext.other, "triplelift_pb").map_err(ambiguous)?,
    };
    let format = match namespace {
        None | Some(Value::Null) => 0,
        Some(Value::Object(fields)) => match unique_field(fields, "format").map_err(ambiguous)? {
            None | Some(Value::Null) => 0,
            Some(value) => value.as_i64().ok_or_else(|| {
                BidderError::bad_server_response("triplelift format must be a signed integer")
            })?,
        },
        Some(_) => {
            return Err(BidderError::bad_server_response(
                "bid.ext.triplelift_pb must be an object",
            ));
        }
    };
    Ok(if matches!(format, 11 | 12 | 17) {
        MediaType::Video
    } else {
        MediaType::Banner
    })
}

#[cfg(test)]
mod tests {
    use super::super::TripleliftBidder;
    use crate::bidders::harness;
    use crate::{
        Bidder as _, BidderError, BidderErrorKind, BidderName, BidderRequestInfo, EntryPoint,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("triplelift-alias"),
            BidderName::new("triplelift"),
            "account",
            None,
            "https://bidder.example.test/auction?sra=1&supplier_id=20",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn request(impressions: &Value) -> BidRequest {
        serde_json::from_value(json!({"id":"r", "imp":impressions, "site":{"id":"publisher"}, "ext":{"keep":{"nested":true}}, "custom":[true,null]})).unwrap()
    }

    fn minimal() -> BidRequest {
        request(&json!([{"id":"i", "banner":{}, "ext":{"bidder":{"inventoryCode":"slot"}}}]))
    }

    fn bid(fields: Value) -> Result<crate::BidderResponse, Vec<BidderError>> {
        harness::decode_bid(
            &TripleliftBidder,
            json!([{"id":"i", "banner":{}, "video":{}, "native":{}}]),
            fields,
        )
    }

    #[test]
    fn no_content_ignores_its_body_and_other_http_status_codes_remain_source_specific() {
        let request = minimal();
        let sent = harness::sent_meta("https://example.test/");
        let no_bid = TripleliftBidder
            .make_bids(
                &request,
                &sent,
                &harness::budgeted_response(204, b"not parsed".to_vec()),
            )
            .unwrap();
        assert!(no_bid.bids.is_empty());
        assert!(no_bid.errors.is_empty());
        assert_eq!(no_bid.currency, "USD");
        for status in [201_u16, 302, 400, 429, 500] {
            let errors = TripleliftBidder
                .make_bids(
                    &request,
                    &sent,
                    &harness::budgeted_response(status, b"private body".to_vec()),
                )
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, if status == 400 { 2 } else { 999 });
            assert_eq!(
                errors[0].message,
                format!(
                    "Unexpected status code: {status}. Run with request.debug = 1 for more info"
                )
            );
        }
    }

    #[test]
    fn sealed_envelope_and_node_failures_are_not_media_classification_errors() {
        let request = minimal();
        let sent = harness::sent_meta("https://example.test/");
        for body in [
            Vec::new(),
            b"private marker".to_vec(),
            serde_json::to_vec(&json!([])).unwrap(),
        ] {
            let errors = TripleliftBidder
                .make_bids(&request, &sent, &harness::budgeted_response(200, body))
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
            assert_eq!(errors[0].message, "invalid bidder response");
        }
        let response = harness::budgeted_response(
            200,
            serde_json::to_vec(&json!({"id":"r","seatbid":[]})).unwrap(),
        )
        .within_node_budget(1);
        let errors = TripleliftBidder
            .make_bids(&request, &sent, &response)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
    }

    #[test]
    fn only_nested_valid_format_codes_select_the_source_media_type() {
        for (format, expected) in [
            (11_i64, MediaType::Video),
            (12, MediaType::Video),
            (17, MediaType::Video),
            (0, MediaType::Banner),
            (-1, MediaType::Banner),
            (18, MediaType::Banner),
            (i64::MAX, MediaType::Banner),
        ] {
            let response = bid(json!({"ext":{"triplelift_pb":{"format":format}}})).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].media_type, expected);
            assert!(response.errors.is_empty());
        }
        for fields in [
            json!({}),
            json!({"ext":null}),
            json!({"ext":{}}),
            json!({"ext":{"triplelift_pb":null}}),
            json!({"ext":{"triplelift_pb":{"format":null}}}),
            json!({"ext":{"format":11_i32}}),
        ] {
            assert_eq!(bid(fields).unwrap().bids[0].media_type, MediaType::Banner);
        }
    }

    #[test]
    fn single_case_insensitive_namespace_and_format_keys_select_video() {
        for namespace in ["triplelift_pb", "Triplelift_Pb", "TRIPLELIFT_PB"] {
            for format in ["format", "Format", "FORMAT"] {
                let response = bid(json!({"ext":{namespace:{format:11_i32}}})).unwrap();
                assert_eq!(response.bids.len(), 1);
                assert_eq!(response.bids[0].media_type, MediaType::Video);
                assert!(response.errors.is_empty());
            }
        }
        let response = bid(json!({"ext":{"tr\u{0130}plelift_pb":{"format":11_i32}}})).unwrap();
        assert_eq!(response.bids.len(), 1);
        assert_eq!(response.bids[0].media_type, MediaType::Banner);
        assert!(response.errors.is_empty());
    }

    #[test]
    fn ambiguous_namespace_or_format_keys_reject_only_the_affected_bid() {
        for ext in [
            json!({"triplelift_pb":{"format":11_i32}, "TRIPLELIFT_PB":{"format":0_i32}}),
            json!({"triplelift_pb":{"format":11_i32}, "Triplelift_Pb":{"format":11_i32}}),
            json!({"triplelift_pb":null, "Triplelift_Pb":{"format":11_i32}}),
            json!({"Triplelift_Pb":{"format":11_i32}, "TRIPLELIFT_PB":{"format":0_i32}}),
            json!({"triplelift_pb":{"format":11_i32, "FORMAT":0_i32}}),
            json!({"triplelift_pb":{"format":11_i32, "Format":11_i32}}),
            json!({"triplelift_pb":{"format":null, "Format":11_i32}}),
            json!({"triplelift_pb":{"Format":11_i32, "FORMAT":0_i32}}),
        ] {
            let response = harness::decode_bids(
                &TripleliftBidder,
                json!([{"id":"i", "banner":{}, "video":{}}]),
                json!([{"seat":"s", "bid":[
                    {"id":"ambiguous", "ext":ext},
                    {"id":"valid", "ext":{"triplelift_pb":{"format":11_i32}}},
                ]}]),
            )
            .unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].bid.id, "valid");
            assert_eq!(response.bids[0].media_type, MediaType::Video);
            assert_eq!(
                response.errors,
                [BidderError::bad_server_response(
                    "triplelift bid contains ambiguous media keys"
                )]
            );
        }
    }

    #[test]
    fn local_format_failures_preserve_good_bids_across_seats() {
        for ext in [
            json!({"triplelift_pb":false}),
            json!({"triplelift_pb":{"format":"11"}}),
            json!({"triplelift_pb":{"format":11.0_f64}}),
            json!({"triplelift_pb":{"format":true}}),
        ] {
            let response = harness::decode_bids(&TripleliftBidder,json!([{"id":"i","banner":{},"video":{}}]),json!([
                {"seat":"bad","bid":[{"id":"invalid","ext":ext}]},
                {"seat":"good","bid":[{"id":"valid","ext":{"triplelift_pb":{"format":11_i32}}}]},
            ])).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].bid.id, "valid");
            assert_eq!(response.bids[0].media_type, MediaType::Video);
            assert_eq!(response.errors.len(), 1);
            assert_eq!(response.errors[0].code, 4);
        }
        let errors = bid(json!({"ext":{"triplelift_pb":false}})).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        let envelope_errors = bid(json!({"ext":"invalid root"})).unwrap_err();
        assert_eq!(envelope_errors[0].code, 14);
    }

    #[test]
    fn contradictory_unknown_unoffered_and_unsent_bids_cannot_be_restored() {
        let unoffered = harness::decode_bid(
            &TripleliftBidder,
            json!([{"id":"i","banner":{}}]),
            json!({"ext":{"triplelift_pb":{"format":11_i32}}}),
        )
        .unwrap_err();
        assert_eq!(
            unoffered,
            [BidderError::bad_server_response(
                "bid response media type was not offered by its impression"
            )]
        );
        for fields in [
            json!({"mtype":2_i32}),
            json!({"impid":"unknown"}),
            json!({"mtype":4_i32}),
        ] {
            assert_eq!(bid(fields).unwrap_err()[0].code, 4);
        }
        let request = request(&json!([
            {"id":"filtered","banner":{},"ext":{"bidder":false}},
            {"id":"sent","banner":{},"ext":{"bidder":{"inventoryCode":"slot"}}},
        ]));
        let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
        let body = serde_json::to_vec(&json!({"id":"r","seatbid":[{"seat":"s","bid":[
            {"id":"unsent","impid":"filtered","price":1.0_f64,"ext":{}},
            {"id":"valid","impid":"sent","price":1.0_f64,"ext":{}},
        ]}]}))
        .unwrap();
        let response = TripleliftBidder
            .make_bids(
                &request,
                calls[0].meta(),
                &harness::budgeted_response(200, body),
            )
            .unwrap();
        assert_eq!(response.bids.len(), 1);
        assert_eq!(response.bids[0].bid.id, "valid");
        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].code, 4);
    }

    #[test]
    fn empty_sent_coverage_cannot_authorize_an_otherwise_valid_bid() {
        let response = harness::budgeted_response(
            200,
            serde_json::to_vec(&json!({"id":"r", "seatbid":[{"seat":"s", "bid":[
                {"id":"valid", "impid":"i", "price":1.0_f64, "ext":{}},
            ]}]}))
            .unwrap(),
        );
        let errors = TripleliftBidder
            .make_bids(
                &minimal(),
                &harness::sent_meta("https://example.test/"),
                &response,
            )
            .unwrap_err();
        assert_eq!(
            errors,
            [BidderError::bad_server_response(
                "bid response references an unsent impression"
            )]
        );
    }

    #[test]
    fn response_currency_is_always_usd_even_when_the_wire_declares_otherwise() {
        for currency in [None, Some(""), Some("USD"), Some("EUR")] {
            let mut body = json!({"id":"r","seatbid":[]});
            if let Some(value) = currency {
                body["cur"] = json!(value);
            }
            let response = TripleliftBidder
                .make_bids(
                    &minimal(),
                    &harness::sent_meta("https://example.test/"),
                    &harness::budgeted_response(200, serde_json::to_vec(&body).unwrap()),
                )
                .unwrap();
            assert_eq!(response.currency, "USD");
            assert!(response.bids.is_empty());
            assert!(response.errors.is_empty());
        }
    }

    #[test]
    fn non_usd_currency_does_not_discard_a_valid_bid_or_rescue_an_invalid_sibling() {
        let request = request(&json!([{"id":"i", "banner":{}, "video":{},
            "ext":{"bidder":{"inventoryCode":"slot"}}}]));
        let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
        let response = TripleliftBidder
            .make_bids(
                &request,
                calls[0].meta(),
                &harness::budgeted_response(
                    200,
                    serde_json::to_vec(&json!({"id":"r", "cur":"EUR", "seatbid":[
                        {"seat":"bad", "bid":[{"id":"invalid", "impid":"i", "price":9.0_f64,
                            "ext":{"triplelift_pb":{"format":false}}}]},
                        {"seat":"good", "bid":[{"id":"valid", "impid":"i", "price":1.5_f64,
                            "ext":{"triplelift_pb":{"format":11_i32}}}]},
                    ]}))
                    .unwrap(),
                ),
            )
            .unwrap();
        assert_eq!(response.currency, "USD");
        assert_eq!(response.bids.len(), 1);
        assert_eq!(response.bids[0].bid.id, "valid");
        assert_eq!(response.bids[0].bid.price.to_bits(), 1.5_f64.to_bits());
        assert_eq!(response.bids[0].media_type, MediaType::Video);
        assert_eq!(
            response.errors,
            [BidderError::bad_server_response(
                "triplelift format must be a signed integer"
            )]
        );
    }
}
