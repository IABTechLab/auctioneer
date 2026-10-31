//! Verbatim Rise source captures, including the named shared media boundary difference.

#[cfg(test)]
mod tests {
    use super::super::super::RiseBidder;
    use crate::Bidder as _;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        BidderError, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse, EntryPoint,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    const MANIFEST: &str = include_str!("../bidder.toml");
    // Verify the adjacent provenance and exact capture bytes before dispatch.
    const MANIFEST_SHA256: &str =
        "9408827eaa02eace8eed20605304e214263daf76650c1ba55905487fc1430c76";

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("rise"),
            BidderName::new("rise"),
            "account",
            None,
            "http://localhost/prebid_server",
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
        let (semantic, calls) = harness::drive(&RiseBidder, &request, &info());
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        if let Some(source_errors) = fixture.get("expectedMakeRequestsErrors") {
            let expected = request_error(name, source_errors);
            assert!(fixture.get("httpCalls").is_none());
            assert!(calls.is_empty());
            assert_eq!(semantic.unwrap_err(), [expected]);
            return;
        }
        semantic.unwrap();
        let source_calls = fixture["httpCalls"].as_array().unwrap();
        assert_eq!(source_calls.len(), 1);
        assert_eq!(calls.len(), 1);
        assert_request_capture(&calls[0], &source_calls[0]["expectedRequest"]);
        let mock = &source_calls[0]["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let result = RiseBidder.make_bids(
            &request,
            calls[0].meta(),
            &harness::budgeted_response(status, serde_json::to_vec(&mock["body"]).unwrap()),
        );
        if name == "shared_unoffered_banner_boundary" {
            assert_unoffered_banner_boundary(&fixture, mock, &result.unwrap_err());
            return;
        }
        if let Some(source_errors) = fixture.get("expectedMakeBidsErrors") {
            assert_response_error(name, &fixture, source_errors, &result.unwrap_err());
            return;
        }
        assert_response_success(&fixture, mock, &result.unwrap());
    }

    fn request_error(name: &str, source_errors: &Value) -> BidderError {
        assert_eq!(source_errors.as_array().unwrap().len(), 1);
        let (source_message, expected) = match name {
            "source_missing_bidder" => (
                "extractOrg: unmarshal ImpExtRise: expect { or n, but found",
                "extractOrg: invalid rise parameters",
            ),
            "source_missing_extension" => (
                "extractOrg: unmarshal bidderExt: expect { or n, but found",
                "extractOrg: invalid rise parameters",
            ),
            "source_missing_identifier" => (
                "extractOrg: no org or publisher_id supplied",
                "extractOrg: no org or publisher_id supplied",
            ),
            _ => panic!("unexpected request failure capture"),
        };
        assert_eq!(source_errors[0]["value"], source_message);
        BidderError::unknown(expected)
    }

    fn assert_request_capture(call: &BidderHttpRequest, expected: &Value) {
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), expected["uri"].as_str().unwrap());
        assert_eq!(
            call.headers().collect::<Vec<_>>(),
            [("Content-Type", "application/json;charset=utf-8")]
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
    }

    fn assert_unoffered_banner_boundary(fixture: &Value, mock: &Value, errors: &[BidderError]) {
        // Source accepts banner mtype for a video-only imp; shared compatibility refuses it.
        let imp = &fixture["mockBidRequest"]["imp"][1];
        assert!(imp.get("video").is_some());
        assert!(imp.get("banner").is_none());
        assert_eq!(
            fixture["expectedBidResponses"][0]["bids"][0]["type"],
            "banner"
        );
        assert_eq!(mock["body"]["seatbid"][0]["bid"][0]["mtype"], json!(1_i32));
        assert_eq!(
            errors,
            [BidderError::bad_server_response(
                "bid response media type was not offered by its impression"
            )]
        );
    }

    fn assert_response_error(
        name: &str,
        fixture: &Value,
        source_errors: &Value,
        errors: &[BidderError],
    ) {
        match name {
            "source_http_bad_request" => {
                assert_eq!(source_errors.as_array().unwrap().len(), 1);
                assert_eq!(
                    source_errors[0]["value"],
                    "Unexpected status code: 400. Run with request.debug = 1 for more info"
                );
                assert_eq!(
                    errors,
                    [BidderError::bad_input(
                        "Unexpected status code: 400. Run with request.debug = 1 for more info"
                    )]
                );
            }
            "source_unsupported_mtype" => {
                assert_eq!(source_errors.as_array().unwrap().len(), 2);
                assert_eq!(source_errors[0]["value"], "unsupported MType 0");
                assert_eq!(source_errors[1]["value"], "unsupported MType 3");
                assert_eq!(fixture["expectedBidResponses"][0]["currency"], "USD");
                assert!(
                    fixture["expectedBidResponses"][0]["bids"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(
                    errors,
                    [
                        BidderError::unknown("unsupported MType 0"),
                        BidderError::unknown("unsupported MType 3")
                    ]
                );
            }
            _ => panic!("unexpected response failure capture"),
        }
    }

    fn assert_response_success(fixture: &Value, mock: &Value, decoded: &BidderResponse) {
        assert_eq!(decoded.currency, mock["body"]["cur"].as_str().unwrap());
        assert!(decoded.errors.is_empty());
        let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
        assert_eq!(expected_responses.len(), 1);
        let expected_bids = expected_responses[0]["bids"].as_array().unwrap();
        assert_eq!(expected_bids.len(), 1);
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(
            serde_json::to_value(&decoded.bids[0].bid).unwrap(),
            expected_bids[0]["bid"]
        );
        let expected_type = match expected_bids[0]["type"].as_str().unwrap() {
            "banner" => MediaType::Banner,
            "video" => MediaType::Video,
            "native" => MediaType::Native,
            _ => panic!("unexpected capture media type"),
        };
        assert_eq!(decoded.bids[0].media_type, expected_type);
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        source_app => "exemplary/banner-and-video-app.json",
        source_gdpr => "exemplary/banner-and-video-gdpr.json",
        source_site => "exemplary/banner-and-video-site.json",
        shared_unoffered_banner_boundary => "exemplary/banner-and-video.json",
        source_org_precedence => "exemplary/simple-banner-both-ids.json",
        source_banner => "exemplary/simple-banner.json",
        source_native => "exemplary/simple-native.json",
        source_video => "exemplary/simple-video.json",
        source_http_bad_request => "supplemental/bad-request.json",
        source_missing_bidder => "supplemental/missing-bidder.json",
        source_missing_extension => "supplemental/missing-extension.json",
        source_unsupported_mtype => "supplemental/missing-mtype.json",
        source_missing_identifier => "supplemental/missing-org.json",
    );

    #[test]
    fn capture_inventory_is_exactly_seven_positive_five_negative_and_one_shared_boundary() {
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        let mut paths: Vec<_> = artifacts
            .iter()
            .filter(|artifact| artifact["category"].as_str() == Some("protocol"))
            .map(|artifact| artifact["retained"]["source_path"].as_str().unwrap())
            .collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                "fixtures/protocol/exemplary/banner-and-video-app.json",
                "fixtures/protocol/exemplary/banner-and-video-gdpr.json",
                "fixtures/protocol/exemplary/banner-and-video-site.json",
                "fixtures/protocol/exemplary/banner-and-video.json",
                "fixtures/protocol/exemplary/simple-banner-both-ids.json",
                "fixtures/protocol/exemplary/simple-banner.json",
                "fixtures/protocol/exemplary/simple-native.json",
                "fixtures/protocol/exemplary/simple-video.json",
                "fixtures/protocol/supplemental/bad-request.json",
                "fixtures/protocol/supplemental/missing-bidder.json",
                "fixtures/protocol/supplemental/missing-extension.json",
                "fixtures/protocol/supplemental/missing-mtype.json",
                "fixtures/protocol/supplemental/missing-org.json",
            ]
        );
        let bytes: i64 = artifacts
            .iter()
            .filter(|artifact| artifact["category"].as_str() == Some("protocol"))
            .map(|artifact| artifact["retained"]["source_bytes"].as_integer().unwrap())
            .sum();
        assert_eq!(bytes, 36_627);
        assert_eq!(artifacts.len(), 29);
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["rise"]
        );
        assert_eq!(definitions["rise"]["defaultEnabled"].as_bool(), Some(false));
        assert!(definitions["rise"].get("aliasOf").is_none());
    }

    #[test]
    fn parse_equivalent_capture_edits_are_refused_before_filtered_protocol_dispatch() {
        let source = include_str!("../fixtures/protocol/exemplary/simple-banner.json");
        let changed = source.replacen('\n', " ", 1);
        assert_ne!(source, changed);
        assert_eq!(source.len(), changed.len());
        assert_eq!(
            serde_json::from_str::<Value>(source).unwrap(),
            serde_json::from_str::<Value>(&changed).unwrap()
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
    }
}
