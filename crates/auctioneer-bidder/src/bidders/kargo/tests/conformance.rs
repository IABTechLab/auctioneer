//! Immutable source protocol captures, not serving or complete migration evidence.

#[cfg(test)]
mod tests {
    use super::super::super::KargoBidder;
    use crate::Bidder as _;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{BidderName, BidderRequestInfo, EntryPoint, OutboundRequestMeta};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::field::JsonField;
    use serde_json::{Value, json};
    use std::panic::catch_unwind;

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "76c2264e2010e4e03e3a615c84974eeaf6a1e84236de357f111105b1d4acd518";

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
        let expected_calls = fixture["httpCalls"].as_array().unwrap();
        assert_eq!(expected_calls.len(), 1);
        let expected = &expected_calls[0]["expectedRequest"];
        let endpoint = expected["uri"].as_str().unwrap();
        let info = BidderRequestInfo::new(
            BidderName::new("kargo"),
            BidderName::new("kargo"),
            "account",
            None,
            endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let calls = harness::drive_ok(&KargoBidder, &request, &info);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].method(), "POST");
        assert_eq!(calls[0].url(), endpoint);
        assert_eq!(
            calls[0].headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json"),
                ("Accept", "application/json")
            ]
        );
        assert_eq!(
            calls[0].covered_imp_ids().collect::<Vec<_>>(),
            expected["impIDs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        let expected_body = &expected["body"];
        if matches!(name, "captured_video" | "captured_no_content") {
            assert_eq!(
                fixture["mockBidRequest"]["imp"][0]["video"].get("mimes"),
                Some(&Value::Null)
            );
            assert!(matches!(
                request.imp[0].video.as_ref().unwrap().mimes,
                JsonField::Null
            ));
            assert_eq!(
                expected_body["imp"][0]["video"].get("mimes"),
                Some(&Value::Null)
            );
        }
        assert_eq!(
            serde_json::from_slice::<Value>(calls[0].body()).unwrap(),
            *expected_body
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);

        assert_response(&fixture, &request, calls[0].meta());
    }

    fn assert_response(fixture: &Value, request: &BidRequest, sent: &OutboundRequestMeta) {
        let mock = &fixture["httpCalls"][0]["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let body = mock
            .get("body")
            .map_or_else(Vec::new, |value| serde_json::to_vec(value).unwrap());
        let decoded =
            KargoBidder.make_bids(request, sent, &harness::budgeted_response(status, body));
        if let Some(errors) = fixture.get("expectedMakeBidsErrors") {
            let actual = decoded.unwrap_err();
            let expected_errors = errors.as_array().unwrap();
            assert_eq!(expected_errors.len(), 1);
            assert_eq!(actual.len(), expected_errors.len());
            assert_eq!(actual[0].code, 4);
            assert_eq!(
                actual[0].message,
                expected_errors[0]["value"].as_str().unwrap()
            );
            return;
        }
        let response = decoded.unwrap();
        assert!(response.errors.is_empty());
        assert_eq!(
            response.currency,
            mock["body"]
                .get("cur")
                .and_then(Value::as_str)
                .unwrap_or("USD")
        );
        let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
        if status == 204 {
            assert!(expected_responses.is_empty());
            assert!(response.bids.is_empty());
            return;
        }
        assert_eq!(expected_responses.len(), 1);
        let expected_bids = expected_responses[0]["bids"].as_array().unwrap();
        assert_eq!(response.bids.len(), expected_bids.len());
        assert_eq!(expected_bids.len(), 1);
        for (actual, expected_bid) in response.bids.iter().zip(expected_bids) {
            let mut expected_value = expected_bid["bid"].clone();
            // Bid.price is f64; normalize only its numeric representation,
            // never rebuild captured expected objects through Auctioneer's model.
            expected_value["price"] = json!(expected_value["price"].as_f64().unwrap());
            assert_eq!(serde_json::to_value(&actual.bid).unwrap(), expected_value);
            assert_eq!(
                actual.media_type,
                match expected_bid["type"].as_str().unwrap() {
                    "banner" => MediaType::Banner,
                    "video" => MediaType::Video,
                    "native" => MediaType::Native,
                    other => panic!("unknown captured format: {other}"),
                }
            );
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_banner => "exemplary/banner.json",
        captured_video => "exemplary/video.json",
        captured_native => "exemplary/native.json",
        captured_bad_request => "supplemental/status-bad-request.json",
        captured_no_content => "supplemental/status-no-content.json",
    );

    #[test]
    fn every_protocol_capture_has_exactly_one_verbatim_source_witness() {
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
                "fixtures/protocol/exemplary/banner.json",
                "fixtures/protocol/exemplary/native.json",
                "fixtures/protocol/exemplary/video.json",
                "fixtures/protocol/supplemental/status-bad-request.json",
                "fixtures/protocol/supplemental/status-no-content.json",
            ]
        );
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["kargo"]
        );
        assert_eq!(
            definitions["kargo"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["kargo"].get("aliasOf").is_none());
        assert_eq!(
            manifest["provenance"]["sources"]["upstream"]["revision"].as_str(),
            Some("d7f8515b86258688304b0d9b6668c6a0e258bc9e")
        );
        assert_eq!(artifacts.len(), 12);
    }

    #[test]
    fn filtered_protocol_dispatch_refuses_changed_bytes_before_execution() {
        let source = include_str!("../fixtures/protocol/exemplary/banner.json");
        run_fixture("banner", "exemplary/banner.json", source);
        let changed = source.replacen('\n', " ", 1);
        assert_eq!(changed.len(), source.len());
        assert_eq!(
            verify_protocol_capture(
                MANIFEST,
                MANIFEST_SHA256,
                "fixtures/protocol/exemplary/banner.json",
                &changed
            ),
            Err("protocol capture digest mismatch")
        );
        assert!(catch_unwind(|| run_fixture("banner", "exemplary/banner.json", &changed)).is_err());
        assert_eq!(
            verify_protocol_capture(
                MANIFEST,
                MANIFEST_SHA256,
                "fixtures/protocol/not-captured.json",
                source
            ),
            Err("protocol capture has no unique witness")
        );
        let changed_manifest = MANIFEST.replacen("factory = true", "factory =false", 1);
        assert_ne!(changed_manifest, MANIFEST);
        assert_eq!(
            verify_protocol_capture(
                &changed_manifest,
                MANIFEST_SHA256,
                "fixtures/protocol/exemplary/banner.json",
                source
            ),
            Err("protocol manifest digest mismatch")
        );
    }
}
