use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors},
    status::{bad_input_or_server, standard_status},
    wire_fields::{canonical_integer, signed_byte as response_signed_byte, signed_integer},
};
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::{Map, Value};

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        bad_input_or_server(
            status,
            format!("unexpected status code: {status}. Run with request.debug = 1 for more info"),
        )
    })? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    // Source decodes every bid DTO before media or sent-call filtering.
    // Refuse the whole response before mutating any integer projection.
    if parsed.seatbid.iter().flat_map(|seat| &seat.bid).any(|bid| {
        signed_integer(&bid.other, "dur").is_err()
            || response_signed_byte(&bid.other, "mtype").is_err()
    }) {
        return Err(vec![BidderError::bad_server_response(
            "invalid unruly response integer fields",
        )]);
    }
    for bid in parsed.seatbid.iter_mut().flat_map(|seat| &mut seat.bid) {
        canonical_integer(&mut bid.other, "dur").map_err(|_error| {
            vec![BidderError::bad_server_response(
                "invalid unruly response integer fields",
            )]
        })?;
    }
    // Source never replaces the helper's USD currency with response.cur.
    parsed.cur.clear();
    let mut classified = classify_response_with_errors(parsed, parse_errors, |bid| {
        if !sent.covered_imp_ids().any(|id| id == bid.impid) {
            return Err(BidderError::bad_server_response(
                "bid response references an unsent impression",
            ));
        }
        let imp = request
            .imp
            .iter()
            .find(|imp| imp.id == bid.impid)
            .ok_or_else(|| {
                BidderError::bad_server_response("bid response references an unknown impression")
            })?;
        let media = if imp.banner.is_some() {
            MediaType::Banner
        } else if imp.video.is_some() {
            MediaType::Video
        } else {
            return Err(BidderError::bad_server_response(
                "unruly bid has no supported impression media",
            ));
        };
        classify_bid(request, bid, Some(media))
    })?;
    for accepted in &mut classified.bids {
        if accepted.media_type != MediaType::Video {
            continue;
        }
        let Some(duration) = accepted
            .bid
            .other
            .get("dur")
            .and_then(Value::as_i64)
            .filter(|duration| *duration > 0)
        else {
            continue;
        };
        // Source's separate TypedBid.BidVideo becomes reserved wire metadata
        // here; provider fields and the original dur remain in place.
        let extension = accepted.bid.ext.get_or_insert_with(Default::default);
        let prebid = extension.prebid.get_or_insert_with(Default::default);
        let video = prebid
            .other
            .entry("video")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(fields) = video {
            fields.insert("duration".into(), Value::from(duration));
            fields
                .entry("primary_category")
                .or_insert_with(|| Value::String(String::new()));
        } else {
            *video = serde_json::json!({"duration":duration,"primary_category":""});
        }
    }
    Ok(classified)
}

#[cfg(test)]
mod tests {
    use super::super::UnrulyBidder;
    use super::super::tests::{info, parameters, request};
    use crate::bidders::harness;
    use crate::{Bidder as _, BidderError, BidderErrorKind};
    use auctioneer_config_data::types::MediaType;
    use serde_json::{Value, json};

    fn assert_integer_dto_failure_before_filtering(fields: &Value) {
        let request = request(json!([
            {"id":"good","video":{}}, {"id":"video","video":{}},
            {"id":"banner","banner":{}}, {"id":"unsupported","audio":{}},
            {"id":"unsent","video":{}}
        ]));
        let sent = harness::sent_meta_with_ids(
            info().endpoint_template,
            ["good", "video", "banner", "unsupported"],
        );
        let before = serde_json::to_value(&request).unwrap();
        for target in ["video", "banner", "unsupported", "unknown", "unsent"] {
            let mut bad = json!({"id":"bad","impid":target,"price":1.0_f64});
            bad.as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            let body = json!({"id":"r","seatbid":[
                {"seat":"first","bid":[{"id":"valid","impid":"good","price":1.25_f64}]},
                {"seat":"second","bid":[bad]}
            ]});
            let errors = UnrulyBidder
                .make_bids(
                    &request,
                    &sent,
                    &harness::budgeted_response(200, serde_json::to_vec(&body).unwrap()),
                )
                .unwrap_err();
            assert_eq!(
                errors,
                [BidderError::bad_server_response(
                    "invalid unruly response integer fields"
                )],
                "fields={fields} target={target}"
            );
            assert_eq!(errors[0].kind, BidderErrorKind::Other);
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn no_content_ignores_body_bad_request_is_bad_input_and_other_statuses_are_server_errors() {
        let request = parameters(json!({"siteId":7_i32}));
        let sent = harness::sent_meta_with_ids(info().endpoint_template, ["i"]);
        let response = UnrulyBidder
            .make_bids(
                &request,
                &sent,
                &harness::budgeted_response(204, vec![0xff]),
            )
            .unwrap();
        assert!(response.bids.is_empty());
        assert!(response.errors.is_empty());
        assert_eq!(response.currency, "USD");
        for status in [201_u16, 302, 400, 401, 429, 500] {
            let errors = UnrulyBidder
                .make_bids(
                    &request,
                    &sent,
                    &harness::budgeted_response(status, vec![0xff]),
                )
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
            assert_eq!(
                errors[0].message,
                format!(
                    "unexpected status code: {status}. Run with request.debug = 1 for more info"
                )
            );
        }
    }

    #[test]
    fn sealed_parse_and_node_failures_are_not_candidate_classification_failures() {
        let request = parameters(json!({"siteId":7_i32}));
        let sent = harness::sent_meta_with_ids(info().endpoint_template, ["i"]);
        for body in [vec![], vec![0xff], serde_json::to_vec(&json!([])).unwrap()] {
            let errors = UnrulyBidder
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
        let errors = UnrulyBidder
            .make_bids(&request, &sent, &response)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
    }

    #[test]
    fn matching_impression_prefers_banner_over_video_without_requiring_response_extensions() {
        for (media, expected) in [
            (
                json!({"id":"i","banner":{},"video":{},"audio":{},"native":{}}),
                MediaType::Banner,
            ),
            (
                json!({"id":"i","video":{},"audio":{},"native":{}}),
                MediaType::Video,
            ),
        ] {
            let response = harness::decode_bid(
                &UnrulyBidder,
                json!([{"id":"unrelated","banner":{}},media]),
                json!({"dur":30_i32}),
            )
            .unwrap();
            assert_eq!(response.bids.len(), 1);
            assert!(response.errors.is_empty());
            assert_eq!(response.bids[0].media_type, expected);
        }
    }

    #[test]
    fn legacy_media_extensions_do_not_override_source_request_based_precedence() {
        for fields in [
            json!({"ext":{"mediaType":"video"}}),
            json!({"ext":{"video":{}}}),
            json!({"ext":{"native":{}}}),
        ] {
            let response = harness::decode_bid(
                &UnrulyBidder,
                json!([{"id":"i","banner":{},"video":{}}]),
                fields,
            )
            .unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].media_type, MediaType::Banner);
            assert!(response.errors.is_empty());
        }
    }

    #[test]
    fn candidate_local_failures_keep_good_bids_in_other_seats() {
        let response = harness::decode_bids(&UnrulyBidder,
            json!([{"id":"i","banner":{}},{"id":"video","video":{}},{"id":"audio","audio":{}}]),
            json!([
                {"seat":"first","bid":[{"id":"unknown","impid":"unknown"},{"id":"good-banner"}]},
                {"seat":"second","bid":[{"id":"bad-media","impid":"audio"},{"id":"good-video","impid":"video"}]}
            ])).unwrap();
        assert_eq!(response.bids.len(), 2);
        assert_eq!(response.bids[0].bid.id, "good-banner");
        assert_eq!(response.bids[0].media_type, MediaType::Banner);
        assert_eq!(response.bids[1].bid.id, "good-video");
        assert_eq!(response.bids[1].media_type, MediaType::Video);
        assert_eq!(response.errors.len(), 2);
        assert!(response.errors.iter().all(|error| error.code == 4));
    }

    #[test]
    fn all_invalid_or_unknown_media_candidates_return_errors_not_silent_no_bid() {
        for imps in [
            json!([{"id":"i","audio":{}}]),
            json!([{"id":"i","native":{}}]),
            json!([{"id":"i"}]),
            json!([{"id":"other","banner":{}}]),
        ] {
            let errors = harness::decode_bid(&UnrulyBidder, imps, json!({})).unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
            assert!(!errors[0].message.contains("other"));
        }
    }

    #[test]
    fn shared_standard_hint_conflicts_cannot_override_the_source_media_choice() {
        for fields in [
            json!({"mtype":2_i32}),
            json!({"ext":{"prebid":{"type":"video"}}}),
        ] {
            let errors = harness::decode_bid(
                &UnrulyBidder,
                json!([{"id":"i","banner":{},"video":{}}]),
                fields,
            )
            .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
    }

    #[test]
    fn exact_sent_call_coverage_is_required_even_when_the_scoped_request_contains_the_imp() {
        let request = request(json!([{"id":"i","banner":{}},{"id":"unsent","video":{}}]));
        for ids in [vec![], vec!["i"]] {
            let sent = harness::sent_meta_with_ids(info().endpoint_template, ids);
            let response = harness::budgeted_response(200,serde_json::to_vec(&json!({
                "id":"r","seatbid":[{"seat":"s","bid":[{"id":"b","impid":"unsent","price":1.0_f64}]}]
            })).unwrap());
            let errors = UnrulyBidder
                .make_bids(&request, &sent, &response)
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
    }

    #[test]
    fn response_currency_is_fixed_usd_without_converting_prices() {
        let request = parameters(json!({"siteId":7_i32}));
        let sent = harness::sent_meta_with_ids(info().endpoint_template, ["i"]);
        for declared_currency in [None, Some(""), Some("USD"), Some("EUR"), Some("unknown")] {
            let mut body = json!({"id":"r","seatbid":[{"seat":"s","bid":[{"id":"b","impid":"i","price":1.25_f64}]}]});
            if let Some(currency) = declared_currency {
                body["cur"] = json!(currency);
            }
            let response = UnrulyBidder
                .make_bids(
                    &request,
                    &sent,
                    &harness::budgeted_response(200, serde_json::to_vec(&body).unwrap()),
                )
                .unwrap();
            assert_eq!(response.currency, "USD");
            assert_eq!(response.bids.len(), 1);
            assert!((response.bids[0].bid.price - 1.25_f64).abs() < f64::EPSILON);
            assert!(response.errors.is_empty());
        }
    }

    #[test]
    fn video_duration_wire_field_is_retained_without_rewriting_bidder_extensions() {
        // Source retains nonzero Dur but omits its zero value. Only the
        // explicitly mapped reserved metadata may be added beside provider fields.
        for duration in [0_i64, 1, 30] {
            for (media, expected) in [("banner", MediaType::Banner), ("video", MediaType::Video)] {
                let response = harness::decode_bid(
                    &UnrulyBidder,
                    json!([{"id":"i",media:{}}]),
                    json!({"dur":duration,"ext":{"provider":{"keep":true}}}),
                )
                .unwrap();
                assert_eq!(response.bids.len(), 1);
                assert_eq!(response.bids[0].media_type, expected);
                let bid = serde_json::to_value(&response.bids[0].bid).unwrap();
                if duration == 0 {
                    assert!(bid.get("dur").is_none());
                } else {
                    assert_eq!(bid.get("dur"), Some(&json!(duration)));
                }
                let expected_extension = if expected == MediaType::Video && duration > 0 {
                    json!({"provider":{"keep":true},"prebid":{"video":{"duration":duration,"primary_category":""}}})
                } else {
                    json!({"provider":{"keep":true}})
                };
                assert_eq!(bid["ext"], expected_extension);
            }
        }
    }

    #[test]
    fn positive_video_duration_materializes_on_serialized_wire_without_losing_extensions() {
        for (media, expected_media) in [("banner", MediaType::Banner), ("video", MediaType::Video)]
        {
            for duration in [-1_i64, 0, 30] {
                for original_ext in [
                    None,
                    Some(json!({
                        "provider":{"keep":[true,null]},
                        "prebid":{"targeting":{"existing":"value"},"note":{"keep":true}}
                    })),
                    Some(json!({
                        "provider":{"keep":[true,null]},
                        "prebid":{"targeting":{"existing":"value"},"note":{"keep":true},
                                  "video":{"duration":7_i32,"primary_category":"IAB1","keep":true}}
                    })),
                ] {
                    let mut fields = json!({"dur":duration});
                    if let Some(extension) = &original_ext {
                        fields["ext"] = extension.clone();
                    }
                    let response =
                        harness::decode_bid(&UnrulyBidder, json!([{"id":"i",media:{}}]), fields)
                            .unwrap();
                    assert!(response.errors.is_empty());
                    assert_eq!(response.bids.len(), 1);
                    assert_eq!(response.bids[0].media_type, expected_media);
                    let wire = serde_json::to_value(&response.bids[0].bid).unwrap();
                    if duration == 0 {
                        assert!(wire.get("dur").is_none());
                    } else {
                        assert_eq!(wire.get("dur"), Some(&json!(duration)));
                    }
                    let mut expected_ext = original_ext;
                    if media == "video" && duration > 0 {
                        let extension = expected_ext.get_or_insert_with(|| json!({}));
                        if extension.get("prebid").is_none() {
                            extension["prebid"] = json!({});
                        }
                        if extension["prebid"].get("video").is_none() {
                            extension["prebid"]["video"] = json!({});
                        }
                        extension["prebid"]["video"]["duration"] = json!(duration);
                        if extension["prebid"]["video"]
                            .get("primary_category")
                            .is_none()
                        {
                            extension["prebid"]["video"]["primary_category"] = json!("");
                        }
                    }
                    assert_eq!(
                        wire.get("ext"),
                        expected_ext.as_ref(),
                        "{media} duration={duration}"
                    );
                }
            }
        }
    }

    #[test]
    fn single_duration_spellings_canonicalize_the_source_bid_and_complete_video_metadata() {
        for key in ["dur", "DUR", "Dur", "dUr"] {
            let response = harness::decode_bid(
                &UnrulyBidder,
                json!([{"id":"i","video":{}}]),
                json!({key:30_i32,"opaque":{"keep":[true,null]},"ext":{"provider":{"keep":true}}}),
            )
            .unwrap();
            assert!(response.errors.is_empty());
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].media_type, MediaType::Video);
            assert_eq!(
                serde_json::to_value(&response.bids[0].bid).unwrap(),
                json!({
                    "id":"bid","impid":"i","price":1.25_f64,"dur":30_i32,
                    "opaque":{"keep":[true,null]},
                    "ext":{"provider":{"keep":true},"prebid":{"video":{"duration":30_i32,"primary_category":""}}}
                })
            );
        }
    }

    #[test]
    fn malformed_duration_values_abort_the_entire_dto_before_any_candidate_filtering() {
        for key in ["dur", "DUR", "Dur"] {
            for value in [
                json!("30"),
                json!(30.0_f64),
                json!(1.5_f64),
                json!(u64::MAX),
                json!(true),
                json!([]),
                json!({}),
            ] {
                assert_integer_dto_failure_before_filtering(&json!({key:value}));
            }
        }
    }

    #[test]
    fn ambiguous_duration_spellings_abort_the_entire_dto_even_when_equal_or_null() {
        for (first, second) in [("dur", "DUR"), ("Dur", "dUr")] {
            for (left, right) in [
                (json!(30_i32), json!(31_i32)),
                (json!(30_i32), json!(30_i32)),
                (json!(30_i32), Value::Null),
                (Value::Null, Value::Null),
            ] {
                assert_integer_dto_failure_before_filtering(&json!({first:left,second:right}));
            }
        }
    }

    #[test]
    fn malformed_mtype_values_abort_the_entire_dto_before_any_candidate_filtering() {
        for key in ["mtype", "MTYPE", "Mtype"] {
            for value in [
                json!("2"),
                json!(2.0_f64),
                json!(1.5_f64),
                json!(128_i32),
                json!(-129_i32),
                json!(u64::MAX),
                json!(true),
                json!([]),
                json!({}),
            ] {
                assert_integer_dto_failure_before_filtering(&json!({key:value}));
            }
        }
    }

    #[test]
    fn ambiguous_mtype_spellings_abort_the_entire_dto_even_when_equal_or_null() {
        for (first, second) in [("mtype", "MTYPE"), ("Mtype", "mType")] {
            for (left, right) in [
                (json!(1_i32), json!(2_i32)),
                (json!(1_i32), json!(1_i32)),
                (json!(1_i32), Value::Null),
                (Value::Null, Value::Null),
            ] {
                assert_integer_dto_failure_before_filtering(&json!({first:left,second:right}));
            }
        }
    }
}
