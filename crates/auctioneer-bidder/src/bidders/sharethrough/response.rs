use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors, sent_media_type},
    status::{bad_input_or_server, standard_status},
};
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        bad_input_or_server(status, "unexpected sharethrough response status")
    })? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    parsed.cur.clear();
    classify_response_with_errors(parsed, parse_errors, |bid| {
        classify_sharethrough_bid(request, sent, bid)
    })
}

fn classify_sharethrough_bid(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    bid: &Bid,
) -> Result<MediaType, BidderError> {
    let sent_media = sent_media_type(sent, &bid.impid)?;
    if !matches!(
        sent_media,
        MediaType::Banner | MediaType::Video | MediaType::Native
    ) {
        return Err(BidderError::bad_server_response(
            "unsupported sharethrough sent media",
        ));
    }
    // Standard media is authoritative; do not erase or copy the original legacy extension.
    if let Some(code) = bid.other.get("mtype") {
        let selected = match code.as_u64() {
            Some(1) => MediaType::Banner,
            Some(2) => MediaType::Video,
            Some(4) => MediaType::Native,
            _ => {
                return Err(BidderError::bad_server_response(
                    "invalid sharethrough response media",
                ));
            }
        };
        let imp = request
            .imp
            .iter()
            .find(|imp| imp.id == bid.impid)
            .ok_or_else(|| {
                BidderError::bad_server_response("bid response references an unknown impression")
            })?;
        let offered = match selected {
            MediaType::Banner => imp.banner.is_some(),
            MediaType::Video => imp.video.is_some(),
            MediaType::Native => imp.native.is_some(),
            MediaType::Audio => false,
        };
        return if offered && selected == sent_media {
            Ok(selected)
        } else {
            Err(BidderError::bad_server_response(
                "sharethrough response media does not match the sent call",
            ))
        };
    }
    if bid
        .ext
        .as_ref()
        .and_then(|ext| ext.prebid.as_ref())
        .and_then(|prebid| prebid.bid_type.as_deref())
        .is_none()
    {
        return Err(BidderError::bad_server_response(
            "sharethrough response lacks a declared media type",
        ));
    }
    classify_bid(request, bid, Some(sent_media))
}

#[cfg(test)]
mod tests {
    use super::super::SharethroughBidder;
    use super::super::tests::{ENDPOINT, banner, calls, mixed_request, request};
    use crate::bidders::harness;
    use crate::correlation::CorrelationMedia;
    use crate::{
        Bidder as _, BidderErrorKind, BidderHttpResponse, BidderResponse, META_BYTES_CAPACITY,
        OutboundRequestMeta, OutboundRequestMetaBuilder,
    };
    use auctioneer_config::types::MediaType;
    use serde_json::{Value, json};

    fn sent(media_option: Option<CorrelationMedia>, ids: &[&str]) -> OutboundRequestMeta {
        let mut bytes = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder =
            OutboundRequestMetaBuilder::new(&mut bytes, u16::try_from(ids.len()).unwrap());
        builder.set_method("POST").unwrap();
        builder.set_url(ENDPOINT).unwrap();
        for (ordinal, id) in ids.iter().enumerate() {
            builder.imp_id(id).unwrap();
            if let Some(media) = media_option {
                builder
                    .correlate_imp(u16::try_from(ordinal).unwrap(), media, 0)
                    .unwrap();
            }
        }
        builder.finish()
    }

    fn response(status: u16, document: &Value) -> BidderHttpResponse {
        harness::budgeted_response(status, serde_json::to_vec(document).unwrap())
    }

    fn one_bid(fields: Value) -> Value {
        let Value::Object(extra) = fields else {
            panic!("bid fields must be an object");
        };
        let mut bid = json!({"id":"good", "impid":"i", "price":1.25_f64});
        bid.as_object_mut().unwrap().extend(extra);
        json!({"id":"r", "cur":"EUR", "seatbid":[{"bid":[bid]}]})
    }

    fn decode(fields: Value, media: CorrelationMedia) -> BidderResponse {
        SharethroughBidder
            .make_bids(
                &mixed_request(),
                &sent(Some(media), &["i"]),
                &response(200, &one_bid(fields)),
            )
            .unwrap()
    }

    #[test]
    fn standard_media_codes_classify_all_three_supported_formats_and_force_usd() {
        for (code, correlation, media) in [
            (1_i32, CorrelationMedia::Banner, MediaType::Banner),
            (2_i32, CorrelationMedia::Video, MediaType::Video),
            (4_i32, CorrelationMedia::Native, MediaType::Native),
        ] {
            let decoded = decode(json!({"mtype":code}), correlation);
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, media);
            assert_eq!(decoded.currency, "USD");
            assert!(decoded.errors.is_empty());
        }
    }

    #[test]
    fn declared_legacy_type_fills_only_missing_standard_media() {
        for (name, correlation, media) in [
            ("banner", CorrelationMedia::Banner, MediaType::Banner),
            ("video", CorrelationMedia::Video, MediaType::Video),
            ("native", CorrelationMedia::Native, MediaType::Native),
        ] {
            let decoded = decode(json!({"ext":{"prebid":{"type":name}}}), correlation);
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, media);
            assert!(decoded.errors.is_empty());
        }
    }

    #[test]
    fn standard_media_takes_priority_over_a_conflicting_legacy_hint() {
        let decoded = decode(
            json!({"mtype":1_i32,"ext":{"prebid":{"type":"video"}}}),
            CorrelationMedia::Banner,
        );
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].media_type, MediaType::Banner);
        assert!(decoded.errors.is_empty());
        assert_eq!(
            decoded.bids[0]
                .bid
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .bid_type
                .as_deref(),
            Some("video")
        );
    }

    #[test]
    fn invalid_standard_media_never_falls_back_to_valid_legacy_type() {
        for invalid in [
            json!(null),
            json!(0_i32),
            json!(3_i32),
            json!(5_i32),
            json!(-1_i32),
            json!(1.5_f64),
            json!("1"),
            json!(true),
        ] {
            let errors = SharethroughBidder
                .make_bids(
                    &mixed_request(),
                    &sent(Some(CorrelationMedia::Banner), &["i"]),
                    &response(
                        200,
                        &one_bid(json!({"mtype":invalid,
                    "ext":{"prebid":{"type":"banner"}}})),
                    ),
                )
                .unwrap_err();
            harness::assert_bad_server_response_errors(&errors, 1);
        }
    }

    #[test]
    fn undeclared_or_unsupported_legacy_media_is_not_guessed_from_sent_media() {
        for fields in [
            json!({}),
            json!({"ext":{"prebid":{"type":"audio"}}}),
            json!({"ext":{"prebid":{"type":"Banner"}}}),
            json!({"ext":{"prebid":{"type":"unknown"}}}),
        ] {
            let errors = SharethroughBidder
                .make_bids(
                    &mixed_request(),
                    &sent(Some(CorrelationMedia::Banner), &["i"]),
                    &response(200, &one_bid(fields)),
                )
                .unwrap_err();
            harness::assert_bad_server_response_errors(&errors, 1);
        }
    }

    #[test]
    fn response_media_must_match_the_exact_split_call_not_other_offered_media() {
        let request = mixed_request();
        let calls = calls(&request);
        assert_eq!(calls.len(), 3);
        for (call, code) in calls.iter().zip([1_i32, 2_i32, 4_i32]) {
            let decoded = SharethroughBidder
                .make_bids(
                    &request,
                    call.meta(),
                    &response(200, &one_bid(json!({"mtype":code}))),
                )
                .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            let wrong_code = if code == 1_i32 { 2_i32 } else { 1_i32 };
            let errors = SharethroughBidder
                .make_bids(
                    &request,
                    call.meta(),
                    &response(200, &one_bid(json!({"mtype":wrong_code}))),
                )
                .unwrap_err();
            harness::assert_bad_server_response_errors(&errors, 1);
        }
    }

    #[test]
    fn unsent_impression_is_rejected_even_when_present_in_the_scoped_request() {
        let request = request(json!([banner("i"), banner("unsent")]));
        let errors = SharethroughBidder
            .make_bids(
                &request,
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &response(200, &one_bid(json!({"impid":"unsent","mtype":1_i32}))),
            )
            .unwrap_err();
        harness::assert_bad_server_response_errors(&errors, 1);
    }

    #[test]
    fn missing_call_correlation_and_empty_coverage_are_not_inferred() {
        for metadata in [
            sent(None, &["i"]),
            sent(Some(CorrelationMedia::Banner), &[]),
        ] {
            let errors = SharethroughBidder
                .make_bids(
                    &mixed_request(),
                    &metadata,
                    &response(200, &one_bid(json!({"mtype":1_i32}))),
                )
                .unwrap_err();
            harness::assert_bad_server_response_errors(&errors, 1);
        }
    }

    #[test]
    fn mixed_bad_bids_preserve_the_good_sibling_and_all_bid_local_errors() {
        let decoded = SharethroughBidder
            .make_bids(
                &mixed_request(),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &response(
                    200,
                    &json!({"id":"r", "cur":"EUR", "seatbid":[{"bid":[
                        {"id":"bad-code", "impid":"i", "price":1.0_f64,"mtype":3_i32},
                        {"id":"good", "impid":"i", "price":1.0_f64,"mtype":1_i32},
                        {"id":"unsent", "impid":"other", "price":1.0_f64,"mtype":1_i32},
                        {"id":"wrong-call", "impid":"i", "price":1.0_f64,"mtype":2_i32}
                    ]}]}),
                ),
            )
            .unwrap();
        assert_eq!(decoded.currency, "USD");
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "good");
        harness::assert_bad_server_response_errors(&decoded.errors, 3);
    }

    #[test]
    fn all_invalid_bids_return_each_bid_local_error_instead_of_empty_success() {
        let errors = SharethroughBidder
            .make_bids(
                &mixed_request(),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &response(
                    200,
                    &json!({"id":"r", "seatbid":[{"bid":[
                        {"id":"audio", "impid":"i", "price":1.0_f64,"mtype":3_i32},
                        {"id":"wrong-call", "impid":"i", "price":1.0_f64,"mtype":2_i32}
                    ]}]}),
                ),
            )
            .unwrap_err();
        harness::assert_bad_server_response_errors(&errors, 2);
    }

    #[test]
    fn empty_successful_response_still_uses_source_usd_currency() {
        let decoded = SharethroughBidder
            .make_bids(
                &mixed_request(),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &response(200, &json!({"id":"r","cur":"EUR","seatbid":[]})),
            )
            .unwrap();
        assert_eq!(decoded.currency, "USD");
        assert!(decoded.bids.is_empty());
        assert!(decoded.errors.is_empty());
    }

    #[test]
    fn no_content_is_empty_success_without_decoding_the_body() {
        let decoded = SharethroughBidder
            .make_bids(
                &mixed_request(),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &harness::budgeted_response(204, b"not JSON".to_vec()),
            )
            .unwrap();
        assert_eq!(decoded.currency, "USD");
        assert!(decoded.bids.is_empty());
        assert!(decoded.errors.is_empty());
    }

    #[test]
    fn bad_request_status_is_bad_input_without_decoding_provider_content() {
        let errors = SharethroughBidder
            .make_bids(
                &mixed_request(),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &harness::budgeted_response(400, b"private provider body".to_vec()),
            )
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 2);
        assert!(!errors[0].message.contains("private provider body"));
    }

    #[test]
    fn zero_and_other_unexpected_statuses_are_fixed_bad_server_responses() {
        for status in [0_u16, 201, 301, 403, 500] {
            let errors = SharethroughBidder
                .make_bids(
                    &mixed_request(),
                    &sent(Some(CorrelationMedia::Banner), &["i"]),
                    &harness::budgeted_response(status, b"private provider body".to_vec()),
                )
                .unwrap_err();
            harness::assert_bad_server_response_errors(&errors, 1);
            assert!(!errors[0].message.contains("private provider body"));
        }
    }

    #[test]
    fn sealed_response_node_budget_is_not_bypassed() {
        let response = BidderHttpResponse::new(
            200,
            vec![],
            serde_json::to_vec(&one_bid(json!({"mtype":1_i32}))).unwrap(),
        )
        .within_node_budget(1);
        let errors = SharethroughBidder
            .make_bids(
                &mixed_request(),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &response,
            )
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
    }
}
