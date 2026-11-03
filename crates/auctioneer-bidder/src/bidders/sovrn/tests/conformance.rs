//! Verbatim source captures dispatched through real shared sinks and sealed responses.

#[cfg(test)]
mod tests {
    use super::super::super::SovrnBidder;
    use crate::Bidder as _;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        BidderErrorKind, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse,
        EntryPoint, OutboundRequestMeta,
    };
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "87a1318b93391d4fe02e2d6e658cc29ee4a96232df0b29f654008c499d06124e";

    fn info(endpoint: &str) -> BidderRequestInfo<'_> {
        BidderRequestInfo::new(
            BidderName::new("sovrn"),
            BidderName::new("sovrn"),
            "account",
            None,
            endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn run_fixture(_name: &str, path: &str, source: &str) {
        verify_protocol_capture(
            MANIFEST,
            MANIFEST_SHA256,
            &format!("fixtures/protocol/{path}"),
            source,
        )
        .unwrap();
        let mut fixture: Value = serde_json::from_str(source).unwrap();
        let request: BidRequest = serde_json::from_value(fixture["mockBidRequest"].take()).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let expected_calls = fixture["httpCalls"].as_array().unwrap();
        // The fixture supplies this endpoint only to the test; no HTTP mapping is promoted.
        let endpoint = expected_calls
            .first()
            .and_then(|call| call["expectedRequest"]["uri"].as_str())
            .unwrap_or("https://bidder.example.test/auction");
        let (semantic, calls) = harness::drive(&SovrnBidder, &request, &info(endpoint));
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        assert_eq!(calls.len(), expected_calls.len());
        if expected_calls.is_empty() {
            assert_eq!(
                fixture["expectedMakeRequestsErrors"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
            assert_eq!(errors[0].kind, BidderErrorKind::Other);
            return;
        }
        semantic.unwrap();
        assert_eq!(expected_calls.len(), 1);
        assert_request(&expected_calls[0]["expectedRequest"], &calls[0], endpoint);
        assert_response(path, &fixture, &request, calls[0].meta());
    }

    fn assert_request(expected: &Value, actual: &BidderHttpRequest, endpoint: &str) {
        assert_eq!(actual.method(), "POST");
        assert_eq!(actual.url(), endpoint);
        assert_eq!(
            actual.covered_imp_ids().collect::<Vec<_>>(),
            expected["impIDs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        let expected_headers = expected["headers"].as_object().unwrap();
        assert_eq!(actual.headers().count(), expected_headers.len());
        for (name, values) in expected_headers {
            assert_eq!(values.as_array().unwrap().len(), 1);
            assert_eq!(
                actual
                    .headers()
                    .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value),
                values[0].as_str()
            );
        }
        assert_eq!(
            serde_json::from_slice::<Value>(actual.body()).unwrap(),
            expected["body"]
        );
    }

    fn assert_response(
        path: &str,
        fixture: &Value,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
    ) {
        let mock = &fixture["httpCalls"][0]["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let response =
            harness::budgeted_response(status, serde_json::to_vec(&mock["body"]).unwrap());
        let result = SovrnBidder.make_bids(request, sent, &response);
        if let Some(expected_errors) = fixture.get("expectedMakeBidsErrors") {
            assert_eq!(path, "supplemental/invalid-imp-id.json");
            assert_eq!(expected_errors.as_array().unwrap().len(), 1);
            let errors = result.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
            assert_eq!(errors[0].kind, BidderErrorKind::Other);
            // Auctioneer diagnostics intentionally do not echo the unknown impression ID.
            assert!(!errors[0].message.contains("test-imp-idx"));
            return;
        }
        let decoded = result.unwrap();
        assert_success(&decoded, &fixture["expectedBidResponses"]);
    }

    fn assert_success(decoded: &BidderResponse, responses: &Value) {
        assert_eq!(decoded.currency, "USD");
        assert!(decoded.errors.is_empty());
        let expected_responses = responses.as_array().unwrap();
        assert_eq!(expected_responses.len(), 1);
        let expected_bids = expected_responses[0].get("bids").and_then(Value::as_array);
        assert_eq!(decoded.bids.len(), expected_bids.map_or(0, Vec::len));
        for (actual_bid, expected_bid) in
            decoded.bids.iter().zip(expected_bids.into_iter().flatten())
        {
            assert_eq!(
                serde_json::to_value(&actual_bid.bid).unwrap(),
                expected_bid["bid"]
            );
            assert_eq!(
                actual_bid.media_type,
                match expected_bid["type"].as_str().unwrap() {
                    "banner" => MediaType::Banner,
                    "video" => MediaType::Video,
                    unknown => panic!("unexpected captured media: {unknown}"),
                }
            );
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_multi_banner => "exemplary/multi-banner.json",
        captured_no_bid => "exemplary/no-bid.json",
        captured_simple_banner => "exemplary/simple-banner.json",
        captured_adunitcode => "supplemental/adunitcode.json",
        captured_blank_device => "supplemental/blank-device.json",
        captured_both_floors => "supplemental/both-custom-default-bidfloor.json",
        captured_camel_tag => "supplemental/camel-case-tagId.json",
        captured_zero_original_floor => "supplemental/custom-overrides-default-bidfloor.json",
        captured_fpd => "supplemental/fpd.json",
        captured_gdpr => "supplemental/gdpr.json",
        captured_invalid_imp_id => "supplemental/invalid-imp-id.json",
        captured_missing_tag => "supplemental/missing-tagid.json",
        captured_no_floor => "supplemental/no-bidfloor.json",
        captured_no_user => "supplemental/no-user.json",
        captured_string_floor => "supplemental/only-custom-bidfloor-as-string.json",
        captured_erroneous_floor => "supplemental/only-custom-bidfloor-erroneus.json",
        captured_custom_floor => "supplemental/only-custom-bidfloor.json",
        captured_default_floor => "supplemental/only-default-bidfloor.json",
        captured_both_floors_string => "supplemental/with-both-custom-default-bidfloor-as-string.json",
        captured_both_floors_duplicate => "supplemental/with-both-custom-default-bidfloor.json",
        captured_custom_floor_duplicate => "supplemental/with-only-custom-bidfloor.json",
        captured_default_floor_duplicate => "supplemental/with-only-default-bidfloor.json",
        captured_full_video => "video/full-video.json",
        captured_multi_banner_video => "video/multi-banner-video.json",
        captured_multi_video => "video/multi-video.json",
        captured_video_no_bid => "video/no-bid.json",
        captured_simple_video => "video/simple-video.json",
        captured_missing_video_parameters => "videosupplemental/missing-all-required-params.json",
        captured_missing_video_maxduration => "videosupplemental/no-maxduration.json",
        captured_missing_video_mimes => "videosupplemental/no-mimes.json",
        captured_missing_video_protocols => "videosupplemental/no-protocols.json",
    );

    #[test]
    fn captured_membership_is_exact_and_native_publication_defaults_inactive() {
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        let paths = artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .map(|row| row["retained"]["source_path"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                "fixtures/protocol/exemplary/multi-banner.json",
                "fixtures/protocol/exemplary/no-bid.json",
                "fixtures/protocol/exemplary/simple-banner.json",
                "fixtures/protocol/supplemental/adunitcode.json",
                "fixtures/protocol/supplemental/blank-device.json",
                "fixtures/protocol/supplemental/both-custom-default-bidfloor.json",
                "fixtures/protocol/supplemental/camel-case-tagId.json",
                "fixtures/protocol/supplemental/custom-overrides-default-bidfloor.json",
                "fixtures/protocol/supplemental/fpd.json",
                "fixtures/protocol/supplemental/gdpr.json",
                "fixtures/protocol/supplemental/invalid-imp-id.json",
                "fixtures/protocol/supplemental/missing-tagid.json",
                "fixtures/protocol/supplemental/no-bidfloor.json",
                "fixtures/protocol/supplemental/no-user.json",
                "fixtures/protocol/supplemental/only-custom-bidfloor-as-string.json",
                "fixtures/protocol/supplemental/only-custom-bidfloor-erroneus.json",
                "fixtures/protocol/supplemental/only-custom-bidfloor.json",
                "fixtures/protocol/supplemental/only-default-bidfloor.json",
                "fixtures/protocol/supplemental/with-both-custom-default-bidfloor-as-string.json",
                "fixtures/protocol/supplemental/with-both-custom-default-bidfloor.json",
                "fixtures/protocol/supplemental/with-only-custom-bidfloor.json",
                "fixtures/protocol/supplemental/with-only-default-bidfloor.json",
                "fixtures/protocol/video/full-video.json",
                "fixtures/protocol/video/multi-banner-video.json",
                "fixtures/protocol/video/multi-video.json",
                "fixtures/protocol/video/no-bid.json",
                "fixtures/protocol/video/simple-video.json",
                "fixtures/protocol/videosupplemental/missing-all-required-params.json",
                "fixtures/protocol/videosupplemental/no-maxduration.json",
                "fixtures/protocol/videosupplemental/no-mimes.json",
                "fixtures/protocol/videosupplemental/no-protocols.json",
            ]
        );
        assert_eq!(artifacts.len(), 44);
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["sovrn"]
        );
        assert_eq!(
            definitions["sovrn"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["sovrn"].get("aliasOf").is_none());
    }

    #[test]
    fn parse_equivalent_capture_edits_are_rejected_before_dispatch() {
        let source = include_str!("../fixtures/protocol/exemplary/simple-banner.json");
        let path = "fixtures/protocol/exemplary/simple-banner.json";
        assert_eq!(
            verify_protocol_capture(MANIFEST, MANIFEST_SHA256, path, source),
            Ok(())
        );
        let changed = source.replacen('\n', " ", 1);
        assert_ne!(changed, source);
        assert_eq!(changed.len(), source.len());
        assert_eq!(
            serde_json::from_str::<Value>(&changed).unwrap(),
            serde_json::from_str::<Value>(source).unwrap()
        );
        assert_eq!(
            verify_protocol_capture(MANIFEST, MANIFEST_SHA256, path, &changed),
            Err("protocol capture digest mismatch")
        );
    }

    #[test]
    fn source_seat_without_a_name_decodes_and_preserves_the_omitted_name() {
        let source = include_str!("../fixtures/protocol/exemplary/simple-banner.json");
        verify_protocol_capture(
            MANIFEST,
            MANIFEST_SHA256,
            "fixtures/protocol/exemplary/simple-banner.json",
            source,
        )
        .unwrap();
        let fixture: Value = serde_json::from_str(source).unwrap();
        let body = &fixture["httpCalls"][0]["mockResponse"]["body"];
        assert_eq!(body["seatbid"][0].get("seat"), None);
        assert_eq!(fixture["expectedBidResponses"][0]["currency"], json!("USD"));
        assert_eq!(
            fixture["expectedBidResponses"][0]["bids"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let decoded = harness::budgeted_response(200, serde_json::to_vec(body).unwrap())
            .parse_bid_response()
            .unwrap();
        assert_eq!(decoded.seatbid.len(), 1);
        assert!(decoded.seatbid[0].seat.is_empty());
        assert_eq!(
            serde_json::to_value(&decoded.seatbid[0]).unwrap(),
            body["seatbid"][0]
        );
    }
}
