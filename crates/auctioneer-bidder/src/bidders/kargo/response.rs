use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors},
    status::standard_status,
};
use crate::{BidderError, BidderHttpResponse, BidderResponse};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};

pub(super) fn make_bids(
    request: &BidRequest,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        BidderError::bad_server_response(format!(
            "Unexpected status code: {status}. Run with request.debug = 1 for more info."
        ))
    })? {
        return Ok(no_bid);
    }
    let (parsed, parse_errors) = response.parse_bid_candidates()?;
    classify_response_with_errors(parsed, parse_errors, |bid| {
        classify_bid(request, bid, Some(media_hint(bid)?))
    })
}

fn media_hint(bid: &Bid) -> Result<MediaType, BidderError> {
    let Some(extension) = &bid.ext else {
        return Ok(MediaType::Banner);
    };
    let mut hints = extension.other.iter().filter(|(key, _value)| {
        // Source Unicode simple lowercase also maps dotted capital I to i.
        // Only that non-ASCII mapping can enter this ASCII field name.
        key.chars()
            .map(|character| match character {
                '\u{0130}' => 'i',
                _ => character.to_ascii_lowercase(),
            })
            .eq("mediatype".chars())
    });
    let declared = hints.next();
    if hints.next().is_some() {
        return Err(BidderError::bad_server_response(
            "Kargo bid contains ambiguous mediaType keys",
        ));
    }
    Ok(match declared.and_then(|(_key, value)| value.as_str()) {
        Some("video") => MediaType::Video,
        Some("native") => MediaType::Native,
        _ => MediaType::Banner,
    })
}

#[cfg(test)]
mod tests {
    use super::super::KargoBidder;
    use crate::Bidder as _;
    use crate::BidderErrorKind;
    use crate::bidders::harness;
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    fn request(impressions: &Value) -> BidRequest {
        serde_json::from_value(json!({
            "id": "request", "imp": impressions,
            "site": {"domain": "publisher.example"},
            "user": {"keywords": "already scoped"},
            "ext": {"preserved": {"flag": true}},
        }))
        .unwrap()
    }

    fn minimal() -> BidRequest {
        request(&json!([{"id": "i", "banner": {}, "ext": {"bidder": {"placementId": "slot"}}}]))
    }

    #[test]
    fn no_content_ignores_even_a_malformed_body_and_returns_no_bid() {
        for body in [Vec::new(), b"private unparsed body".to_vec()] {
            let result = KargoBidder
                .make_bids(
                    &minimal(),
                    &harness::sent_meta("https://example.test"),
                    &harness::budgeted_response(204, body),
                )
                .unwrap();
            assert!(result.bids.is_empty());
            assert!(result.errors.is_empty());
            assert_eq!(result.currency, "USD");
        }
    }

    #[test]
    fn every_non_success_status_including_bad_request_is_bad_server_response() {
        for status in [199, 201, 206, 299, 302, 400, 429, 500] {
            let errors = KargoBidder
                .make_bids(
                    &minimal(),
                    &harness::sent_meta("https://example.test"),
                    &harness::budgeted_response(status, b"private body".to_vec()),
                )
                .unwrap_err();
            assert_eq!(
                errors,
                [crate::BidderError::bad_server_response(format!(
                    "Unexpected status code: {status}. Run with request.debug = 1 for more info."
                ))]
            );
        }
    }

    #[test]
    fn success_requires_a_valid_bounded_response_without_echoing_body_values() {
        for body in [
            Vec::new(),
            b"private-marker".to_vec(),
            serde_json::to_vec(&json!([])).unwrap(),
            serde_json::to_vec(&json!(null)).unwrap(),
            serde_json::to_vec(&json!({"private-marker": true})).unwrap(),
        ] {
            let errors = KargoBidder
                .make_bids(
                    &minimal(),
                    &harness::sent_meta("https://example.test"),
                    &harness::budgeted_response(200, body),
                )
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
            assert_eq!(errors[0].message, "invalid bidder response");
        }
        let bytes = serde_json::to_vec(&json!({"id": "response", "seatbid": []})).unwrap();
        let response = harness::budgeted_response(200, bytes).within_node_budget(1);
        let errors = KargoBidder
            .make_bids(
                &minimal(),
                &harness::sent_meta("https://example.test"),
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
    fn legacy_hints_select_video_and_native_on_the_matching_impression() {
        for (name, media) in [("video", MediaType::Video), ("native", MediaType::Native)] {
            let response = harness::decode_bid(
                &KargoBidder,
                json!([{"id": "i", "banner": {}, "video": {}, "native": {}}]),
                json!({"ext": {"mediaType": name}}),
            )
            .unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].media_type, media);
            assert!(response.errors.is_empty());
        }
    }

    #[test]
    fn source_case_insensitive_media_keys_preserve_case_sensitive_values() {
        for key in ["mediaType", "MediaType", "MEDIATYPE", "med\u{0130}aType"] {
            for (value, expected) in [
                ("video", MediaType::Video),
                ("native", MediaType::Native),
                ("Video", MediaType::Banner),
            ] {
                let response = harness::decode_bid(
                    &KargoBidder,
                    json!([{"id": "i", "banner": {}, "video": {}, "native": {}}]),
                    json!({"ext": {key: value}}),
                )
                .unwrap();
                assert_eq!(response.bids[0].media_type, expected, "{key}/{value}");
            }
        }
        for key in ["med\u{0131}aType", "medi\u{0307}aType", "med\u{212A}aType"] {
            let response = harness::decode_bid(
                &KargoBidder,
                json!([{"id": "i", "banner": {}, "video": {}}]),
                json!({"ext": {key: "video"}}),
            )
            .unwrap();
            assert_eq!(response.bids[0].media_type, MediaType::Banner, "{key}");
        }
    }

    #[test]
    fn ambiguous_media_key_spellings_fail_in_either_wire_order() {
        // Lexical controls: Value maps sort keys, so only these raw wire bodies
        // distinguish the source's two last-occurrence precedence orders.
        for body in [
            br#"{"id":"r","seatbid":[{"seat":"s","bid":[{"id":"b","impid":"i","price":1,"ext":{"mediaType":"video","MEDIATYPE":"native"}}]}]}"#.as_slice(),
            br#"{"id":"r","seatbid":[{"seat":"s","bid":[{"id":"b","impid":"i","price":1,"ext":{"MEDIATYPE":"native","mediaType":"video"}}]}]}"#.as_slice(),
        ] {
            let request = request(&json!([{"id": "i", "banner": {}, "video": {}, "native": {}}]));
            let errors = KargoBidder
                .make_bids(
                    &request,
                    &harness::sent_meta("https://example.test"),
                    &harness::budgeted_response(200, body.to_vec()),
                )
                .unwrap_err();
            assert_eq!(
                errors,
                [crate::BidderError::bad_server_response(
                    "Kargo bid contains ambiguous mediaType keys"
                )]
            );
        }
        let response = harness::decode_bids(
            &KargoBidder,
            json!([{"id": "i", "banner": {}, "video": {}}]),
            json!([{"seat": "s", "bid": [
                {"id": "ambiguous", "ext": {"mediaType": "video", "med\u{0130}aType": null}},
                {"id": "valid", "ext": {"mediaType": "video"}},
            ]}]),
        )
        .unwrap();
        assert_eq!(response.bids.len(), 1);
        assert_eq!(response.bids[0].bid.id, "valid");
        assert_eq!(response.bids[0].media_type, MediaType::Video);
        assert_eq!(
            response.errors,
            [crate::BidderError::bad_server_response(
                "Kargo bid contains ambiguous mediaType keys"
            )]
        );
    }

    #[test]
    fn extension_root_types_refuse_only_that_bid_not_valid_siblings() {
        for ext in [json!([]), json!("video"), json!(2_i32)] {
            let decoded = harness::decode_bids(
                &KargoBidder,
                json!([{"id": "i", "banner": {}}]),
                json!([{"bid": [{"id":"bad", "ext": ext}, {"id":"good"}]}]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "good");
            let errors = decoded.errors;
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        }
    }

    #[test]
    fn absent_unknown_and_nonstring_legacy_hints_fall_back_to_banner() {
        for fields in [
            json!({}),
            json!({"ext": null}),
            json!({"ext": {}}),
            json!({"ext": {"mediaType": null}}),
            json!({"ext": {"mediaType": "banner"}}),
            json!({"ext": {"mediaType": "audio"}}),
            json!({"ext": {"mediaType": "VIDEO"}}),
            json!({"ext": {"mediaType": 2_i32}}),
            json!({"ext": {"mediaType": {}}}),
            json!({"ext": {"mediaType": ["video"]}}),
        ] {
            let response = harness::decode_bid(
                &KargoBidder,
                json!([{"id": "i", "banner": {}, "video": {}, "native": {}}]),
                fields,
            )
            .unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].media_type, MediaType::Banner);
        }
    }

    #[test]
    fn shared_classification_rejects_conflicting_unoffered_and_unknown_impression_hints() {
        for fields in [
            json!({"mtype": 2_i32}),
            json!({"mtype": 1_i32, "ext": {"mediaType": "video"}}),
            json!({"ext": {"mediaType": "native"}}),
            json!({"impid": "not-sent"}),
            json!({"mtype": 0_i32}),
        ] {
            let errors = harness::decode_bid(
                &KargoBidder,
                json!([{"id": "i", "banner": {}, "video": {}}]),
                fields,
            )
            .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
        let response = harness::decode_bid(
            &KargoBidder,
            json!([{"id": "i", "video": {}}]),
            json!({"mtype": 2_i32, "ext": {"mediaType": "video"}}),
        )
        .unwrap();
        assert_eq!(response.bids[0].media_type, MediaType::Video);
    }

    #[test]
    fn bad_candidates_do_not_discard_valid_bids_across_multiple_seats() {
        let response = harness::decode_bids(&KargoBidder, json!([{"id": "i", "banner": {}, "video": {}}]), json!([
            {"seat": "one", "bid": [{"id": "bad-type", "mtype": 0_i32}, {"id": "good-banner", "adm": "<div>creative</div>"}]},
            {"seat": "two", "bid": [{"id": "good-video", "ext": {"mediaType": "video"}}, {"id": "bad-id", "impid": "unknown"}]},
        ])).unwrap();
        assert_eq!(
            response
                .bids
                .iter()
                .map(|bid| bid.bid.id.as_str())
                .collect::<Vec<_>>(),
            ["good-banner", "good-video"]
        );
        assert_eq!(
            response
                .bids
                .iter()
                .map(|bid| bid.media_type)
                .collect::<Vec<_>>(),
            [MediaType::Banner, MediaType::Video]
        );
        assert_eq!(
            response.bids[0].bid.adm.as_deref(),
            Some("<div>creative</div>")
        );
        assert_eq!(response.errors.len(), 2);
        assert!(response.errors.iter().all(|error| error.code == 4));
        let errors = harness::decode_bids(
            &KargoBidder,
            json!([{"id": "i", "banner": {}}]),
            json!([
                {"seat": "one", "bid": [{"mtype": 0_i32}, {"impid": "unknown"}]},
            ]),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn response_currency_is_preserved_with_only_the_shared_usd_default() {
        for currency in [None, Some(""), Some("USD"), Some("EUR"), Some("gbp")] {
            let mut body = json!({"id": "response", "seatbid": []});
            if let Some(name) = currency {
                body["cur"] = json!(name);
            }
            let response = KargoBidder
                .make_bids(
                    &minimal(),
                    &harness::sent_meta("https://example.test"),
                    &harness::budgeted_response(200, serde_json::to_vec(&body).unwrap()),
                )
                .unwrap();
            assert!(response.bids.is_empty());
            assert!(response.errors.is_empty());
            assert_eq!(
                response.currency,
                currency.filter(|value| !value.is_empty()).unwrap_or("USD")
            );
        }
    }
}
