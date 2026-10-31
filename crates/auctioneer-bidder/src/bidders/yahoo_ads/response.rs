use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors, sent_media_type},
    status::standard_status,
    wire_fields::{canonical_integer, signed_byte, signed_integer},
};
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |_status| {
        BidderError::bad_server_response("unexpected yahoo ads response status")
    })? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    // The source decodes all DTOs before filtering any bids. These fields
    // remain in the shared map, so validate the entire response first.
    for bid in parsed.seatbid.iter().flat_map(|seat| &seat.bid) {
        signed_integer(&bid.other, "dur").map_err(|_error| integer_error())?;
        signed_byte(&bid.other, "mtype").map_err(|_error| integer_error())?;
    }
    for bid in parsed.seatbid.iter_mut().flat_map(|seat| &mut seat.bid) {
        canonical_integer(&mut bid.other, "dur").map_err(|_error| integer_error())?;
        canonical_integer(&mut bid.other, "mtype").map_err(|_error| integer_error())?;
    }
    // Yahoo leaves the source response helper's USD default unchanged.
    parsed.cur.clear();
    classify_response_with_errors(parsed, parse_errors, |bid| {
        let media = sent_media_type(sent, &bid.impid)?;
        if !matches!(media, MediaType::Banner | MediaType::Video) {
            return Err(BidderError::bad_server_response(
                "unsupported yahoo ads sent media",
            ));
        }
        classify_bid(request, bid, Some(media))
    })
}

fn integer_error() -> Vec<BidderError> {
    vec![BidderError::bad_server_response(
        "invalid yahoo ads response integer fields",
    )]
}

#[cfg(test)]
mod tests {
    use super::super::YahooAdsBidder;
    use super::super::tests::{ENDPOINT, banner, info, mixed_request, request};
    use crate::bidders::harness;
    use crate::correlation::CorrelationMedia;
    use crate::{
        Bidder as _, BidderError, BidderErrorKind, BidderHttpResponse, META_BYTES_CAPACITY,
        OutboundRequestMeta, OutboundRequestMetaBuilder,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    fn video_request() -> BidRequest {
        request(json!([{
            "id":"i", "video":{"mimes":["video/mp4"],"w":640,"h":360},
            "ext":{"bidder":{"dcn":"site","pos":"position"}}
        }]))
    }

    fn sent(media: CorrelationMedia, ids: &[&str]) -> OutboundRequestMeta {
        let mut bytes = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta =
            OutboundRequestMetaBuilder::new(&mut bytes, u16::try_from(ids.len()).unwrap());
        meta.set_method("POST").unwrap();
        meta.set_url(ENDPOINT).unwrap();
        for (ordinal, id) in ids.iter().enumerate() {
            meta.imp_id(id).unwrap();
            meta.correlate_imp(u16::try_from(ordinal).unwrap(), media, 0)
                .unwrap();
        }
        meta.finish()
    }

    fn response(status: u16, document: &Value) -> BidderHttpResponse {
        harness::budgeted_response(status, serde_json::to_vec(document).unwrap())
    }

    fn one_bid(fields: Value) -> Value {
        let Value::Object(extra) = fields else {
            panic!("bid fields must be an object");
        };
        let mut bid = json!({"id":"bid", "impid":"i", "price":1.25_f64});
        bid.as_object_mut().unwrap().extend(extra);
        json!({"id":"r", "seatbid":[{"bid":[bid]}]})
    }

    #[test]
    fn actual_mixed_impression_response_uses_its_banner_first_annotation() {
        let request = mixed_request();
        let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let decoded = YahooAdsBidder
            .make_bids(
                &request,
                calls[0].meta(),
                &response(200, &one_bid(json!({}))),
            )
            .unwrap();
        assert!(decoded.errors.is_empty());
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].media_type, MediaType::Banner);
    }

    #[test]
    fn opposite_call_media_hint_is_bid_local_even_when_original_imp_offers_both() {
        let request = mixed_request();
        let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let decoded = YahooAdsBidder
            .make_bids(
                &request,
                calls[0].meta(),
                &response(
                    200,
                    &json!({"id":"r", "seatbid":[{"bid":[
                        {"id":"good", "impid":"i", "price":1.25_f64, "mtype":1_i32},
                        {"id":"wrong", "impid":"i", "price":1.0_f64, "mtype":2_i32}
                    ]}]}),
                ),
            )
            .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "good");
        assert_eq!(decoded.bids[0].media_type, MediaType::Banner);
        assert_eq!(
            decoded.errors,
            [BidderError::bad_server_response(
                "bid response contains conflicting media type hints"
            )]
        );
    }

    #[test]
    fn no_content_ignores_body_and_returns_usd_without_parsing() {
        let decoded = YahooAdsBidder
            .make_bids(
                &mixed_request(),
                &sent(CorrelationMedia::Banner, &["i"]),
                &response(204, &json!({"seatbid":"not a valid envelope"})),
            )
            .unwrap();
        assert!(decoded.bids.is_empty());
        assert!(decoded.errors.is_empty());
        assert_eq!(decoded.currency, "USD");
    }

    #[test]
    fn every_non_200_non_204_status_including_400_is_bad_server_response() {
        for status in [201, 400, 401, 429, 500] {
            let errors = YahooAdsBidder
                .make_bids(
                    &mixed_request(),
                    &sent(CorrelationMedia::Banner, &["i"]),
                    &response(status, &json!({})),
                )
                .unwrap_err();
            assert_eq!(
                errors,
                [BidderError::bad_server_response(
                    "unexpected yahoo ads response status"
                )]
            );
            assert_eq!(errors[0].kind, BidderErrorKind::Other);
        }
    }

    #[test]
    fn valid_wire_currency_is_ignored_without_changing_bid_price() {
        for currency in [None, Some(""), Some("USD"), Some("EUR")] {
            let mut document = one_bid(json!({}));
            if let Some(value) = currency {
                document["cur"] = json!(value);
            }
            let decoded = YahooAdsBidder
                .make_bids(
                    &video_request(),
                    &sent(CorrelationMedia::Video, &["i"]),
                    &response(200, &document),
                )
                .unwrap();
            assert_eq!(decoded.currency, "USD");
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.price.to_bits(), 1.25_f64.to_bits());
            assert_eq!(decoded.bids[0].media_type, MediaType::Video);
        }
    }

    #[test]
    fn response_seats_preserve_good_siblings_when_ids_are_unknown_or_unsent() {
        let request = request(json!([banner("i"), banner("private-unsent")]));
        let decoded = YahooAdsBidder
            .make_bids(
                &request,
                &sent(CorrelationMedia::Banner, &["i"]),
                &response(
                    200,
                    &json!({"id":"r","seatbid":[
                        {"seat":"first","bid":[{"id":"good","impid":"i","price":1.25_f64}]},
                        {"seat":"second","bid":[
                            {"id":"unsent","impid":"private-unsent","price":1.0_f64},
                            {"id":"unknown","impid":"private-unknown","price":1.0_f64}
                        ]}
                    ]}),
                ),
            )
            .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "good");
        assert_eq!(decoded.errors.len(), 2);
        for error in &decoded.errors {
            assert_eq!(error.code, 4);
            assert_eq!(error.kind, BidderErrorKind::Other);
            assert!(!error.message.contains("private-"));
        }
    }

    #[test]
    fn covered_but_unannotated_call_is_not_inferred_from_original_request() {
        let errors = YahooAdsBidder
            .make_bids(
                &request(json!([banner("i")])),
                &harness::sent_meta_with_ids(ENDPOINT, ["i"]),
                &response(200, &one_bid(json!({}))),
            )
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::Other);
        assert_ne!(errors[0].message, "yahoo ads protocol not implemented");
    }

    #[test]
    fn unrelated_audio_or_native_call_media_is_not_accepted_as_yahoo_delivery() {
        for media in [CorrelationMedia::Audio, CorrelationMedia::Native] {
            let errors = YahooAdsBidder
                .make_bids(
                    &mixed_request(),
                    &sent(media, &["i"]),
                    &response(200, &one_bid(json!({}))),
                )
                .unwrap_err();
            assert_eq!(
                errors,
                [BidderError::bad_server_response(
                    "unsupported yahoo ads sent media"
                )]
            );
        }
    }

    #[test]
    fn empty_seat_arrays_return_an_empty_usd_response() {
        for document in [
            json!({"id":"r"}),
            json!({"id":"r","seatbid":[]}),
            json!({"id":"r","seatbid":[{"bid":[]}]}),
        ] {
            let decoded = YahooAdsBidder
                .make_bids(
                    &mixed_request(),
                    &sent(CorrelationMedia::Banner, &["i"]),
                    &response(200, &document),
                )
                .unwrap();
            assert!(decoded.bids.is_empty());
            assert!(decoded.errors.is_empty());
            assert_eq!(decoded.currency, "USD");
        }
    }

    #[test]
    fn malformed_or_wrong_shaped_envelopes_keep_shared_parse_provenance() {
        for body in [
            Vec::new(),
            serde_json::to_vec(&json!(true)).unwrap(),
            serde_json::to_vec(&json!({"id":"r","seatbid":"private-body"})).unwrap(),
        ] {
            let errors = YahooAdsBidder
                .make_bids(
                    &mixed_request(),
                    &sent(CorrelationMedia::Banner, &["i"]),
                    &harness::budgeted_response(200, body),
                )
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
            assert_eq!(errors[0].message, "invalid bidder response");
        }
    }

    #[test]
    fn node_refusal_cannot_be_mistaken_for_a_source_semantic_error() {
        let response = BidderHttpResponse::new(
            200,
            vec![],
            serde_json::to_vec(&one_bid(json!({}))).unwrap(),
        )
        .within_node_budget(1);
        let errors = YahooAdsBidder
            .make_bids(
                &mixed_request(),
                &sent(CorrelationMedia::Banner, &["i"]),
                &response,
            )
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(
            errors[0].message,
            "bidder response exceeded the decode budget"
        );
    }

    #[test]
    fn malformed_integer_dto_fields_are_response_fatal_before_id_filtering() {
        for fields in [
            json!({"dur":"30"}),
            json!({"dur":true}),
            json!({"mtype":128_i32}),
            json!({"mtype":"1"}),
            json!({"dur":30_i32,"DUR":null}),
            json!({"Mtype":null,"MTYPE":null}),
        ] {
            let mut bad = json!({"id":"bad","impid":"private-unsent","price":1.0_f64});
            bad.as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            let errors = YahooAdsBidder
                .make_bids(
                    &mixed_request(),
                    &sent(CorrelationMedia::Banner, &["i"]),
                    &response(
                        200,
                        &json!({"id":"r","seatbid":[{"bid":[
                            {"id":"good","impid":"i","price":1.25_f64}, bad
                        ]}]}),
                    ),
                )
                .unwrap_err();
            assert_eq!(
                errors,
                [BidderError::bad_server_response(
                    "invalid yahoo ads response integer fields"
                )]
            );
            assert_eq!(errors[0].kind, BidderErrorKind::Other);
        }
    }

    #[test]
    fn canonical_integer_projection_preserves_provider_metadata_without_unruly_enrichment() {
        for (duration, expected_duration) in [
            (json!(30_i32), Some(json!(30_i32))),
            (json!(-1_i32), Some(json!(-1_i32))),
            (Value::Null, None),
            (json!(0_i32), None),
        ] {
            let decoded = YahooAdsBidder
                .make_bids(
                    &video_request(),
                    &sent(CorrelationMedia::Video, &["i"]),
                    &response(
                        200,
                        &one_bid(json!({"Dur":duration,"MTYPE":2_i32,
                    "ext":{"provider":{"keep":[true,null]}}, "opaque":"keep"})),
                    ),
                )
                .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert!(decoded.errors.is_empty());
            let wire = serde_json::to_value(&decoded.bids[0].bid).unwrap();
            assert_eq!(wire.get("dur"), expected_duration.as_ref());
            assert!(wire.get("Dur").is_none());
            assert_eq!(wire["mtype"], 2_i32);
            assert!(wire.get("MTYPE").is_none());
            assert_eq!(wire["ext"], json!({"provider":{"keep":[true,null]}}));
            assert_eq!(wire["opaque"], "keep");
        }
    }
}
