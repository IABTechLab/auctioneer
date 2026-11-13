//! Verbatim `OpenX` source captures and named Auctioneer contract boundaries.

#[cfg(test)]
mod tests {
    use super::super::super::OpenxBidder;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::correlation::CorrelationError;
    use crate::{
        Bidder as _, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse,
        EndpointRejectReason, EndpointValidator, EntryPoint, OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{collections::BTreeSet, env::var_os, process::Command, str::from_utf8};

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "2b6ef83a4952220deb88bd6e8f7536bdcfa07181f8845f0b850f6777f912b62d";
    const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
    const ENDPOINT: &str = "https://bidder.example.test/prebid";
    const SOURCE_ENDPOINT: &str = "http://rtb.openx.net/prebid";

    struct AllowEndpoint;

    impl EndpointValidator for AllowEndpoint {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

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
            BidderName::new("fake_alias"),
            BidderName::new("openx"),
            "fake_account",
            None,
            ENDPOINT,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let http = fixture["httpCalls"].as_array().unwrap();
        assert!(!http.is_empty());
        if path == "exemplary/optional-params.json" {
            // The source emits duplicate covered IDs. Auctioneer refuses that call;
            // the original bytes/expectations remain evidence, not a corrected replay.
            assert_eq!(http.len(), 1);
            assert_eq!(
                http[0]["expectedRequest"]["impIDs"],
                json!(["test-imp-id", "test-imp-id"])
            );
            assert_eq!(request.imp.len(), 2);
            assert_eq!(request.imp[0].id, request.imp[1].id);
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &OpenxBidder,
                &request,
                &info,
                SinkLimits::new(8192, 100),
                1,
                &AllowEndpoint,
            );
            semantic.unwrap();
            assert!(outcome.accepted.is_empty());
            assert_eq!(
                outcome.errors,
                [OutboundBuildError::Correlation(
                    CorrelationError::DuplicateCoverage
                )]
            );
            assert_eq!(outcome.rejected_call_count, 0);
            assert_eq!(remaining, 1);
            assert_eq!(http[0]["mockResponse"]["status"], 204_u16);
            assert_eq!(fixture["expectedBidResponses"], json!([]));
        } else {
            let calls = harness::drive_ok(&OpenxBidder, &request, &info);
            assert_eq!(calls.len(), http.len());
            let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
            assert_eq!(expected_responses.len(), http.len());
            for ((call, exchange), expected) in calls.iter().zip(http).zip(expected_responses) {
                assert_request(call, &exchange["expectedRequest"]);
                let mock = &exchange["mockResponse"];
                assert_eq!(mock["status"], 200_u16);
                let decoded = OpenxBidder
                    .make_bids(
                        &request,
                        call.meta(),
                        &harness::budgeted_response(
                            200,
                            serde_json::to_vec(&mock["body"]).unwrap(),
                        ),
                    )
                    .unwrap();
                assert_success(&decoded, expected);
                if path == "exemplary/fledge.json" || path == "exemplary/fledge-no-bids.json" {
                    assert_eq!(decoded.auction_configs.len(), 1);
                    let config = &decoded.auction_configs[0];
                    assert_eq!(config.imp_id, "test-request-id");
                    assert_eq!(config.imp_id, request.id);
                    assert!(!request.imp.iter().any(|imp| imp.id == config.imp_id));
                    assert!(!call.covered_imp_ids().any(|id| id == config.imp_id));
                    // This is independent adapter output, NOT accepted exchange output.
                    // The exchange's separately owned coverage check must drop this ID.
                } else {
                    assert!(decoded.auction_configs.is_empty());
                }
            }
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn assert_request(call: &BidderHttpRequest, expected: &Value) {
        assert_eq!(expected["uri"], SOURCE_ENDPOINT);
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), ENDPOINT);
        assert_eq!(call.headers().count(), 2);
        for header in [
            ("Content-Type", "application/json;charset=utf-8"),
            ("Accept", "application/json"),
        ] {
            assert!(call.headers().any(|actual| actual == header));
        }
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

    fn assert_success(decoded: &BidderResponse, expected: &Value) {
        assert_eq!(decoded.currency, expected["currency"]);
        assert!(decoded.errors.is_empty());
        let expected_bids = expected["bids"].as_array().unwrap();
        assert_eq!(decoded.bids.len(), expected_bids.len());
        for (actual, row) in decoded.bids.iter().zip(expected_bids) {
            assert_eq!(
                serde_json::to_value(actual.media_type).unwrap(),
                row["type"]
            );
            let mut expected_bid = row["bid"].clone();
            // JSON prices have one numeric meaning; the typed model stores f64.
            let expected_price = expected_bid["price"].as_f64().unwrap();
            assert_eq!(actual.bid.price.to_bits(), expected_price.to_bits());
            expected_bid["price"] = json!(expected_price);
            // Source always returns BidVideo, even for non-video/zero-duration bids.
            // Auctioneer has no separate TypedBid metadata channel; compare its wire owner.
            let duration = row["bid"].get("dur").and_then(Value::as_i64).unwrap_or(0);
            let category = row["bid"]
                .get("cat")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_str)
                .unwrap_or("");
            let video = json!({"duration":duration, "primary_category":category});
            if let Some(captured) = row.get("video") {
                assert_eq!(captured, &video);
            }
            expected_bid["ext"] = json!({"prebid":{"video":video}});
            assert_eq!(serde_json::to_value(&actual.bid).unwrap(), expected_bid);
        }
        let configs = expected
            .get("fledgeauctionconfigs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        assert_eq!(decoded.auction_configs.len(), configs.len());
        for (actual, expected_config) in decoded.auction_configs.iter().zip(configs) {
            assert_eq!(expected_config["bidder"], "openx");
            assert_eq!(actual.imp_id, expected_config["impid"]);
            assert_eq!(actual.config, expected_config["config"]);
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_fledge_no_bids_unsent_id_boundary => "exemplary/fledge-no-bids.json",
        captured_fledge_unsent_id_boundary => "exemplary/fledge.json",
        captured_impression_extension_passthrough => "exemplary/imp-ext-passthrough.json",
        captured_optional_parameters_duplicate_coverage_boundary => "exemplary/optional-params.json",
        captured_banner => "exemplary/simple-banner.json",
        captured_native => "exemplary/simple-native.json",
        captured_video => "exemplary/simple-video.json",
        captured_rewarded_video => "exemplary/video-rewarded.json",
        captured_missing_mtype => "supplemental/missing-mtype.json",
        captured_multiple_impressions => "supplemental/multi-imp.json",
    );

    #[test]
    fn exact_capture_membership_and_parameter_literals_have_frozen_witnesses() {
        assert_eq!(hex::encode(Sha256::digest(MANIFEST)), MANIFEST_SHA256);
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["openx"]
        );
        assert_eq!(
            definitions["openx"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["openx"].get("aliasOf").is_none());
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        let captures: Vec<_> = artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .collect();
        assert_eq!(captures.len(), 10);
        assert_eq!(
            captures
                .iter()
                .map(|row| row["source_bytes"].as_integer().unwrap())
                .sum::<i64>(),
            37_709
        );
        let source_members: BTreeSet<_> = captures
            .iter()
            .map(|row| row["source_path"].as_str().unwrap())
            .collect();
        assert_eq!(
            source_members,
            BTreeSet::from([
                "adapters/openx/openxtest/exemplary/fledge-no-bids.json",
                "adapters/openx/openxtest/exemplary/fledge.json",
                "adapters/openx/openxtest/exemplary/imp-ext-passthrough.json",
                "adapters/openx/openxtest/exemplary/optional-params.json",
                "adapters/openx/openxtest/exemplary/simple-banner.json",
                "adapters/openx/openxtest/exemplary/simple-native.json",
                "adapters/openx/openxtest/exemplary/simple-video.json",
                "adapters/openx/openxtest/exemplary/video-rewarded.json",
                "adapters/openx/openxtest/supplemental/missing-mtype.json",
                "adapters/openx/openxtest/supplemental/multi-imp.json",
            ])
        );
        for row in captures {
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
            let witnesses: Vec<_> = artifacts
                .iter()
                .filter(|row| {
                    row.get("retained")
                        .and_then(|retained| retained.get("source_path"))
                        .and_then(toml::Value::as_str)
                        == Some(path)
                })
                .collect();
            assert_eq!(witnesses.len(), 1);
            assert_eq!(
                witnesses[0]["retained"]["source_bytes"].as_integer(),
                Some(i64::try_from(bytes.len()).unwrap())
            );
            assert_eq!(
                witnesses[0]["retained"]["source_sha256"].as_str().unwrap(),
                hex::encode(Sha256::digest(bytes))
            );
        }
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        assert_eq!(cases["valid"].as_array().unwrap().len(), 7);
        assert_eq!(cases["invalid"].as_array().unwrap().len(), 15);
        let schema: Value = serde_json::from_str(include_str!("../parameters.json")).unwrap();
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft4)
            .should_validate_formats(true)
            .build(&schema)
            .unwrap();
        for (group, valid) in [("valid", true), ("invalid", false)] {
            for literal in cases[group].as_array().unwrap() {
                let value: Value = serde_json::from_str(literal.as_str().unwrap()).unwrap();
                assert_eq!(validator.is_valid(&value), valid);
            }
        }
    }

    #[test]
    fn changed_capture_bytes_fail_integrity_before_filtered_dispatch() {
        let path = "fixtures/protocol/exemplary/fledge-no-bids.json";
        let source = include_str!("../fixtures/protocol/exemplary/fledge-no-bids.json");
        let changed = source.replacen('\n', " ", 1);
        assert_eq!(source.len(), changed.len());
        assert_eq!(
            serde_json::from_str::<Value>(source).unwrap(),
            serde_json::from_str::<Value>(&changed).unwrap()
        );
        assert_eq!(
            verify_protocol_capture(MANIFEST, MANIFEST_SHA256, path, &changed),
            Err("protocol capture digest mismatch")
        );
    }

    #[test]
    #[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR; pinned Git objects only, never Go execution"]
    fn source_objects_match_pinned_trees_artifacts_and_parameter_literals() {
        let checkout = var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap();
        let git = |operation: &str, object: &str| {
            let output = Command::new("git")
                .args(["--no-replace-objects", "-C"])
                .arg(&checkout)
                .args([operation, object])
                .output()
                .unwrap();
            assert!(output.status.success());
            output.stdout
        };
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        assert_eq!(
            manifest["provenance"]["sources"]["upstream"]["revision"].as_str(),
            Some(REVISION)
        );
        let trees = manifest["provenance"]["trees"].as_array().unwrap();
        assert_eq!(trees.len(), 3);
        for row in trees {
            let object = format!("{REVISION}:{}", row["source_path"].as_str().unwrap());
            assert_eq!(
                from_utf8(&git("rev-parse", &object)).unwrap().trim(),
                row["tree"].as_str().unwrap()
            );
        }
        let rows = manifest["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), 27);
        let mut parameter_source = Vec::new();
        for row in rows {
            let object = format!("{REVISION}:{}", row["source_path"].as_str().unwrap());
            let bytes = git("show", &object);
            assert_eq!(
                i64::try_from(bytes.len()).unwrap(),
                row["source_bytes"].as_integer().unwrap()
            );
            assert_eq!(
                hex::encode(Sha256::digest(&bytes)),
                row["source_sha256"].as_str().unwrap()
            );
            assert_eq!(
                from_utf8(&git("rev-parse", &object)).unwrap().trim(),
                row["source_blob"].as_str().unwrap()
            );
            if row["source_path"].as_str() == Some("adapters/openx/params_test.go") {
                parameter_source = bytes;
            }
        }
        let source = from_utf8(&parameter_source).unwrap();
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        for (name, group, count) in [
            ("validParams", "valid", 7),
            ("invalidParams", "invalid", 15),
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
                .filter_map(|(index, literal)| (!index.is_multiple_of(2)).then_some(literal))
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
