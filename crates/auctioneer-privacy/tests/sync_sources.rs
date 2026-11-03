#[cfg(test)]
mod tests {
    use auctioneer_config_data::types::{AccountConfig, HostConfig};
    use auctioneer_privacy::{
        GdprSignalIn, Signal, SyncEndpoint, SyncPrivacyError, SyncPrivacyInput, gcl::Gcl,
        gvl::GvlStore, resolve_sync_privacy, tcf::MAX_TOTAL_INPUT_LEN,
    };
    use time::OffsetDateTime;

    fn input<'a>(
        gdpr: GdprSignalIn<'a>,
        account: &'a AccountConfig,
        host: &'a HostConfig,
        gvl: &'a GvlStore,
        gcl: &'a Gcl,
    ) -> SyncPrivacyInput<'a> {
        SyncPrivacyInput {
            gdpr,
            consent: None,
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account,
            host,
            endpoint: SyncEndpoint::CookieSync,
            gvl,
            gcl,
            now: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn cookie_json_gdpr_accepts_only_numeric_zero_one_and_null() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        for (wire, expected) in [
            (GdprSignalIn::CookieAbsent, Signal::Ambiguous),
            (GdprSignalIn::CookieNull, Signal::Ambiguous),
            (GdprSignalIn::CookieNumber(0), Signal::No),
            (GdprSignalIn::CookieNumber(1), Signal::Yes),
        ] {
            let mut request = input(wire, &account, &host, &gvl, &gcl);
            request.consent = Some("opaque");
            let result = resolve_sync_privacy(request).unwrap();
            assert_eq!(result.parsed_request_signal, expected);
        }
        for wire in [GdprSignalIn::CookieNumber(2), GdprSignalIn::CookieOther] {
            assert_eq!(
                resolve_sync_privacy(input(wire, &account, &host, &gvl, &gcl)).err(),
                Some(SyncPrivacyError::MalformedGdpr)
            );
        }
    }

    #[test]
    fn setuid_query_gdpr_distinguishes_empty_from_whitespace() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        for (wire, expected) in [
            (None, Signal::Ambiguous),
            (Some(""), Signal::Ambiguous),
            (Some("0"), Signal::No),
            (Some("1"), Signal::Yes),
        ] {
            let mut request = input(GdprSignalIn::SetUidQuery(wire), &account, &host, &gvl, &gcl);
            request.endpoint = SyncEndpoint::SetUid;
            request.consent = Some("opaque");
            assert_eq!(
                resolve_sync_privacy(request).unwrap().parsed_request_signal,
                expected
            );
        }
        for wire in [" ", "01", "2", "true"] {
            let mut request = input(
                GdprSignalIn::SetUidQuery(Some(wire)),
                &account,
                &host,
                &gvl,
                &gcl,
            );
            request.endpoint = SyncEndpoint::SetUid;
            assert_eq!(
                resolve_sync_privacy(request).err(),
                Some(SyncPrivacyError::MalformedGdpr)
            );
        }
    }

    #[test]
    fn sid_keeps_raw_text_but_uses_numeric_membership() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut request = input(GdprSignalIn::CookieOther, &account, &host, &gvl, &gcl);
        request.gpp_sid = Some("02,6,2,0006");
        request.gpp = Some("DBACNY~selected-tcf~1YNN");
        request.consent = Some("legacy-tcf");
        request.us_privacy = Some("1NYN");
        let result = resolve_sync_privacy(request).unwrap();
        assert_eq!(result.parsed_request_signal, Signal::Yes);
        assert_eq!(result.raw_gpp_sid(), Some("02,6,2,0006"));
        assert!(result.sid().contains(2));
        assert!(result.sid().contains(6));
        assert_eq!(result.selected_consent(), Some("selected-tcf"));
        assert_eq!(result.selected_usp(), Some("1YNN"));
    }

    #[test]
    fn sentinel_overrides_invalid_legacy_without_selecting_gpp_section() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut request = input(GdprSignalIn::CookieOther, &account, &host, &gvl, &gcl);
        request.gpp_sid = Some("-1");
        request.gpp = Some("DBABM~selected-tcf");
        request.consent = Some("legacy-tcf");
        let result = resolve_sync_privacy(request).unwrap();
        assert_eq!(result.parsed_request_signal, Signal::No);
        assert_eq!(result.raw_gpp_sid(), Some("-1"));
        assert!(result.sid().is_no_applicable_section());
        assert_eq!(result.selected_consent(), Some("legacy-tcf"));
    }

    #[test]
    fn invalid_sid_and_overlong_list_are_closed_errors() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        for sid in [
            "0", "-01", "-2", "-1,2", "-1,-1", "2,-1", "2,", ",2", "2,,6", "+2", " 2", "2 ",
            "65536",
        ] {
            let mut request = input(GdprSignalIn::CookieAbsent, &account, &host, &gvl, &gcl);
            request.gpp_sid = Some(sid);
            assert_eq!(
                resolve_sync_privacy(request).err(),
                Some(SyncPrivacyError::MalformedGppSid),
                "{sid}"
            );
        }
        let sid = vec!["2"; 65].join(",");
        let mut request = input(GdprSignalIn::CookieAbsent, &account, &host, &gvl, &gcl);
        request.gpp_sid = Some(&sid);
        assert_eq!(
            resolve_sync_privacy(request).err(),
            Some(SyncPrivacyError::GppSidLimitExceeded)
        );
    }

    #[test]
    fn active_empty_tcf_is_authoritative_and_required_empty_is_endpoint_specific() {
        let account = AccountConfig::default();
        let mut host = HostConfig::default();
        host.gdpr.default_value = "1".into();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut request = input(GdprSignalIn::CookieAbsent, &account, &host, &gvl, &gcl);
        request.gpp_sid = Some("2");
        request.gpp = Some("DBABM~");
        request.consent = Some("legacy-tcf");
        assert_eq!(
            resolve_sync_privacy(request).err(),
            Some(SyncPrivacyError::MissingRequiredConsent)
        );

        let mut setuid_request =
            input(GdprSignalIn::SetUidQuery(None), &account, &host, &gvl, &gcl);
        setuid_request.endpoint = SyncEndpoint::SetUid;
        let result = resolve_sync_privacy(setuid_request).unwrap();
        assert_eq!(result.parsed_request_signal, Signal::Ambiguous);
        assert_eq!(result.normalized_request_signal, Signal::Yes);
    }

    #[test]
    fn invalid_active_usp_clears_without_legacy_fallback() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut request = input(GdprSignalIn::CookieAbsent, &account, &host, &gvl, &gcl);
        request.gpp_sid = Some("6");
        request.gpp = Some("DBABTA~invalid-usp");
        request.us_privacy = Some("1YNN");
        assert_eq!(resolve_sync_privacy(request).unwrap().selected_usp(), None);
    }

    #[test]
    fn malformed_gpp_is_fatal_and_diagnostics_do_not_echo_private_input() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut request = input(GdprSignalIn::CookieNumber(0), &account, &host, &gvl, &gcl);
        request.gpp = Some("private-consent-token");
        assert_eq!(
            resolve_sync_privacy(request).err(),
            Some(SyncPrivacyError::MalformedGpp)
        );
        let error = SyncPrivacyError::MalformedGpp;
        assert!(!format!("{error:?} {error}").contains("private-consent-token"));
    }

    #[test]
    fn invalid_query_gdpr_debug_does_not_echo_wire_value() {
        let wire = GdprSignalIn::SetUidQuery(Some("private-invalid-gdpr"));
        assert!(!format!("{wire:?}").contains("private-invalid-gdpr"));
    }

    #[test]
    fn sync_context_does_not_advertise_unused_ccpa_enforcement() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let context = resolve_sync_privacy(input(
            GdprSignalIn::CookieNumber(0),
            &account,
            &host,
            &gvl,
            &gcl,
        ))
        .unwrap();
        assert!(!format!("{context:?}").contains("ccpa_enforce"));
    }

    #[test]
    fn malformed_nonempty_tcf_is_deferred_until_permission_parsing() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut request = input(GdprSignalIn::CookieNumber(1), &account, &host, &gvl, &gcl);
        request.consent = Some("private-malformed-consent");
        let mut context = resolve_sync_privacy(request).expect("nonempty TCF is deferred");
        assert_eq!(
            context.selected_consent(),
            Some("private-malformed-consent")
        );
        assert!(!format!("{context:?}").contains("private-malformed-consent"));
        let error = context.parse_selected_tcf().expect_err("invalid TCF");
        assert!(!format!("{error:?} {error}").contains("private-malformed-consent"));
    }

    #[test]
    fn discarded_legacy_privacy_inputs_still_obey_limits() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let oversized_consent = "x".repeat(MAX_TOTAL_INPUT_LEN + 1);
        let mut request = input(GdprSignalIn::CookieAbsent, &account, &host, &gvl, &gcl);
        request.gpp_sid = Some("2");
        request.gpp = Some("DBABM~selected-tcf");
        request.consent = Some(&oversized_consent);
        assert_eq!(
            resolve_sync_privacy(request).err(),
            Some(SyncPrivacyError::InputLimitExceeded)
        );

        let oversized_usp = "x".repeat(32 * 1024 + 1);
        let mut usp_request = input(GdprSignalIn::CookieAbsent, &account, &host, &gvl, &gcl);
        usp_request.gpp_sid = Some("6");
        usp_request.gpp = Some("DBABTA~1YNN");
        usp_request.us_privacy = Some(&oversized_usp);
        assert_eq!(
            resolve_sync_privacy(usp_request).err(),
            Some(SyncPrivacyError::InputLimitExceeded)
        );
    }

    #[test]
    fn sid_override_does_not_hide_oversized_setuid_gdpr_value() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let oversized = "x".repeat(32 * 1024 + 1);
        let mut request = input(
            GdprSignalIn::SetUidQuery(Some(&oversized)),
            &account,
            &host,
            &gvl,
            &gcl,
        );
        request.endpoint = SyncEndpoint::SetUid;
        request.gpp_sid = Some("-1");
        assert_eq!(
            resolve_sync_privacy(request).err(),
            Some(SyncPrivacyError::InputLimitExceeded)
        );
    }
}
