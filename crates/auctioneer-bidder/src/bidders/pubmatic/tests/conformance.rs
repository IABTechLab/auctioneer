//! Immutable `PubMatic` source captures and explicit Auctioneer safety boundaries.

#[cfg(test)]
mod tests {
    use super::super::super::PubmaticBidder;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        Bidder as _, BidderError, BidderHttpRequest, BidderHttpResponse, BidderName,
        BidderRequestInfo, BidderResponse, ConstructionDiagnostic, EndpointRejectReason,
        EndpointValidator, EntryPoint, SinkLimits, SinkOutcome,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use jsonschema::{Draft, options};
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{
        collections::BTreeSet,
        env::var_os,
        ffi::OsStr,
        io::Write as _,
        path::Path,
        process::{Command, Stdio},
        str::from_utf8,
    };
    use toml::Value as TomlValue;
    use url::Url;

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str =
        "6160e5eeab24243fff7cb2edebeddde719a15621abf8778d8b9f89e2e7efb9ba";
    const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
    const ENDPOINT: &str = "https://bidder.example.test/translator";
    const SOURCE_ENDPOINT: &str = "https://hbopenbid.pubmatic.com/translator?source=prebid-server";
    const SOURCE_PREFIX: &str = "adapters/pubmatic/pubmatictest/";
    const CAPTURE_PATHS: [&str; 23] = [
        "exemplary/banner.json",
        "exemplary/fledge.json",
        "exemplary/native.json",
        "exemplary/video.json",
        "supplemental/app.json",
        "supplemental/audio.json",
        "supplemental/bid_ext_ibv_true.json",
        "supplemental/dctrAndPmZoneID.json",
        "supplemental/displayManagerInAppExt.json",
        "supplemental/displayManagerInAppExtPrebid.json",
        "supplemental/extra-bid.json",
        "supplemental/gptSlotNameInImpExt.json",
        "supplemental/gptSlotNameInImpExtPbAdslot.json",
        "supplemental/impExt.json",
        "supplemental/invalidparam.json",
        "supplemental/multiplemedia.json",
        "supplemental/native_invalid_adm.json",
        "supplemental/nilReqExt.json",
        "supplemental/noAdSlot.json",
        "supplemental/pmZoneIDInKeywords.json",
        "supplemental/reqBidderParams.json",
        "supplemental/reqExtPrebidAliases.json",
        "supplemental/trimPublisherID.json",
    ];

    struct AllowEndpoint;

    impl EndpointValidator for AllowEndpoint {
        fn validate(
            &self,
            _url: &Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    fn info(requested: &str) -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new(requested),
            BidderName::new("pubmatic"),
            "fake_account",
            None,
            ENDPOINT,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn verified_fixture(path: &str, source: &str) -> Value {
        verify_protocol_capture(
            MANIFEST,
            MANIFEST_SHA256,
            &format!("fixtures/protocol/{path}"),
            source,
        )
        .unwrap();
        serde_json::from_str(source).unwrap()
    }

    fn run_fixture(_name: &str, path: &str, source: &str) {
        let fixture = verified_fixture(path, source);
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        // The source alias is the actual requested identity for this replay. An
        // independent canonical replay below proves arbitrary aliases cannot win.
        let requested = if path == "supplemental/reqExtPrebidAliases.json" {
            "pubmatic-alias"
        } else {
            "pubmatic"
        };
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &PubmaticBidder,
            &request,
            &info(requested),
            SinkLimits::new(256 * 1024, 100),
            1,
            &AllowEndpoint,
        );
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(outcome.rejected_call_count, 0);
        if matches!(
            path,
            "supplemental/audio.json" | "supplemental/invalidparam.json"
        ) {
            assert_rejected_fixture(&fixture, path, &semantic.unwrap_err(), &outcome, remaining);
        } else {
            semantic.unwrap();
            assert_eq!(remaining, 0);
            assert_eq!(outcome.diagnostics.iter().count(), 0);
            let exchanges = fixture["httpCalls"].as_array().unwrap();
            let responses = fixture["expectedBidResponses"].as_array().unwrap();
            assert_eq!(exchanges.len(), 1);
            assert_eq!(responses.len(), 1);
            assert_eq!(outcome.accepted.len(), 1);
            let call = &outcome.accepted[0];
            assert_request(call, &exchanges[0]["expectedRequest"]);
            let mock = &exchanges[0]["mockResponse"];
            assert_eq!(mock["status"], 200_u16);
            let decoded = PubmaticBidder.make_bids(
                &request,
                call.meta(),
                &harness::budgeted_response(200, serde_json::to_vec(&mock["body"]).unwrap()),
            );
            assert_fixture_response(&fixture, path, &request, call, decoded);
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn assert_rejected_fixture(
        fixture: &Value,
        path: &str,
        errors: &[BidderError],
        outcome: &SinkOutcome,
        remaining: u32,
    ) {
        let expected_errors = fixture["expectedMakeRequestsErrors"].as_array().unwrap();
        let count = if path == "supplemental/audio.json" {
            1
        } else {
            6
        };
        assert_eq!(expected_errors.len(), count);
        // Source returns one input-echoing error per rejected impression.
        // Auctioneer retains those reasons in bounded diagnostics and returns one
        // terminal unknown error when no impression survives.
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 999);
        assert!(
            errors
                .iter()
                .all(|error| !error.message.contains("AdTag_Div1"))
        );
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 1);
        let diagnostic = if path == "supplemental/audio.json" {
            ConstructionDiagnostic::UnsupportedMediaType
        } else {
            ConstructionDiagnostic::InvalidImpressionParameters
        };
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            vec![diagnostic; count]
        );
        assert!(fixture.get("httpCalls").is_none());
    }

    fn assert_fixture_response(
        fixture: &Value,
        path: &str,
        request: &BidRequest,
        call: &BidderHttpRequest,
        decoded: Result<BidderResponse, Vec<BidderError>>,
    ) {
        let responses = fixture["expectedBidResponses"].as_array().unwrap();
        let mock = &fixture["httpCalls"][0]["mockResponse"];
        match path {
            "supplemental/bid_ext_ibv_true.json" => {
                // Original capture asks for video only but returns banner. ibv
                // describes metadata, not permission to change offered media.
                assert!(request.imp[0].banner.is_none());
                assert!(request.imp[0].video.is_some());
                assert_eq!(responses[0]["bids"][0]["bid"]["mtype"], 1_u32);
                harness::assert_bad_server_response_errors(&decoded.unwrap_err(), 1);
            }
            "supplemental/native_invalid_adm.json" => {
                let captured = &responses[0]["bids"][0]["bid"];
                assert_eq!(captured["adm"], "{");
                let id = captured["impid"].as_str().unwrap();
                assert!(!call.covered_imp_ids().any(|sent| sent == id));
                assert_eq!(
                    fixture["expectedMakeBidsErrors"].as_array().unwrap().len(),
                    1
                );
                harness::assert_bad_server_response_errors(&decoded.unwrap_err(), 1);
            }
            _ => {
                if path == "supplemental/app.json" {
                    // Go reports an error unmarshalling absent Bid.ext but keeps
                    // the bid. Rust safely treats absent optional metadata as absent.
                    assert_eq!(
                        fixture["expectedMakeBidsErrors"].as_array().unwrap().len(),
                        1
                    );
                    assert!(mock["body"]["seatbid"][0]["bid"][0].get("ext").is_none());
                } else {
                    assert!(fixture.get("expectedMakeBidsErrors").is_none());
                }
                assert_success(&decoded.unwrap(), &responses[0]);
            }
        }
    }

    fn assert_request(call: &BidderHttpRequest, expected: &Value) {
        assert_eq!(expected["uri"], SOURCE_ENDPOINT);
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), ENDPOINT);
        assert_eq!(
            call.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
            ]
        );
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
        let bids = expected["bids"].as_array().unwrap();
        assert!(!bids.is_empty());
        assert_eq!(decoded.bids.len(), bids.len());
        for (actual, row) in decoded.bids.iter().zip(bids) {
            assert_eq!(
                serde_json::to_value(actual.media_type).unwrap(),
                row["type"]
            );
            let mut bid = row["bid"].clone();
            let price = bid["price"].as_f64().unwrap();
            assert_eq!(actual.bid.price.to_bits(), price.to_bits());
            bid["price"] = json!(price);
            let has_source_metadata = bid.get("ext").is_some_and(Value::is_object);
            let duration = bid["ext"]["video"]["duration"].as_i64().unwrap_or(0);
            let video = json!({"duration":duration, "primary_category":""});
            if let Some(captured) = row.get("video") {
                assert_eq!(captured, &video);
            }
            let reported_media = if bid["ext"]["ibv"].as_bool() == Some(true) {
                json!("video")
            } else {
                row["type"].clone()
            };
            let priority = bid["ext"]["prebiddealpriority"]
                .as_i64()
                .filter(|value| *value > 0);
            // Source TypedBid channels have existing Auctioneer wire owners. Expected
            // values come from the frozen source row, never from decoded output.
            if !has_source_metadata {
                bid["ext"] = json!({});
            }
            if !bid["ext"]["prebid"].is_object() {
                bid["ext"]["prebid"] = json!({});
            }
            bid["ext"]["prebid"]["video"] = video;
            if has_source_metadata {
                bid["ext"]["prebid"]["meta"] = json!({"mediaType":reported_media});
            }
            if let Some(deal_priority) = priority {
                bid["ext"]["prebid"]["dealpriority"] = json!(deal_priority);
            }
            assert_eq!(serde_json::to_value(&actual.bid).unwrap(), bid);
            if let Some(seat) = row.get("seat") {
                // TypedBid intentionally has no authoritative seat/bidder slot.
                // Provider marketplace survives as payload only, not a targeting identity.
                assert_eq!(seat, "groupm");
                assert_eq!(bid["ext"]["marketplace"], "groupm");
                assert!(bid["ext"]["prebid"].get("targetbiddercode").is_none());
            }
        }
        let configs = expected
            .get("fledgeauctionconfigs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        assert_eq!(decoded.auction_configs.len(), configs.len());
        for (actual, captured) in decoded.auction_configs.iter().zip(configs) {
            assert_eq!(actual.imp_id, captured["impid"]);
            assert_eq!(actual.config, captured["config"]);
        }
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        captured_banner => "exemplary/banner.json",
        captured_fledge => "exemplary/fledge.json",
        captured_native => "exemplary/native.json",
        captured_video => "exemplary/video.json",
        captured_app_absent_bid_extension_boundary => "supplemental/app.json",
        captured_unsupported_audio => "supplemental/audio.json",
        captured_ibv_unoffered_media_boundary => "supplemental/bid_ext_ibv_true.json",
        captured_direct_targeting_precedence => "supplemental/dctrAndPmZoneID.json",
        captured_flat_sdk => "supplemental/displayManagerInAppExt.json",
        captured_prebid_sdk_precedence => "supplemental/displayManagerInAppExtPrebid.json",
        captured_marketplace_payload_identity_boundary => "supplemental/extra-bid.json",
        captured_gam_adslot => "supplemental/gptSlotNameInImpExt.json",
        captured_pbadslot => "supplemental/gptSlotNameInImpExtPbAdslot.json",
        captured_fpd_gpid_skadn => "supplemental/impExt.json",
        captured_invalid_adslots => "supplemental/invalidparam.json",
        captured_multiple_media_strips_audio => "supplemental/multiplemedia.json",
        captured_invalid_native_unsent_boundary => "supplemental/native_invalid_adm.json",
        captured_empty_root_extension => "supplemental/nilReqExt.json",
        captured_absent_adslot => "supplemental/noAdSlot.json",
        captured_keyword_zone_normalization => "supplemental/pmZoneIDInKeywords.json",
        captured_root_wrapper_precedence => "supplemental/reqBidderParams.json",
        captured_requested_alias_identity => "supplemental/reqExtPrebidAliases.json",
        captured_publisher_whitespace => "supplemental/trimPublisherID.json",
    );

    #[test]
    fn captured_alias_map_cannot_override_canonical_requested_identity() {
        let fixture = verified_fixture(
            "supplemental/reqExtPrebidAliases.json",
            include_str!("../fixtures/protocol/supplemental/reqExtPrebidAliases.json"),
        );
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
        assert_eq!(calls.len(), 1);
        let mut expected = fixture["httpCalls"][0]["expectedRequest"].clone();
        assert_eq!(
            expected["body"]["ext"]["wrapper"]["biddercode"],
            "pubmatic-alias"
        );
        expected["body"]["ext"]["wrapper"]["biddercode"] = json!("pubmatic");
        assert_request(&calls[0], &expected);
        assert_eq!(serde_json::to_value(request).unwrap(), before);
    }

    #[test]
    fn captured_fpd_with_multiple_keys_has_stable_order_after_direct_prefix() {
        let fixture = verified_fixture(
            "supplemental/impExt.json",
            include_str!("../fixtures/protocol/supplemental/impExt.json"),
        );
        let mut input = fixture["mockBidRequest"].clone();
        input["imp"][0]["ext"]["data"]
            .as_object_mut()
            .unwrap()
            .extend(
                json!({"z":" last ", "a":25_i32})
                    .as_object()
                    .unwrap()
                    .clone(),
            );
        let request: BidRequest = serde_json::from_value(input).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let mut expected = fixture["httpCalls"][0]["expectedRequest"].clone();
        expected["body"]["imp"][0]["ext"]["key_val"] =
            json!("k1=v1|k2=v2|a=25|sport=rugby,cricket|z=last");
        for _iteration in 0_u32..3 {
            let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
            assert_eq!(calls.len(), 1);
            assert_request(&calls[0], &expected);
        }
        assert_eq!(serde_json::to_value(request).unwrap(), before);
    }

    #[test]
    fn captured_invalid_native_adm_is_bid_local_once_unsent_id_is_isolated() {
        let invalid = verified_fixture(
            "supplemental/native_invalid_adm.json",
            include_str!("../fixtures/protocol/supplemental/native_invalid_adm.json"),
        );
        let valid = verified_fixture(
            "exemplary/native.json",
            include_str!("../fixtures/protocol/exemplary/native.json"),
        );
        let request: BidRequest =
            serde_json::from_value(invalid["mockBidRequest"].clone()).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
        assert_eq!(calls.len(), 1);
        let mut body = invalid["httpCalls"][0]["mockResponse"]["body"].clone();
        assert_eq!(
            body["seatbid"][0]["bid"][0]["impid"],
            "test-native-imp-invalid-adm"
        );
        body["seatbid"][0]["bid"][0]["impid"] = json!("test-native-imp");
        let bad_only = PubmaticBidder.make_bids(
            &request,
            calls[0].meta(),
            &harness::budgeted_response(200, serde_json::to_vec(&body).unwrap()),
        );
        harness::assert_bad_server_response_errors(&bad_only.unwrap_err(), 1);
        body["seatbid"][0]["bid"]
            .as_array_mut()
            .unwrap()
            .push(valid["httpCalls"][0]["mockResponse"]["body"]["seatbid"][0]["bid"][0].clone());
        let mut decoded = PubmaticBidder
            .make_bids(
                &request,
                calls[0].meta(),
                &harness::budgeted_response(200, serde_json::to_vec(&body).unwrap()),
            )
            .unwrap();
        harness::assert_bad_server_response_errors(&decoded.errors, 1);
        decoded.errors.clear();
        assert_success(&decoded, &valid["expectedBidResponses"][0]);
        assert_eq!(serde_json::to_value(request).unwrap(), before);
    }

    #[test]
    fn captured_ibv_is_metadata_only_when_banner_was_actually_offered() {
        let fixture = verified_fixture(
            "supplemental/bid_ext_ibv_true.json",
            include_str!("../fixtures/protocol/supplemental/bid_ext_ibv_true.json"),
        );
        let mut input = fixture["mockBidRequest"].clone();
        input["imp"][0]["banner"] = json!({"format":[{"w":300_i32,"h":250_i32}]});
        let request: BidRequest = serde_json::from_value(input).unwrap();
        let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
        assert_eq!(calls.len(), 1);
        let body = &fixture["httpCalls"][0]["mockResponse"]["body"];
        let decoded = PubmaticBidder
            .make_bids(
                &request,
                calls[0].meta(),
                &harness::budgeted_response(200, serde_json::to_vec(body).unwrap()),
            )
            .unwrap();
        assert_success(&decoded, &fixture["expectedBidResponses"][0]);
    }

    #[test]
    fn captured_banner_does_not_supply_a_fallback_for_missing_or_invalid_standard_mtype() {
        let fixture = verified_fixture(
            "exemplary/banner.json",
            include_str!("../fixtures/protocol/exemplary/banner.json"),
        );
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
        assert_eq!(calls.len(), 1);
        for mtype in [
            None,
            Some(json!(null)),
            Some(json!(0_i32)),
            Some(json!(44_i32)),
            Some(json!("1")),
            Some(json!(1.5_f64)),
        ] {
            let mut body = fixture["httpCalls"][0]["mockResponse"]["body"].clone();
            let mut bad = body["seatbid"][0]["bid"][0].clone();
            bad["id"] = json!("invalid-mtype");
            if let Some(media_type) = mtype {
                bad["mtype"] = media_type;
            } else {
                bad.as_object_mut().unwrap().remove("mtype");
            }
            body["seatbid"][0]["bid"]
                .as_array_mut()
                .unwrap()
                .insert(0, bad);
            let mut decoded = PubmaticBidder
                .make_bids(
                    &request,
                    calls[0].meta(),
                    &harness::budgeted_response(200, serde_json::to_vec(&body).unwrap()),
                )
                .unwrap();
            harness::assert_bad_server_response_errors(&decoded.errors, 1);
            decoded.errors.clear();
            assert_success(&decoded, &fixture["expectedBidResponses"][0]);
        }
    }

    #[test]
    fn captured_body_cannot_turn_204_into_a_bid_or_consume_a_parse_budget() {
        let fixture = verified_fixture(
            "exemplary/banner.json",
            include_str!("../fixtures/protocol/exemplary/banner.json"),
        );
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
        assert_eq!(calls.len(), 1);
        let body = serde_json::to_vec(&fixture["httpCalls"][0]["mockResponse"]["body"]).unwrap();
        for bytes in [Vec::new(), body] {
            let response = BidderHttpResponse::new(204, Vec::new(), bytes).within_node_budget(0);
            let decoded = PubmaticBidder
                .make_bids(&request, calls[0].meta(), &response)
                .unwrap();
            assert!(decoded.bids.is_empty());
            assert!(decoded.errors.is_empty());
            assert!(decoded.auction_configs.is_empty());
            assert_eq!(decoded.currency, "USD");
            assert_eq!(response.max_parsed_nodes(), 0);
        }
    }

    #[test]
    fn empty_responses_require_the_shared_openrtb_id_without_a_second_decoder() {
        let fixture = verified_fixture(
            "exemplary/banner.json",
            include_str!("../fixtures/protocol/exemplary/banner.json"),
        );
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
        assert_eq!(calls.len(), 1);
        // The reference defaults an absent ID. Auctioneer keeps the shared required
        // OpenRTB field rather than introducing a bidder-local envelope parser.
        let decoded = PubmaticBidder
            .make_bids(
                &request,
                calls[0].meta(),
                &harness::budgeted_response(200, serde_json::to_vec(&json!({"id":"r"})).unwrap()),
            )
            .unwrap();
        assert!(decoded.bids.is_empty());
        assert!(decoded.errors.is_empty());
        assert!(decoded.auction_configs.is_empty());
        assert_eq!(decoded.currency, "USD");
        for body in [Vec::new(), serde_json::to_vec(&json!({})).unwrap()] {
            let errors = PubmaticBidder
                .make_bids(
                    &request,
                    calls[0].meta(),
                    &harness::budgeted_response(200, body),
                )
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
        }
    }

    #[test]
    fn source_status_errors_are_classified_before_attempting_to_parse_the_body() {
        let fixture = verified_fixture(
            "exemplary/banner.json",
            include_str!("../fixtures/protocol/exemplary/banner.json"),
        );
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let calls = harness::drive_ok(&PubmaticBidder, &request, &info("pubmatic"));
        assert_eq!(calls.len(), 1);
        for (status, code) in [(400_u16, 2), (201, 4), (302, 4), (500, 4)] {
            let response =
                BidderHttpResponse::new(status, Vec::new(), Vec::new()).within_node_budget(0);
            let errors = PubmaticBidder
                .make_bids(&request, calls[0].meta(), &response)
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, code);
        }
    }

    #[test]
    fn exact_capture_membership_and_parameter_literals_have_frozen_witnesses() {
        assert_eq!(format!("{:x}", Sha256::digest(MANIFEST)), MANIFEST_SHA256);
        let manifest: TomlValue = toml::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        let captures: Vec<_> = artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("protocol"))
            .collect();
        assert_eq!(captures.len(), 23);
        assert_eq!(
            captures
                .iter()
                .map(|row| row["source_bytes"].as_integer().unwrap())
                .sum::<i64>(),
            104_454
        );
        let expected_members: BTreeSet<_> = CAPTURE_PATHS
            .iter()
            .map(|path| format!("{SOURCE_PREFIX}{path}"))
            .collect();
        assert_eq!(expected_members.len(), 23);
        let source_members: BTreeSet<_> = captures
            .iter()
            .map(|row| row["source_path"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(source_members, expected_members);
        for row in captures {
            assert_eq!(row["source"].as_str(), Some("upstream"));
            assert_eq!(row["source_bytes"], row["retained"]["source_bytes"]);
            assert_eq!(row["source_sha256"], row["retained"]["source_sha256"]);
            let suffix = row["source_path"]
                .as_str()
                .unwrap()
                .strip_prefix(SOURCE_PREFIX)
                .unwrap();
            assert_eq!(
                row["retained"]["source_path"].as_str().unwrap(),
                format!("fixtures/protocol/{suffix}")
            );
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
                        .and_then(TomlValue::as_str)
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
                format!("{:x}", Sha256::digest(bytes))
            );
        }
        assert_parameter_literals();
    }

    fn assert_parameter_literals() {
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        assert_eq!(cases["valid"].as_array().unwrap().len(), 6);
        assert_eq!(cases["invalid"].as_array().unwrap().len(), 20);
        assert_eq!(cases["invalid"][0], "");
        let schema: Value = serde_json::from_str(include_str!("../parameters.json")).unwrap();
        let validator = options()
            .with_draft(Draft::Draft4)
            .should_validate_formats(true)
            .build(&schema)
            .unwrap();
        let mut malformed = 0_u32;
        for (group, valid) in [("valid", true), ("invalid", false)] {
            for literal in cases[group].as_array().unwrap() {
                let source = literal.as_str().unwrap();
                serde_json::from_str::<Value>(source).map_or_else(
                    |_parse_error| {
                        assert!(!valid);
                        assert!(source.is_empty());
                        malformed = malformed.saturating_add(1);
                    },
                    |value| assert_eq!(validator.is_valid(&value), valid, "{source}"),
                );
            }
        }
        assert_eq!(malformed, 1);
    }

    #[test]
    fn changed_capture_bytes_fail_integrity_before_filtered_dispatch() {
        let path = "fixtures/protocol/exemplary/banner.json";
        let source = include_str!("../fixtures/protocol/exemplary/banner.json");
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

    fn source_object_info(checkout: &OsStr, object: &str) -> Vec<String> {
        let mut child = Command::new("git")
            .args(["--no-replace-objects", "-C"])
            .arg(checkout)
            .args(["cat-file", "--batch-check"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "{object}").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let line = String::from_utf8(output.stdout).unwrap();
        let fields: Vec<_> = line.split_whitespace().map(str::to_owned).collect();
        assert_eq!(fields.len(), 3);
        fields
    }

    #[test]
    #[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR; pinned Git objects only, never Go execution"]
    fn source_objects_match_pinned_trees_artifacts_and_parameter_literals() {
        let checkout = var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap();
        let show = |object: &str| {
            let output = Command::new("git")
                .args(["--no-replace-objects", "-C"])
                .arg(&checkout)
                .args(["show", object])
                .output()
                .unwrap();
            assert!(output.status.success());
            output.stdout
        };
        assert_eq!(format!("{:x}", Sha256::digest(MANIFEST)), MANIFEST_SHA256);
        let manifest: TomlValue = toml::from_str(MANIFEST).unwrap();
        assert_eq!(
            manifest["provenance"]["sources"]["upstream"]["revision"].as_str(),
            Some(REVISION)
        );
        let trees = manifest["provenance"]["trees"].as_array().unwrap();
        assert_eq!(trees.len(), 3);
        let mut source_members = BTreeSet::new();
        for row in trees {
            let path = row["source_path"].as_str().unwrap();
            let object = format!("{REVISION}:{path}");
            let fields = source_object_info(&checkout, &object);
            assert_eq!(fields[0], row["tree"].as_str().unwrap());
            assert_eq!(fields[1], "tree");
            if row["category"].as_str() == Some("protocol") {
                let bytes = show(&object);
                for name in from_utf8(&bytes).unwrap().lines().skip(2) {
                    assert_eq!(Path::new(name).extension(), Some(OsStr::new("json")));
                    assert!(!name.contains('/'));
                    assert!(source_members.insert(format!("{path}/{name}")));
                }
            }
        }
        assert_eq!(
            source_members,
            CAPTURE_PATHS
                .iter()
                .map(|path| format!("{SOURCE_PREFIX}{path}"))
                .collect()
        );
        let rows = manifest["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), 32);
        let mut parameter_source = Vec::new();
        for row in rows {
            let path = row["source_path"].as_str().unwrap();
            let object = format!("{REVISION}:{path}");
            let bytes = show(&object);
            assert_eq!(
                i64::try_from(bytes.len()).unwrap(),
                row["source_bytes"].as_integer().unwrap()
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                row["source_sha256"].as_str().unwrap()
            );
            let fields = source_object_info(&checkout, &object);
            assert_eq!(fields[0], row["source_blob"].as_str().unwrap());
            assert_eq!(fields[1], "blob");
            assert_eq!(fields[2].parse::<usize>().unwrap(), bytes.len());
            if path == "adapters/pubmatic/params_test.go" {
                parameter_source = bytes;
            }
        }
        let source = from_utf8(&parameter_source).unwrap();
        let cases: Value =
            serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
        for (name, group, count) in [
            ("validParams", "valid", 6),
            ("invalidParams", "invalid", 20),
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
