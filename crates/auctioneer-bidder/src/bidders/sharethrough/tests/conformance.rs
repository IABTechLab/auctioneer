//! Immutable Sharethrough captures and reviewed source/version boundaries.

#[cfg(test)]
mod tests {
    use super::super::super::SharethroughBidder;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        Bidder as _, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse, EntryPoint,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use jsonschema::Draft;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{
        collections::BTreeSet, env::var_os, fs, path::Path, process::Command, str::from_utf8,
    };

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "89d2b9fcdedb691b384f9078832431f80f1952a82fbfd0155fbc036638332aea";
    const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
    const ENDPOINT: &str = "https://bidder.example.test/sharethrough";

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
            BidderName::new("sharethrough"),
            BidderName::new("sharethrough"),
            "account",
            None,
            ENDPOINT,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let calls = harness::drive_ok(&SharethroughBidder, &request, &info);
        let exchanges = fixture["httpCalls"].as_array().unwrap();
        assert!(!exchanges.is_empty());
        assert_eq!(calls.len(), exchanges.len());
        let mut success_count = 0_usize;
        for (call, exchange) in calls.iter().zip(exchanges) {
            assert_request(call, &exchange["expectedRequest"]);
            let mock = &exchange["mockResponse"];
            let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
            let result = SharethroughBidder.make_bids(
                &request,
                call.meta(),
                &harness::budgeted_response(status, serde_json::to_vec(&mock["body"]).unwrap()),
            );
            match status {
                200 => {
                    let expected = &fixture["expectedBidResponses"][success_count];
                    assert_success(&result.unwrap(), expected);
                    success_count = success_count.checked_add(1).unwrap();
                }
                204 => {
                    let decoded = result.unwrap();
                    assert!(decoded.bids.is_empty());
                    assert!(decoded.errors.is_empty());
                    assert_eq!(decoded.currency, "USD");
                }
                400 => {
                    let errors = result.unwrap_err();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, 2);
                }
                500 => harness::assert_bad_server_response_errors(&result.unwrap_err(), 1),
                _ => panic!("unreviewed source capture status"),
            }
        }
        assert_eq!(
            fixture
                .get("expectedBidResponses")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            success_count
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn assert_request(call: &BidderHttpRequest, expected: &Value) {
        assert_eq!(expected["uri"], "http://whatever.url");
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), ENDPOINT);
        assert_eq!(call.headers().count(), 2);
        for (name, value) in [
            ("Content-Type", "application/json;charset=utf-8"),
            ("Accept", "application/json"),
        ] {
            assert_eq!(expected["headers"][name], json!([value]));
            assert!(call.headers().any(|actual| actual == (name, value)));
        }
        let mut expected_body = expected["body"].clone();
        assert_eq!(expected_body["source"]["ext"]["str"], "10.0");
        assert_eq!(expected_body["source"]["ext"]["version"], "");
        // Approved package-version substitution changes only this comparison copy.
        expected_body["source"]["ext"]["version"] = json!(env!("CARGO_PKG_VERSION"));
        assert_eq!(
            serde_json::from_slice::<Value>(call.body()).unwrap(),
            expected_body
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
        assert_eq!(call.covered_imp_ids().count(), 1);
        assert!(call.meta().imp_correlation(0).is_some());
        assert!(call.meta().imp_correlation(1).is_none());
    }

    fn assert_success(decoded: &BidderResponse, expected: &Value) {
        assert_eq!(decoded.currency, "USD");
        assert_eq!(expected["currency"], "USD");
        assert!(decoded.errors.is_empty());
        assert!(decoded.auction_configs.is_empty());
        let expected_bids = expected["bids"].as_array().unwrap();
        assert_eq!(decoded.bids.len(), expected_bids.len());
        for (actual, row) in decoded.bids.iter().zip(expected_bids) {
            assert_eq!(
                serde_json::to_value(actual.media_type).unwrap(),
                row["type"]
            );
            let mut expected_bid = row["bid"].clone();
            let price = expected_bid["price"].as_f64().unwrap();
            assert_eq!(actual.bid.price.to_bits(), price.to_bits());
            expected_bid["price"] = json!(price);
            assert_eq!(serde_json::to_value(&actual.bid).unwrap(), expected_bid);
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_app_banner => "exemplary/app-banner.json",
        captured_app_native => "exemplary/app-native.json",
        captured_app_video => "exemplary/app-video.json",
        captured_web_banner => "exemplary/web-banner.json",
        captured_web_video => "exemplary/web-video.json",
        captured_bad_request => "supplemental/error-bad-request.json",
        captured_internal_server_error => "supplemental/error-internal-server.json",
        captured_multiple_impressions => "supplemental/multi-imp.json",
        captured_split_multiformat => "supplemental/multiformat-impression.json",
        captured_no_fill => "supplemental/no-fill.json",
        captured_supply_chain => "supplemental/schain.json",
    );

    #[test]
    fn exact_capture_membership_retained_bytes_and_parameter_literals_are_frozen() {
        assert_eq!(format!("{:x}", Sha256::digest(MANIFEST)), MANIFEST_SHA256);
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["sharethrough"]
        );
        assert_eq!(
            definitions["sharethrough"]["defaultEnabled"].as_bool(),
            Some(false)
        );
        assert!(definitions["sharethrough"].get("aliasOf").is_none());
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 25);
        let captures: Vec<_> = artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .collect();
        assert_eq!(captures.len(), 11);
        assert_eq!(
            captures
                .iter()
                .try_fold(0_i64, |sum, row| sum
                    .checked_add(row["source_bytes"].as_integer().unwrap()))
                .unwrap(),
            37_909
        );
        assert_eq!(
            captures
                .iter()
                .map(|row| row["retained"]["source_path"].as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "fixtures/protocol/exemplary/app-banner.json",
                "fixtures/protocol/exemplary/app-native.json",
                "fixtures/protocol/exemplary/app-video.json",
                "fixtures/protocol/exemplary/web-banner.json",
                "fixtures/protocol/exemplary/web-video.json",
                "fixtures/protocol/supplemental/error-bad-request.json",
                "fixtures/protocol/supplemental/error-internal-server.json",
                "fixtures/protocol/supplemental/multi-imp.json",
                "fixtures/protocol/supplemental/multiformat-impression.json",
                "fixtures/protocol/supplemental/no-fill.json",
                "fixtures/protocol/supplemental/schain.json",
            ])
        );
        for row in captures {
            assert_eq!(row["source_bytes"], row["retained"]["source_bytes"]);
            assert_eq!(row["source_sha256"], row["retained"]["source_sha256"]);
            assert!(row.get("transformation").is_none());
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders/sharethrough");
        let mut retained_count = 0_usize;
        for row in artifacts {
            let Some(retained) = row.get("retained") else {
                continue;
            };
            let path = retained["source_path"].as_str().unwrap();
            assert_eq!(Path::new(path).extension().unwrap(), "json");
            let bytes = fs::read(root.join(path)).unwrap();
            assert_eq!(
                retained["source_bytes"].as_integer(),
                Some(i64::try_from(bytes.len()).unwrap())
            );
            assert_eq!(
                retained["source_sha256"].as_str().unwrap(),
                format!("{:x}", Sha256::digest(bytes))
            );
            retained_count = retained_count.checked_add(1).unwrap();
        }
        assert_eq!(retained_count, 13);
        assert_parameter_literals();
    }

    fn assert_parameter_literals() {
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        assert_eq!(cases["valid"].as_array().unwrap().len(), 7);
        assert_eq!(cases["invalid"].as_array().unwrap().len(), 14);
        let schema: Value = serde_json::from_str(include_str!("../parameters.json")).unwrap();
        let validator = jsonschema::options()
            .with_draft(Draft::Draft4)
            .should_validate_formats(true)
            .build(&schema)
            .unwrap();
        for (group, valid) in [("valid", true), ("invalid", false)] {
            for literal_value in cases[group].as_array().unwrap() {
                let literal = literal_value.as_str().unwrap();
                if let Ok(value) = serde_json::from_str::<Value>(literal) {
                    assert_eq!(validator.is_valid(&value), valid);
                } else {
                    assert!(!valid);
                    assert!(literal.is_empty());
                }
            }
        }
    }

    #[test]
    fn parse_equivalent_capture_drift_is_refused_before_protocol_dispatch() {
        let path = "fixtures/protocol/exemplary/app-banner.json";
        let source = include_str!("../fixtures/protocol/exemplary/app-banner.json");
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
    fn source_objects_sharethrough_match_pinned_trees_artifacts_and_parameter_literals() {
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
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 25);
        let mut parameter_source = Vec::new();
        for row in artifacts {
            let path = row["source_path"].as_str().unwrap();
            let object = format!("{REVISION}:{path}");
            let bytes = git("show", &object);
            assert_eq!(
                row["source_bytes"].as_integer(),
                Some(i64::try_from(bytes.len()).unwrap())
            );
            assert_eq!(
                row["source_sha256"].as_str().unwrap(),
                format!("{:x}", Sha256::digest(&bytes))
            );
            assert_eq!(
                from_utf8(&git("rev-parse", &object)).unwrap().trim(),
                row["source_blob"].as_str().unwrap()
            );
            if path == "adapters/sharethrough/params_test.go" {
                parameter_source = bytes;
            }
        }
        let source = from_utf8(&parameter_source).unwrap();
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        for (name, group, count) in [
            ("validParams", "valid", 7_usize),
            ("invalidParams", "invalid", 14_usize),
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
