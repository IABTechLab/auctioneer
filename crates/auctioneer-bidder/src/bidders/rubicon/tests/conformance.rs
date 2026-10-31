//! Pinned Rubicon captures; imported bytes are never repaired or used as generated output.

use super::super::RubiconBidder;
use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
use crate::correlation::CorrelationMedia;
use crate::{Bidder as _, BidderHttpRequest, BidderName, BidderRequestInfo, EntryPoint};
use auctioneer_config::BidderSettings;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::bidder_credentials::BidderCredentials;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, env::var_os, process::Command, str::from_utf8};

const MANIFEST: &str = include_str!("../bidder.toml");
const MANIFEST_SHA256: &str = "4c6c0181cfd81c0ed16d3ba439ba1b6e414d7b098853bf40539231f6d779a61e";
const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
const ENDPOINT: &str = "https://bidder.example.test/a/api/exchange";

pub(super) fn with_source_context<T>(run: impl FnOnce(&BidderRequestInfo<'_>) -> T) -> T {
    // All credential values and the external URL are TestJsonSamples literals,
    // not operator credentials or invented defaults. The reference is test-only.
    let settings: BidderSettings = serde_json::from_value(json!({
        "implementation":"rubicon", "values":{
            "credentials_secret":"fake_credential_bundle", "external_url":"http://hosturl.com",
            "region":"us-east"
        }
    }))
    .unwrap();
    let credentials = BidderCredentials::parse(
        &serde_json::to_vec(&json!({
            "schema_version":1_u8, "username":"xuser", "password":"xpass",
            "tracker":"pbs-test-tracker"
        }))
        .unwrap(),
    )
    .unwrap();
    let mut info = BidderRequestInfo::new(
        BidderName::new("fake_alias"),
        BidderName::new("rubicon"),
        "fake_account",
        None,
        ENDPOINT,
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    );
    info.set_settings(Some("rubicon"), Some(&settings), None)
        .unwrap();
    info.credentials = Some(&credentials);
    run(&info)
}

fn assert_json(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(left), Value::Number(right)) => {
            // Decimal spellings are not a second protocol; preserve exact f64 meaning.
            assert_eq!(
                left.as_f64().unwrap().to_bits(),
                right.as_f64().unwrap().to_bits()
            );
        }
        (Value::Array(left), Value::Array(right)) => {
            assert_eq!(left.len(), right.len());
            for (actual_item, expected_item) in left.iter().zip(right) {
                assert_json(actual_item, expected_item);
            }
        }
        (Value::Object(left), Value::Object(right)) => {
            assert_eq!(
                left.keys().collect::<Vec<_>>(),
                right.keys().collect::<Vec<_>>()
            );
            for (key, value) in right {
                assert_json(&left[key], value);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

fn assert_request(call: &BidderHttpRequest, expected: &Value) {
    assert_eq!(expected["uri"], "uri?tk_xint=pbs-test-tracker");
    assert_eq!(call.url(), format!("{ENDPOINT}?tk_xint=pbs-test-tracker"));
    assert_eq!(call.method(), "POST");
    let headers = call.headers().collect::<Vec<_>>();
    assert_eq!(headers.len(), 4);
    for header in [
        ("Content-Type", "application/json;charset=utf-8"),
        ("Accept", "application/json"),
        ("User-Agent", "prebid-server/1.0"),
        ("Authorization", "Basic eHVzZXI6eHBhc3M="),
    ] {
        assert!(headers.contains(&header));
    }
    let mut expected_body = expected["body"].clone();
    let target = &mut expected_body["imp"][0]["ext"]["rp"]["target"];
    assert_eq!(target["pbs_version"], "");
    target["pbs_version"] = json!(env!("CARGO_PKG_VERSION"));
    let actual: Value = serde_json::from_slice(call.body()).unwrap();
    assert_json(&actual, &expected_body);
    let expected_ids = expected["impIDs"].as_array().unwrap();
    assert_eq!(expected_ids.len(), 1);
    assert_eq!(
        call.covered_imp_ids().collect::<Vec<_>>(),
        expected_ids
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    let imp = &actual["imp"][0];
    let media = if imp.get("video").is_some() {
        CorrelationMedia::Video
    } else if imp.get("banner").is_some() {
        CorrelationMedia::Banner
    } else {
        assert!(imp.get("native").is_some());
        CorrelationMedia::Native
    };
    assert_eq!(call.meta().imp_correlation(0).unwrap().media(), media);
    assert!(call.meta().imp_correlation(1).is_none());
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
    let request: BidRequest = serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
    let before = serde_json::to_value(&request).unwrap();
    with_source_context(|info| {
        if let Some(expected_errors) = fixture.get("expectedMakeRequestsErrors") {
            assert_eq!(expected_errors.as_array().unwrap().len(), 1);
            let (semantic, calls) = harness::drive(&RubiconBidder, &request, info);
            assert!(calls.is_empty());
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
            assert_eq!(
                errors[0].message,
                if path == "supplemental/no-format-elements.json" {
                    "rubicon impression requires banner dimensions"
                } else {
                    assert_eq!(path, "supplemental/invalid-bidder-ext.json");
                    "rubicon invalid impression parameters"
                }
            );
        } else {
            let expected_calls = fixture["httpCalls"].as_array().unwrap();
            assert!(!expected_calls.is_empty());
            let calls = harness::drive_ok(&RubiconBidder, &request, info);
            assert_eq!(calls.len(), expected_calls.len());
            let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
            assert_eq!(expected_responses.len(), calls.len());
            // The source iterates a Go map; bind by actual emitted media rather
            // than relying on the capture's arbitrary order for split calls.
            let mut matched = BTreeSet::new();
            for call in &calls {
                let body: Value = serde_json::from_slice(call.body()).unwrap();
                let index = expected_calls
                    .iter()
                    .position(|row| {
                        let expected = &row["expectedRequest"]["body"]["imp"][0];
                        let actual = &body["imp"][0];
                        ["banner", "video", "native"].into_iter().all(|media| {
                            expected.get(media).is_some() == actual.get(media).is_some()
                        })
                    })
                    .unwrap();
                assert!(matched.insert(index));
                let exchange = &expected_calls[index];
                assert_request(call, &exchange["expectedRequest"]);
                let mock = &exchange["mockResponse"];
                assert_eq!(mock["status"], 200_u16);
                let response =
                    harness::budgeted_response(200, serde_json::to_vec(&mock["body"]).unwrap());
                let decoded = RubiconBidder
                    .make_bids(&request, call.meta(), &response)
                    .unwrap();
                let expected = &expected_responses[index];
                assert_eq!(decoded.currency, expected["currency"]);
                assert!(decoded.errors.is_empty());
                assert!(decoded.auction_configs.is_empty());
                let expected_bids = expected["bids"].as_array().unwrap();
                assert!(!expected_bids.is_empty());
                assert_eq!(decoded.bids.len(), expected_bids.len());
                for (actual, expected_bid) in decoded.bids.iter().zip(expected_bids) {
                    assert_eq!(
                        serde_json::to_value(actual.media_type).unwrap(),
                        expected_bid["type"]
                    );
                    assert_json(
                        &serde_json::to_value(&actual.bid).unwrap(),
                        &expected_bid["bid"],
                    );
                }
            }
            assert_eq!(matched.len(), expected_calls.len());
        }
    });
    assert_eq!(serde_json::to_value(&request).unwrap(), before);
}

protocol_fixtures!(run_fixture, "../fixtures/protocol/";
    captured_25_to_26_transition => "exemplary/25-26-transition-period.json",
    captured_app_impression_fpd => "exemplary/app-imp-fpd.json",
    captured_split_multiformat => "exemplary/bidonmultiformat.json",
    captured_flexible_parameter_numbers => "exemplary/flexible-schema.json",
    captured_forced_secure => "exemplary/hardcode-secure.json",
    captured_unsplit_media_precedence => "exemplary/non-bidonmultiformat.json",
    captured_banner_and_iab_segments => "exemplary/simple-banner.json",
    captured_proprietary_native_response => "exemplary/simple-native.json",
    captured_video => "exemplary/simple-video.json",
    captured_site_impression_fpd => "exemplary/site-imp-fpd.json",
    captured_user_fpd => "exemplary/user-fpd.json",
    captured_invalid_parameters => "supplemental/invalid-bidder-ext.json",
    captured_missing_banner_dimensions => "supplemental/no-format-elements.json",
    captured_site_without_content_data => "supplemental/no-site-content-data.json",
    captured_site_without_content => "supplemental/no-site-content.json",
);

#[test]
fn exact_capture_membership_and_retained_bytes_are_independently_frozen() {
    assert_eq!(format!("{:x}", Sha256::digest(MANIFEST)), MANIFEST_SHA256);
    let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
    assert_eq!(manifest["native_only"].as_bool(), Some(true));
    let definitions = manifest["definitions"].as_table().unwrap();
    assert_eq!(
        definitions.keys().map(String::as_str).collect::<Vec<_>>(),
        ["rubicon"]
    );
    assert_eq!(
        definitions["rubicon"]["defaultEnabled"].as_bool(),
        Some(false)
    );
    assert!(definitions["rubicon"].get("aliasOf").is_none());
    let rows = manifest["provenance"]["artifacts"].as_array().unwrap();
    assert_eq!(rows.len(), 29);
    let captured: BTreeSet<_> = rows
        .iter()
        .filter(|row| row["category"].as_str() == Some("protocol"))
        .map(|row| row["source_path"].as_str().unwrap())
        .collect();
    assert_eq!(
        captured,
        BTreeSet::from([
            "adapters/rubicon/rubicontest/exemplary/25-26-transition-period.json",
            "adapters/rubicon/rubicontest/exemplary/app-imp-fpd.json",
            "adapters/rubicon/rubicontest/exemplary/bidonmultiformat.json",
            "adapters/rubicon/rubicontest/exemplary/flexible-schema.json",
            "adapters/rubicon/rubicontest/exemplary/hardcode-secure.json",
            "adapters/rubicon/rubicontest/exemplary/non-bidonmultiformat.json",
            "adapters/rubicon/rubicontest/exemplary/simple-banner.json",
            "adapters/rubicon/rubicontest/exemplary/simple-native.json",
            "adapters/rubicon/rubicontest/exemplary/simple-video.json",
            "adapters/rubicon/rubicontest/exemplary/site-imp-fpd.json",
            "adapters/rubicon/rubicontest/exemplary/user-fpd.json",
            "adapters/rubicon/rubicontest/supplemental/invalid-bidder-ext.json",
            "adapters/rubicon/rubicontest/supplemental/no-format-elements.json",
            "adapters/rubicon/rubicontest/supplemental/no-site-content-data.json",
            "adapters/rubicon/rubicontest/supplemental/no-site-content.json",
        ])
    );
    let protocol = rows
        .iter()
        .filter(|row| row["category"].as_str() == Some("protocol"));
    let mut total = 0_i64;
    for row in protocol {
        assert_eq!(row["source_bytes"], row["retained"]["source_bytes"]);
        assert_eq!(row["source_sha256"], row["retained"]["source_sha256"]);
        assert!(row.get("transformation").is_none());
        total += row["source_bytes"].as_integer().unwrap();
    }
    assert_eq!(total, 109_040);
    let schema = include_bytes!("../parameters.json");
    let schema_row = rows
        .iter()
        .find(|row| row["category"].as_str() == Some("schema"))
        .unwrap();
    assert_eq!(
        schema_row["source_bytes"].as_integer(),
        Some(i64::try_from(schema.len()).unwrap())
    );
    assert_eq!(
        schema_row["source_sha256"].as_str().unwrap(),
        format!("{:x}", Sha256::digest(schema))
    );
}

#[test]
fn a_same_length_capture_change_is_rejected_before_filtered_protocol_dispatch() {
    let source = include_str!("../fixtures/protocol/exemplary/simple-banner.json");
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
            "fixtures/protocol/exemplary/simple-banner.json",
            &changed
        ),
        Err("protocol capture digest mismatch")
    );
}

#[test]
#[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR; pinned Git objects only, never source execution"]
fn source_objects_match_pinned_rubicon_tree_and_all_artifacts() {
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
    assert_eq!(trees.len(), 1);
    for row in trees {
        let object = format!("{REVISION}:{}", row["source_path"].as_str().unwrap());
        assert_eq!(
            from_utf8(&git("rev-parse", &object)).unwrap().trim(),
            row["tree"].as_str().unwrap()
        );
    }
    let rows = manifest["provenance"]["artifacts"].as_array().unwrap();
    assert_eq!(rows.len(), 29);
    for row in rows {
        let object = format!("{REVISION}:{}", row["source_path"].as_str().unwrap());
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
    }
    let source_bytes = git(
        "show",
        &format!("{REVISION}:adapters/rubicon/rubicon_test.go"),
    );
    let source = from_utf8(&source_bytes).unwrap();
    let samples = source.split_once("func TestJsonSamples").unwrap().1;
    for literal in [
        "\"xuser\"",
        "\"xpass\"",
        "\"pbs-test-tracker\"",
        "\"http://hosturl.com\"",
    ] {
        assert!(samples.contains(literal));
    }
}
