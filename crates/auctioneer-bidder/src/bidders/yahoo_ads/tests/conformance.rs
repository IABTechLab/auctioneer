//! Immutable Yahoo captures, with explicit transport and shared-boundary differences.

#[cfg(test)]
mod tests {
    use super::super::super::YahooAdsBidder;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::correlation::CorrelationMedia;
    use crate::{
        Bidder as _, BidderError, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse,
        ConstructionDiagnostic, EndpointRejectReason, EndpointValidator, EntryPoint, SinkLimits,
        SinkOutcome,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};
    use std::{collections::BTreeMap, panic::catch_unwind};

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "c83149b1418026b6b9c962379a37ff412726c01505420db291305267ceb9d90b";
    // Source captures deliberately keep uri="". This is not a production route.
    const TEST_ENDPOINT: &str = "https://bidder.example.test/yahoo-capture";

    struct TestEndpoint;

    impl EndpointValidator for TestEndpoint {
        fn validate(
            &self,
            url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if url.as_str() == TEST_ENDPOINT {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
    }

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("yahooAds"),
            BidderName::new("yahoo_ads"),
            "account",
            None,
            TEST_ENDPOINT,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

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
        let (semantic, outcome, _) = harness::drive_with_limits(
            &YahooAdsBidder,
            &request,
            &info(),
            SinkLimits::new(256 * 1024, 1),
            1,
            &TestEndpoint,
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        assert!(outcome.errors.is_empty());

        if let Some(source_errors) = fixture.get("expectedMakeRequestsErrors") {
            assert_request_failure(name, &fixture, source_errors, semantic, &outcome);
            return;
        }

        let source_calls = fixture["httpCalls"].as_array().unwrap();
        assert_eq!(source_calls.len(), 1);
        let expected = &source_calls[0]["expectedRequest"];
        assert_eq!(expected["uri"], "");
        if name == "captured_unsupported_media_boundary" {
            assert_unsupported_media(&fixture, expected, semantic, &outcome);
            return;
        }

        semantic.unwrap();
        assert!(outcome.diagnostics.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        let call = &outcome.accepted[0];
        assert_request(&fixture, expected, call);
        assert_response(name, &fixture, &request, call, &before);
    }

    fn assert_request_failure(
        name: &str,
        fixture: &Value,
        source_errors: &Value,
        semantic: Result<(), Vec<BidderError>>,
        outcome: &SinkOutcome,
    ) {
        assert_eq!(source_errors.as_array().unwrap().len(), 1);
        assert!(fixture.get("httpCalls").is_none());
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            semantic.unwrap_err(),
            [BidderError::bad_input("no valid yahoo ads impressions")]
        );
        let (source_message, diagnostic) = match name {
            "captured_missing_extension" => (
                "imp #0: ext.bidder not provided",
                ConstructionDiagnostic::InvalidImpressionParameters,
            ),
            "captured_empty_format_direct_adapter_schema_boundary" => (
                "No sizes provided for Banner []",
                ConstructionDiagnostic::InvalidImpression,
            ),
            "captured_zero_height_direct_adapter_schema_boundary" => (
                "Invalid sizes provided for Banner 728x0",
                ConstructionDiagnostic::InvalidImpression,
            ),
            "captured_zero_width_direct_adapter_schema_boundary" => (
                "Invalid sizes provided for Banner 0x90",
                ConstructionDiagnostic::InvalidImpression,
            ),
            _ => panic!("unexpected request-error capture"),
        };
        assert_eq!(source_errors[0]["value"], source_message);
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [diagnostic]
        );
        if name.ends_with("direct_adapter_schema_boundary") {
            // These original captures omit source-schema-required pos.
            // Direct adapter decoding must still reach banner validation.
            let schema: Value = serde_json::from_str(include_str!("../parameters.json")).unwrap();
            assert_eq!(schema["required"], serde_json::json!(["dcn", "pos"]));
            assert!(
                fixture["mockBidRequest"]["imp"][0]["ext"]["bidder"]
                    .get("pos")
                    .is_none()
            );
        }
    }

    fn assert_unsupported_media(
        fixture: &Value,
        expected: &Value,
        semantic: Result<(), Vec<BidderError>>,
        outcome: &SinkOutcome,
    ) {
        // The source emits this unsupported audio call and ignores its bid.
        // The candidate instead refuses the work locally, explicitly.
        assert!(fixture["mockBidRequest"]["imp"][0].get("audio").is_some());
        assert!(expected["body"]["imp"][0].get("audio").is_some());
        assert_eq!(fixture["expectedBidResponses"][0]["currency"], "USD");
        assert!(
            fixture["expectedBidResponses"][0]["bids"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            semantic.unwrap_err(),
            [BidderError::bad_input("no valid yahoo ads impressions")]
        );
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::UnsupportedMediaType]
        );
    }

    fn assert_request(fixture: &Value, expected: &Value, call: &BidderHttpRequest) {
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), TEST_ENDPOINT);
        assert_ne!(call.url(), expected["uri"].as_str().unwrap());
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
        if let Some(headers) = expected.get("headers") {
            let source_headers: BTreeMap<String, Vec<String>> =
                serde_json::from_value(headers.clone()).unwrap();
            let expected_headers: BTreeMap<_, _> = source_headers
                .into_iter()
                .map(|(key, value)| (key.to_ascii_lowercase(), value))
                .collect();
            let actual_headers: BTreeMap<_, _> = call
                .headers()
                .map(|(key, value)| (key.to_ascii_lowercase(), vec![value.to_owned()]))
                .collect();
            assert_eq!(actual_headers, expected_headers);
        }
        let expected_media = if fixture["mockBidRequest"]["imp"][0].get("banner").is_some() {
            CorrelationMedia::Banner
        } else {
            CorrelationMedia::Video
        };
        assert_eq!(
            call.meta().imp_correlation(0).unwrap().media(),
            expected_media
        );
    }

    fn assert_response(
        name: &str,
        fixture: &Value,
        request: &BidRequest,
        call: &BidderHttpRequest,
        before: &Value,
    ) {
        let mock = &fixture["httpCalls"][0]["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let decoded = YahooAdsBidder.make_bids(
            request,
            call.meta(),
            &harness::budgeted_response(status, serde_json::to_vec(&mock["body"]).unwrap()),
        );
        assert_eq!(&serde_json::to_value(request).unwrap(), before);
        if let Some(source_errors) = fixture.get("expectedMakeBidsErrors") {
            assert_response_failure(name, source_errors, decoded);
            return;
        }
        assert_response_success(fixture, &decoded.unwrap());
    }

    fn assert_response_failure(
        name: &str,
        source_errors: &Value,
        decoded: Result<BidderResponse, Vec<BidderError>>,
    ) {
        assert_eq!(source_errors.as_array().unwrap().len(), 1);
        let (source_message, local_message) = match name {
            "captured_server_error" => (
                "Unexpected status code: 500.",
                "unexpected yahoo ads response status",
            ),
            "captured_unknown_impression" => (
                "Unknown ad unit code 'wrong'",
                "bid response references an unsent impression",
            ),
            _ => panic!("unexpected response-error capture"),
        };
        assert_eq!(source_errors[0]["value"], source_message);
        assert_eq!(
            decoded.unwrap_err(),
            [BidderError::bad_server_response(local_message)]
        );
    }

    fn assert_response_success(fixture: &Value, decoded: &BidderResponse) {
        assert_eq!(decoded.currency, "USD");
        assert!(decoded.errors.is_empty());
        let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
        assert_eq!(expected_responses.len(), 1);
        assert_eq!(expected_responses[0]["currency"], "USD");
        let expected_bids = expected_responses[0]["bids"].as_array().unwrap();
        assert_eq!(expected_bids.len(), 1);
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(
            serde_json::to_value(&decoded.bids[0].bid).unwrap(),
            expected_bids[0]["bid"]
        );
        let media = match expected_bids[0]["type"].as_str().unwrap() {
            "banner" => MediaType::Banner,
            "video" => MediaType::Video,
            _ => panic!("unexpected source media"),
        };
        assert_eq!(decoded.bids[0].media_type, media);
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_app_banner => "exemplary/simple-app-banner.json",
        captured_banner => "exemplary/simple-banner.json",
        captured_video => "exemplary/simple-video.json",
        captured_empty_format_direct_adapter_schema_boundary => "supplemental/empty-banner-format.json",
        captured_zero_height_direct_adapter_schema_boundary => "supplemental/invalid-banner-height.json",
        captured_zero_width_direct_adapter_schema_boundary => "supplemental/invalid-banner-width.json",
        captured_unsupported_media_boundary => "supplemental/non-supported-requests-bids-ignored.json",
        captured_missing_extension => "supplemental/required-nobidder-info.json",
        captured_server_error => "supplemental/server-error.json",
        captured_unknown_impression => "supplemental/server-response-wrong-impid.json",
        captured_gpp_overwrite => "supplemental/simple-banner-gpp-overwrite.json",
        captured_gpp => "supplemental/simple-banner-gpp.json",
        captured_partial_dimensions => "supplemental/simple-banner-ignore-width-when-height-missing.json",
    );

    #[test]
    fn inventory_has_thirteen_verbatim_captures_and_default_inactive_native_publication() {
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["key"].as_str(), Some("yahoo_ads"));
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["yahooAds"]
        );
        assert_eq!(
            definitions["yahooAds"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["yahooAds"].get("aliasOf").is_none());
        assert_eq!(
            manifest["provenance"]["sources"]["upstream"]["revision"].as_str(),
            Some("d7f8515b86258688304b0d9b6668c6a0e258bc9e")
        );
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 31);
        let captures: Vec<_> = artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .collect();
        assert_eq!(captures.len(), 13);
        assert_eq!(
            captures
                .iter()
                .map(|row| row["source_bytes"].as_integer().unwrap())
                .sum::<i64>(),
            25_589
        );
        assert_eq!(
            captures
                .iter()
                .map(|row| row["retained"]["source_path"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "fixtures/protocol/exemplary/simple-app-banner.json",
                "fixtures/protocol/exemplary/simple-banner.json",
                "fixtures/protocol/exemplary/simple-video.json",
                "fixtures/protocol/supplemental/empty-banner-format.json",
                "fixtures/protocol/supplemental/invalid-banner-height.json",
                "fixtures/protocol/supplemental/invalid-banner-width.json",
                "fixtures/protocol/supplemental/non-supported-requests-bids-ignored.json",
                "fixtures/protocol/supplemental/required-nobidder-info.json",
                "fixtures/protocol/supplemental/server-error.json",
                "fixtures/protocol/supplemental/server-response-wrong-impid.json",
                "fixtures/protocol/supplemental/simple-banner-gpp-overwrite.json",
                "fixtures/protocol/supplemental/simple-banner-gpp.json",
                "fixtures/protocol/supplemental/simple-banner-ignore-width-when-height-missing.json",
            ]
        );
        for capture in captures {
            assert_eq!(capture["source_bytes"], capture["retained"]["source_bytes"]);
            assert_eq!(
                capture["source_sha256"],
                capture["retained"]["source_sha256"]
            );
            assert!(capture.get("transformation").is_none());
        }
        let path = "parameters.json";
        let source = include_str!("../parameters.json");
        let matching: Vec<_> = artifacts
            .iter()
            .filter(|row| {
                row.get("retained")
                    .and_then(|value| value.get("source_path"))
                    .and_then(toml::Value::as_str)
                    == Some(path)
            })
            .collect();
        assert_eq!(matching.len(), 1);
        let row = matching[0];
        assert_eq!(
            row["source_bytes"].as_integer(),
            Some(i64::try_from(source.len()).unwrap())
        );
        assert_eq!(
            row["source_sha256"].as_str(),
            Some(format!("{:x}", Sha256::digest(source.as_bytes())).as_str())
        );
    }

    #[test]
    fn changed_capture_is_refused_before_filtered_protocol_dispatch() {
        let source = include_str!("../fixtures/protocol/exemplary/simple-banner.json");
        let changed = source.replacen('\n', " ", 1);
        assert_eq!(changed.len(), source.len());
        assert_eq!(
            serde_json::from_str::<Value>(&changed).unwrap(),
            serde_json::from_str::<Value>(source).unwrap()
        );
        assert_eq!(
            verify_protocol_capture(
                MANIFEST,
                MANIFEST_SHA256,
                "fixtures/protocol/exemplary/simple-banner.json",
                source
            ),
            Ok(())
        );
        assert_eq!(
            verify_protocol_capture(
                MANIFEST,
                MANIFEST_SHA256,
                "fixtures/protocol/exemplary/simple-banner.json",
                &changed
            ),
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
    }
}
