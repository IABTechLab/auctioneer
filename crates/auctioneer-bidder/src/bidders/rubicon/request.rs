use super::floor::floor;
use super::parameters::parameters;
use super::projection::Request;
use super::validation::{
    Failure, media, media_eligibility, validate_conversion_fields, validate_request_fields,
};
use crate::bidders::url::json_utf8_headers;
use crate::meta::{META_BIDDER_REGION_BYTES, SinkAbort};
use crate::{
    BidderError, BidderRequestInfo, ConstructionDiagnostic, EndpointBuilder, OutboundRequestSink,
};
use auctioneer_config::{
    BidderSettings,
    bidder_settings::rubicon::{SettingsRef, resolve},
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub(super) fn credential_key<'a>(
    request: &BidRequest,
    info: &'a BidderRequestInfo<'_>,
) -> Result<Option<&'a str>, BidderError> {
    validate_conversion_fields(request)?;
    regional_host(info)?;
    if (request.site.is_none() && request.app.is_none())
        || !request.imp.iter().any(|imp| {
            parameters(imp).is_ok_and(|params| {
                validate_request_fields(request, imp).is_ok()
                    && floor(imp, info).is_ok()
                    && media(imp, params.multi)
                        .into_iter()
                        .flatten()
                        .any(|selected| media_eligibility(imp, selected).is_ok())
            })
        })
    {
        return Err(BidderError::bad_input("rubicon no eligible impression"));
    }
    credential_reference(info).map(Some)
}

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    validate_conversion_fields(request).map_err(|error| vec![error])?;
    let host = regional_host(info).map_err(|error| vec![error])?;
    credential_reference(info).map_err(|error| vec![error])?;
    let credentials = info.credentials.ok_or_else(|| {
        vec![BidderError::failed_to_request_bids(
            "rubicon credentials unavailable",
        )]
    })?;
    if request.site.is_none() && request.app.is_none() {
        return Err(vec![BidderError::bad_input(
            "rubicon inventory is required",
        )]);
    }
    let external_url = host_settings(info).external_url().unwrap_or("");
    let mut attempted = false;
    let mut first_failure = None;
    for imp in &request.imp {
        let Ok(params) = parameters(imp) else {
            first_failure.get_or_insert(Failure::Parameters);
            sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
            continue;
        };
        if validate_request_fields(request, imp).is_err() {
            first_failure.get_or_insert(Failure::Fields);
            sink.diagnostic(ConstructionDiagnostic::InvalidImpression);
            continue;
        }
        let selections = media(imp, params.multi);
        if selections.iter().all(Option::is_none) {
            first_failure.get_or_insert(Failure::Unsupported);
            sink.diagnostic(ConstructionDiagnostic::UnsupportedMediaType);
            continue;
        }
        let Ok(floor) = floor(imp, info) else {
            first_failure.get_or_insert(Failure::Floor);
            sink.diagnostic(ConstructionDiagnostic::InvalidImpression);
            continue;
        };
        for selected in selections.into_iter().flatten() {
            if let Err(failure) = media_eligibility(imp, selected) {
                first_failure.get_or_insert(failure);
                sink.diagnostic(ConstructionDiagnostic::InvalidImpression);
                continue;
            }
            attempted = true;
            let projected = Request {
                original: request,
                imp,
                params,
                selected,
                floor,
                credentials,
                external_url,
                raw_cap: sink.max_outbound_raw_bytes(),
            };
            let _outcome = sink.write_json(
                info,
                |meta| {
                    meta.set_method("POST")?;
                    let mut endpoint = EndpointBuilder::from_request(info, &[]);
                    if info.endpoint_template.contains("{{.Host}}") {
                        endpoint
                            .select_host(host)
                            .map_err(|_error| SinkAbort::MetaOverCap {
                                limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
                            })?;
                    }
                    endpoint.expand_with_query_into(meta, &[("tk_xint", credentials.tracker())])?;
                    json_utf8_headers(meta)?;
                    meta.header("User-Agent", "prebid-server/1.0")?;
                    meta.header("Authorization", credentials.authorization())?;
                    meta.imp_id(&imp.id)?;
                    meta.correlate_imp(0, selected, 0)?;
                    Ok(())
                },
                &projected,
            );
        }
    }
    if attempted {
        Ok(())
    } else {
        Err(vec![first_failure.unwrap_or(Failure::Unsupported).error()])
    }
}

fn host_settings<'a>(info: &'a BidderRequestInfo<'_>) -> SettingsRef<'a> {
    resolve(
        info.settings().and_then(BidderSettings::as_rubicon),
        info.base_settings().and_then(BidderSettings::as_rubicon),
    )
}

fn regional_host(info: &BidderRequestInfo<'_>) -> Result<&'static str, BidderError> {
    host_settings(info)
        .region()
        .and_then(regional_host_for)
        .ok_or_else(|| BidderError::failed_to_request_bids("rubicon invalid region"))
}

pub(super) fn regional_host_for(region: &str) -> Option<&'static str> {
    match region {
        "us-east" => Some("exapi-us-east.rubiconproject.com"),
        "us-west" => Some("exapi-us-west.rubiconproject.com"),
        "eu" => Some("exapi-eu.rubiconproject.com"),
        "apac" => Some("exapi-apac.rubiconproject.com"),
        _ => None,
    }
}

pub(super) fn valid_credential_reference(key: &str) -> bool {
    !key.is_empty() && key.len() <= 128 && !key.bytes().any(|byte| byte.is_ascii_control())
}

fn credential_reference<'a>(info: &'a BidderRequestInfo<'_>) -> Result<&'a str, BidderError> {
    host_settings(info)
        .credentials_secret()
        .filter(|key| valid_credential_reference(key))
        .ok_or_else(|| BidderError::failed_to_request_bids("rubicon invalid credential reference"))
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::bidders::harness;
    use crate::bidders::rubicon::test_support::{
        ENDPOINT, EndpointPolicy, Transactions, banner, body, context, outbound, request, settings,
    };
    use crate::bidders::rubicon::{RubiconBidder, tests as conformance};
    use crate::correlation::CorrelationMedia;
    use crate::{
        Bidder as _, BidderError, BidderHttpRequest, BidderName, BidderRequestInfo, CallBudget,
        ConstructionDiagnostic, EndpointRejectReason, EndpointValidator, EntryPoint,
        OutboundRequestSink, SinkLimits,
    };
    use auctioneer_config::BidderSettings;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::{cell::Cell, slice::from_ref};

    const REGIONAL_TEMPLATE: &str = "https://{{.Host}}/a/api/exchange";
    const REGIONAL_HOSTS: [&str; 4] = [
        "exapi-us-east.rubiconproject.com",
        "exapi-us-west.rubiconproject.com",
        "exapi-eu.rubiconproject.com",
        "exapi-apac.rubiconproject.com",
    ];

    struct RegionalEndpointPolicy;

    impl EndpointValidator for RegionalEndpointPolicy {
        fn validate(
            &self,
            url: &url::Url,
            info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if url.scheme() != "https" {
                return Err(EndpointRejectReason::SchemeNotAllowed);
            }
            if !url.username().is_empty() || url.password().is_some() {
                return Err(EndpointRejectReason::UserinfoRejected);
            }
            if url.port().is_some()
                || url.path() != "/a/api/exchange"
                || url.fragment().is_some()
                || info.host_allowlist != Some(REGIONAL_HOSTS.as_slice())
                || !REGIONAL_HOSTS.contains(&url.host_str().unwrap_or_default())
            {
                return Err(EndpointRejectReason::HostNotAllowed);
            }
            Ok(())
        }
    }

    fn region_settings(region: Option<&str>) -> BidderSettings {
        let mut values = json!({
            "credentials_secret":"fake_credential_bundle",
            "external_url":"http://hosturl.com"
        });
        if let Some(selected) = region {
            values["region"] = json!(selected);
        }
        settings(&values)
    }

    fn region_context<'a>(
        requested: Option<&'a BidderSettings>,
        base: Option<&'a BidderSettings>,
        endpoint: &'a str,
        hosts: Option<&'a [&'a str]>,
    ) -> BidderRequestInfo<'a> {
        let mut info = BidderRequestInfo::new(
            BidderName::new("fake_alias"),
            BidderName::new("rubicon"),
            "fake_account",
            None,
            endpoint,
            &[],
            hosts,
            None,
            EntryPoint::Auction,
            &[],
        );
        info.set_settings(Some("rubicon"), requested, base).unwrap();
        info
    }

    fn assert_region_route(region: &str, expected_host: &str) {
        let settings = region_settings(Some(region));
        let request = request(&json!([banner("first"), banner("second")]));
        let before = serde_json::to_value(&request).unwrap();
        conformance::with_source_context(|source| {
            let mut info = region_context(
                Some(&settings),
                None,
                REGIONAL_TEMPLATE,
                Some(&REGIONAL_HOSTS),
            );
            assert!(info.credentials.is_none());
            assert_eq!(
                RubiconBidder.credential_key(&request, &info).unwrap(),
                Some("fake_credential_bundle")
            );
            info.credentials = source.credentials;
            let (result, outcome, remaining) = harness::drive_with_limits(
                &RubiconBidder,
                &request,
                &info,
                SinkLimits::new(256 * 1024, 2),
                3,
                &RegionalEndpointPolicy,
            );
            result.unwrap();
            assert!(outcome.errors.is_empty());
            assert!(outcome.diagnostics.is_empty());
            assert_eq!(outcome.rejected_call_count, 0);
            assert_eq!(outcome.accepted.len(), 2);
            assert_eq!(remaining, 1);
            for (call, id) in outcome.accepted.iter().zip(["first", "second"]) {
                assert_eq!(
                    call.url(),
                    format!("https://{expected_host}/a/api/exchange?tk_xint=pbs-test-tracker")
                );
                assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), [id]);
                assert!(
                    call.headers()
                        .any(|header| header == ("Authorization", "Basic eHVzZXI6eHBhc3M="))
                );
                let body = body(call);
                assert_eq!(body["imp"][0]["ext"]["rp"]["target"]["pbs_login"], "xuser");
                assert_eq!(
                    body["imp"][0]["ext"]["rp"]["target"]["pbs_url"],
                    "http://hosturl.com"
                );
            }
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn assert_region_refusal(
        request: &BidRequest,
        requested: &BidderSettings,
        base: Option<&BidderSettings>,
    ) {
        let before = serde_json::to_value(request).unwrap();
        conformance::with_source_context(|source| {
            let mut info = region_context(Some(requested), base, ENDPOINT, None);
            let expected = BidderError::failed_to_request_bids("rubicon invalid region");
            assert_eq!(
                RubiconBidder.credential_key(request, &info).unwrap_err(),
                expected
            );
            // Hold a sibling's permit while varying this bidder's available capacity.
            for credentials in [None, source.credentials] {
                info.credentials = credentials;
                for available in [0_u32, 1] {
                    let initial_permits = available.checked_add(1).unwrap();
                    let budget = CallBudget::new(initial_permits);
                    let sibling = budget.try_reserve().unwrap();
                    let transactions = Transactions(Cell::new(0));
                    let mut sink = OutboundRequestSink::new(
                        SinkLimits::new(256 * 1024, 1),
                        &budget,
                        &transactions,
                    );
                    let result = RubiconBidder.make_requests(request, &info, &mut sink);
                    let outcome = sink.into_outcome_discarding_accepted();
                    assert_eq!(result.unwrap_err().as_slice(), from_ref(&expected));
                    assert!(outcome.accepted.is_empty());
                    assert!(outcome.errors.is_empty());
                    assert!(outcome.diagnostics.is_empty());
                    assert_eq!(outcome.rejected_call_count, 0);
                    assert_eq!(transactions.0.get(), 0);
                    assert_eq!(budget.remaining(), available);
                    drop(sibling);
                    assert_eq!(budget.remaining(), initial_permits);
                }
            }
        });
        assert_eq!(serde_json::to_value(request).unwrap(), before);
    }

    #[test]
    fn region_us_east_routes_to_approved_host() {
        assert_region_route("us-east", "exapi-us-east.rubiconproject.com");
    }

    #[test]
    fn region_us_west_routes_to_approved_host() {
        assert_region_route("us-west", "exapi-us-west.rubiconproject.com");
    }

    #[test]
    fn region_eu_routes_to_approved_host() {
        assert_region_route("eu", "exapi-eu.rubiconproject.com");
    }

    #[test]
    fn region_apac_routes_to_approved_host() {
        assert_region_route("apac", "exapi-apac.rubiconproject.com");
    }

    #[test]
    fn region_missing_refuses_before_credential_selection_or_sink_work() {
        assert_region_refusal(
            &request(&json!([banner("i")])),
            &region_settings(None),
            None,
        );
    }

    #[test]
    fn region_empty_refuses_before_credential_selection_or_sink_work() {
        assert_region_refusal(
            &request(&json!([banner("i")])),
            &region_settings(Some("")),
            None,
        );
    }

    #[test]
    fn region_unknown_refuses_without_case_trimming_host_or_url_fallback() {
        let request = request(&json!([banner("i")]));
        for invalid in [
            "unknown",
            "US-EAST",
            "us_east",
            "us-east ",
            " eu",
            "exapi-us-east",
            "exapi-us-east.rubiconproject.com",
            "https://exapi-us-east.rubiconproject.com/a/api/exchange",
        ] {
            assert_region_refusal(&request, &region_settings(Some(invalid)), None);
        }
    }

    #[test]
    fn region_invalid_requested_value_does_not_fall_back_to_a_valid_base() {
        let request = request(&json!([banner("i")]));
        let base = region_settings(Some("us-east"));
        for invalid in ["", "unknown"] {
            assert_region_refusal(&request, &region_settings(Some(invalid)), Some(&base));
        }
    }

    #[test]
    fn region_requested_value_overrides_base_and_absent_or_null_inherits_it() {
        let request = request(&json!([banner("i")]));
        let before = serde_json::to_value(&request).unwrap();
        let base = region_settings(Some("us-east"));
        for (values, host) in [
            (json!({}), "exapi-us-east.rubiconproject.com"),
            (json!({"region":null}), "exapi-us-east.rubiconproject.com"),
            (json!({"region":"eu"}), "exapi-eu.rubiconproject.com"),
        ] {
            let requested = settings(&values);
            conformance::with_source_context(|source| {
                let mut info = region_context(
                    Some(&requested),
                    Some(&base),
                    REGIONAL_TEMPLATE,
                    Some(&REGIONAL_HOSTS),
                );
                assert_eq!(
                    RubiconBidder.credential_key(&request, &info).unwrap(),
                    Some("fake_credential_bundle")
                );
                info.credentials = source.credentials;
                let (result, outcome, remaining) = harness::drive_with_limits(
                    &RubiconBidder,
                    &request,
                    &info,
                    SinkLimits::new(256 * 1024, 1),
                    1,
                    &RegionalEndpointPolicy,
                );
                result.unwrap();
                assert!(outcome.errors.is_empty());
                assert_eq!(outcome.accepted.len(), 1);
                assert_eq!(remaining, 0);
                assert_eq!(
                    outcome.accepted[0].url(),
                    format!("https://{host}/a/api/exchange?tk_xint=pbs-test-tracker")
                );
            });
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn region_request_parameters_and_supplied_credentials_cannot_replace_host_settings() {
        let mut imp = banner("i");
        imp["ext"]["bidder"]["region"] = json!("us-east");
        imp["ext"]["bidder"]["Region"] = json!("exapi-us-east");
        imp["ext"]["bidder"]["host"] = json!("exapi-us-east.rubiconproject.com");
        let mut wire = json!({"id":"r", "site":{}, "imp":[imp],
            "ext":{"region":"eu","Region":"exapi-eu"}});
        wire["user"] = json!({"buyeruid":"apac"});
        let request: BidRequest = serde_json::from_value(wire).unwrap();
        assert_region_refusal(&request, &region_settings(None), None);
    }

    #[test]
    fn region_valid_selection_keeps_a_constant_test_endpoint_unchanged() {
        let settings = region_settings(Some("apac"));
        let request = request(&json!([banner("i")]));
        let before = serde_json::to_value(&request).unwrap();
        conformance::with_source_context(|source| {
            let mut info = region_context(Some(&settings), None, ENDPOINT, None);
            info.credentials = source.credentials;
            let (result, outcome, remaining) = harness::drive_with_limits(
                &RubiconBidder,
                &request,
                &info,
                SinkLimits::new(256 * 1024, 1),
                1,
                &EndpointPolicy(true),
            );
            result.unwrap();
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(remaining, 0);
            assert_eq!(
                outcome.accepted[0].url(),
                format!("{ENDPOINT}?tk_xint=pbs-test-tracker")
            );
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn region_manifest_declares_optional_string_and_default_inactive_native_publication() {
        let manifest: toml::Value = toml::from_str(include_str!("bidder.toml")).unwrap();
        assert_eq!(
            manifest["settings"]["fields"]["region"].as_str(),
            Some("optional_string")
        );
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
        for invalid in [json!(1_i32), json!(false), json!([]), json!({})] {
            let error = serde_json::from_value::<BidderSettings>(json!({
                "implementation":"rubicon","values":{"region":invalid}
            }))
            .unwrap_err();
            assert!(error.is_data());
        }
    }

    #[test]
    fn credential_preflight_finds_a_valid_sibling_and_borrows_one_host_reference() {
        let settings = settings(&json!({
            "credentials_secret":"fake_credential_bundle","region":"us-east"
        }));
        let mut info = context();
        info.set_settings(Some("rubicon"), Some(&settings), None)
            .unwrap();
        let mut invalid = banner("invalid");
        invalid["ext"]["bidder"] = json!([]);
        let request = request(&json!([invalid, banner("valid")]));
        let key = RubiconBidder
            .credential_key(&request, &info)
            .unwrap()
            .unwrap();
        let expected = settings
            .as_rubicon()
            .unwrap()
            .credentials_secret
            .as_deref()
            .unwrap();
        assert_eq!(key, "fake_credential_bundle");
        assert_eq!(key.as_ptr(), expected.as_ptr());
        assert!(info.credentials.is_none());
    }

    #[test]
    fn credential_reference_config_admission_enforces_byte_and_ascii_control_bounds() {
        let request = request(&json!([banner("i")]));
        for (key, valid) in [
            (String::new(), false),
            ("x".to_owned(), true),
            ("x".repeat(128), true),
            ("x".repeat(129), false),
            ("\u{e9}".repeat(64), true),
            ("\u{e9}".repeat(65), false),
            (" ".to_owned(), true),
            ("fake\nreference".to_owned(), false),
            ("fake\u{7f}reference".to_owned(), false),
            ("fake\u{85}reference".to_owned(), true),
        ] {
            let decoded = serde_json::from_value::<BidderSettings>(json!({
                "implementation":"rubicon", "values":{"credentials_secret":key,"region":"us-east"}
            }));
            if !valid {
                let error = decoded.unwrap_err();
                assert!(error.is_data());
                assert!(error.to_string().contains("invalid credential reference"));
                continue;
            }
            let settings = decoded.unwrap();
            let mut info = context();
            info.set_settings(Some("rubicon"), Some(&settings), None)
                .unwrap();
            let resolved = RubiconBidder.credential_key(&request, &info);
            assert_eq!(resolved.unwrap(), Some(key.as_str()));
        }
    }

    #[test]
    fn requested_and_base_host_fields_resolve_independently_and_null_still_falls_back() {
        let request = request(&json!([banner("i")]));
        let base = settings(
            &json!({"credentials_secret":"fake_base", "external_url":"http://hosturl.com",
                "region":"us-east"}),
        );
        for values in [
            json!({}),
            json!({"credentials_secret":"fake_requested"}),
            json!({"external_url":"http://test.url/"}),
            json!({"credentials_secret":null}),
        ] {
            let requested = settings(&values.clone());
            let mut info = context();
            info.set_settings(Some("rubicon"), Some(&requested), Some(&base))
                .unwrap();
            let actual = RubiconBidder.credential_key(&request, &info);
            match values.get("credentials_secret").and_then(Value::as_str) {
                Some(value) => assert_eq!(actual.unwrap(), Some(value)),
                None => assert_eq!(actual.unwrap(), Some("fake_base")),
            }
        }
        let region_only = settings(&json!({"region":"us-east"}));
        let mut no_reference = context();
        no_reference
            .set_settings(Some("rubicon"), Some(&region_only), None)
            .unwrap();
        let error = RubiconBidder
            .credential_key(&request, &no_reference)
            .unwrap_err();
        assert_eq!(error.code, 5);
        assert_eq!(error.message, "rubicon invalid credential reference");
    }

    #[test]
    fn empty_requested_reference_is_rejected_before_base_fallback_or_settings_construction() {
        let decoded = serde_json::from_value::<BidderSettings>(json!({
            "implementation":"rubicon","values":{"credentials_secret":""}
        }));
        let error = decoded.unwrap_err();
        assert!(error.is_data());
        assert!(error.to_string().contains("invalid credential reference"));
    }

    #[test]
    fn parameters_cannot_supply_or_override_a_secret_reference() {
        let mut imp = banner("i");
        imp["ext"]["bidder"]["credentials_secret"] = json!("untrusted_reference");
        let request = request(&json!([imp]));
        conformance::with_source_context(|info| {
            assert_eq!(
                RubiconBidder.credential_key(&request, info).unwrap(),
                Some("fake_credential_bundle")
            );
            let mut no_settings = context();
            let region_only = settings(&json!({"region":"us-east"}));
            no_settings
                .set_settings(Some("rubicon"), Some(&region_only), None)
                .unwrap();
            no_settings.credentials = info.credentials;
            assert_eq!(
                RubiconBidder
                    .credential_key(&request, &no_settings)
                    .unwrap_err()
                    .message,
                "rubicon invalid credential reference"
            );
        });
    }

    #[test]
    fn each_impression_has_its_own_call_ids_and_credentials() {
        let request = request(&json!([banner("first"), banner("second")]));
        let before = serde_json::to_value(&request).unwrap();
        let calls = outbound(&request);
        assert_eq!(calls.len(), 2);
        for (call, id) in calls.iter().zip(["first", "second"]) {
            assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), [id]);
            assert_eq!(body(call)["imp"].as_array().unwrap().len(), 1);
            assert_eq!(body(call)["imp"][0]["id"], id);
            assert_eq!(
                call.meta().imp_correlation(0).unwrap().media(),
                CorrelationMedia::Banner
            );
            assert_eq!(call.url(), format!("{ENDPOINT}?tk_xint=pbs-test-tracker"));
            assert!(
                call.headers()
                    .any(|header| header == ("Authorization", "Basic eHVzZXI6eHBhc3M="))
            );
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn invalid_impressions_and_floors_preserve_their_valid_siblings_and_call_budget() {
        let mut invalid = banner("bad");
        invalid["ext"]["bidder"] = json!([]);
        let mut foreign = banner("foreign");
        foreign["bidfloor"] = json!(1.0_f64);
        foreign["bidfloorcur"] = json!("EUR");
        let request = request(&json!([banner("first"), invalid, foreign, banner("last")]));
        conformance::with_source_context(|info| {
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &RubiconBidder,
                &request,
                info,
                SinkLimits::new(8192, 4),
                4,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), 2);
            assert_eq!(remaining, 2);
            assert!(outcome.errors.is_empty());
            assert_eq!(
                outcome
                    .accepted
                    .iter()
                    .flat_map(BidderHttpRequest::covered_imp_ids)
                    .collect::<Vec<_>>(),
                ["first", "last"]
            );
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [
                    ConstructionDiagnostic::InvalidImpressionParameters,
                    ConstructionDiagnostic::InvalidImpression
                ]
            );
        });
    }

    #[test]
    fn capped_body_and_endpoint_refusals_refund_slots_without_mutating_the_request() {
        let request = request(&json!([banner("i")]));
        let before = serde_json::to_value(&request).unwrap();
        conformance::with_source_context(|info| {
            for (cap, slots, allow) in [(8192_usize, 0_u32, true), (8, 1, true), (8192, 1, false)] {
                let (semantic, outcome, remaining) = harness::drive_with_limits(
                    &RubiconBidder,
                    &request,
                    info,
                    SinkLimits::new(cap, 1),
                    slots,
                    &EndpointPolicy(allow),
                );
                semantic.unwrap();
                assert!(outcome.accepted.is_empty());
                assert_eq!(outcome.errors.len(), 1);
                assert_eq!(remaining, slots);
            }
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }
}
