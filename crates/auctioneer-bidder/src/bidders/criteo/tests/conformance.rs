//! Verbatim source captures and explicitly named shared-decoder boundaries.

#[cfg(test)]
mod tests {
    use super::super::super::CriteoBidder;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        Bidder as _, BidderErrorKind, BidderHttpRequest, BidderName, BidderRequestInfo,
        BidderResponse, EntryPoint,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{collections::BTreeSet, env::var_os, process::Command, str::from_utf8};

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "5612afd69701e4411a58abbdb05063164e9313a70218e985fb91f00fda1defd0";
    const ENDPOINT: &str = "https://bidder.example.test/openrtb/pbs/auction/request?profile=230";

    fn run_fixture(_name: &str, path: &str, source: &str) {
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
        let info = BidderRequestInfo::new(
            BidderName::new("criteo-alias"),
            BidderName::new("criteo"),
            "account",
            None,
            ENDPOINT,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let calls = harness::drive_ok(&CriteoBidder, &request, &info);
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        let http = fixture["httpCalls"].as_array().unwrap();
        assert_eq!(http.len(), 1);
        assert_request(call, &http[0]["expectedRequest"]);
        assert_captured_response(&request, call, &fixture);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn assert_request(call: &BidderHttpRequest, expected: &Value) {
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), ENDPOINT);
        assert_eq!(
            expected["uri"],
            "https://ssp-bidder.criteo.com/openrtb/pbs/auction/request?profile=230"
        );
        if let Some(headers) = expected.get("headers") {
            assert_eq!(headers, &json!({}));
        }
        assert_eq!(call.headers().count(), 0);
        assert_eq!(
            serde_json::from_slice::<Value>(call.body()).unwrap(),
            expected["body"]
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
    }

    fn assert_captured_response(request: &BidRequest, call: &BidderHttpRequest, fixture: &Value) {
        let mock = &fixture["httpCalls"][0]["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let bytes = mock
            .get("body")
            .map(|body| serde_json::to_vec(body).unwrap())
            .unwrap_or_default();
        let decoded = CriteoBidder.make_bids(
            request,
            call.meta(),
            &harness::budgeted_response(status, bytes),
        );
        if status == 204 {
            let response = decoded.unwrap();
            assert!(response.bids.is_empty());
            assert!(response.auction_configs.is_empty());
            assert!(response.errors.is_empty());
            assert_eq!(fixture["expectedMakeBidsErrors"], json!([]));
        } else if status != 200 {
            let errors = decoded.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
            assert_eq!(
                errors[0].message,
                fixture["expectedMakeBidsErrors"][0]["value"]
            );
        } else {
            // Seven original success captures omit the required OpenRTB response.id.
            // Assert their original sealed refusal, then separately replay an explicit
            // envelope-only correction to exercise their Criteo media/config contract.
            let response = if mock["body"].get("id").is_none() {
                let errors = decoded.unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, 14);
                assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
                assert_eq!(errors[0].message, "invalid bidder response");
                let mut with_required_id = mock["body"].clone();
                with_required_id["id"] = json!("explicit-openrtb-envelope-boundary");
                CriteoBidder
                    .make_bids(
                        request,
                        call.meta(),
                        &harness::budgeted_response(
                            200,
                            serde_json::to_vec(&with_required_id).unwrap(),
                        ),
                    )
                    .unwrap()
            } else {
                decoded.unwrap()
            };
            let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
            assert_eq!(expected_responses.len(), 1);
            assert_success(&response, &expected_responses[0]);
        }
    }

    fn assert_success(response: &BidderResponse, expected: &Value) {
        assert_eq!(response.currency, expected["currency"]);
        assert!(response.errors.is_empty());
        let bids = expected["bids"].as_array().unwrap();
        assert!(!bids.is_empty());
        assert_eq!(response.bids.len(), bids.len());
        for (actual_bid, expected_bid_row) in response.bids.iter().zip(bids) {
            assert_eq!(
                serde_json::to_value(actual_bid.media_type).unwrap(),
                expected_bid_row["type"]
            );
            let mut expected_bid = expected_bid_row["bid"].clone();
            if let Some(name) = expected_bid["ext"]["prebid"]["networkName"]
                .as_str()
                .map(str::to_owned)
            {
                expected_bid["ext"]["prebid"]["meta"] = json!({"networkName":name});
            }
            assert_eq!(serde_json::to_value(&actual_bid.bid).unwrap(), expected_bid);
        }
        let configs = expected
            .get("fledgeAuctionConfigs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        assert_eq!(response.auction_configs.len(), configs.len());
        for (actual_config, expected_config) in response.auction_configs.iter().zip(configs) {
            assert_eq!(actual_config.imp_id, expected_config["impid"]);
            assert_eq!(actual_config.config, expected_config["config"]);
            assert_eq!(expected_config["bidder"], "criteo");
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_banner_cookie_uid => "exemplary/simple-banner-cookie-uid.json",
        captured_banner_inapp => "exemplary/simple-banner-inapp.json",
        captured_banner_multiple_bids => "exemplary/simple-banner-multiple-bids.json",
        captured_banner_paapi => "exemplary/simple-banner-paapi.json",
        captured_banner_uid => "exemplary/simple-banner-uid.json",
        captured_multi_type_banner => "exemplary/simple-multi-type-banner.json",
        captured_multi_type_video => "exemplary/simple-multi-type-video.json",
        captured_native => "exemplary/simple-native.json",
        captured_video => "exemplary/simple-video.json",
        captured_204 => "supplemental/204-response-from-target.json",
        captured_400 => "supplemental/400-response-from-target.json",
        captured_500 => "supplemental/500-response-from-target.json",
        captured_banner_ipv6 => "supplemental/simple-banner-with-ipv6.json",
    );

    #[test]
    fn capture_membership_and_literal_parameter_cases_have_frozen_witnesses() {
        assert_eq!(hex::encode(Sha256::digest(MANIFEST)), MANIFEST_SHA256);
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["criteo"]
        );
        assert_eq!(
            definitions["criteo"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["criteo"].get("aliasOf").is_none());
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
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
            42_563
        );
        assert_eq!(
            captures
                .iter()
                .map(|row| row["source_path"].as_str().unwrap())
                .collect::<BTreeSet<_>>()
                .len(),
            13
        );
        for row in &captures {
            assert_eq!(row["source_bytes"], row["retained"]["source_bytes"]);
            assert_eq!(row["source_sha256"], row["retained"]["source_sha256"]);
            assert!(row.get("transformation").is_none());
        }
        for (path, bytes) in [
            (
                "parameters.json",
                include_bytes!("../parameters.json").as_slice(),
            ),
            (
                "fixtures/parameters.json",
                include_bytes!("../fixtures/parameters.json").as_slice(),
            ),
        ] {
            let rows: Vec<_> = artifacts
                .iter()
                .filter(|row| {
                    row.get("retained")
                        .and_then(|value| value.get("source_path"))
                        .and_then(toml::Value::as_str)
                        == Some(path)
                })
                .collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(
                rows[0]["retained"]["source_bytes"].as_integer(),
                Some(i64::try_from(bytes.len()).unwrap())
            );
            assert_eq!(
                rows[0]["retained"]["source_sha256"].as_str().unwrap(),
                hex::encode(Sha256::digest(bytes))
            );
        }
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        assert_eq!(cases["valid"].as_array().unwrap().len(), 16);
        assert_eq!(cases["invalid"].as_array().unwrap().len(), 19);
        let schema: Value = serde_json::from_str(include_str!("../parameters.json")).unwrap();
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft4)
            .build(&schema)
            .unwrap();
        for (group, accepted) in [("valid", true), ("invalid", false)] {
            for literal in cases[group].as_array().unwrap() {
                let parsed = serde_json::from_str::<Value>(literal.as_str().unwrap());
                assert_eq!(
                    parsed.as_ref().is_ok_and(|value| validator.is_valid(value)),
                    accepted
                );
            }
        }
    }

    #[test]
    fn changed_capture_bytes_are_refused_before_filtered_dispatch() {
        let source = include_str!("../fixtures/protocol/exemplary/simple-banner-paapi.json");
        let changed = source.replacen('\n', " ", 1);
        assert_eq!(source.len(), changed.len());
        assert_eq!(
            serde_json::from_str::<Value>(source).unwrap(),
            serde_json::from_str::<Value>(&changed).unwrap()
        );
        assert_eq!(
            verify_protocol_capture(
                MANIFEST,
                MANIFEST_SHA256,
                "fixtures/protocol/exemplary/simple-banner-paapi.json",
                &changed
            ),
            Err("protocol capture digest mismatch")
        );
    }

    #[test]
    fn bidder_uses_only_the_sealed_parser_and_no_body_or_logging_escape_hatch() {
        let source = [
            include_str!("../mod.rs"),
            include_str!("../request.rs"),
            include_str!("../response.rs"),
        ];
        let implementation = source
            .map(|module| module.split("#[cfg(test)]\nmod tests").next().unwrap())
            .join("\n");
        for forbidden in [
            "into_body",
            "from_slice",
            "from_str",
            "println!",
            "eprintln!",
            "tracing::",
            ".clone()",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "forbidden production operation: {forbidden}"
            );
        }
        // Exactly one closed candidate parser owns response and extension extraction.
        assert_eq!(implementation.matches("parse_bid_candidates()").count(), 1);
        assert_eq!(implementation.matches("parse_bid_response()").count(), 0);
    }

    #[test]
    #[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR; pinned Git objects only, never source execution"]
    fn source_objects_and_extracted_parameter_literals_match_the_pinned_revision() {
        let checkout = var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap();
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let revision = manifest["provenance"]["sources"]["upstream"]["revision"]
            .as_str()
            .unwrap();
        assert_eq!(revision, "d7f8515b86258688304b0d9b6668c6a0e258bc9e");
        let rows = manifest["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), 26);
        let mut parameter_source = Vec::new();
        for row in rows {
            let object = format!("{revision}:{}", row["source_path"].as_str().unwrap());
            let bytes = Command::new("git")
                .args(["--no-replace-objects", "-C"])
                .arg(&checkout)
                .args(["show", &object])
                .output()
                .unwrap();
            assert!(bytes.status.success());
            assert_eq!(
                i64::try_from(bytes.stdout.len()).unwrap(),
                row["source_bytes"].as_integer().unwrap()
            );
            assert_eq!(
                hex::encode(Sha256::digest(&bytes.stdout)),
                row["source_sha256"].as_str().unwrap()
            );
            let blob = Command::new("git")
                .args(["--no-replace-objects", "-C"])
                .arg(&checkout)
                .args(["rev-parse", &object])
                .output()
                .unwrap();
            assert!(blob.status.success());
            assert_eq!(
                from_utf8(&blob.stdout).unwrap().trim(),
                row["source_blob"].as_str().unwrap()
            );
            if row["source_path"].as_str() == Some("adapters/criteo/params_test.go") {
                parameter_source = bytes.stdout;
            }
        }
        let source = from_utf8(&parameter_source).unwrap();
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        for (name, group, count) in [
            ("validParams", "valid", 16),
            ("invalidParams", "invalid", 19),
        ] {
            let declaration = format!("var {name} = []string{{");
            let body = source
                .split_once(&declaration)
                .unwrap()
                .1
                .split_once("\n}")
                .unwrap()
                .0;
            let literals: Vec<_> = body
                .split('`')
                .enumerate()
                .filter_map(|(index, value)| (!index.is_multiple_of(2)).then_some(value))
                .collect();
            assert_eq!(literals.len(), count);
            assert_eq!(
                literals,
                cases[group]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|literal| literal.as_str().unwrap())
                    .collect::<Vec<_>>()
            );
        }
    }
}
