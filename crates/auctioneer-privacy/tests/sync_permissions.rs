#[cfg(test)]
use auctioneer_privacy::test_support::tc_encode;

#[cfg(test)]
mod tests {
    use std::ptr::from_ref;

    use auctioneer_config::types::{
        AccountConfig, BidderInfo, EnforceAlgo, HostConfig, PurposeEnforcement, SkipWhen, TriState,
    };
    use auctioneer_privacy::{
        GdprSignalIn, SyncDecision, SyncEndpoint, SyncPrivacyContext, SyncPrivacyError,
        SyncPrivacyInput, SyncerLookup,
        gcl::{CmpRecord, Gcl},
        gvl::{Gvl, GvlStore},
        resolve_sync_privacy,
        tcf::{MAX_TOTAL_INPUT_LEN, TcfParseError, parse_tcf_v2},
    };
    use serde_json::json;
    use time::{OffsetDateTime, macros::datetime};

    use super::tc_encode::{
        PubRestrictionSpec, RangeEntrySpec, VendorSectionSpec, encode_core, valid_core_spec,
        with_minimal_dv,
    };

    const NOW: OffsetDateTime = datetime!(2026-09-01 00:00 UTC);

    fn configured_host() -> HostConfig {
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.host_vendor_id = Some(8);
        host.gdpr.tcf2.purposes.insert(
            1,
            PurposeEnforcement {
                enforce_purpose: TriState::Enabled,
                enforce_vendors: TriState::Enabled,
                ..Default::default()
            },
        );
        host
    }

    fn consent(purpose_one: bool, vendor_consent: bool) -> String {
        let mut spec = valid_core_spec();
        spec.purposes_consent = if purpose_one { vec![1] } else { vec![] };
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: 8,
            set: if vendor_consent { vec![8] } else { vec![] },
        };
        with_minimal_dv(&encode_core(&spec))
    }

    fn store(vendor_deleted: Option<&str>) -> GvlStore {
        store_with(5, vendor_deleted)
    }

    fn store_with(policy_version: u32, vendor_deleted: Option<&str>) -> GvlStore {
        store_with_purposes(policy_version, vendor_deleted, &[1])
    }

    fn store_with_purposes(
        policy_version: u32,
        vendor_deleted: Option<&str>,
        purposes: &[u8],
    ) -> GvlStore {
        let gvl = Gvl::load_from_json(
            &json!({
                "gvlSpecificationVersion":3_i32,"tcfPolicyVersion":policy_version,
                "vendorListVersion":348_i32,"lastUpdated":"2026-07-01T00:00:00Z",
                "purposes":{"1":{"id":1_i32,"name":"Storage"},
                    "2":{"id":2_i32,"name":"Ads"}},
                "specialPurposes":{},"features":{},"specialFeatures":{},
                "vendors":{"8":{"id":8_i32,"name":"Vendor","purposes":purposes,
                    "legIntPurposes":[],"flexiblePurposes":[],"specialFeatures":[],
                    "deletedDate":vendor_deleted}}
            })
            .to_string(),
            None,
        )
        .expect("valid test GVL");
        GvlStore::for_test(vec![(3, 348, gvl)])
    }

    fn valid_gcl() -> Gcl {
        Gcl::for_test(
            vec![CmpRecord {
                id: 300,
                deleted_date: None,
            }],
            datetime!(2027-01-01 00:00 UTC),
        )
    }

    fn cookie_context<'a>(
        account: &'a AccountConfig,
        host: &'a HostConfig,
        gvl: &'a GvlStore,
        gcl: &'a Gcl,
        consent: &'a str,
    ) -> SyncPrivacyContext<'a> {
        resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(1),
            consent: Some(consent),
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account,
            host,
            endpoint: SyncEndpoint::CookieSync,
            gvl,
            gcl,
            now: NOW,
        })
        .expect("valid extraction")
    }

    #[test]
    fn active_malformed_tcf_is_a_deferred_host_permission_state() {
        let account = AccountConfig::default();
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.host_vendor_id = Some(8);
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut context = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(1),
            consent: Some("private-malformed-tcf"),
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now: OffsetDateTime::UNIX_EPOCH,
        })
        .expect("nonempty TCF is deferred");

        assert_eq!(
            context.cookie_sync_host_permission(),
            SyncDecision::MalformedTcf
        );
    }

    #[test]
    fn nonzero_host_id_does_not_hide_malformed_tcf() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr.host_vendor_id = Some(u32::MAX);
        let gvl = GvlStore::for_test(vec![]);
        let gcl = valid_gcl();
        let mut context = cookie_context(&account, &host, &gvl, &gcl, "malformed-tcf");
        assert_eq!(
            context.cookie_sync_host_permission(),
            SyncDecision::MalformedTcf
        );
    }

    #[test]
    fn authenticated_host_obeys_purpose_one_enforcement() {
        let account = AccountConfig::default();
        let host = configured_host();
        let gvl = store(None);
        let gcl = valid_gcl();
        for (purpose_one, expected) in [
            (true, SyncDecision::Allowed),
            (false, SyncDecision::BlockedByPrivacy),
        ] {
            let tc = consent(purpose_one, true);
            let mut context = cookie_context(&account, &host, &gvl, &gcl, &tc);
            assert_eq!(context.cookie_sync_host_permission(), expected);
        }
    }

    #[test]
    fn account_purpose_one_algorithm_overrides_distinguishing_host_policy() {
        // Full requires a Purpose 1 declaration; Basic needs the consent bits only.
        let gvl = store_with_purposes(5, None, &[2]);
        let gcl = valid_gcl();
        let tc = consent(true, true);
        let bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..Default::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for (host_algo, account_algo, expected) in [
            (EnforceAlgo::Full, None, SyncDecision::BlockedByPrivacy),
            (EnforceAlgo::Basic, None, SyncDecision::Allowed),
            (
                EnforceAlgo::Full,
                Some(EnforceAlgo::Basic),
                SyncDecision::Allowed,
            ),
            (
                EnforceAlgo::Basic,
                Some(EnforceAlgo::Full),
                SyncDecision::BlockedByPrivacy,
            ),
        ] {
            let mut account = AccountConfig::default();
            if let Some(enforce_algo) = account_algo {
                account.gdpr.purposes.insert(
                    1,
                    PurposeEnforcement {
                        enforce_algo,
                        ..Default::default()
                    },
                );
            }
            let mut host = configured_host();
            host.gdpr.tcf2.purposes.get_mut(&1).unwrap().enforce_algo = host_algo;
            for host_vendor in [8, 0] {
                host.gdpr.host_vendor_id = Some(host_vendor);
                for endpoint in [SyncEndpoint::CookieSync, SyncEndpoint::SetUid] {
                    let mut context = resolve_sync_privacy(SyncPrivacyInput {
                        gdpr: match endpoint {
                            SyncEndpoint::CookieSync => GdprSignalIn::CookieNumber(1),
                            SyncEndpoint::SetUid => GdprSignalIn::SetUidQuery(Some("1")),
                        },
                        consent: Some(&tc),
                        us_privacy: None,
                        gpp: None,
                        gpp_sid: None,
                        account: &account,
                        host: &host,
                        endpoint,
                        gvl: &gvl,
                        gcl: &gcl,
                        now: NOW,
                    })
                    .unwrap();
                    let host_expected = if host_vendor == 0 {
                        SyncDecision::Allowed
                    } else {
                        expected
                    };
                    assert_eq!(
                        context.cookie_sync_host_permission(),
                        host_expected,
                        "host: {host_algo:?}/{account_algo:?}/{host_vendor}/{endpoint:?}"
                    );
                    assert_eq!(
                        context.setuid_permission(&lookup),
                        host_expected,
                        "setuid: {host_algo:?}/{account_algo:?}/{host_vendor}/{endpoint:?}"
                    );
                    assert_eq!(
                        context.cookie_sync_candidate_permission(&lookup),
                        expected,
                        "candidate: {host_algo:?}/{account_algo:?}/{host_vendor}/{endpoint:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn excess_tcf_segments_below_byte_cap_remain_resource_decisions() {
        let valid = consent(true, true);
        parse_tcf_v2(&valid).expect("valid consent before appending segments");
        let tc = format!("{valid}.IABA.IABA");
        assert!(tc.len() < 32 * 1024);
        assert!(tc.len() < MAX_TOTAL_INPUT_LEN);
        assert!(matches!(
            parse_tcf_v2(&tc),
            Err(TcfParseError::TooManyTrailingSegments { actual: 3 })
        ));
        let account = AccountConfig::default();
        let mut host = configured_host();
        let gvl = store(None);
        let gcl = valid_gcl();
        let bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..Default::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for endpoint in [SyncEndpoint::CookieSync, SyncEndpoint::SetUid] {
            for host_vendor in [8, 0] {
                host.gdpr.host_vendor_id = Some(host_vendor);
                let mut context = resolve_sync_privacy(SyncPrivacyInput {
                    gdpr: match endpoint {
                        SyncEndpoint::CookieSync => GdprSignalIn::CookieNumber(1),
                        SyncEndpoint::SetUid => GdprSignalIn::SetUidQuery(Some("1")),
                    },
                    consent: Some(&tc),
                    us_privacy: None,
                    gpp: None,
                    gpp_sid: None,
                    account: &account,
                    host: &host,
                    endpoint,
                    gvl: &gvl,
                    gcl: &gcl,
                    now: NOW,
                })
                .expect("segment exhaustion is deferred to permission evaluation");
                let host_expected = if host_vendor == 0 {
                    SyncDecision::Allowed
                } else {
                    SyncDecision::ResourceLimit
                };
                assert_eq!(context.cookie_sync_host_permission(), host_expected);
                assert_eq!(context.setuid_permission(&lookup), host_expected);
                assert_eq!(
                    context.cookie_sync_candidate_permission(&lookup),
                    SyncDecision::ResourceLimit,
                    "{endpoint:?}/{host_vendor}"
                );
            }
        }
    }

    #[test]
    fn host_vendor_zero_bypasses_only_host_permission() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr.host_vendor_id = Some(0);
        let gvl = GvlStore::for_test(vec![]);
        let gcl = valid_gcl();
        let mut context = cookie_context(&account, &host, &gvl, &gcl, "malformed-tcf");
        let bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..BidderInfo::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        assert_eq!(context.cookie_sync_host_permission(), SyncDecision::Allowed);
        assert_eq!(
            context.cookie_sync_candidate_permission(&lookup),
            SyncDecision::BlockedByPrivacy
        );
        let mut setuid = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::SetUidQuery(Some("1")),
            consent: Some("malformed-tcf"),
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::SetUid,
            gvl: &gvl,
            gcl: &gcl,
            now: NOW,
        })
        .unwrap();
        assert_eq!(setuid.setuid_permission(&lookup), SyncDecision::Allowed);
    }

    #[test]
    fn setuid_activity_denial_precedes_deferred_tcf_error() {
        let mut denied_account = AccountConfig::default();
        denied_account.privacy.allowactivities =
            serde_json::from_value(json!({"syncUser":{"default":false}})).unwrap();
        let allowed_account = AccountConfig::default();
        let host = configured_host();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = valid_gcl();
        let bidder = BidderInfo::default();
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for (account, expected) in [
            (&denied_account, SyncDecision::BlockedByActivity),
            (&allowed_account, SyncDecision::MalformedTcf),
        ] {
            let mut context = resolve_sync_privacy(SyncPrivacyInput {
                gdpr: GdprSignalIn::SetUidQuery(Some("1")),
                consent: Some("malformed-tcf"),
                us_privacy: None,
                gpp: None,
                gpp_sid: None,
                account,
                host: &host,
                endpoint: SyncEndpoint::SetUid,
                gvl: &gvl,
                gcl: &gcl,
                now: NOW,
            })
            .expect("malformed nonempty TCF is deferred");
            assert_eq!(context.setuid_permission(&lookup), expected);
            assert_eq!(
                context.cookie_sync_candidate_permission(&lookup),
                if expected == SyncDecision::BlockedByActivity {
                    SyncDecision::BlockedByActivity
                } else {
                    SyncDecision::BlockedByPrivacy
                }
            );
        }
    }

    #[test]
    fn selected_tcf_is_cached_across_host_permission_checks() {
        let account = AccountConfig::default();
        let host = configured_host();
        let gvl = store(None);
        let gcl = valid_gcl();
        let tc = consent(true, true);
        let mut context = cookie_context(&account, &host, &gvl, &gcl, &tc);
        let first = from_ref(context.parse_selected_tcf().unwrap().unwrap());
        assert_eq!(context.cookie_sync_host_permission(), SyncDecision::Allowed);
        assert_eq!(context.cookie_sync_host_permission(), SyncDecision::Allowed);
        let after = from_ref(context.parse_selected_tcf().unwrap().unwrap());
        assert_eq!(first, after);
    }

    #[test]
    fn sync_user_activity_uses_normalized_requested_identity() {
        let mut account = AccountConfig::default();
        account.privacy.allowactivities = serde_json::from_value(json!({
            "syncUser":{"default":true,"rules":[{"allow":false,"condition":{
                "componentName":["fake_normalized"],"componentType":["bidder"]
            }}]}
        }))
        .unwrap();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = valid_gcl();
        let bidder = BidderInfo::default();
        let lookup = SyncerLookup {
            requested_name: "fake_display_alias",
            normalized_name: "fake_normalized",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        let mut context = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(0),
            consent: None,
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now: NOW,
        })
        .unwrap();
        assert_eq!(
            context.cookie_sync_candidate_permission(&lookup),
            SyncDecision::BlockedByActivity
        );
    }

    #[test]
    fn unauthenticated_consent_and_deleted_vendor_deny_before_unenforced_grants() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr.tcf2.purposes.insert(
            1,
            PurposeEnforcement {
                enforce_purpose: TriState::Disabled,
                enforce_vendors: TriState::Disabled,
                vendor_exceptions: vec!["fake_base".into()],
                ..Default::default()
            },
        );
        let tc = consent(true, true);
        for case in [
            "valid",
            "stale-gcl",
            "missing-cmp",
            "deleted-cmp",
            "missing-gvl",
            "old-policy-gvl",
            "deleted-vendor",
        ] {
            let gvl = match case {
                "missing-gvl" => GvlStore::for_test(vec![]),
                "old-policy-gvl" => store_with(3, None),
                "deleted-vendor" => store(Some("2026-09-01T00:00:00Z")),
                _ => store(None),
            };
            let gcl = match case {
                "stale-gcl" => Gcl::for_test(
                    vec![CmpRecord {
                        id: 300,
                        deleted_date: None,
                    }],
                    NOW,
                ),
                "missing-cmp" => Gcl::for_test(vec![], datetime!(2027-01-01 00:00 UTC)),
                "deleted-cmp" => Gcl::for_test(
                    vec![CmpRecord {
                        id: 300,
                        deleted_date: Some(NOW),
                    }],
                    datetime!(2027-01-01 00:00 UTC),
                ),
                _ => valid_gcl(),
            };
            let mut context = cookie_context(&account, &host, &gvl, &gcl, &tc);
            let expected = if case == "valid" {
                SyncDecision::Allowed
            } else {
                SyncDecision::BlockedByPrivacy
            };
            assert_eq!(context.cookie_sync_host_permission(), expected, "{case}");
        }
    }

    #[test]
    fn publisher_not_allowed_beats_treatment_and_unenforced_purpose() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr.tcf2.purposes.insert(
            1,
            PurposeEnforcement {
                enforce_purpose: TriState::Disabled,
                enforce_vendors: TriState::Disabled,
                ..Default::default()
            },
        );
        let gvl = store(None);
        let gcl = valid_gcl();
        let mut spec = valid_core_spec();
        spec.purpose_one_treatment = true;
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: 8,
            set: vec![8],
        };
        spec.pub_restrictions = vec![PubRestrictionSpec {
            purpose_id: 1,
            restriction_type: 0,
            entries: vec![RangeEntrySpec::Single(8)],
        }];
        let restricted = with_minimal_dv(&encode_core(&spec));
        let mut context = cookie_context(&account, &host, &gvl, &gcl, &restricted);
        assert_eq!(
            context.cookie_sync_host_permission(),
            SyncDecision::BlockedByPrivacy
        );
    }

    #[test]
    fn purpose_one_treatment_resolves_each_override_field() {
        let gvl = store(None);
        let gcl = valid_gcl();
        let mut spec = valid_core_spec();
        spec.purpose_one_treatment = true;
        spec.purposes_consent.clear();
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: 8,
            set: vec![],
        };
        let tc = with_minimal_dv(&encode_core(&spec));
        for (host_enabled, host_access, account_enabled, account_access, expected) in [
            (true, true, None, None, SyncDecision::Allowed),
            (false, true, None, None, SyncDecision::BlockedByPrivacy),
            (true, false, None, None, SyncDecision::BlockedByPrivacy),
            (
                true,
                true,
                Some(false),
                None,
                SyncDecision::BlockedByPrivacy,
            ),
            (
                true,
                true,
                None,
                Some(false),
                SyncDecision::BlockedByPrivacy,
            ),
            (false, false, Some(true), Some(true), SyncDecision::Allowed),
        ] {
            let mut account = AccountConfig::default();
            account.gdpr.purpose_one_treatment.enabled = account_enabled;
            account.gdpr.purpose_one_treatment.access_allowed = account_access;
            let mut host = configured_host();
            host.gdpr.tcf2.purpose_one_treatment.enabled = host_enabled;
            host.gdpr.tcf2.purpose_one_treatment.access_allowed = host_access;
            let mut context = cookie_context(&account, &host, &gvl, &gcl, &tc);
            assert_eq!(
                context.cookie_sync_host_permission(),
                expected,
                "host={host_enabled}/{host_access} account={account_enabled:?}/{account_access:?}"
            );
        }
    }

    #[test]
    fn candidate_base_exception_does_not_become_host_exception() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr
            .tcf2
            .purposes
            .get_mut(&1)
            .unwrap()
            .vendor_exceptions = vec!["fake_base".into()];
        let gvl = store(None);
        let gcl = valid_gcl();
        let tc = consent(false, false);
        let bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..BidderInfo::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };

        let mut host_context = cookie_context(&account, &host, &gvl, &gcl, &tc);
        assert_eq!(
            host_context.cookie_sync_host_permission(),
            SyncDecision::BlockedByPrivacy
        );
        host.gdpr.host_vendor_id = Some(0);
        let mut candidate_context = cookie_context(&account, &host, &gvl, &gcl, &tc);
        assert_eq!(
            candidate_context.cookie_sync_candidate_permission(&lookup),
            SyncDecision::Allowed
        );
    }

    #[test]
    fn host_permission_never_uses_configured_bidder_exceptions() {
        let gvl = store(None);
        let gcl = valid_gcl();
        // Purpose consent makes a mistaken basic-list match grant without vendor consent.
        let tc = consent(true, false);
        let lists: [&[&str]; 4] = [&[], &[""], &["", "\0"], &["\0"]];
        let bidder = BidderInfo::default();
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for basic in lists {
            for exceptions in lists {
                let mut account = AccountConfig::default();
                account.gdpr.basic_enforcement_vendors =
                    basic.iter().map(|name| (*name).to_owned()).collect();
                let mut host = configured_host();
                host.gdpr
                    .tcf2
                    .purposes
                    .get_mut(&1)
                    .unwrap()
                    .vendor_exceptions = exceptions.iter().map(|name| (*name).to_owned()).collect();
                let mut context = cookie_context(&account, &host, &gvl, &gcl, &tc);
                assert_eq!(
                    context.cookie_sync_host_permission(),
                    SyncDecision::BlockedByPrivacy,
                    "basic={basic:?} exceptions={exceptions:?}"
                );
                assert_eq!(
                    context.setuid_permission(&lookup),
                    SyncDecision::BlockedByPrivacy
                );
            }
        }
    }

    #[test]
    fn account_opt_in_permission_ladder_includes_disabled_host_tcf() {
        let gvl = store(None);
        let gcl = valid_gcl();
        let allowed = consent(true, true);
        let denied = consent(false, true);
        let bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..Default::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        let states = [TriState::Unset, TriState::Disabled, TriState::Enabled];
        for host_enabled in [false, true] {
            for tcf_enabled in [false, true] {
                for global in states {
                    for web in states {
                        let mut account = AccountConfig::default();
                        account.gdpr.enabled = global;
                        account.gdpr.channel_enabled.web = web;
                        let enabled =
                            host_enabled
                                && match (web, global) {
                                    (TriState::Enabled, _)
                                    | (TriState::Unset, TriState::Enabled) => true,
                                    (TriState::Disabled, _)
                                    | (TriState::Unset, TriState::Disabled) => false,
                                    (TriState::Unset, TriState::Unset) => tcf_enabled,
                                };
                        for (tc, active_host) in [
                            ("malformed-tcf", SyncDecision::MalformedTcf),
                            (denied.as_str(), SyncDecision::BlockedByPrivacy),
                            (allowed.as_str(), SyncDecision::Allowed),
                        ] {
                            for endpoint in [SyncEndpoint::CookieSync, SyncEndpoint::SetUid] {
                                // Host zero also proves the candidate gate is independently active.
                                for host_vendor in [0, 8] {
                                    let mut host = configured_host();
                                    host.gdpr.enabled = host_enabled;
                                    host.gdpr.tcf2.enabled = tcf_enabled;
                                    host.gdpr.host_vendor_id = Some(host_vendor);
                                    let mut context = resolve_sync_privacy(SyncPrivacyInput {
                                        gdpr: match endpoint {
                                            SyncEndpoint::CookieSync => {
                                                GdprSignalIn::CookieNumber(1)
                                            }
                                            SyncEndpoint::SetUid => {
                                                GdprSignalIn::SetUidQuery(Some("1"))
                                            }
                                        },
                                        consent: Some(tc),
                                        us_privacy: None,
                                        gpp: None,
                                        gpp_sid: None,
                                        account: &account,
                                        host: &host,
                                        endpoint,
                                        gvl: &gvl,
                                        gcl: &gcl,
                                        now: NOW,
                                    })
                                    .expect("nonempty consent extracts even on inactive paths");
                                    let host_expected = if enabled && host_vendor != 0 {
                                        active_host
                                    } else {
                                        SyncDecision::Allowed
                                    };
                                    assert_eq!(
                                        context.cookie_sync_host_permission(),
                                        host_expected
                                    );
                                    assert_eq!(context.setuid_permission(&lookup), host_expected);
                                    let candidate_expected =
                                        if enabled && active_host != SyncDecision::Allowed {
                                            SyncDecision::BlockedByPrivacy
                                        } else {
                                            SyncDecision::Allowed
                                        };
                                    assert_eq!(
                                        context.cookie_sync_candidate_permission(&lookup),
                                        candidate_expected,
                                        "host={host_enabled} tcf={tcf_enabled} global={global:?} web={web:?}"
                                    );
                                    assert_eq!(
                                        context.gdpr_permissions_enabled, enabled,
                                        "host={host_enabled} tcf={tcf_enabled} global={global:?} web={web:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn usp_propagation_has_no_ccpa_sync_denial_stage() {
        let base_host = configured_host();
        let gvl = store(None);
        let gcl = valid_gcl();
        let tc = consent(true, true);
        let bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..Default::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for global in [TriState::Unset, TriState::Disabled, TriState::Enabled] {
            for web in [TriState::Unset, TriState::Disabled, TriState::Enabled] {
                for host_enforce in [false, true] {
                    let mut host = base_host.clone();
                    host.ccpa.enforce = host_enforce;
                    let mut account = AccountConfig::default();
                    account.ccpa.enabled = global;
                    account.ccpa.channel_enabled.web = web;
                    for (usp, gpp, sid, selected) in [
                        ("1YNN", None, None, Some("1YNN")),
                        ("1YYN", None, None, Some("1YYN")),
                        ("invalid", None, None, None),
                        ("1YNN", Some("DBABTA~invalid-usp"), Some("6"), None),
                        ("1YNN", Some("DBABTA~1YYN"), Some("6"), Some("1YYN")),
                        ("1YNN", Some("DBABTA~invalid-usp"), Some("2"), Some("1YNN")),
                    ] {
                        let mut context = resolve_sync_privacy(SyncPrivacyInput {
                            gdpr: GdprSignalIn::CookieNumber(1),
                            consent: Some(&tc),
                            us_privacy: Some(usp),
                            gpp,
                            gpp_sid: sid,
                            account: &account,
                            host: &host,
                            endpoint: SyncEndpoint::CookieSync,
                            gvl: &gvl,
                            gcl: &gcl,
                            now: NOW,
                        })
                        .unwrap();
                        assert_eq!(context.selected_usp(), selected, "{usp}/{gpp:?}/{sid:?}");
                        assert_eq!(context.cookie_sync_host_permission(), SyncDecision::Allowed);
                        assert_eq!(
                            context.cookie_sync_candidate_permission(&lookup),
                            SyncDecision::Allowed
                        );
                        assert_eq!(context.setuid_permission(&lookup), SyncDecision::Allowed);
                    }
                }
            }
        }
    }

    #[test]
    fn basic_candidate_grant_does_not_bypass_host_or_use_purpose_one_li() {
        let mut account = AccountConfig::default();
        account.gdpr.basic_enforcement_vendors = vec!["fake_base".into()];
        let mut host = configured_host();
        let gvl = store(None);
        let gcl = valid_gcl();
        let bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..BidderInfo::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        let tc = consent(true, false);
        let mut host_context = cookie_context(&account, &host, &gvl, &gcl, &tc);
        assert_eq!(
            host_context.cookie_sync_host_permission(),
            SyncDecision::BlockedByPrivacy
        );
        host.gdpr.host_vendor_id = Some(0);
        let mut candidate_context = cookie_context(&account, &host, &gvl, &gcl, &tc);
        assert_eq!(
            candidate_context.cookie_sync_candidate_permission(&lookup),
            SyncDecision::Allowed
        );

        let mut spec = valid_core_spec();
        spec.purposes_consent.clear();
        spec.purposes_li_transparency = vec![1];
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 8,
            entries: vec![],
        };
        let li_only = with_minimal_dv(&encode_core(&spec));
        let mut li_context = cookie_context(&account, &host, &gvl, &gcl, &li_only);
        assert_eq!(
            li_context.cookie_sync_candidate_permission(&lookup),
            SyncDecision::BlockedByPrivacy
        );
    }

    #[test]
    fn candidate_vendor_id_must_be_present_nonzero_known_and_live() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr.host_vendor_id = Some(0);
        let gvl = store(None);
        let deleted_gvl = store(Some("2026-09-01T00:00:00Z"));
        let gcl = valid_gcl();
        let tc = consent(true, true);
        for (id, deleted, expected) in [
            (None, false, SyncDecision::BlockedByPrivacy),
            (Some(0), false, SyncDecision::BlockedByPrivacy),
            (Some(9), false, SyncDecision::BlockedByPrivacy),
            (Some(8), false, SyncDecision::Allowed),
            (Some(8), true, SyncDecision::BlockedByPrivacy),
        ] {
            let bidder = BidderInfo {
                gvl_vendor_id: id,
                ..BidderInfo::default()
            };
            let lookup = SyncerLookup {
                requested_name: "fake_alias",
                normalized_name: "fake_alias",
                base_name: "fake_base",
                bidder: &bidder,
                syncer: &bidder.user_sync,
                syncer_key: "family",
            };
            let selected_gvl = if deleted { &deleted_gvl } else { &gvl };
            let mut context = cookie_context(&account, &host, selected_gvl, &gcl, &tc);
            assert_eq!(
                context.cookie_sync_candidate_permission(&lookup),
                expected,
                "vendor={id:?} deleted={deleted}"
            );
        }
    }

    #[test]
    fn setuid_checks_host_not_bidder_vendor_or_skip_when() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        let gvl = store(None);
        let gcl = valid_gcl();
        let full = consent(true, true);
        let denied = consent(false, true);
        let mut bidder = BidderInfo {
            gvl_vendor_id: None,
            ..BidderInfo::default()
        };
        bidder.user_sync.skip_when = SkipWhen {
            gdpr: true,
            gpp_sid: vec!["-1".into()],
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for (tc, expected) in [
            (full.as_str(), SyncDecision::Allowed),
            (denied.as_str(), SyncDecision::BlockedByPrivacy),
        ] {
            let mut context = resolve_sync_privacy(SyncPrivacyInput {
                gdpr: GdprSignalIn::SetUidQuery(Some("1")),
                consent: Some(tc),
                us_privacy: None,
                gpp: None,
                gpp_sid: None,
                account: &account,
                host: &host,
                endpoint: SyncEndpoint::SetUid,
                gvl: &gvl,
                gcl: &gcl,
                now: NOW,
            })
            .unwrap();
            assert_eq!(context.setuid_permission(&lookup), expected);
        }
        host.gdpr.host_vendor_id = Some(0);
        let mut context = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::SetUidQuery(Some("bad-legacy")),
            consent: Some("malformed-tcf"),
            us_privacy: None,
            gpp: None,
            gpp_sid: Some("-1"),
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::SetUid,
            gvl: &gvl,
            gcl: &gcl,
            now: NOW,
        })
        .unwrap();
        assert_eq!(context.setuid_permission(&lookup), SyncDecision::Allowed);
        bidder.user_sync.enabled = false;
        let disabled_lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        assert_eq!(
            context.setuid_permission(&disabled_lookup),
            SyncDecision::BlockedByDisabledUsersync
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "matrix keeps error precedence cases together"
    )]
    fn matrix_extraction_errors_precede_activity_and_ambiguous_setuid_defers_empty_consent() {
        let mut account = AccountConfig::default();
        account.privacy.allowactivities =
            serde_json::from_value(json!({"syncUser":{"default":false}})).unwrap();
        let mut host = configured_host();
        host.gdpr.default_value = "1".into();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = valid_gcl();
        for (name, endpoint, gdpr, consent, sid, gpp, expected) in [
            (
                "cookie parsed yes empty",
                SyncEndpoint::CookieSync,
                GdprSignalIn::CookieNumber(1),
                None,
                None,
                None,
                SyncPrivacyError::MissingRequiredConsent,
            ),
            (
                "setuid parsed yes empty",
                SyncEndpoint::SetUid,
                GdprSignalIn::SetUidQuery(Some("1")),
                None,
                None,
                None,
                SyncPrivacyError::MissingRequiredConsent,
            ),
            (
                "cookie ambiguous default yes empty",
                SyncEndpoint::CookieSync,
                GdprSignalIn::CookieAbsent,
                None,
                None,
                None,
                SyncPrivacyError::MissingRequiredConsent,
            ),
            (
                "invalid sid",
                SyncEndpoint::CookieSync,
                GdprSignalIn::CookieNumber(0),
                None,
                Some("0"),
                None,
                SyncPrivacyError::MalformedGppSid,
            ),
            (
                "invalid gpp",
                SyncEndpoint::CookieSync,
                GdprSignalIn::CookieNumber(0),
                None,
                None,
                Some("invalid-gpp"),
                SyncPrivacyError::MalformedGpp,
            ),
            (
                "invalid legacy",
                SyncEndpoint::SetUid,
                GdprSignalIn::SetUidQuery(Some("invalid")),
                None,
                None,
                None,
                SyncPrivacyError::MalformedGdpr,
            ),
        ] {
            let result = resolve_sync_privacy(SyncPrivacyInput {
                gdpr,
                consent,
                us_privacy: None,
                gpp,
                gpp_sid: sid,
                account: &account,
                host: &host,
                endpoint,
                gvl: &gvl,
                gcl: &gcl,
                now: NOW,
            });
            assert_eq!(result.err(), Some(expected), "{name}");
        }

        let bidder = BidderInfo::default();
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        let mut deferred = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::SetUidQuery(None),
            consent: None,
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::SetUid,
            gvl: &gvl,
            gcl: &gcl,
            now: NOW,
        })
        .expect("ambiguous setuid empty consent is deferred");
        assert_eq!(
            deferred.setuid_permission(&lookup),
            SyncDecision::BlockedByActivity
        );
        assert_eq!(
            deferred.cookie_sync_host_permission(),
            SyncDecision::BlockedByPrivacy
        );
    }

    #[test]
    fn malformed_tcf_is_ignored_when_gdpr_permission_path_is_inactive() {
        let gvl = GvlStore::for_test(vec![]);
        let gcl = valid_gcl();
        let bidder = BidderInfo::default();
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for case in ["host off", "web off", "signal no"] {
            let mut host = configured_host();
            let mut account = AccountConfig::default();
            match case {
                "host off" => host.gdpr.enabled = false,
                "web off" => account.gdpr.channel_enabled.web = TriState::Disabled,
                _ => {}
            }
            for endpoint in [SyncEndpoint::CookieSync, SyncEndpoint::SetUid] {
                let gdpr = if case == "signal no" {
                    match endpoint {
                        SyncEndpoint::CookieSync => GdprSignalIn::CookieNumber(0),
                        SyncEndpoint::SetUid => GdprSignalIn::SetUidQuery(Some("0")),
                    }
                } else {
                    match endpoint {
                        SyncEndpoint::CookieSync => GdprSignalIn::CookieNumber(1),
                        SyncEndpoint::SetUid => GdprSignalIn::SetUidQuery(Some("1")),
                    }
                };
                let mut context = resolve_sync_privacy(SyncPrivacyInput {
                    gdpr,
                    consent: Some("malformed-tcf"),
                    us_privacy: None,
                    gpp: None,
                    gpp_sid: None,
                    account: &account,
                    host: &host,
                    endpoint,
                    gvl: &gvl,
                    gcl: &gcl,
                    now: NOW,
                })
                .unwrap();
                assert_eq!(
                    context.cookie_sync_host_permission(),
                    SyncDecision::Allowed,
                    "{case}"
                );
                assert_eq!(
                    context.cookie_sync_candidate_permission(&lookup),
                    SyncDecision::Allowed,
                    "{case}"
                );
                assert_eq!(
                    context.setuid_permission(&lookup),
                    SyncDecision::Allowed,
                    "{case}"
                );
            }
        }
    }

    #[test]
    fn candidate_skip_uses_exact_sid_text_and_disabled_precedes_skip() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr.host_vendor_id = Some(0);
        let gvl = store(None);
        let gcl = valid_gcl();
        let tc = consent(true, true);
        let mut bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..BidderInfo::default()
        };
        bidder.user_sync.skip_when = SkipWhen {
            gdpr: false,
            gpp_sid: vec!["2,6".into(), "-1".into()],
        };
        for (sid, expected) in [
            ("2,6", SyncDecision::BlockedByRegulationScope),
            ("6,2", SyncDecision::Allowed),
            ("02,6", SyncDecision::Allowed),
            ("-1", SyncDecision::BlockedByRegulationScope),
        ] {
            let lookup = SyncerLookup {
                requested_name: "fake_alias",
                normalized_name: "fake_alias",
                base_name: "fake_base",
                bidder: &bidder,
                syncer: &bidder.user_sync,
                syncer_key: "family",
            };
            let mut context = resolve_sync_privacy(SyncPrivacyInput {
                gdpr: GdprSignalIn::CookieNumber(0),
                consent: Some(&tc),
                us_privacy: Some("1YNN"),
                gpp: None,
                gpp_sid: Some(sid),
                account: &account,
                host: &host,
                endpoint: SyncEndpoint::CookieSync,
                gvl: &gvl,
                gcl: &gcl,
                now: NOW,
            })
            .unwrap();
            assert_eq!(context.selected_usp(), Some("1YNN"));
            assert_eq!(
                context.cookie_sync_candidate_permission(&lookup),
                expected,
                "{sid}"
            );
        }
        bidder.user_sync.enabled = false;
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        let mut context = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(0),
            consent: None,
            us_privacy: None,
            gpp: None,
            gpp_sid: Some("-1"),
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now: NOW,
        })
        .unwrap();
        assert_eq!(
            context.cookie_sync_candidate_permission(&lookup),
            SyncDecision::BlockedByDisabledUsersync
        );
    }

    #[test]
    fn gdpr_skip_uses_parsed_wire_scope_not_host_default() {
        let account = AccountConfig::default();
        let mut host = configured_host();
        host.gdpr.host_vendor_id = Some(0);
        host.gdpr.default_value = "1".to_owned();
        let gvl = store(None);
        let gcl = valid_gcl();
        let tc = consent(true, true);
        let mut bidder = BidderInfo {
            gvl_vendor_id: Some(8),
            ..BidderInfo::default()
        };
        bidder.user_sync.skip_when = SkipWhen {
            gdpr: true,
            gpp_sid: Vec::new(),
        };
        let lookup = SyncerLookup {
            requested_name: "fake_bidder",
            normalized_name: "fake_bidder",
            base_name: "fake_bidder",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        for (gdpr, expected) in [
            (
                GdprSignalIn::CookieNumber(1),
                SyncDecision::BlockedByRegulationScope,
            ),
            (GdprSignalIn::CookieNumber(0), SyncDecision::Allowed),
            (GdprSignalIn::CookieAbsent, SyncDecision::Allowed),
        ] {
            let mut context = resolve_sync_privacy(SyncPrivacyInput {
                gdpr,
                consent: Some(&tc),
                us_privacy: None,
                gpp: None,
                gpp_sid: None,
                account: &account,
                host: &host,
                endpoint: SyncEndpoint::CookieSync,
                gvl: &gvl,
                gcl: &gcl,
                now: NOW,
            })
            .expect("wire privacy");
            assert_eq!(context.cookie_sync_candidate_permission(&lookup), expected);
        }
    }
}
