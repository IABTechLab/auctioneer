//! Verbatim source expectations; shared-boundary differences remain explicit.

#[cfg(test)]
mod tests {
    use super::super::super::UnrulyBidder;
    use crate::Bidder as _;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        BidderError, BidderErrorKind, BidderName, BidderRequestInfo, BidderResponse, EntryPoint,
        OutboundRequestMeta,
    };
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::panic::catch_unwind;

    const MANIFEST: &str = include_str!("../bidder.toml");
    // Frozen witness for the adjacent candidate and verbatim source captures.
    const MANIFEST_SHA256: &str =
        "ced154e515abfe7f514468fdbf44cc580c5e29adb10b8238075631ec297bc6f7";

    fn run_fixture(name: &str, path: &str, source: &str) {
        verify_protocol_capture(
            MANIFEST,
            MANIFEST_SHA256,
            &format!("fixtures/protocol/{path}"),
            source,
        )
        .unwrap();
        let fixture: Value = serde_json::from_str(source).unwrap();
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let captured_calls = fixture.get("httpCalls").and_then(Value::as_array);
        let endpoint = captured_calls
            .and_then(|calls| calls.first())
            .and_then(|call| call["expectedRequest"]["uri"].as_str())
            .unwrap_or("https://bidder.example.test/unruly_prebid_server");
        let info = BidderRequestInfo::new(
            BidderName::new("unruly"),
            BidderName::new("unruly"),
            "account",
            None,
            endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let (semantic, calls) = harness::drive(&UnrulyBidder, &request, &info);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        if name == "captured_missing_extension" {
            assert!(captured_calls.is_none());
            assert!(calls.is_empty());
            let expected_errors = fixture["expectedMakeRequestsErrors"].as_array().unwrap();
            assert_eq!(expected_errors.len(), 1);
            assert_eq!(
                expected_errors[0]["value"],
                json!("ext data not provided in imp id=test-missing-ext-id. Abort all Request")
            );
            let errors = semantic.unwrap_err();
            assert_eq!(
                errors,
                [BidderError::bad_input(
                    "invalid unruly impression extension"
                )]
            );
            assert_eq!(errors[0].kind, BidderErrorKind::Other);
            return;
        }
        semantic.unwrap();
        let expected_calls = captured_calls.unwrap();
        assert_eq!(expected_calls.len(), 1);
        assert_eq!(calls.len(), 1);
        let expected = &expected_calls[0]["expectedRequest"];
        let call = &calls[0];
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), endpoint);
        assert_eq!(
            call.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
            ]
        );
        assert_eq!(
            call.covered_imp_ids().collect::<Vec<_>>(),
            expected["impIDs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            serde_json::from_slice::<Value>(call.body()).unwrap(),
            expected["body"]
        );
        assert_response(name, &fixture, &request, call.meta());
    }

    fn assert_response(
        name: &str,
        fixture: &Value,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
    ) {
        let mock = &fixture["httpCalls"][0]["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let body = mock
            .get("body")
            .map_or_else(Vec::new, |value| serde_json::to_vec(value).unwrap());
        let decoded =
            UnrulyBidder.make_bids(request, sent, &harness::budgeted_response(status, body));
        if let Some(expected_errors) = fixture.get("expectedMakeBidsErrors") {
            let errors = decoded.unwrap_err();
            assert_response_errors(name, fixture, status, expected_errors, &errors);
            return;
        }
        let response = decoded.unwrap();
        assert_success_response(name, fixture, mock, status, &response);
    }

    fn assert_response_errors(
        name: &str,
        fixture: &Value,
        status: u16,
        expected_errors: &Value,
        errors: &[BidderError],
    ) {
        assert_eq!(expected_errors.as_array().unwrap().len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, BidderErrorKind::Other);
        if name == "captured_unknown_impression" {
            assert_eq!(status, 200);
            assert_eq!(
                fixture["expectedBidResponses"],
                json!([{"currency":"USD","bids":[]}])
            );
            assert_eq!(
                expected_errors[0]["value"],
                json!(
                    "Bid response imp ID sdf not found in bid request containing imps [test-imp-id]\n"
                )
            );
            // Shared contract puts all-invalid local errors in Err and
            // requires fixed diagnostics instead of source ID interpolation.
            assert_eq!(
                errors,
                &[BidderError::bad_server_response(
                    "bid response references an unsent impression"
                )]
            );
        } else {
            assert!(matches!(status, 400 | 401));
            assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
            assert_eq!(
                errors[0].message,
                expected_errors[0]["value"].as_str().unwrap()
            );
        }
    }

    fn assert_success_response(
        name: &str,
        fixture: &Value,
        mock: &Value,
        status: u16,
        response: &BidderResponse,
    ) {
        assert_eq!(response.currency, "USD");
        assert!(response.errors.is_empty());
        let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
        if status == 204 {
            assert!(expected_responses.is_empty());
            assert!(response.bids.is_empty());
            return;
        }
        assert_eq!(expected_responses.len(), 1);
        let expected_bids = expected_responses[0]["bids"].as_array().unwrap();
        assert_eq!(expected_bids.len(), 1);
        assert_eq!(response.bids.len(), expected_bids.len());
        for (actual, expected) in response.bids.iter().zip(expected_bids) {
            let actual_wire = serde_json::to_value(&actual.bid).unwrap();
            assert_bid_fields(name, mock, &actual_wire, &expected["bid"]);
            assert_eq!(
                actual.media_type,
                match expected["type"].as_str().unwrap() {
                    "banner" => MediaType::Banner,
                    "video" => MediaType::Video,
                    other => panic!("unsupported captured media type: {other}"),
                }
            );
        }
        if name == "captured_video" {
            assert_eq!(expected_bids[0]["bid"]["dur"], json!(30_i32));
            assert!(expected_bids[0].get("video").is_none());
            // The capture omits its optional TypedBid video expectation. The
            // separately pinned implementation establishes the duration mapping.
        }
    }

    fn assert_bid_fields(name: &str, mock: &Value, actual_wire: &Value, expected: &Value) {
        if name != "captured_video" {
            assert_eq!(actual_wire, expected);
            return;
        }
        // Source Bid and TypedBid.BidVideo are separate. Prove the original
        // has no extension and the sole added root field is exactly the
        // reviewed metadata before comparing every original source field.
        assert!(mock["body"]["seatbid"][0]["bid"][0].get("ext").is_none());
        let source_fields = expected.as_object().unwrap();
        assert!(!source_fields.contains_key("ext"));
        assert_eq!(expected["dur"], json!(30_i32));
        assert_eq!(
            actual_wire["ext"],
            json!({"prebid":{"video":{"duration":30_i32,"primary_category":""}}})
        );
        assert_eq!(
            actual_wire["ext"]["prebid"]["video"]["duration"],
            json!(30_i32)
        );
        let actual_fields = actual_wire.as_object().unwrap();
        assert_eq!(
            Some(actual_fields.len()),
            source_fields.len().checked_add(1)
        );
        for (key, value) in source_fields {
            assert_eq!(actual_fields.get(key), Some(value));
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_app => "exemplary/banner-and-video-app.json",
        captured_gdpr => "exemplary/banner-and-video-gdpr.json",
        captured_site => "exemplary/banner-and-video-site.json",
        captured_banner_and_video => "exemplary/banner-and-video.json",
        captured_banner => "exemplary/simple-banner.json",
        captured_video => "exemplary/simple-video.json",
        captured_missing_extension => "supplemental/missing-extension.json",
        captured_unknown_impression => "supplemental/no-matching-impid.json",
        captured_no_content => "supplemental/status-code-204.json",
        captured_bad_request => "supplemental/status-code-400.json",
        captured_unauthorized => "supplemental/status-code-401.json",
    );

    #[test]
    fn capture_inventory_has_eleven_unique_verbatim_witnesses_and_default_inactive_native_publication()
     {
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        let captures: Vec<_> = artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .map(|row| row["retained"]["source_path"].as_str().unwrap())
            .collect();
        assert_eq!(
            captures,
            [
                "fixtures/protocol/exemplary/banner-and-video-app.json",
                "fixtures/protocol/exemplary/banner-and-video-gdpr.json",
                "fixtures/protocol/exemplary/banner-and-video-site.json",
                "fixtures/protocol/exemplary/banner-and-video.json",
                "fixtures/protocol/exemplary/simple-banner.json",
                "fixtures/protocol/exemplary/simple-video.json",
                "fixtures/protocol/supplemental/missing-extension.json",
                "fixtures/protocol/supplemental/no-matching-impid.json",
                "fixtures/protocol/supplemental/status-code-204.json",
                "fixtures/protocol/supplemental/status-code-400.json",
                "fixtures/protocol/supplemental/status-code-401.json",
            ]
        );
        assert_eq!(artifacts.len(), 25);
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["unruly"]
        );
        assert_eq!(
            definitions["unruly"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["unruly"].get("aliasOf").is_none());
        assert_eq!(
            manifest["provenance"]["sources"]["upstream"]["revision"].as_str(),
            Some("d7f8515b86258688304b0d9b6668c6a0e258bc9e")
        );
        for row in artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
        {
            assert_eq!(row["source_bytes"], row["retained"]["source_bytes"]);
            assert_eq!(row["source_sha256"], row["retained"]["source_sha256"]);
            assert!(row.get("transformation").is_none());
        }
        let missing = include_str!("../fixtures/protocol/supplemental/missing-extension.json");
        assert!(!missing.ends_with('\n'));
    }

    #[test]
    fn filtered_dispatch_rejects_changed_capture_bytes_before_bidder_execution() {
        let source = include_str!("../fixtures/protocol/exemplary/simple-banner.json");
        let path = "fixtures/protocol/exemplary/simple-banner.json";
        assert_eq!(
            verify_protocol_capture(MANIFEST, MANIFEST_SHA256, path, source),
            Ok(())
        );
        let changed = source.replacen('\n', " ", 1);
        assert_eq!(changed.len(), source.len());
        assert_eq!(
            serde_json::from_str::<Value>(&changed).unwrap(),
            serde_json::from_str::<Value>(source).unwrap()
        );
        assert_eq!(
            verify_protocol_capture(MANIFEST, MANIFEST_SHA256, path, &changed),
            Err("protocol capture digest mismatch")
        );
        assert!(
            catch_unwind(|| run_fixture(
                "captured_banner",
                "exemplary/simple-banner.json",
                &changed
            ))
            .is_err()
        );
        assert_eq!(
            verify_protocol_capture(
                MANIFEST,
                MANIFEST_SHA256,
                "fixtures/protocol/not-captured.json",
                source
            ),
            Err("protocol capture has no unique witness")
        );
        assert_eq!(
            verify_protocol_capture(MANIFEST, &"0".repeat(64), path, source),
            Err("protocol manifest digest mismatch")
        );
    }
}
