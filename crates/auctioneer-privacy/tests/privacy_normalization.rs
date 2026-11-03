#[cfg(test)]
use auctioneer_privacy::test_support::tc_encode;

#[cfg(test)]
mod tests {
    use auctioneer_config_data::types::{
        AccountConfig, EnforceAlgo, HostConfig, PurposeEnforcement, TriState,
    };
    use auctioneer_openrtb::WarningCode;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_privacy::ccpa::NoSale;
    use auctioneer_privacy::gcl::{CmpRecord, Gcl};
    use auctioneer_privacy::gpp::Limits as GppLimits;
    use auctioneer_privacy::gvl::{Gvl, GvlStore};
    use auctioneer_privacy::purposes::{
        EnforceFlags, purpose_allowed, purpose_allowed_basic, purpose_effective_algo,
    };
    use auctioneer_privacy::sources::{self, ConsentSource};
    use auctioneer_privacy::tcf::{PubRestrictionType, parse_tcf_v2};
    use auctioneer_privacy::{
        AuctionPermissions, FetchDenial, PrivacyContext, PrivacyError, ResolveOptions,
    };
    use proptest::prelude::*;
    use serde_json::{Map, Value, json};
    use time::{Duration, macros::datetime};

    use super::tc_encode::{
        PubRestrictionSpec, RangeEntrySpec, VendorSectionSpec, encode_core, valid_core_spec,
        with_minimal_dv,
    };

    const CONSENT: &str = include_str!("data/tcf_v2/valid_full_consent.txt");
    const PUBLISHER_BASIS_ORDERS: &[&[u8]] =
        &[&[], &[0], &[1], &[1, 1], &[2], &[2, 2], &[1, 2], &[2, 1]];

    fn request(mut value: Value) -> BidRequest {
        value["id"] = json!("normalization");
        value["imp"] = json!([]);
        serde_json::from_value(value).unwrap()
    }

    fn host() -> HostConfig {
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.default_value = "0".into();
        for id in 1..=10 {
            host.gdpr.tcf2.purposes.insert(
                id,
                PurposeEnforcement {
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Enabled,
                    ..Default::default()
                },
            );
        }
        host
    }

    fn store() -> GvlStore {
        store_with(5, None)
    }

    fn store_with(policy: u32, vendor_deleted: Option<&str>) -> GvlStore {
        let tcf = parse_tcf_v2(CONSENT.trim()).unwrap();
        let purposes: Map<String, Value> = (1_i32..=10_i32)
            .map(|id| {
                (
                    id.to_string(),
                    json!({"id":id,"name":format!("Purpose {id}")}),
                )
            })
            .collect();
        let gvl = Gvl::load_from_json(
            &json!({
                "gvlSpecificationVersion":3_i32,"tcfPolicyVersion":policy,
                "vendorListVersion":tcf.vendor_list_version,"lastUpdated":"2026-07-01T00:00:00Z",
                "purposes":purposes,"specialPurposes":{},"features":{},
                "specialFeatures":{"1":{"id":1_i32,"name":"Geolocation"}},
                "vendors":{"8":{"id":8_i32,"name":"Vendor","purposes":[1_i32,2_i32,3_i32,4_i32,5_i32,6_i32,7_i32,8_i32,9_i32,10_i32],
                    "legIntPurposes":[],"flexiblePurposes":[],"specialFeatures":[1_i32],"deletedDate":vendor_deleted}}
            })
            .to_string(),
            None,
        )
        .unwrap();
        GvlStore::for_test(vec![(3, u32::from(tcf.vendor_list_version), gvl)])
    }

    fn resolve(
        req: &BidRequest,
        account: &AccountConfig,
        host: &HostConfig,
        nsp: bool,
    ) -> PrivacyContext {
        let tcf = parse_tcf_v2(CONSENT.trim()).unwrap();
        let gcl = Gcl::for_test(
            vec![CmpRecord {
                id: tcf.cmp_id,
                deleted_date: None,
            }],
            datetime!(2027-01-01 00:00 UTC),
        );
        let names = req
            .ext
            .as_ref()
            .and_then(|ext| ext.prebid.as_ref())
            .map_or(&[][..], |ext| ext.nosale.as_slice());
        PrivacyContext::resolve(
            req,
            account,
            host,
            &store(),
            &gcl,
            datetime!(2026-09-01 00:00 UTC),
            ResolveOptions {
                gdpr_exempt: nsp,
                no_sale: NoSale::validate(names, |name| ["fake_core", "fake_ax"].contains(&name))
                    .unwrap(),
            },
        )
        .unwrap()
        .context
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 128,
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        #[test]
        fn joint_source_and_policy_inputs_preserve_independent_decisions(
            sid_index in 0_usize..5,
            valid_envelope in any::<bool>(),
            legacy_grants in any::<bool>(),
            gdpr_enabled in any::<bool>(),
            nsp in any::<bool>(),
            ccpa_enabled in any::<bool>(),
            opted_out in any::<bool>(),
            lmt_enabled in any::<bool>(),
            activity_deny in any::<bool>(),
        ) {
            let sid = [&[][..], &[2_i32], &[6_i32], &[2_i32, 6_i32], &[7_i32]][sid_index];
            let req = request(json!({
                "regs": {"gdpr": 1_i32, "gpp_sid": sid,
                    "gpp": if valid_envelope { "DBABM~invalid-selected" } else { "malformed" },
                    "us_privacy": if opted_out { "1NYN" } else { "1YNN" }},
                "user": {"consent": if legacy_grants { CONSENT.trim() } else { "invalid-legacy" }},
                "device": {"lmt": 1_i32},
                "ext": {"prebid": {"aliasgvlids": {"fake_ax": 8_i32}}},
            }));
            let before = serde_json::to_vec(&req).unwrap();
            let mut config = host();
            config.gdpr.enabled = gdpr_enabled;
            config.ccpa.enforce = ccpa_enabled;
            config.lmt.enforce = lmt_enabled;
            let mut account = AccountConfig::default();
            account.privacy.allowactivities = serde_json::from_value(json!({
                "fetchBids": {"default": !activity_deny},
                "transmitUfpd": {"default": !activity_deny}
            })).unwrap();
            let mut ctx = resolve(&req, &account, &config, nsp);
            let decision = ctx.bidder_decision("fake_core", "fake_ax");
            prop_assert_eq!(decision.ccpa, ccpa_enabled && opted_out);
            prop_assert_eq!(decision.lmt, lmt_enabled);
            prop_assert_eq!(decision.transmit_ufpd, !activity_deny);
            let gdpr_bypassed =
                !gdpr_enabled || nsp || (!sid.is_empty() && !sid.contains(&2_i32));
            let granting_legacy_selected = (sid.is_empty() || !valid_envelope) && legacy_grants;
            let expected_allow = gdpr_bypassed || granting_legacy_selected;
            prop_assert_eq!(decision.gdpr == AuctionPermissions::AllowAll, expected_allow);
            let expected_fetch_denial = if activity_deny {
                Some(FetchDenial::Activity)
            } else {
                (!expected_allow).then_some(FetchDenial::Gdpr)
            };
            prop_assert_eq!(decision.fetch_denial, expected_fetch_denial);
            prop_assert!(ctx.take_warnings().len() <= 8);
            prop_assert_eq!(serde_json::to_vec(&req).unwrap(), before);
        }
    }

    #[test]
    fn gpp_tcf_uses_same_authorization_and_never_retries_authoritative_denial() {
        let account = AccountConfig::default();
        let host = host();
        let consent = CONSENT.trim();
        for (gpp, expected) in [
            (None, true),
            (Some(format!("DBABM~{consent}")), true),
            (Some("DBABM~".into()), false),
            (Some("DBABM~invalid-selected".into()), false),
            (Some("malformed-envelope".into()), true),
            (Some("DBABTA~1YYN".into()), true),
        ] {
            let req = request(
                json!({"regs":{"gpp_sid":[2_i32],"gpp":gpp},"user":{"consent":consent},
            "ext":{"prebid":{"aliasgvlids":{"fake_ax":8_i32}}}}),
            );
            let mut ctx = resolve(&req, &account, &host, false);
            assert_eq!(
                ctx.permissions("fake_core", "fake_ax") == AuctionPermissions::AllowAll,
                expected
            );
            if req.regs.as_ref().unwrap().gpp.as_deref() == Some("DBABM~invalid-selected") {
                let warnings = ctx.take_warnings();
                assert_eq!(warnings.len(), 2);
                assert!(
                    warnings
                        .iter()
                        .all(|w| w.scope.as_deref() == Some("regs.gpp"))
                );
                assert!(warnings[1].message.contains("section 2"));
                assert!(!warnings[1].message.contains("invalid-selected"));
            }
        }
    }

    #[test]
    fn gdpr_off_and_nsp_do_not_bypass_other_policies_or_parse_inactive_tcf() {
        for nsp in [false, true] {
            let mut host = host();
            host.gdpr.enabled = nsp;
            host.ccpa.enforce = true;
            host.lmt.enforce = true;
            let mut account = AccountConfig::default();
            account.privacy.allowactivities = serde_json::from_value(json!({
                "fetchBids":{"default":false},"transmitUfpd":{"default":false},
                "transmitPreciseGeo":{"default":false},"transmitTid":{"default":false}
            }))
            .unwrap();
            let req = request(
                json!({"regs":{"gpp":"DBACNY~invalid-tcf~1NYN","gpp_sid":[2_i32,6_i32],"coppa":1_i32},
            "device":{"lmt":1_i32}}),
            );
            let mut ctx = resolve(&req, &account, &host, nsp);
            let decision = ctx.bidder_decision("fake_core", "fake_ax");
            assert_eq!(decision.fetch_denial, Some(FetchDenial::Activity));
            assert_eq!(decision.gdpr, AuctionPermissions::AllowAll);
            assert!(
                !decision.transmit_ufpd && !decision.transmit_precise_geo && !decision.transmit_tid
            );
            assert!(decision.ccpa && decision.lmt && decision.coppa);
            assert!(ctx.take_warnings().is_empty());
        }
    }

    #[test]
    fn ccpa_channel_account_host_precedence_uses_inventory_not_request_channel_text() {
        for usp in ["1NYN", "private-malformed", " "] {
            for channel in ["site", "app", "dooh"] {
                for (channel_state, account_state, host_value, expected) in [
                    (TriState::Disabled, TriState::Enabled, true, false),
                    (TriState::Enabled, TriState::Disabled, false, true),
                    (TriState::Unset, TriState::Disabled, true, false),
                    (TriState::Unset, TriState::Enabled, false, true),
                    (TriState::Unset, TriState::Unset, true, true),
                    (TriState::Unset, TriState::Unset, false, false),
                ] {
                    let mut account = AccountConfig::default();
                    account.ccpa.enabled = account_state;
                    match channel {
                        "app" => account.ccpa.channel_enabled.app = channel_state,
                        "dooh" => account.ccpa.channel_enabled.dooh = channel_state,
                        _ => account.ccpa.channel_enabled.web = channel_state,
                    }
                    let mut host = HostConfig::default();
                    host.ccpa.enforce = host_value;
                    let mut value = json!({"regs":{"us_privacy":usp},"ext":{"prebid":{"channel":{"name":"video"}}}});
                    value[channel] = json!({});
                    let ctx = resolve(&request(value), &account, &host, false);
                    assert_eq!(
                        ctx.bidder_decision("fake_core", "fake_ax").ccpa,
                        expected,
                        "{channel}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_sale_exempts_only_the_exact_name_and_only_ccpa() {
        for usp in ["1NYN", "private-malformed", " "] {
            let mut host = HostConfig::default();
            host.ccpa.enforce = true;
            host.lmt.enforce = true;
            for (names, core_exempt, alias_exempt) in [
                (json!([]), false, false),
                (json!(["fake_core"]), true, false),
                (json!(["fake_ax"]), false, true),
                (json!(["*"]), true, true),
            ] {
                let req = request(
                    json!({"regs":{"us_privacy":usp,"coppa":1_i32},"device":{"lmt":1_i32},
            "ext":{"prebid":{"nosale":names}}}),
                );
                let ctx = resolve(&req, &AccountConfig::default(), &host, false);
                for (name, exempt) in [("fake_core", core_exempt), ("fake_ax", alias_exempt)] {
                    let decision = ctx.bidder_decision("fake_core", name);
                    assert_eq!(decision.ccpa, !exempt);
                    assert!(decision.lmt && decision.coppa);
                }
            }
        }
    }

    #[test]
    fn selected_usp_enforcement_distinguishes_malformed_absent_and_unselected_inputs() {
        let mut host = HostConfig::default();
        host.ccpa.enforce = true;
        for (label, regs, expected, warning_count) in [
            ("absent", json!({}), false, 0),
            ("empty", json!({"us_privacy":""}), false, 0),
            ("opt-out", json!({"us_privacy":"1NYN"}), true, 0),
            ("no-opt-out", json!({"us_privacy":"1YNN"}), false, 0),
            ("unspecified", json!({"us_privacy":"1---"}), false, 0),
            (
                "malformed-legacy",
                json!({"us_privacy":"private-malformed"}),
                true,
                1,
            ),
            ("whitespace-legacy", json!({"us_privacy":" "}), true, 1),
            (
                "malformed-active",
                json!({"gpp":"DBABTA~private-malformed","gpp_sid":[6_i32],"us_privacy":"1YNN"}),
                true,
                2,
            ),
            (
                "whitespace-active",
                json!({"gpp":"DBABTA~ ","gpp_sid":[6_i32],"us_privacy":"1YNN"}),
                true,
                2,
            ),
            (
                "unselected-malformed-legacy",
                json!({"gpp":"DBABTA~1YNN","gpp_sid":[6_i32],"us_privacy":"private-malformed"}),
                false,
                2,
            ),
            (
                "empty-active-falls-back",
                json!({"gpp":"DBABTA~","gpp_sid":[6_i32],"us_privacy":"private-malformed"}),
                true,
                1,
            ),
            (
                "inactive-malformed-payload",
                json!({"gpp":"DBABTA~private-malformed","gpp_sid":[]}),
                false,
                0,
            ),
            (
                "missing-active-section",
                json!({"gpp":"DBABM~opaque-tcf","gpp_sid":[6_i32]}),
                false,
                0,
            ),
            (
                "malformed-envelope-valid-legacy",
                json!({"gpp":"private-malformed","gpp_sid":[6_i32],"us_privacy":"1YNN"}),
                false,
                1,
            ),
            (
                "malformed-envelope-malformed-legacy",
                json!({"gpp":"private-malformed","gpp_sid":[6_i32],"us_privacy":"private-malformed"}),
                true,
                2,
            ),
        ] {
            let req = request(json!({"regs":regs}));
            let before = serde_json::to_vec(&req).unwrap();
            let mut ctx = resolve(&req, &AccountConfig::default(), &host, false);
            let decision = ctx.bidder_decision("fake_core", "fake_ax");
            assert_eq!(decision.ccpa, expected, "{label}");
            assert!(
                decision.transmit_ufpd && decision.transmit_precise_geo && decision.transmit_tid,
                "{label}"
            );
            assert!(!decision.coppa && !decision.lmt, "{label}");
            let warnings = ctx.take_warnings();
            assert_eq!(warnings.len(), warning_count, "{label}");
            assert!(
                !format!("{warnings:?}").contains("private-malformed"),
                "{label}"
            );
            assert_eq!(serde_json::to_vec(&req).unwrap(), before, "{label}");
        }
    }

    #[test]
    fn empty_gpp_usp_falls_back_but_invalid_selected_usp_does_not() {
        let mut host = HostConfig::default();
        host.ccpa.enforce = true;
        for (selected, expected, warning_count) in
            [("", true, 0), ("1YNY", false, 1), (" ", true, 2)]
        {
            let req = request(
                json!({"regs":{"gpp":format!("DBABTA~{selected}"),"gpp_sid":[6_i32],"us_privacy":"1NYN"}}),
            );
            let mut ctx = resolve(&req, &AccountConfig::default(), &host, false);
            assert_eq!(ctx.bidder_decision("fake_core", "fake_ax").ccpa, expected);
            assert_eq!(ctx.take_warnings().len(), warning_count);
        }
    }

    #[test]
    fn limits_are_terminal_before_disabled_policy_returns() {
        let req = request(json!({"regs":{"gpp":"A".repeat(0x0004_0001)}}));
        let result = PrivacyContext::resolve(
            &req,
            &AccountConfig::default(),
            &HostConfig::default(),
            &GvlStore::for_test(vec![]),
            &Gcl::for_test(vec![], datetime!(2027-01-01 00:00 UTC)),
            datetime!(2026-09-01 00:00 UTC),
            ResolveOptions::default(),
        );
        assert!(matches!(result, Err(PrivacyError::GppLimitExceeded)));
    }

    #[test]
    fn authenticity_diagnostics_follow_selected_source_but_staleness_is_scope_free() {
        let now = datetime!(2026-09-01 00:00 UTC);
        let tcf = parse_tcf_v2(CONSENT.trim()).unwrap();
        for gpp in [false, true] {
            for failure in ["cmp", "gvl", "stale"] {
                let value = if gpp {
                    json!({"regs":{"gpp_sid":[2_i32],"gpp":format!("DBABM~{}",CONSENT.trim())}})
                } else {
                    json!({"regs":{"gdpr":1_i32},"user":{"consent":CONSENT.trim()}})
                };
                let mut req = request(value);
                req.ext =
                    serde_json::from_value(json!({"prebid":{"aliasgvlids":{"fake_ax":8_i32}}}))
                        .unwrap();
                let cmps = if failure == "cmp" {
                    vec![]
                } else {
                    vec![CmpRecord {
                        id: tcf.cmp_id,
                        deleted_date: None,
                    }]
                };
                let gcl = Gcl::for_test(
                    cmps,
                    if failure == "stale" {
                        now
                    } else {
                        datetime!(2027-01-01 00:00 UTC)
                    },
                );
                let selected_store = if failure == "gvl" {
                    GvlStore::for_test(vec![])
                } else {
                    store()
                };
                let mut ctx = PrivacyContext::resolve(
                    &req,
                    &AccountConfig::default(),
                    &host(),
                    &selected_store,
                    &gcl,
                    now,
                    ResolveOptions::default(),
                )
                .unwrap()
                .context;
                let warnings = ctx.take_warnings();
                assert_eq!(warnings.len(), 1, "{failure}");
                assert_eq!(
                    warnings[0].code.as_u32(),
                    match failure {
                        "cmp" => 10001,
                        "gvl" => 10018,
                        "stale" => 10019,
                        _ => panic!("unknown authenticity failure: {failure}"),
                    }
                );
                assert!(matches!(
                    ctx.permissions("fake_core", "fake_ax"),
                    AuctionPermissions::Partial {
                        allow_bid_request: false,
                        pass_id: false,
                        ..
                    }
                ));
                assert_eq!(
                    warnings[0].scope.as_deref(),
                    if failure == "stale" {
                        None
                    } else if gpp {
                        Some("regs.gpp")
                    } else {
                        Some("user.consent")
                    }
                );
                if gpp && failure != "stale" {
                    assert!(warnings[0].message.contains("section 2"));
                }
                assert!(!warnings[0].message.contains(CONSENT.trim()));
                assert!(ctx.take_warnings().is_empty());
            }
        }
    }

    #[test]
    fn both_fetch_denials_have_activity_reason_and_do_not_grant_scrub_permissions() {
        let mut account = AccountConfig::default();
        account.privacy.allowactivities =
            serde_json::from_value(json!({"fetchBids":{"default":false}})).unwrap();
        let ctx = resolve(
            &request(json!({"regs":{"gdpr":1_i32}})),
            &account,
            &host(),
            false,
        );
        let decision = ctx.bidder_decision("fake_core", "fake_ax");
        assert_eq!(decision.fetch_denial, Some(FetchDenial::Activity));
        assert!(matches!(
            decision.gdpr,
            AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                ..
            }
        ));
    }

    #[test]
    fn gpp_and_legacy_share_cmp_vendor_deletion_boundaries_and_policy_mismatch() {
        let now = datetime!(2026-09-01 00:00 UTC);
        let parsed = parse_tcf_v2(CONSENT.trim()).unwrap();
        for gpp in [false, true] {
            for case in [
                "control",
                "cmp_future",
                "cmp_now",
                "cmp_past",
                "vendor_future",
                "vendor_now",
                "vendor_past",
                "old_policy",
            ] {
                let mut value = if gpp {
                    json!({"regs":{"gpp_sid":[2_i32],"gpp":format!("DBABM~{}",CONSENT.trim())}})
                } else {
                    json!({"regs":{"gdpr":1_i32},"user":{"consent":CONSENT.trim()}})
                };
                value["ext"] = json!({"prebid":{"aliasgvlids":{"fake_ax":8_i32}}});
                let req = request(value);
                let deleted_date = match case {
                    "cmp_now" => Some(now),
                    "cmp_past" => Some(now - Duration::SECOND),
                    "cmp_future" => Some(now + Duration::SECOND),
                    _ => None,
                };
                let vendor_deleted = match case {
                    "vendor_now" => Some("2026-09-01T00:00:00Z"),
                    "vendor_past" => Some("2026-08-31T23:59:59Z"),
                    "vendor_future" => Some("2026-09-01T00:00:01Z"),
                    _ => None,
                };
                let selected_store =
                    store_with(if case == "old_policy" { 3 } else { 5 }, vendor_deleted);
                let gcl = Gcl::for_test(
                    vec![CmpRecord {
                        id: parsed.cmp_id,
                        deleted_date,
                    }],
                    datetime!(2027-01-01 00:00 UTC),
                );
                let ctx = PrivacyContext::resolve(
                    &req,
                    &AccountConfig::default(),
                    &host(),
                    &selected_store,
                    &gcl,
                    now,
                    ResolveOptions::default(),
                )
                .unwrap()
                .context;
                let permissions = ctx.permissions("fake_core", "fake_ax");
                if ["control", "cmp_future", "vendor_future"].contains(&case) {
                    assert_eq!(permissions, AuctionPermissions::AllowAll, "{gpp} {case}");
                } else {
                    assert!(
                        matches!(
                            permissions,
                            AuctionPermissions::Partial {
                                allow_bid_request: false,
                                pass_id: false,
                                ..
                            }
                        ),
                        "{gpp} {case}: {permissions:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn conflicting_publisher_bases_deny_bid_requests_and_ids_from_legacy_and_gpp() {
        let baseline = parse_tcf_v2(CONSENT.trim()).unwrap();
        for order in [vec![1], vec![1, 2], vec![2, 1]] {
            let mut spec = valid_core_spec();
            spec.cmp_id = baseline.cmp_id;
            spec.vendor_list_version = baseline.vendor_list_version;
            spec.tcf_policy_version = 5;
            spec.purposes_consent = vec![1, 2];
            spec.vendor_consent = VendorSectionSpec::Range {
                max_vendor_id: 8,
                entries: vec![RangeEntrySpec::Single(8)],
            };
            spec.pub_restrictions = order
                .iter()
                .map(|kind| PubRestrictionSpec {
                    purpose_id: 2,
                    restriction_type: *kind,
                    entries: vec![RangeEntrySpec::Span(7, 8)],
                })
                .collect();
            let raw = with_minimal_dv(&encode_core(&spec));
            let parsed = parse_tcf_v2(&raw).expect("cross-record overlap is parseable");
            assert!(parsed.check_pub_restriction(2, PubRestrictionType::RequireConsent, 8));
            assert_eq!(
                parsed.check_pub_restriction(2, PubRestrictionType::RequireLegitInterest, 8),
                order.len() == 2
            );
            for gpp in [false, true] {
                let mut value = if gpp {
                    json!({"regs":{"gpp_sid":[2_i32],"gpp":format!("DBABM~{raw}")}})
                } else {
                    json!({"regs":{"gdpr":1_i32},"user":{"consent":raw}})
                };
                value["ext"] = json!({"prebid":{"aliasgvlids":{"fake_ax":8_i32}}});
                let mut ctx = resolve(&request(value), &AccountConfig::default(), &host(), false);
                assert!(
                    ctx.take_warnings().is_empty(),
                    "must exercise a usable TC string"
                );
                let expected = if order.len() == 1 {
                    AuctionPermissions::AllowAll
                } else {
                    AuctionPermissions::Partial {
                        allow_bid_request: false,
                        pass_id: false,
                        pass_geo: true,
                    }
                };
                assert_eq!(
                    ctx.permissions("fake_core", "fake_ax"),
                    expected,
                    "{order:?} gpp={gpp}"
                );
                assert_eq!(
                    ctx.bidder_decision("fake_core", "fake_ax").fetch_denial,
                    if order.len() == 1 {
                        None
                    } else {
                        Some(FetchDenial::Gdpr)
                    }
                );
            }
        }
    }

    fn encoded_purpose_two_consent(order: &[u8], vendor_consented: bool, li_only: bool) -> String {
        let baseline = parse_tcf_v2(CONSENT.trim()).unwrap();
        let mut spec = valid_core_spec();
        spec.cmp_id = baseline.cmp_id;
        spec.vendor_list_version = baseline.vendor_list_version;
        spec.tcf_policy_version = 5;
        // Purpose 2 is the only identity-granting purpose in this fixture.
        spec.purposes_consent = if li_only { vec![1] } else { vec![1, 2] };
        spec.purposes_li_transparency = if li_only { vec![2] } else { vec![] };
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 8,
            entries: if vendor_consented {
                vec![RangeEntrySpec::Single(8)]
            } else {
                vec![]
            },
        };
        spec.pub_restrictions = order
            .iter()
            .map(|kind| PubRestrictionSpec {
                purpose_id: 2,
                restriction_type: *kind,
                entries: vec![RangeEntrySpec::Span(7, 8)],
            })
            .collect();
        let raw = with_minimal_dv(&encode_core(&spec));
        let parsed = parse_tcf_v2(&raw).expect("cross-record overlap is parseable");
        assert_eq!(parsed.publisher_restrictions.len(), order.len());
        for (kind, restriction) in [
            (0, PubRestrictionType::NotAllowed),
            (1, PubRestrictionType::RequireConsent),
            (2, PubRestrictionType::RequireLegitInterest),
        ] {
            for purpose in [1, 2, 3] {
                for vendor in [0, 6, 7, 8, 9] {
                    assert_eq!(
                        parsed.check_pub_restriction(purpose, restriction, vendor),
                        purpose == 2 && [7, 8].contains(&vendor) && order.contains(&kind),
                        "{order:?} purpose={purpose} vendor={vendor} kind={kind}"
                    );
                }
            }
        }
        assert_eq!(parsed.vendor_consent.contains(8), vendor_consented);
        assert_eq!(
            parsed.purposes_consent.iter().copied().collect::<Vec<_>>(),
            spec.purposes_consent
        );
        assert_eq!(
            parsed
                .purposes_li_transparency
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            spec.purposes_li_transparency
        );
        assert!(!parsed.vendor_li.contains(8));
        raw
    }

    fn assert_purpose_two_permissions(
        raw: &str,
        account: &AccountConfig,
        host: &HostConfig,
        expected: &AuctionPermissions,
        bidder_config: (&str, &str, u16),
        case: &str,
    ) {
        let (core, bidder, vendor_id) = bidder_config;
        let granted = matches!(
            expected,
            AuctionPermissions::AllowAll
                | AuctionPermissions::Partial {
                    allow_bid_request: true,
                    ..
                }
        );
        for (gpp, granting_legacy) in [(false, false), (true, false), (true, true)] {
            let mut value = if gpp {
                json!({"regs":{"gpp_sid":[2_i32],"gpp":format!("DBABM~{raw}")}})
            } else {
                json!({"regs":{"gdpr":1_i32},"user":{"consent":raw}})
            };
            if granting_legacy {
                value["user"] = json!({"consent":CONSENT.trim()});
            }
            value["ext"] = json!({"prebid":{"aliasgvlids":{}}});
            value["ext"]["prebid"]["aliasgvlids"][bidder] = json!(vendor_id);
            let req = request(value);
            let label = format!("{case} gpp={gpp} granting_legacy={granting_legacy}");
            let normalized =
                sources::normalize(&req, GppLimits::default()).expect("usable envelope and scope");
            assert_eq!(
                normalized.tcf().source(),
                if gpp {
                    ConsentSource::Gpp
                } else {
                    ConsentSource::Legacy
                },
                "{label}"
            );
            assert_eq!(normalized.tcf().raw(), Some(raw), "{label}: no fallback");
            let mut ctx = resolve(&req, account, host, false);
            for purpose_id in 3..=10 {
                let flags = EnforceFlags {
                    enforce_purpose: true,
                    enforce_vendors: true,
                };
                let allowed = match purpose_effective_algo(&ctx, purpose_id, core) {
                    EnforceAlgo::Basic => {
                        purpose_allowed_basic(&ctx, purpose_id, core, vendor_id, flags, false)
                    }
                    EnforceAlgo::Full => purpose_allowed(&ctx, purpose_id, core, vendor_id, flags),
                };
                assert!(
                    !allowed,
                    "{label}: purpose {purpose_id} must not rescue PassID"
                );
            }
            assert_eq!(&ctx.permissions(core, bidder), expected, "{label}");
            let decision = ctx.bidder_decision(core, bidder);
            assert_eq!(&decision.gdpr, expected, "{label}");
            assert_eq!(
                decision.fetch_denial,
                (!granted).then_some(FetchDenial::Gdpr),
                "{label}"
            );
            let warnings = ctx.take_warnings();
            assert_eq!(warnings.len(), usize::from(granting_legacy), "{label}");
            if granting_legacy {
                assert_eq!(
                    warnings[0].code,
                    WarningCode::InvalidPrivacyConsent,
                    "{label}"
                );
                assert_eq!(warnings[0].scope.as_deref(), Some("regs.gpp"), "{label}");
                assert_eq!(
                    warnings[0].message,
                    "regs.gpp section 2 and user.consent conflict; using section 2",
                    "{label}"
                );
            }
            assert!(ctx.take_warnings().is_empty(), "{label}");
        }
    }

    fn purpose_two_permissions(allow_bid_request: bool, pass_id: bool) -> AuctionPermissions {
        if allow_bid_request && pass_id {
            AuctionPermissions::AllowAll
        } else {
            AuctionPermissions::Partial {
                allow_bid_request,
                pass_id,
                pass_geo: true,
            }
        }
    }

    #[test]
    fn per_purpose_basic_denies_conflicting_publisher_bases_and_account_replaces_host() {
        for &order in PUBLISHER_BASIS_ORDERS {
            for vendor_consented in [true, false] {
                let raw = encoded_purpose_two_consent(order, vendor_consented, false);
                for (host_algo, account_algo, effective_algo) in [
                    (EnforceAlgo::Full, None, EnforceAlgo::Full),
                    (EnforceAlgo::Basic, None, EnforceAlgo::Basic),
                    (
                        EnforceAlgo::Full,
                        Some(EnforceAlgo::Basic),
                        EnforceAlgo::Basic,
                    ),
                    (
                        EnforceAlgo::Basic,
                        Some(EnforceAlgo::Full),
                        EnforceAlgo::Full,
                    ),
                ] {
                    let mut host = host();
                    host.gdpr.tcf2.purposes.get_mut(&2).unwrap().enforce_algo = host_algo;
                    let mut account = AccountConfig::default();
                    if let Some(enforce_algo) = account_algo {
                        account.gdpr.purposes.insert(
                            2,
                            PurposeEnforcement {
                                enforce_algo,
                                ..Default::default()
                            },
                        );
                    }
                    assert!(account.gdpr.basic_enforcement_vendors.is_empty());
                    let denied = order.contains(&0) || (order.contains(&1) && order.contains(&2));
                    let granted = vendor_consented
                        && !denied
                        && (effective_algo == EnforceAlgo::Basic || !order.contains(&2));
                    assert_purpose_two_permissions(
                        &raw,
                        &account,
                        &host,
                        &purpose_two_permissions(granted, granted),
                        ("fake_core", "fake_ax", 8),
                        &format!(
                            "{order:?} vendor_consented={vendor_consented} host={host_algo:?} account={account_algo:?}"
                        ),
                    );
                }
            }
        }
    }

    #[test]
    fn account_basic_vendors_honor_publisher_denials_and_match_core_names() {
        let host = host();
        for &order in PUBLISHER_BASIS_ORDERS {
            for vendor_consented in [true, false] {
                let raw = encoded_purpose_two_consent(order, vendor_consented, false);
                for listed in [None, Some("fake_ax"), Some("fake_core")] {
                    let mut account = AccountConfig::default();
                    account.gdpr.basic_enforcement_vendors =
                        listed.into_iter().map(str::to_owned).collect();
                    assert!(account.gdpr.purposes.is_empty());
                    let denied = order.contains(&0) || (order.contains(&1) && order.contains(&2));
                    let granted = !denied
                        && (listed == Some("fake_core")
                            || (vendor_consented && !order.contains(&2)));
                    assert_purpose_two_permissions(
                        &raw,
                        &account,
                        &host,
                        &purpose_two_permissions(granted, granted),
                        ("fake_core", "fake_ax", 8),
                        &format!("{order:?} vendor_consented={vendor_consented} listed={listed:?}"),
                    );
                }
            }
        }
    }

    #[test]
    fn publisher_denials_precede_disabled_flags_and_core_policy_bypasses() {
        for order in [&[][..], &[0], &[1, 2], &[2, 1]] {
            let raw = encoded_purpose_two_consent(order, false, false);
            for algo in [EnforceAlgo::Full, EnforceAlgo::Basic] {
                for (purpose, vendors, excepted, basic, allow_bid_request, pass_id) in [
                    (
                        TriState::Disabled,
                        TriState::Disabled,
                        None,
                        None,
                        true,
                        false,
                    ),
                    (
                        TriState::Enabled,
                        TriState::Disabled,
                        None,
                        None,
                        true,
                        false,
                    ),
                    (
                        TriState::Disabled,
                        TriState::Enabled,
                        None,
                        Some("fake_core"),
                        true,
                        true,
                    ),
                    (
                        TriState::Disabled,
                        TriState::Enabled,
                        None,
                        Some("fake_ax"),
                        false,
                        false,
                    ),
                    (
                        TriState::Enabled,
                        TriState::Enabled,
                        Some("fake_core"),
                        None,
                        true,
                        true,
                    ),
                    (
                        TriState::Enabled,
                        TriState::Enabled,
                        Some("fake_ax"),
                        None,
                        false,
                        false,
                    ),
                ] {
                    let host = host();
                    let mut account = AccountConfig::default();
                    account.gdpr.purposes.insert(
                        2,
                        PurposeEnforcement {
                            enforce_algo: algo,
                            enforce_purpose: purpose,
                            enforce_vendors: vendors,
                            vendor_exceptions: excepted.into_iter().map(str::to_owned).collect(),
                        },
                    );
                    account.gdpr.basic_enforcement_vendors =
                        basic.into_iter().map(str::to_owned).collect();
                    let unrestricted = order.is_empty();
                    assert_purpose_two_permissions(
                        &raw,
                        &account,
                        &host,
                        &purpose_two_permissions(
                            unrestricted && allow_bid_request,
                            unrestricted && pass_id,
                        ),
                        ("fake_core", "fake_ax", 8),
                        &format!(
                            "{order:?} algo={algo:?} purpose={purpose:?} vendors={vendors:?} exception={excepted:?} basic={basic:?}"
                        ),
                    );
                }
            }
        }
    }

    #[test]
    fn basic_purpose_two_li_only_grants_require_a_nonzero_resolved_vendor() {
        assert!(auctioneer_config_data::bidder("fake_no_gvl").is_none());
        for &order in PUBLISHER_BASIS_ORDERS {
            let raw = encoded_purpose_two_consent(order, false, true);
            let denied = order.contains(&0) || (order.contains(&1) && order.contains(&2));
            for (core, bidder, vendor_id) in [
                ("fake_core", "fake_ax", 0),
                ("fake_core", "fake_ax", 8),
                ("fake_no_gvl", "fake_no_gvl", 0),
                ("fake_no_gvl", "fake_ax", 8),
            ] {
                for (host_algo, account_algo, basic_core, effective_basic) in [
                    (EnforceAlgo::Full, None, None, false),
                    (EnforceAlgo::Basic, None, None, true),
                    (EnforceAlgo::Full, Some(EnforceAlgo::Basic), None, true),
                    (EnforceAlgo::Basic, Some(EnforceAlgo::Full), None, false),
                    (EnforceAlgo::Full, None, Some(true), true),
                    (EnforceAlgo::Full, None, Some(false), false),
                ] {
                    let mut host = host();
                    host.gdpr.tcf2.purposes.get_mut(&2).unwrap().enforce_algo = host_algo;
                    let mut account = AccountConfig::default();
                    if let Some(enforce_algo) = account_algo {
                        account.gdpr.purposes.insert(
                            2,
                            PurposeEnforcement {
                                enforce_algo,
                                ..Default::default()
                            },
                        );
                    }
                    if let Some(list_core) = basic_core {
                        account.gdpr.basic_enforcement_vendors =
                            vec![if list_core { core } else { "fake_ax" }.to_owned()];
                    }
                    let granted = effective_basic && vendor_id != 0 && !denied;
                    assert_purpose_two_permissions(
                        &raw,
                        &account,
                        &host,
                        &purpose_two_permissions(granted, granted),
                        (core, bidder, vendor_id),
                        &format!(
                            "{order:?} {core}/{bidder} vendor={vendor_id} host={host_algo:?} account={account_algo:?} basic_core={basic_core:?}"
                        ),
                    );
                }
            }
        }
    }

    #[test]
    fn gpp_and_legacy_enforce_the_same_publisher_restrictions() {
        let baseline = parse_tcf_v2(CONSENT.trim()).unwrap();
        let mut spec = valid_core_spec();
        spec.cmp_id = baseline.cmp_id;
        spec.vendor_list_version = baseline.vendor_list_version;
        spec.tcf_policy_version = 5;
        spec.purposes_consent = vec![1, 2];
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 8,
            entries: vec![RangeEntrySpec::Span(8, 8)],
        };
        let control = with_minimal_dv(&encode_core(&spec));
        spec.pub_restrictions.push(PubRestrictionSpec {
            purpose_id: 2,
            restriction_type: 0,
            entries: vec![RangeEntrySpec::Single(8)],
        });
        let restricted = with_minimal_dv(&encode_core(&spec));
        assert!(parse_tcf_v2(&restricted).unwrap().check_pub_restriction(
            2,
            PubRestrictionType::NotAllowed,
            8
        ));
        for (raw, expected) in [
            (&control, AuctionPermissions::AllowAll),
            (
                &restricted,
                AuctionPermissions::Partial {
                    allow_bid_request: false,
                    pass_id: false,
                    pass_geo: true,
                },
            ),
        ] {
            for gpp in [false, true] {
                let mut value = if gpp {
                    json!({"regs":{"gpp_sid":[2_i32],"gpp":format!("DBABM~{raw}")}})
                } else {
                    json!({"regs":{"gdpr":1_i32},"user":{"consent":raw}})
                };
                value["ext"] = json!({"prebid":{"aliasgvlids":{"fake_ax":8_i32}}});
                let ctx = resolve(&request(value), &AccountConfig::default(), &host(), false);
                assert_eq!(
                    ctx.permissions("fake_core", "fake_ax"),
                    expected,
                    "gpp={gpp}"
                );
            }
        }
    }
}
