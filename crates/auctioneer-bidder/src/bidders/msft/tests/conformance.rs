//! Source protocol captures only; ordinary units remain inline beside their owners.

#[cfg(test)]
mod tests {
    use super::super::super::{
        CAPTURE_MANIFEST_SHA256, MsftBidder,
        request::{member_mismatch, request_ext_error},
        response::{status_error, unsupported_bid_type as unsupported_bid_type_error},
    };
    use crate::Bidder as _;
    use crate::BidderSettings;
    use crate::bidders::harness::{self, protocol_fixtures, verify_protocol_capture};
    use crate::{
        BidderError, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse,
        ConstructionDiagnostic, EndpointRejectReason, EndpointValidator, EntryPoint, SinkLimits,
        SinkOutcome, TypedBid,
    };
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
    use auctioneer_runtime::bounded_json::JsonFieldShape;
    use serde_json::{Value, json};

    const MANIFEST: &str = include_str!("../bidder.toml");
    const MANIFEST_SHA256: &str = CAPTURE_MANIFEST_SHA256;

    const MSFT_OPERATION_FRAGMENTS: [(&str, &[&str]); 10] = [
        (
            "params",
            &[
                "placement_id",
                "member/inv_code",
                "oneOf",
                "schema qualification distinct",
            ],
        ),
        (
            "member_consistency",
            &[
                "on valid impressions",
                "replacing existing member_id values",
                "before calls",
            ],
        ),
        (
            "imp_transform",
            &[
                "pub_click from pubclick",
                "nonempty gpid",
                "inv_code overwrites tagid",
                "banner_frameworks fills banner.api",
                "both are nil",
                "input is unchanged",
            ],
        ),
        (
            "sdk_version",
            &[
                "missing or empty displaymanagerver",
                "app.ext.prebid.source",
                "app.ext.prebid.version",
                "may be empty",
                "nonempty explicit displaymanagerver",
            ],
        ),
        (
            "request_ext",
            &[
                "include_brand_category",
                "brand_category_uniqueness",
                "discarding unknown appnexus keys",
                "request.ext.prebid.targeting.includebrandcategory",
                "video-entrypoint default 6",
            ],
        ),
        (
            "chunks",
            &[
                "at most 10 per POST",
                "10/11/21",
                "Content-Type application/json;charset=utf-8",
                "Accept application/json",
                "x-openrtb-version 2.6",
                "cannot allocate refused bodies",
            ],
        ),
        (
            "bid_local_media",
            &[
                "bid.ext.appnexus.bid_ad_type 0/1/3",
                "only that bid",
                "not mtype",
                "conflicting-hint checks remain required",
            ],
        ),
        (
            "category_video_deal",
            &[
                "95 pinned brand_category_id-to-IAB mappings",
                "without adapter-local rejection",
                "Every accepted bid",
                "creative_info.video.duration",
                "empty primary_category",
                "zero/negative duration and non-video bids",
                "deal_priority without a positivity filter",
                "default USD",
            ],
        ),
        (
            "entrypoints",
            &[
                "is_amp to 1",
                "hb_source_video",
                "Video impression presence alone does not",
                "Neither creates a new public route",
            ],
        ),
        (
            "status_envelope",
            &[
                "204 is no bid without body decoding",
                "400 is BadInput",
                "other non-200 statuses are BadServerResponse",
                "sealed decoding",
                "defaults currency to USD",
            ],
        ),
    ];

    #[test]
    fn priority_contract_uses_source_wire_names_and_semantics() {
        let wave: toml::Value = toml::from_str(include_str!(
            "../../../../tests/conformance/fixtures/waves/priority.toml"
        ))
        .unwrap();
        let owner = wave["bidders"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"].as_str() == Some("msft"))
            .unwrap();
        assert_eq!(wave["review_status"].as_str(), Some("pending"));
        assert_eq!(owner["base"].as_str(), Some("msft"));
        assert_eq!(owner["implementation"].as_str(), Some("msft"));
        assert_eq!(owner["routing"].as_str(), Some("https_default_pending"));
        let operations = owner["operations"].as_array().unwrap();
        assert_eq!(operations.len(), 13);
        for (id, fragments) in MSFT_OPERATION_FRAGMENTS {
            let operation = operations
                .iter()
                .find(|row| row["id"].as_str() == Some(id))
                .unwrap();
            assert_eq!(
                operation["driver"].as_str(),
                Some("auctioneer-bidder::msft"),
                "{id}"
            );
            for &fragment in fragments {
                assert!(
                    operation["assertion"].as_str().unwrap().contains(fragment),
                    "{id}: missing {fragment}"
                );
            }
        }
        for operation in operations {
            for obsolete in [
                "placementId",
                "invCode",
                "bannerFrameworks",
                "includeBrandCategory",
            ] {
                assert!(
                    !operation["assertion"].as_str().unwrap().contains(obsolete),
                    "{}: {obsolete}",
                    operation["id"]
                );
            }
        }
    }

    mod immutable_source {
        use sha2::{Digest as _, Sha256};
        use std::collections::BTreeSet;
        use std::env::var_os;
        use std::process::Command;

        const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";

        fn source_object(object: &str) -> Vec<u8> {
            let repository = var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap();
            let output = Command::new("git")
                .args(["--no-replace-objects", "-C"])
                .arg(repository)
                .args(["show", object])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "cannot inspect immutable source object {object}"
            );
            output.stdout
        }

        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ParameterCapture {
            schema_version: u8,
            cases: Vec<ParameterCase>,
        }

        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ParameterCase {
            group: String,
            index: usize,
            valid: bool,
            input: String,
            source_offset: usize,
            bytes: usize,
            sha256: String,
        }

        #[test]
        #[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR; immutable Git objects only"]
        fn source_objects_parameter_captures_preserve_all_source_literals_offsets_and_verdicts() {
            let bytes = source_object(&format!("{REVISION}:adapters/msft/params_test.go"));
            let text = String::from_utf8(bytes).unwrap();
            let capture: ParameterCapture =
                serde_json::from_str(include_str!("../fixtures/parameters.json")).unwrap();
            assert_eq!(capture.schema_version, 1);
            assert_eq!(capture.cases.len(), 33);
            let mut checked = 0_usize;
            for (group, count, valid) in [("validParams", 12, true), ("invalidParams", 21, false)] {
                let declaration = format!("var {group} = []string{{");
                let body = text
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
                let rows: Vec<_> = capture
                    .cases
                    .iter()
                    .filter(|row| row.group == group)
                    .collect();
                assert_eq!(literals.len(), count);
                assert_eq!(rows.len(), count);
                for (index, (literal, row)) in literals.iter().zip(rows).enumerate() {
                    assert_eq!(row.index, index);
                    assert_eq!(row.valid, valid);
                    assert_eq!(row.input, *literal);
                    assert_eq!(row.bytes, literal.len());
                    assert_eq!(row.sha256, hex::encode(Sha256::digest(literal.as_bytes())));
                    let end = row.source_offset.checked_add(row.bytes).unwrap();
                    assert_eq!(text.get(row.source_offset..end), Some(row.input.as_str()));
                    assert_eq!(
                        text.as_bytes()
                            .get(row.source_offset.checked_sub(1).unwrap()),
                        Some(&b'`')
                    );
                    assert_eq!(text.as_bytes().get(end), Some(&b'`'));
                    checked += 1;
                }
            }
            assert_eq!(checked, 33);
        }

        #[test]
        #[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR; immutable Git objects only"]
        fn source_objects_category_witness_preserves_the_complete_pinned_source_map() {
            let bytes = source_object(&format!("{REVISION}:adapters/msft/iab_categories.go"));
            let text = String::from_utf8(bytes).unwrap();
            let body = text
                .split_once("var iabCategoryMap = map[int]string{")
                .unwrap()
                .1
                .split_once("\n}")
                .unwrap()
                .0;
            let pairs: Vec<(i64, String)> = body
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let (key, value) = line.trim().split_once(':').unwrap();
                    (
                        key.parse().unwrap(),
                        serde_json::from_str(value.trim().trim_end_matches(',')).unwrap(),
                    )
                })
                .collect();
            let captured: Vec<(i64, String)> =
                serde_json::from_str(include_str!("../fixtures/iab_categories.json")).unwrap();
            assert_eq!(pairs.len(), 95);
            assert_eq!(
                pairs
                    .iter()
                    .map(|(key, _value)| *key)
                    .collect::<BTreeSet<_>>()
                    .len(),
                95
            );
            assert_eq!(captured, pairs);
        }
    }

    struct CaptureEndpoint;
    impl EndpointValidator for CaptureEndpoint {
        fn validate(
            &self,
            url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if url.scheme() == "http" && url.host_str() == Some("any.url") && url.path() == "/" {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
    }

    fn run_fixture(name: &str, path: &str, source: &str) {
        let fixture = load_fixture(path, source);
        let request: BidRequest =
            serde_json::from_value(fixture["mockBidRequest"].clone()).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let settings = fixture_settings();
        let info = fixture_info(&fixture, path, &settings);
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info,
            SinkLimits::new(256 * 1024, 100),
            16,
            &CaptureEndpoint,
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        assert!(outcome.errors.is_empty());
        if let Some(source_errors) = fixture.get("expectedMakeRequestsErrors") {
            assert_request_errors(name, source_errors, semantic, &outcome, remaining);
            return;
        }
        semantic.unwrap();
        assert!(outcome.diagnostics.is_empty());
        let source_calls = fixture["httpCalls"].as_array().unwrap();
        assert!(!source_calls.is_empty());
        assert_eq!(outcome.accepted.len(), source_calls.len());
        assert_eq!(
            remaining,
            16_u32
                .checked_sub(u32::try_from(source_calls.len()).unwrap())
                .unwrap()
        );
        let mut positive_responses = 0_usize;
        for (index, (call, captured)) in outcome.accepted.iter().zip(source_calls).enumerate() {
            assert_request_capture(call, &captured["expectedRequest"]);
            if assert_response_capture(name, &fixture, &request, call, captured, index) {
                positive_responses = positive_responses.checked_add(1).unwrap();
            }
        }
        assert_eq!(
            positive_responses,
            usize::from(!matches!(
                name,
                "split_eleven"
                    | "http_400"
                    | "unsupported_bid_type"
                    | "sdk_repeated_with_unsent_response_identity"
            ))
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn load_fixture(path: &str, source: &str) -> Value {
        verify_protocol_capture(
            MANIFEST,
            MANIFEST_SHA256,
            &format!("fixtures/protocol/{path}"),
            source,
        )
        .unwrap();
        serde_json::from_str(source).unwrap()
    }

    fn fixture_settings() -> BidderSettings {
        // Literal values from pinned TestJsonSamplesWithExtraInfo, not actual output.
        serde_json::from_value(json!({"implementation":"msft",
            "settings":{"hb_source":50,"hb_source_video":60}}))
        .unwrap()
    }

    fn fixture_info<'a>(
        fixture: &Value,
        path: &str,
        settings: &'a BidderSettings,
    ) -> BidderRequestInfo<'a> {
        let entry = if path.starts_with("test/amp/") {
            EntryPoint::Amp
        } else if path.starts_with("test/video/") || path.starts_with("test-extrainfo/video/") {
            EntryPoint::Video
        } else {
            EntryPoint::Auction
        };
        let mut info = BidderRequestInfo::new(
            BidderName::new("msft"),
            BidderName::new("msft"),
            "account",
            None,
            "http://any.url",
            &[],
            None,
            None,
            entry,
            &[],
        );
        info.original_request_ext_shape = if fixture["mockBidRequest"].get("ext").is_some() {
            JsonFieldShape::Object
        } else {
            JsonFieldShape::Absent
        };
        if path.starts_with("test-extrainfo/") {
            info.set_settings(Some("msft"), Some(settings), None)
                .unwrap();
        }
        info
    }

    fn assert_request_errors(
        name: &str,
        source_errors: &Value,
        semantic: Result<(), Vec<BidderError>>,
        outcome: &SinkOutcome,
        remaining: u32,
    ) {
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 16);
        match name {
            "none_valid" => {
                semantic.unwrap();
                assert_eq!(source_errors.as_array().unwrap().len(), 2);
                assert_eq!(
                    source_errors[0]["value"],
                    "failed to parse ext for impression id 'test-imp-id-1'"
                );
                assert_eq!(
                    source_errors[1]["value"],
                    "failed to parse ext for impression id 'test-imp-id-2'"
                );
                assert_eq!(
                    outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                    [ConstructionDiagnostic::InvalidImpressionParameters; 2]
                );
            }
            "member_conflict" => {
                assert_eq!(source_errors.as_array().unwrap().len(), 1);
                assert_eq!(
                    source_errors[0]["value"],
                    "member id mismatch: all impressions must use the same member id"
                );
                assert_eq!(semantic.unwrap_err(), [member_mismatch()]);
                assert!(outcome.diagnostics.is_empty());
            }
            "invalid_request_namespace" => {
                assert_eq!(source_errors.as_array().unwrap().len(), 1);
                assert_eq!(source_errors[0]["value"], "malformed request ext.appnexus");
                assert_eq!(semantic.unwrap_err(), [request_ext_error()]);
                assert!(outcome.diagnostics.is_empty());
            }
            _ => panic!("unexpected request failure capture"),
        }
    }

    fn assert_request_capture(call: &BidderHttpRequest, expected: &Value) {
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), expected["uri"].as_str().unwrap());
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
        assert_eq!(
            call.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
                ("x-openrtb-version", "2.6")
            ]
        );
    }

    fn assert_response_capture(
        name: &str,
        fixture: &Value,
        request: &BidRequest,
        call: &BidderHttpRequest,
        captured: &Value,
        index: usize,
    ) -> bool {
        let mock = &captured["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let bytes = mock
            .get("body")
            .map(serde_json::to_vec)
            .transpose()
            .unwrap()
            .unwrap_or_default();
        let result = MsftBidder.make_bids(
            request,
            call.meta(),
            &harness::budgeted_response(status, bytes),
        );
        if status == 204 {
            assert_eq!(fixture["httpCalls"].as_array().unwrap().len(), 2);
            assert!(fixture.get("expectedBidResponses").is_none());
            assert!(result.unwrap().bids.is_empty());
            return false;
        }
        if name == "http_400" || name == "unsupported_bid_type" {
            assert_response_errors(name, fixture, &result.unwrap_err());
            return false;
        }
        if name == "sdk_repeated_with_unsent_response_identity" {
            assert_eq!(
                call.covered_imp_ids().collect::<Vec<_>>(),
                ["test-imp-id-1", "test-imp-id-2"]
            );
            let expected = &fixture["expectedBidResponses"][index]["bids"];
            assert_eq!(expected.as_array().unwrap().len(), 1);
            assert_eq!(expected[0]["bid"]["impid"], "test-imp-id");
            assert_eq!(
                result.unwrap_err(),
                [BidderError::bad_server_response(
                    "bid response references an unsent impression"
                )]
            );
            return false;
        }
        assert_response_success(&result.unwrap(), &fixture["expectedBidResponses"][index]);
        true
    }

    fn assert_response_errors(name: &str, fixture: &Value, errors: &[BidderError]) {
        match name {
            "http_400" => {
                assert_eq!(
                    fixture["expectedMakeBidsErrors"].as_array().unwrap().len(),
                    1
                );
                assert_eq!(
                    fixture["expectedMakeBidsErrors"][0]["value"],
                    "Unexpected status code: 400. Run with request.debug = 1 for more info"
                );
                assert_eq!(errors, [status_error(400)]);
            }
            "unsupported_bid_type" => {
                assert_eq!(
                    fixture["expectedMakeBidsErrors"].as_array().unwrap().len(),
                    1
                );
                assert_eq!(
                    fixture["expectedMakeBidsErrors"][0]["value"],
                    "unrecognized bid_ad_type in response: 2"
                );
                assert_eq!(fixture["expectedBidResponses"][0]["currency"], "USD");
                assert!(
                    fixture["expectedBidResponses"][0]["bids"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
                // Auctioneer returns Err for all-invalid, rather than an empty response plus errors.
                assert_eq!(errors, [unsupported_bid_type_error()]);
            }
            _ => panic!("unexpected response failure capture"),
        }
    }

    fn assert_response_success(decoded: &BidderResponse, response: &Value) {
        assert!(decoded.errors.is_empty());
        assert_eq!(decoded.currency, response["currency"].as_str().unwrap());
        let expected_bids = response["bids"].as_array().unwrap();
        assert!(!expected_bids.is_empty());
        assert_eq!(decoded.bids.len(), expected_bids.len());
        for (actual, expected) in decoded.bids.iter().zip(expected_bids) {
            assert_bid_capture(actual, expected);
        }
    }

    fn assert_bid_capture(actual: &TypedBid, expected: &Value) {
        assert_original_bid_and_metadata(&actual.bid, &expected["bid"]);
        let media = match expected["type"].as_str().unwrap() {
            "banner" => MediaType::Banner,
            "video" => MediaType::Video,
            "native" => MediaType::Native,
            _ => panic!("unexpected captured media"),
        };
        assert_eq!(actual.media_type, media);
    }

    fn assert_original_bid_and_metadata(actual: &Bid, source_bid: &Value) {
        assert!(source_bid["ext"].get("prebid").is_none());
        let appnexus = &source_bid["ext"]["appnexus"];
        let duration = appnexus
            .pointer("/creative_info/video/duration")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let priority = appnexus["deal_priority"].as_i64().unwrap_or(0);
        let mut wire_expected = source_bid.clone();
        wire_expected["ext"]["prebid"] = json!({
            "video":{"duration":duration,"primary_category":""}});
        if priority != 0 {
            wire_expected["ext"]["prebid"]["dealpriority"] = json!(priority);
        }
        assert_eq!(serde_json::to_value(actual).unwrap(), wire_expected);
    }

    protocol_fixtures!(run_fixture,"../fixtures/protocol/";
        amp => "test/amp/simple-banner.json",
        all_parameters_direct_adapter_schema_boundary => "test/exemplary/all-params.json",
        banner => "test/exemplary/simple-banner.json",
        native => "test/exemplary/simple-native.json",
        video_in_auction => "test/exemplary/simple-video.json",
        existing_banner_api => "test/supplemental/banner-api-not-overridden.json",
        category_override => "test/supplemental/bid-cat-brand-category-override.json",
        ambiguous_categories => "test/supplemental/bid-cat-multiple.json",
        existing_category => "test/supplemental/bid-cat.json",
        unsupported_bid_type => "test/supplemental/bid-type-invalid.json",
        sdk_missing_source => "test/supplemental/displaymanagerver-generated-missing-source.json",
        sdk_missing_version => "test/supplemental/displaymanagerver-generated-missing-version.json",
        sdk_repeated_with_unsent_response_identity => "test/supplemental/displaymanagerver-generated-multiple-imps.json",
        sdk_generated => "test/supplemental/displaymanagerver-generated.json",
        explicit_dimensions => "test/supplemental/explicit-dimensions.json",
        foreign_currency => "test/supplemental/foreign-currency.json",
        gpid => "test/supplemental/gpid-passthrough.json",
        none_valid => "test/supplemental/imps-none-valid.json",
        category_flags => "test/supplemental/include-brand-category.json",
        inventory_override => "test/supplemental/inv-code-overridden.json",
        member_conflict => "test/supplemental/member-id-conflict.json",
        member_query => "test/supplemental/member-id.json",
        multi_bid => "test/supplemental/multi-bid.json",
        invalid_request_namespace => "test/supplemental/request-ext-appnexus-invalid.json",
        typed_request_namespace => "test/supplemental/request-ext-appnexus-passthrough.json",
        split_eleven => "test/supplemental/request-splitting.json",
        http_400 => "test/supplemental/status-400.json",
        unmapped_category => "test/supplemental/unmapped-brand-category.json",
        video_entrypoint => "test/video/simple-video.json",
        overridden_auction_source => "test-extrainfo/exemplary/simple-banner.json",
        overridden_video_source => "test-extrainfo/video/simple-video.json",
    );

    #[test]
    fn capture_inventory_has_all_thirty_one_verbatim_sources_and_default_inactive_native_publication()
     {
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        let artifacts = manifest["provenance"]["artifacts"].as_array().unwrap();
        let captures: Vec<_> = artifacts
            .iter()
            .filter(|artifact| artifact["category"].as_str() == Some("protocol"))
            .collect();
        assert_eq!(captures.len(), 31);
        assert_eq!(
            captures
                .iter()
                .map(|artifact| artifact["source_bytes"].as_integer().unwrap())
                .sum::<i64>(),
            117_400
        );
        assert_eq!(manifest["native_only"].as_bool(), Some(true));
        let definitions = manifest["definitions"].as_table().unwrap();
        assert_eq!(
            definitions.keys().map(String::as_str).collect::<Vec<_>>(),
            ["msft"]
        );
        assert_eq!(definitions["msft"]["defaultEnabled"].as_bool(), Some(false));
        assert!(definitions["msft"].get("aliasOf").is_none());
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        for capture in captures {
            let retained = &capture["retained"];
            assert_eq!(retained.as_table().unwrap().len(), 3);
            assert!(
                retained["source_path"]
                    .as_str()
                    .unwrap()
                    .starts_with("fixtures/protocol/")
            );
            assert_eq!(retained["source_bytes"], capture["source_bytes"]);
            assert_eq!(retained["source_sha256"], capture["source_sha256"]);
            assert!(capture.get("transformation").is_none());
        }
        let schema: Value = serde_json::from_str(include_str!("../parameters.json")).unwrap();
        assert_eq!(
            schema["oneOf"],
            json!([{"required":["placement_id"]},{"required":["inv_code","member"]}])
        );
    }

    #[test]
    fn parse_equivalent_capture_drift_is_refused_before_dispatch() {
        let source = include_str!("../fixtures/protocol/test/exemplary/simple-banner.json");
        assert!(!source.ends_with('\n'));
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
                "fixtures/protocol/test/exemplary/simple-banner.json",
                &changed
            ),
            Err("protocol capture digest mismatch")
        );
    }
}
