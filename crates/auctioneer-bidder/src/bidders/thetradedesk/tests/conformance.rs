//! Original captures and explicit source-versus-shared boundaries, not delivery acceptance.

#[cfg(test)]
mod generated_settings_tests {
    use crate::{
        BidderSettings, bidder_endpoint::thetradedesk::Values, bidder_settings::thetradedesk,
    };
    use serde_json::json;
    use std::ptr;

    const PAIRS: [(&str, &str); 1] = Values {
        supply_id: "constant",
    }
    .as_pairs();

    #[test]
    fn generated_settings_keep_owned_field_and_implementation_key() {
        let settings: BidderSettings = serde_json::from_value(json!({
            "implementation": "thetradedesk", "settings": {}
        }))
        .unwrap();
        assert_eq!(settings.implementation_key(), "thetradedesk");
        assert_eq!(settings.as_thetradedesk().unwrap().supply_id, None);
        assert!(settings.as_msft().is_none());

        let wire = json!({
            "implementation": "thetradedesk", "settings": {"supply_id": "reference-route"}
        });
        let populated: BidderSettings = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(
            thetradedesk::resolve(populated.as_thetradedesk(), None).supply_id(),
            Some("reference-route")
        );
        assert_eq!(serde_json::to_value(&populated).unwrap(), wire);
    }

    #[test]
    fn settings_refuse_public_aliases_and_another_owners_field() {
        for wire in [
            json!({"implementation": "ttd", "settings": {}}),
            json!({"implementation": "thetradedesk", "settings": {"hb_source": 5_i64}}),
        ] {
            assert!(
                serde_json::from_value::<BidderSettings>(wire)
                    .unwrap_err()
                    .is_data()
            );
        }
    }

    #[test]
    fn generated_bindings_keep_supply_id_mapping_and_borrowed_layout() {
        assert_eq!(PAIRS, [("SupplyId", "constant")]);
        let value = String::from("caller-owned route");
        let pairs = Values { supply_id: &value }.as_pairs();
        assert_eq!(pairs, [("SupplyId", "caller-owned route")]);
        assert!(ptr::eq(pairs[0].1.as_ptr(), value.as_ptr()));
        assert_eq!(size_of::<Values<'_>>(), size_of::<&str>());
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{TheTradeDeskBidder, invalid_parameters};
    use crate::BidderSettings;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        Bidder as _, BidderError, BidderErrorKind, BidderHttpRequest, BidderName,
        BidderRequestInfo, BidderResponse, EndpointRejectReason, EndpointValidator, EntryPoint,
        SinkLimits,
    };
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde::Deserialize;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{
        collections::{BTreeMap, BTreeSet},
        env::var_os,
        process::Command,
        str::from_utf8,
    };

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "f1da2aa0cd1840ef01d238d540afd4ca9006f0f591e9fa9d04d5fd7d76ebdc5e";
    const TEST_TEMPLATE: &str = "https://bidder.example.test/bid/bidder/{{.SupplyId}}";

    struct TestEndpoint;

    impl EndpointValidator for TestEndpoint {
        fn validate(
            &self,
            url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if url.scheme() == "https"
                && url.host_str() == Some("bidder.example.test")
                && url.path().starts_with("/bid/bidder/")
                && url.query().is_none()
                && url.fragment().is_none()
            {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
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
        // TestJsonSamples explicitly supplies this setting. It is not a runtime default.
        let settings: BidderSettings = serde_json::from_value(json!({
            "implementation":"thetradedesk", "settings":{"supply_id":"ttd"},
        }))
        .unwrap();
        let mut info = BidderRequestInfo::new(
            BidderName::new("ttd"),
            BidderName::new("thetradedesk"),
            "account",
            None,
            TEST_TEMPLATE,
            &["SupplyId"],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        info.set_settings(Some("thetradedesk"), None, Some(&settings))
            .unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &TheTradeDeskBidder,
            &request,
            &info,
            SinkLimits::new(256 * 1024, 2),
            1,
            &TestEndpoint,
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        assert!(outcome.errors.is_empty());
        assert!(outcome.diagnostics.iter().next().is_none());
        if let Some(errors) = fixture.get("expectedMakeRequestsErrors") {
            assert!(matches!(
                name,
                "captured_invalid_publisher" | "captured_invalid_supply"
            ));
            assert_eq!(errors.as_array().unwrap().len(), 1);
            assert_eq!(semantic.unwrap_err(), [invalid_parameters()]);
            assert_eq!(fixture["httpCalls"], json!([]));
            assert!(outcome.accepted.is_empty());
            assert_eq!(remaining, 1);
            return;
        }
        semantic.unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(outcome.accepted.len(), 1);
        let http = fixture["httpCalls"].as_array().unwrap();
        assert_eq!(http.len(), 1);
        let expected_request = &http[0]["expectedRequest"];
        let call = &outcome.accepted[0];
        assert_request_call(path, &fixture, expected_request, call);
        assert_captured_response(name, path, &request, call, &fixture);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn assert_request_call(
        path: &str,
        fixture: &Value,
        expected_request: &Value,
        call: &BidderHttpRequest,
    ) {
        assert_eq!(call.method(), "POST");
        let actual_url = url::Url::parse(call.url()).unwrap();
        let source_url = url::Url::parse(expected_request["uri"].as_str().unwrap()).unwrap();
        assert_eq!(source_url.host_str(), Some("direct.adsrvr.org"));
        assert_eq!(actual_url.host_str(), Some("bidder.example.test"));
        assert_eq!(actual_url.path(), source_url.path());
        assert_request_body(
            path,
            &fixture["mockBidRequest"],
            serde_json::from_slice::<Value>(call.body()).unwrap(),
            &expected_request["body"],
        );
        assert_eq!(
            call.covered_imp_ids().collect::<Vec<_>>(),
            expected_request["impIDs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        let expected_headers: BTreeMap<String, Vec<String>> =
            serde_json::from_value(expected_request["headers"].clone()).unwrap();
        let actual_headers: BTreeMap<_, _> = call
            .headers()
            .map(|(header_name, value)| (header_name.to_owned(), vec![value.to_owned()]))
            .collect();
        assert_eq!(actual_headers, expected_headers);
    }

    fn assert_captured_response(
        name: &str,
        path: &str,
        request: &BidRequest,
        call: &BidderHttpRequest,
        fixture: &Value,
    ) {
        let mock = &fixture["httpCalls"][0]["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let bytes = mock
            .get("body")
            .map(|body| serde_json::to_vec(body).unwrap())
            .unwrap_or_default();
        let decoded = TheTradeDeskBidder.make_bids(
            request,
            call.meta(),
            &harness::budgeted_response(status, bytes),
        );
        match name {
            "captured_ortb_video" => assert_video_response(&decoded.unwrap(), fixture),
            "captured_prebidjs_unsent_identity_boundary" => {
                assert_eq!(
                    fixture["expectedBidResponses"][0]["bids"]
                        .as_array()
                        .unwrap()
                        .len(),
                    1
                );
                assert_ne!(
                    mock["body"]["seatbid"][0]["bid"][0]["impid"],
                    fixture["mockBidRequest"]["imp"][0]["id"]
                );
                assert_eq!(
                    decoded.unwrap_err(),
                    [BidderError::bad_server_response(
                        "bid response references an unsent impression"
                    )]
                );
            }
            "captured_empty_200" => {
                assert!(mock.get("body").is_none());
                assert_eq!(
                    fixture["expectedMakeBidsErrors"].as_array().unwrap().len(),
                    1
                );
                assert_parse_error(&decoded.unwrap_err());
            }
            "captured_204" => {
                let response = decoded.unwrap();
                assert_eq!(response.currency, "USD");
                assert!(response.bids.is_empty());
                assert!(response.errors.is_empty());
                assert_eq!(
                    fixture["expectedBidResponses"][0],
                    json!({"currency":"USD", "bids":[]})
                );
            }
            "captured_400" | "captured_500" => {
                let errors = decoded.unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
                assert_eq!(
                    errors[0].message,
                    fixture["expectedMakeBidsErrors"][0]["value"]
                );
            }
            _ => assert_missing_id_response(name, path, mock, fixture, &decoded.unwrap_err()),
        }
    }

    fn assert_video_response(response: &BidderResponse, fixture: &Value) {
        assert!(response.errors.is_empty());
        let expected_response = &fixture["expectedBidResponses"][0];
        assert_eq!(
            response.currency,
            expected_response["currency"].as_str().unwrap()
        );
        assert_eq!(response.bids.len(), 1);
        // No response normalization: price is f64, w/h are i32, and metadata
        // (cid/dur/cat/cattax/adomain/mtype) survives in the flattened map.
        assert_eq!(
            serde_json::to_value(&response.bids[0].bid).unwrap(),
            expected_response["bids"][0]["bid"]
        );
        assert_eq!(response.bids[0].media_type, MediaType::Video);
    }

    fn assert_missing_id_response(
        name: &str,
        path: &str,
        mock: &Value,
        fixture: &Value,
        errors: &[BidderError],
    ) {
        assert!(name.ends_with("missing_id_boundary"));
        assert!(mock["body"].is_object());
        assert!(mock["body"].get("id").is_none());
        assert_parse_error(errors);
        if path == "supplemental/invalid-mtype.json" {
            assert_eq!(
                fixture["expectedMakeBidsErrors"][0]["value"],
                "unsupported mtype: -1"
            );
        } else {
            let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
            assert_eq!(expected_responses.len(), 1);
            assert!(!expected_responses[0]["bids"].as_array().unwrap().is_empty());
        }
    }

    fn assert_request_body(path: &str, input: &Value, mut actual: Value, expected: &Value) {
        // Only these source omissions are allowed. Assert preservation first, then
        // exclude the field from this comparison copy, never from the sent body.
        match path {
            "exemplary/ortb_response_video_parsing.json" => {
                assert_eq!(input["imp"].as_array().unwrap().len(), 1);
                assert_eq!(input.pointer("/test"), Some(&json!(0_i32)));
                assert_eq!(input.pointer("/imp/0/instl"), Some(&json!(0_i32)));
                assert!(input.pointer("/imp/0/video/content").unwrap().is_object());
                assert_preserved_request_field(&mut actual, input, expected, "", "test");
                assert_preserved_request_field(&mut actual, input, expected, "/imp/0", "instl");
                assert_preserved_request_field(
                    &mut actual,
                    input,
                    expected,
                    "/imp/0/video",
                    "content",
                );
            }
            "exemplary/simple-video.json" | "exemplary/use-supply-source-id-param.json" => {
                let parents: &[&str] = if path == "exemplary/simple-video.json" {
                    &["/imp/0/video"]
                } else {
                    &["/imp/0/video", "/imp/1/video"]
                };
                assert_eq!(input["imp"].as_array().unwrap().len(), parents.len());
                for parent in parents {
                    assert_eq!(
                        input.pointer(parent).unwrap().get("minduration"),
                        Some(&json!(0_i32))
                    );
                    assert_preserved_request_field(
                        &mut actual,
                        input,
                        expected,
                        parent,
                        "minduration",
                    );
                }
            }
            _ => {}
        }
        assert_eq!(&actual, expected, "source request body mismatch: {path}");
    }

    fn assert_preserved_request_field(
        actual: &mut Value,
        input: &Value,
        expected: &Value,
        parent: &str,
        key: &str,
    ) {
        let original = input
            .pointer(parent)
            .unwrap()
            .as_object()
            .unwrap()
            .get(key)
            .unwrap();
        assert!(
            !expected
                .pointer(parent)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key(key),
            "source omission changed: {parent}/{key}"
        );
        let actual_parent = actual.pointer_mut(parent).unwrap().as_object_mut().unwrap();
        assert_eq!(
            actual_parent.get(key),
            Some(original),
            "input field was not preserved: {parent}/{key}"
        );
        assert_eq!(actual_parent.remove(key).as_ref(), Some(original));
    }

    fn assert_parse_error(errors: &[BidderError]) {
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 14);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(errors[0].message, "invalid bidder response");
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_ortb_video => "exemplary/ortb_response_video_parsing.json",
        captured_prebidjs_unsent_identity_boundary => "exemplary/prebidjs_response_video_parsing.json",
        captured_banner_inapp_missing_id_boundary => "exemplary/simple-banner-inapp.json",
        captured_formats_multiple_bids_missing_id_boundary =>
            "exemplary/simple-banner-multiple-bids-and-formats.json",
        captured_multiple_bids_missing_id_boundary => "exemplary/simple-banner-multiple-bids.json",
        captured_banner_missing_id_boundary => "exemplary/simple-banner.json",
        captured_empty_publisher_schema_and_missing_id_boundary =>
            "exemplary/simple-empty-publisherId.json",
        captured_multitype_banner_missing_id_boundary => "exemplary/simple-multi-type-banner.json",
        captured_multitype_video_missing_id_boundary => "exemplary/simple-multi-type-video.json",
        captured_native_missing_id_boundary => "exemplary/simple-native.json",
        captured_video_missing_id_boundary => "exemplary/simple-video.json",
        captured_impression_supply_missing_id_boundary => "exemplary/use-supply-source-id-param.json",
        captured_empty_200 => "supplemental/200-response-from-target.json",
        captured_204 => "supplemental/204-response-from-target.json",
        captured_400 => "supplemental/400-response-from-target.json",
        captured_500 => "supplemental/500-response-from-target.json",
        captured_price_macros_missing_id_boundary => "supplemental/auction-price-macro-replacement.json",
        captured_invalid_mtype_missing_id_boundary => "supplemental/invalid-mtype.json",
        captured_invalid_publisher => "supplemental/invalid-publisher.json",
        captured_invalid_supply => "supplemental/invalid-supplySourceId.json",
    );

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Retained {
        #[serde(rename = "source_path")]
        path: String,
        #[serde(rename = "source_bytes")]
        bytes: usize,
        #[serde(rename = "source_sha256")]
        sha256: String,
    }

    fn retained_witness(category: &str, source: &str, bytes: &[u8]) {
        assert_eq!(
            hex::encode(Sha256::digest(MANIFEST.as_bytes())),
            MANIFEST_SHA256
        );
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let matches: Vec<_> = manifest["provenance"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| {
                row["category"].as_str() == Some(category)
                    && row["source_path"].as_str() == Some(source)
            })
            .collect();
        assert_eq!(matches.len(), 1);
        let retained: Retained = matches[0]["retained"].clone().try_into().unwrap();
        assert_eq!(retained.bytes, bytes.len());
        assert_eq!(retained.sha256, hex::encode(Sha256::digest(bytes)));
        assert_eq!(
            retained.path,
            if category == "schema" {
                "parameters.json"
            } else {
                "fixtures/parameters.json"
            }
        );
        assert_eq!(matches[0]["source_blob"].as_str().unwrap().len(), 40);
    }

    #[test]
    fn parameter_captures_bind_three_accepts_nine_rejects_to_immutable_source() {
        let bytes = include_bytes!("../fixtures/parameters.json");
        retained_witness("parameter", "adapters/thetradedesk/params_test.go", bytes);
        retained_witness(
            "schema",
            "static/bidder-params/thetradedesk.json",
            include_bytes!("../parameters.json"),
        );
        let cases: Value = serde_json::from_slice(bytes).unwrap();
        let valid = cases["valid"].as_array().unwrap();
        let invalid = cases["invalid"].as_array().unwrap();
        assert_eq!(valid.len(), 3);
        assert_eq!(invalid.len(), 9);
        let schema: Value = serde_json::from_str(include_str!("../parameters.json")).unwrap();
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft4)
            .build(&schema)
            .unwrap();
        for (rows, expected) in [(valid, true), (invalid, false)] {
            for input in rows {
                let parsed = serde_json::from_str::<Value>(input.as_str().unwrap());
                let accepted = parsed.as_ref().is_ok_and(|value| validator.is_valid(value));
                assert_eq!(accepted, expected);
            }
        }
        // This is direct-adapter coverage, not permission to skip shared schema qualification.
        assert!(!validator.is_valid(&json!({"publisherId":""})));
    }

    #[test]
    fn source_capture_membership_and_counts_cannot_silently_shrink() {
        assert_eq!(
            hex::encode(Sha256::digest(MANIFEST.as_bytes())),
            MANIFEST_SHA256
        );
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        assert!(manifest.get("endpoint_macros").is_none());
        let endpoint_fields = manifest["endpoint"]["fields"].as_table().unwrap();
        assert_eq!(endpoint_fields.len(), 1);
        assert_eq!(endpoint_fields["supply_id"].as_str(), Some("SupplyId"));
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["thetradedesk", "ttd"]
        );
        assert_eq!(
            definitions["thetradedesk"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["thetradedesk"].get("aliasOf").is_none());
        assert_eq!(definitions["ttd"]["defaultEnabled"].as_bool(), Some(false));
        assert_eq!(definitions["ttd"]["aliasOf"].as_str(), Some("thetradedesk"));
        assert_eq!(
            manifest["settings"]["fields"]["supply_id"].as_str(),
            Some("optional_string")
        );
        assert_eq!(
            manifest["provenance"]["sources"]["upstream"]["revision"].as_str(),
            Some("d7f8515b86258688304b0d9b6668c6a0e258bc9e")
        );
        let rows: Vec<_> = manifest["provenance"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .collect();
        assert_eq!(rows.len(), 20);
        assert_eq!(
            rows.iter()
                .map(|row| row["source_bytes"].as_integer().unwrap())
                .sum::<i64>(),
            69_721
        );
        let unique: BTreeSet<_> = rows
            .iter()
            .map(|row| row["source_path"].as_str().unwrap())
            .collect();
        assert_eq!(unique.len(), 20);
        assert_eq!(
            rows.iter()
                .filter(|row| row["source_path"].as_str().unwrap().contains("/exemplary/"))
                .count(),
            12
        );
    }

    #[test]
    #[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR; explicitly executed by the pinned-source CI step"]
    fn source_objects_match_all_witnesses_and_extracted_parameter_strings() {
        let source = var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap();
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let revision = manifest["provenance"]["sources"]["upstream"]["revision"]
            .as_str()
            .unwrap();
        let rows = manifest["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), 32);
        let mut parameter_source = Vec::new();
        for row in rows {
            let object = format!("{revision}:{}", row["source_path"].as_str().unwrap());
            let bytes = Command::new("git")
                .arg("--no-replace-objects")
                .arg("-C")
                .arg(&source)
                .arg("show")
                .arg(&object)
                .output()
                .unwrap();
            assert!(bytes.status.success());
            assert_eq!(
                u64::try_from(bytes.stdout.len()).unwrap(),
                u64::try_from(row["source_bytes"].as_integer().unwrap()).unwrap()
            );
            assert_eq!(
                hex::encode(Sha256::digest(&bytes.stdout)),
                row["source_sha256"].as_str().unwrap()
            );
            let blob = Command::new("git")
                .arg("--no-replace-objects")
                .arg("-C")
                .arg(&source)
                .arg("rev-parse")
                .arg(&object)
                .output()
                .unwrap();
            assert!(blob.status.success());
            assert_eq!(
                from_utf8(&blob.stdout).unwrap().trim(),
                row["source_blob"].as_str().unwrap()
            );
            if row["category"].as_str() == Some("parameter") {
                parameter_source = bytes.stdout;
            }
        }
        let text = from_utf8(&parameter_source).unwrap();
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        for (source_name, group, count) in
            [("validParams", "valid", 3), ("invalidParams", "invalid", 9)]
        {
            let declaration = format!("var {source_name} = []string{{");
            let tail = text.split_once(&declaration).unwrap().1;
            let body = tail.split_once("\n}").unwrap().0;
            let literals: Vec<_> = body
                .split('`')
                .enumerate()
                .filter_map(|(index, span)| (!index.is_multiple_of(2)).then_some(span))
                .collect();
            assert_eq!(literals.len(), count);
            assert_eq!(
                literals,
                cases[group]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap())
                    .collect::<Vec<_>>()
            );
        }
    }
}
