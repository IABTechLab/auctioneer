use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors},
    status::{debug_status_error, standard_status},
    wire_fields::{IntegerFieldError, signed_byte},
};
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
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
    // Source decodes mtype as int8 before any per-bid classification.
    for bid in parsed.seatbid.iter().flat_map(|seat| &seat.bid) {
        markup_type(bid).map_err(|error| vec![error])?;
    }
    let currency = mem::take(&mut parsed.cur);
    let mut decoded = classify_response_with_errors(parsed, parse_errors, |bid| {
        let media_type = match markup_type(bid)? {
            1 => MediaType::Banner,
            2 => MediaType::Video,
            4 => MediaType::Native,
            unsupported => {
                return Err(BidderError::unknown(format!(
                    "unsupported MType {unsupported}"
                )));
            }
        };
        if !sent.covered_imp_ids().any(|id| id == bid.impid) {
            return Err(BidderError::bad_server_response(
                "bid response references an unsent impression",
            ));
        }
        classify_bid(request, bid, Some(media_type))
    })?;
    // Rise overwrites the source USD default even when wire currency is empty.
    decoded.currency = currency;
    Ok(decoded)
}

fn markup_type(bid: &Bid) -> Result<i8, BidderError> {
    signed_byte(&bid.other, "mtype").map_err(|error| {
        BidderError::failed_to_unmarshal(match error {
            IntegerFieldError::Invalid => "invalid rise mtype",
            IntegerFieldError::Ambiguous => "rise response contains ambiguous mtype keys",
        })
    })
}

#[cfg(test)]
mod tests {
    use super::super::RiseBidder;
    use crate::bidders::harness;
    use crate::{Bidder as _, BidderError, BidderErrorKind};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    fn request(impressions: Value) -> BidRequest {
        let mut document = json!({
            "id":"request",
            "site":{"domain":"publisher.example"},
            "user":{"keywords":"already scoped"},
            "regs":{"ext":{"gdpr":1_i32}},
            "ext":{"preserved":{"flag":true}},
        });
        document["imp"] = impressions;
        serde_json::from_value(document).unwrap()
    }

    fn minimal() -> BidRequest {
        request(json!([{"id":"i", "banner":{}, "ext":{"bidder":{"org":"org"}}}]))
    }

    fn all_formats() -> Value {
        json!([{"id":"i", "banner":{}, "video":{}, "native":{}, "audio":{}}])
    }

    #[test]
    fn no_content_does_not_parse_a_body_and_other_status_codes_keep_rise_error_codes() {
        let request = minimal();
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/pbs", ["i"]);
        let decoded = RiseBidder
            .make_bids(
                &request,
                &sent,
                &harness::budgeted_response(204, b"not parsed".to_vec()),
            )
            .unwrap();
        assert!(decoded.bids.is_empty());
        assert!(decoded.errors.is_empty());
        for status in [201_u16, 302, 400, 429, 500] {
            let errors = RiseBidder
                .make_bids(
                    &request,
                    &sent,
                    &harness::budgeted_response(status, b"private marker".to_vec()),
                )
                .unwrap_err();
            let message = format!(
                "Unexpected status code: {status}. Run with request.debug = 1 for more info"
            );
            assert_eq!(
                errors,
                [if status == 400 {
                    BidderError::bad_input(message)
                } else {
                    BidderError::bad_server_response(message)
                }]
            );
        }
    }

    #[test]
    fn malformed_envelopes_and_node_budget_refusals_use_the_sealed_parser() {
        for body in [
            Vec::new(),
            b"private marker".to_vec(),
            serde_json::to_vec(&json!([])).unwrap(),
            serde_json::to_vec(&json!({"id":"r", "seatbid":false})).unwrap(),
        ] {
            let errors = RiseBidder
                .make_bids(
                    &minimal(),
                    &harness::sent_meta("https://example.test/"),
                    &harness::budgeted_response(200, body),
                )
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
            assert_eq!(errors[0].message, "invalid bidder response");
        }
        let response = harness::budgeted_response(
            200,
            serde_json::to_vec(&json!({"id":"r", "seatbid":[]})).unwrap(),
        )
        .within_node_budget(1);
        let errors = RiseBidder
            .make_bids(
                &minimal(),
                &harness::sent_meta("https://example.test/"),
                &response,
            )
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
    }

    #[test]
    fn only_mtype_one_two_and_four_classify_even_when_other_hints_are_absent() {
        for (code, expected) in [
            (1_i32, MediaType::Banner),
            (2_i32, MediaType::Video),
            (4_i32, MediaType::Native),
        ] {
            let decoded =
                harness::decode_bid(&RiseBidder, all_formats(), json!({"mtype":code})).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, expected);
            assert!(decoded.errors.is_empty());
        }
    }

    #[test]
    fn absent_null_audio_and_unknown_mtype_do_not_fall_back_to_offered_media_or_extension() {
        for (fields, value) in [
            (json!({}), 0_i64),
            (json!({"mtype":null}), 0),
            (json!({"mtype":0_i32}), 0),
            (json!({"mtype":3_i32}), 3),
            (json!({"mtype":5_i32}), 5),
            (json!({"mtype":-1_i32}), -1),
            (json!({"mtype":-128_i32}), -128),
            (json!({"mtype":127_i32}), 127),
            (json!({"ext":{"prebid":{"type":"banner"}}}), 0),
        ] {
            let errors = harness::decode_bid(&RiseBidder, all_formats(), fields).unwrap_err();
            assert_eq!(
                errors,
                [BidderError::unknown(format!("unsupported MType {value}"))]
            );
        }
    }

    #[test]
    fn invalid_media_bids_do_not_discard_valid_siblings_across_seats() {
        let decoded = harness::decode_bids(&RiseBidder, all_formats(), json!([
            {"seat":"first", "bid":[{"id":"missing"}, {"id":"banner", "mtype":1_i32}]},
            {"seat":"second", "bid":[{"id":"audio", "mtype":3_i32}, {"id":"video", "mtype":2_i32}]},
            {"seat":"third", "bid":[{"id":"native", "mtype":4_i32}]},
        ])).unwrap();
        assert_eq!(
            decoded
                .bids
                .iter()
                .map(|bid| (bid.bid.id.as_str(), bid.media_type))
                .collect::<Vec<_>>(),
            [
                ("banner", MediaType::Banner),
                ("video", MediaType::Video),
                ("native", MediaType::Native)
            ]
        );
        assert_eq!(
            decoded.errors,
            [
                BidderError::unknown("unsupported MType 0"),
                BidderError::unknown("unsupported MType 3")
            ]
        );
    }

    #[test]
    fn all_unsupported_media_returns_every_error_instead_of_silent_no_bid() {
        let errors = harness::decode_bids(
            &RiseBidder,
            all_formats(),
            json!([
                {"seat":"one", "bid":[{}, {"mtype":3_i32}]},
                {"seat":"two", "bid":[{"mtype":5_i32}]},
            ]),
        )
        .unwrap_err();
        assert_eq!(
            errors,
            [
                BidderError::unknown("unsupported MType 0"),
                BidderError::unknown("unsupported MType 3"),
                BidderError::unknown("unsupported MType 5")
            ]
        );
    }

    #[test]
    fn single_case_equivalent_mtype_keys_select_the_source_media() {
        for key in ["mtype", "Mtype", "MTYPE"] {
            let decoded =
                harness::decode_bid(&RiseBidder, all_formats(), json!({key:2_i32})).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, MediaType::Video);
            assert!(decoded.errors.is_empty());
        }
    }

    #[test]
    fn competing_mtype_spellings_have_decode_scope_even_with_equal_or_null_values() {
        for (left, right) in [("mtype", "Mtype"), ("Mtype", "MTYPE")] {
            for competitor in [json!(1_i32), json!(2_i32), Value::Null] {
                let errors = harness::decode_bids(
                    &RiseBidder,
                    all_formats(),
                    json!([
                        {"seat":"bad", "bid":[{"id":"ambiguous", left:1_i32, right:competitor}]},
                        {"seat":"good", "bid":[{"id":"valid", "mtype":2_i32}]},
                    ]),
                )
                .unwrap_err();
                assert_eq!(
                    errors,
                    [BidderError::failed_to_unmarshal(
                        "rise response contains ambiguous mtype keys"
                    )]
                );
            }
            let errors =
                harness::decode_bid(&RiseBidder, all_formats(), json!({left:null, right:null}))
                    .unwrap_err();
            assert_eq!(
                errors,
                [BidderError::failed_to_unmarshal(
                    "rise response contains ambiguous mtype keys"
                )]
            );
        }
    }

    #[test]
    fn malformed_recognized_mtype_has_source_response_decode_scope() {
        for key in ["mtype", "Mtype", "MTYPE"] {
            for value in [
                json!("private marker"),
                json!(true),
                json!(2.0_f64),
                json!([]),
                json!({}),
                json!(128_i32),
                json!(-129_i32),
                json!(u64::MAX),
            ] {
                let errors = harness::decode_bids(
                    &RiseBidder,
                    all_formats(),
                    json!([
                        {"seat":"bad", "bid":[{"id":"invalid", key:value}]},
                        {"seat":"good", "bid":[{"id":"valid", "mtype":2_i32}]},
                    ]),
                )
                .unwrap_err();
                assert_eq!(
                    errors,
                    [BidderError::failed_to_unmarshal("invalid rise mtype")]
                );
            }
        }
    }

    #[test]
    fn response_currency_including_empty_is_preserved_with_and_without_a_valid_bid() {
        for currency in [None, Some(""), Some("USD"), Some("EUR"), Some("usd")] {
            for with_bid in [false, true] {
                let mut body = json!({"id":"r", "seatbid":[]});
                if let Some(explicit_currency) = currency {
                    body["cur"] = json!(explicit_currency);
                }
                if with_bid {
                    body["seatbid"] = json!([{"seat":"s", "bid":[{"id":"b", "impid":"i", "price":1.25_f64, "mtype":1_i32}]}]);
                }
                let response = harness::budgeted_response(200, serde_json::to_vec(&body).unwrap());
                let decoded = RiseBidder
                    .make_bids(
                        &minimal(),
                        &harness::sent_meta_with_ids("https://example.test/", ["i"]),
                        &response,
                    )
                    .unwrap();
                assert_eq!(decoded.currency, currency.unwrap_or(""));
                assert_eq!(decoded.bids.len(), usize::from(with_bid));
                assert!(decoded.errors.is_empty());
                if with_bid {
                    assert_eq!(decoded.bids[0].bid.price.to_bits(), 1.25_f64.to_bits());
                }
            }
        }
    }

    #[test]
    fn shared_media_compatibility_does_not_authorize_unknown_unoffered_or_conflicting_bids() {
        for fields in [
            json!({"mtype":2_i32}),
            json!({"mtype":1_i32, "impid":"unknown"}),
            json!({"mtype":1_i32, "ext":{"prebid":{"type":"video"}}}),
        ] {
            let errors = harness::decode_bid(&RiseBidder, json!([{"id":"i", "banner":{}}]), fields)
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
    }

    #[test]
    fn sent_call_coverage_cannot_be_recovered_from_an_unsent_context_impression() {
        let request = request(json!([
            {"id":"unsent", "banner":{}, "ext":{"bidder":{"org":"same"}}},
            {"id":"sent", "banner":{}, "ext":{"bidder":{"org":"same"}}},
        ]));
        // Each sent-call context represents an independently delivered response.
        let body = serde_json::to_vec(
            &json!({"id":"r", "cur":"EUR", "seatbid":[{"seat":"s", "bid":[
                {"id":"unsent-bid", "impid":"unsent", "price":8.0_f64, "mtype":1_i32},
                {"id":"sent-bid", "impid":"sent", "price":1.0_f64, "mtype":1_i32},
            ]}]}),
        )
        .unwrap();
        let decoded = RiseBidder
            .make_bids(
                &request,
                &harness::sent_meta_with_ids("https://example.test/", ["sent"]),
                &harness::budgeted_response(200, body.clone()),
            )
            .unwrap();
        assert_eq!(decoded.currency, "EUR");
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "sent-bid");
        assert_eq!(
            decoded.errors,
            [BidderError::bad_server_response(
                "bid response references an unsent impression"
            )]
        );
        let errors = RiseBidder
            .make_bids(
                &request,
                &harness::sent_meta("https://example.test/"),
                &harness::budgeted_response(200, body),
            )
            .unwrap_err();
        assert_eq!(
            errors,
            vec![
                BidderError::bad_server_response("bid response references an unsent impression");
                2
            ]
        );
    }
}
