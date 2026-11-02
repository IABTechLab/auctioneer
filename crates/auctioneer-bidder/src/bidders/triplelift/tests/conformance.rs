//! Immutable source captures; typed-boundary differences are not protocol parity.

#[cfg(test)]
mod tests {
    use super::super::super::TripleliftBidder;
    use crate::Bidder as _;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        BidderError, BidderErrorKind, BidderHttpRequest, BidderName, BidderRequestInfo,
        BidderResponse, EntryPoint, OutboundRequestMeta,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, error::Category, json};
    use std::panic::catch_unwind;

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "c2a45d4b3cc28b728efaf2fec766bd4cd860adba168de4397e949d2fb3e7831c";

    fn info(endpoint: &str) -> BidderRequestInfo<'_> {
        BidderRequestInfo::new(
            BidderName::new("triplelift"),
            BidderName::new("triplelift"),
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

    fn run_fixture(name: &str, path: &str, source: &str) {
        verify_protocol_capture(
            MANIFEST,
            MANIFEST_SHA256,
            &format!("fixtures/protocol/{path}"),
            source,
        )
        .unwrap();
        let fixture: Value = serde_json::from_str(source).unwrap();
        let typed = serde_json::from_value::<BidRequest>(fixture["mockBidRequest"].clone());
        if name == "typed_ingress_refusal" {
            // Source accepts the raw ext then rejects it inside the bidder;
            // the shared typed ingress rejects it earlier. No bidder dispatch.
            assert_eq!(fixture["mockBidRequest"]["imp"][0]["ext"], json!("aaa"));
            assert_eq!(typed.unwrap_err().classify(), Category::Data);
            assert!(fixture["httpCalls"].as_array().unwrap().is_empty());
            assert_eq!(
                fixture["expectedMakeRequestsErrors"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            return;
        }
        let request = typed.unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let endpoint = fixture["httpCalls"][0]["expectedRequest"]["uri"]
            .as_str()
            .unwrap_or("https://bidder.example.test/auction?sra=1&supplier_id=20");
        let (semantic, calls) = harness::drive(&TripleliftBidder, &request, &info(endpoint));
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        if let Some(expected_errors) = fixture.get("expectedMakeRequestsErrors") {
            assert!(matches!(
                name,
                "captured_bad_bidder" | "captured_unsupported_imp"
            ));
            assert_eq!(expected_errors.as_array().unwrap().len(), 2);
            assert!(calls.is_empty());
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::unknown("No valid impressions for triplelift")]
            );
            assert!(fixture["httpCalls"].as_array().unwrap().is_empty());
            return;
        }
        semantic.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(fixture["httpCalls"].as_array().unwrap().len(), 1);
        assert_request(name, &fixture["httpCalls"][0]["expectedRequest"], &calls[0]);
        assert_response(name, &fixture, &request, calls[0].meta());
    }

    fn assert_request(name: &str, expected: &Value, actual: &BidderHttpRequest) {
        assert_eq!(actual.method(), "POST");
        assert_eq!(actual.url(), expected["uri"].as_str().unwrap());
        assert_eq!(
            actual.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json")
            ]
        );
        assert_eq!(
            actual.covered_imp_ids().collect::<Vec<_>>(),
            expected["impIDs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        let mut body = expected["body"].clone();
        if name == "captured_optional_params" {
            // Source writes integral float64 as 20; the Rust f64 is 20.0.
            // Normalize only this demonstrated numeric representation.
            assert_eq!(body["imp"][0]["bidfloor"].as_i64(), Some(20));
            body["imp"][0]["bidfloor"] = json!(20.0_f64);
        }
        assert_eq!(
            serde_json::from_slice::<Value>(actual.body()).unwrap(),
            body
        );
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
            TripleliftBidder.make_bids(request, sent, &harness::budgeted_response(status, body));
        if let Some(source_errors) = fixture.get("expectedMakeBidsErrors") {
            let errors = decoded.unwrap_err();
            assert_eq!(source_errors.as_array().unwrap().len(), 1);
            assert_eq!(errors.len(), 1);
            if name == "typed_response_refusal" {
                // Non-object bid.ext fails the existing sealed envelope parser,
                // not the source's later per-bid extension decoder.
                assert_eq!(status, 200);
                assert_eq!(mock["body"]["seatbid"][0]["bid"][0]["ext"], json!("aaa"));
                assert_eq!(errors[0].code, 14);
                assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
                assert_eq!(errors[0].message, "invalid bidder response");
            } else {
                assert!(matches!(status, 400 | 302));
                assert_eq!(errors[0].code, if status == 400 { 2 } else { 999 });
                assert_eq!(
                    errors[0].message,
                    source_errors[0]["value"].as_str().unwrap()
                );
            }
            return;
        }
        let response = decoded.unwrap();
        assert_success(
            &response,
            fixture["expectedBidResponses"].as_array().unwrap(),
            status,
        );
    }

    fn assert_success(response: &BidderResponse, expected: &[Value], status: u16) {
        assert_eq!(response.currency, "USD");
        assert!(response.errors.is_empty());
        if status == 204 {
            assert!(expected.is_empty());
            assert!(response.bids.is_empty());
            return;
        }
        assert_eq!(expected.len(), 1);
        assert_eq!(expected[0]["currency"], json!("USD"));
        let expected_bids = expected[0]["bids"].as_array().unwrap();
        assert_eq!(expected_bids.len(), 1);
        assert_eq!(response.bids.len(), expected_bids.len());
        for (actual, expected_bid) in response.bids.iter().zip(expected_bids) {
            assert_eq!(
                serde_json::to_value(&actual.bid).unwrap(),
                expected_bid["bid"]
            );
            assert_eq!(
                actual.media_type,
                match expected_bid["type"].as_str().unwrap() {
                    "banner" => MediaType::Banner,
                    "video" => MediaType::Video,
                    other => panic!("unknown captured format: {other}"),
                }
            );
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_optional_params => "exemplary/optional-params.json",
        captured_banner => "exemplary/simple-banner.json",
        captured_video => "exemplary/simple-video.json",
        typed_ingress_refusal => "supplemental/badext.json",
        captured_bad_bidder => "supplemental/badextbidder.json",
        captured_unsupported_imp => "supplemental/badlyformed.json",
        typed_response_refusal => "supplemental/badresponseext.json",
        captured_bad_request => "supplemental/badstatuscode.json",
        captured_other_status => "supplemental/notgoodstatuscode.json",
        captured_video_11 => "supplemental/video-format-11.json",
        captured_video_12 => "supplemental/video-format-12.json",
        captured_video_17 => "supplemental/video-format-17.json",
    );

    #[test]
    fn captured_case_membership_keeps_typed_boundaries_distinct_from_protocol_dispatch() {
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        let actual: Vec<_> = artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .map(|row| row["retained"]["source_path"].as_str().unwrap())
            .collect();
        assert_eq!(
            actual,
            [
                "fixtures/protocol/exemplary/optional-params.json",
                "fixtures/protocol/exemplary/simple-banner.json",
                "fixtures/protocol/exemplary/simple-video.json",
                "fixtures/protocol/supplemental/badext.json",
                "fixtures/protocol/supplemental/badextbidder.json",
                "fixtures/protocol/supplemental/badlyformed.json",
                "fixtures/protocol/supplemental/badresponseext.json",
                "fixtures/protocol/supplemental/badstatuscode.json",
                "fixtures/protocol/supplemental/notgoodstatuscode.json",
                "fixtures/protocol/supplemental/video-format-11.json",
                "fixtures/protocol/supplemental/video-format-12.json",
                "fixtures/protocol/supplemental/video-format-17.json",
            ]
        );
        assert_eq!(artifacts.len(), 22);
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["triplelift"]
        );
        assert_eq!(
            definitions["triplelift"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["triplelift"].get("aliasOf").is_none());
    }

    #[test]
    fn filtered_dispatch_rejects_parse_equivalent_capture_edits_before_execution() {
        let source = include_str!("../fixtures/protocol/exemplary/simple-banner.json");
        run_fixture("captured_banner", "exemplary/simple-banner.json", source);
        let changed = source.replacen('\n', " ", 1);
        assert_ne!(changed, source);
        assert_eq!(changed.len(), source.len());
        assert_eq!(
            serde_json::from_str::<Value>(&changed).unwrap(),
            serde_json::from_str::<Value>(source).unwrap()
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
