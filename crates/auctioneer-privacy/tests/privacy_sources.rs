#[cfg(test)]
mod tests {
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_privacy::{
        PrivacyError, Signal, Usp,
        gpp::Limits,
        sources::{ConsentSource, EnvelopeState, normalize},
        warnings::gcl_stale,
    };
    use serde_json::{Value, json};

    fn request(gpp: &str, sid: Value, tcf: &str, usp: &str) -> BidRequest {
        let mut value = json!({
            "id": "sources", "regs": {"gpp": gpp, "us_privacy": usp},
            "user": {"consent": tcf}
        });
        value["regs"]["gpp_sid"] = sid;
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn absent_and_malformed_envelopes_allow_legacy_fallback_but_sid_still_controls_scope() {
        for (raw, state, warning_count) in [
            ("", EnvelopeState::Absent, 0),
            ("bad-envelope", EnvelopeState::Malformed, 1),
            ("DBAA", EnvelopeState::Valid, 0),
        ] {
            let req = request(raw, json!([2_i32, 6_i32]), "legacy", "1YYN");
            let sources = normalize(&req, Limits::default()).unwrap();
            assert_eq!(sources.envelope_state(), state);
            assert_eq!(sources.wire_signal(), Signal::Yes);
            assert_eq!(sources.tcf().raw(), Some("legacy"));
            assert_eq!(sources.tcf().source(), ConsentSource::Legacy);
            assert!(sources.usp().unwrap().sale_opt_out());
            assert_eq!(sources.into_warnings(None, None).len(), warning_count);
        }
    }

    #[test]
    fn empty_active_tcf_is_authoritative_but_empty_usp_uses_legacy() {
        let req = request("DBACNY~~", json!([2_i32, 6_i32]), "granting-legacy", "1YYN");
        let sources = normalize(&req, Limits::default()).unwrap();
        assert_eq!(sources.tcf().raw(), Some(""));
        assert_eq!(sources.tcf().source(), ConsentSource::Gpp);
        assert_eq!(sources.active_tcf(), Some(""));
        assert!(sources.usp().unwrap().sale_opt_out());
        assert_eq!(sources.active_usp(), None);
        assert!(sources.into_warnings(None, None).is_empty());
    }

    #[test]
    fn nonempty_invalid_active_usp_never_retries_legacy() {
        for selected in ["bad", " "] {
            let req = request(&format!("DBABTA~{selected}"), json!([6_i32]), "", "1YYN");
            let sources = normalize(&req, Limits::default()).unwrap();
            assert_eq!(sources.usp(), None);
            assert!(sources.usp_is_malformed());
            assert_eq!(sources.active_usp(), None);
            assert!(!sources.clear_legacy_usp());
            let warnings = sources.into_warnings(None, None);
            assert_eq!(warnings.len(), 2);
            assert!(
                warnings
                    .iter()
                    .all(|w| w.scope.as_deref() == Some("regs.gpp"))
            );
            assert!(warnings[0].message.contains("conflict"));
            assert!(warnings[1].message.contains("section 6"));
            assert!(warnings.iter().all(|w| w.code.as_u32() == 10001));
        }
    }

    #[test]
    fn selected_malformed_state_is_not_absence_or_a_projection_value() {
        for (gpp, sid, legacy, malformed) in [
            ("", json!(null), "", false),
            ("", json!(null), "1---", false),
            ("", json!(null), "private-malformed", true),
            ("DBABTA~private-malformed", json!([6_i32]), "", true),
            ("DBABTA~private-malformed", json!([]), "", false),
            ("DBABTA~1YNN", json!([6_i32]), "private-malformed", false),
            ("DBABTA~", json!([6_i32]), "private-malformed", true),
        ] {
            let req = request(gpp, sid, "", legacy);
            let sources = normalize(&req, Limits::default()).unwrap();
            assert_eq!(sources.usp_is_malformed(), malformed, "{gpp}, {legacy}");
            if malformed {
                assert!(sources.usp().is_none());
                assert!(sources.projection().usp.is_none());
            }
            assert!(!format!("{sources:?}").contains("private-malformed"));
            assert!(!format!("{:?}", sources.projection()).contains("private-malformed"));
        }
    }

    #[test]
    fn inactive_payloads_are_opaque_and_never_projected() {
        for sid in [json!(null), json!([]), json!([7_i32])] {
            let req = request(
                "DBACNY~malformed-tcf~malformed-usp",
                sid.clone(),
                "legacy",
                "1NYN",
            );
            let sources = normalize(&req, Limits::default()).unwrap();
            assert_eq!(sources.tcf().raw(), Some("legacy"));
            assert_eq!(sources.active_tcf(), None);
            assert_eq!(sources.active_usp(), None);
            assert!(sources.usp().unwrap().sale_opt_out());
            assert_eq!(
                sources.downgrade_gdpr(),
                if sid.is_null() { None } else { Some(0_i32) }
            );
            assert!(sources.into_warnings(None, None).is_empty());
        }
    }

    #[test]
    fn legacy_usp_is_validated_even_when_gpp_selected_and_warnings_are_ordered() {
        let req = request(
            "DBACNY~selected-tcf~bad-gpp-usp",
            json!([2_i32, 6_i32]),
            "legacy-tcf",
            "bad-legacy-usp",
        );
        let sources = normalize(&req, Limits::default()).unwrap();
        assert!(sources.clear_legacy_usp());
        let validation = sources.tcf().malformed_warning();
        let warnings = sources.into_warnings(Some(validation), None);
        assert_eq!(warnings.len(), 5);
        assert_eq!(warnings[0].scope.as_deref(), Some("regs.us_privacy"));
        assert!(
            warnings[1].message.contains("section 2") && warnings[1].message.contains("conflict")
        );
        assert!(
            warnings[2].message.contains("section 6") && warnings[2].message.contains("conflict")
        );
        assert!(
            warnings[3].message.contains("section 2") && !warnings[3].message.contains("conflict")
        );
        assert!(
            warnings[4].message.contains("section 6") && !warnings[4].message.contains("conflict")
        );
        for warning in warnings {
            for secret in [
                "selected-tcf",
                "legacy-tcf",
                "bad-gpp-usp",
                "bad-legacy-usp",
            ] {
                assert!(!warning.message.contains(secret));
            }
        }
    }

    #[test]
    fn limits_are_terminal_without_policy_configuration_or_fallback() {
        let req = request(
            "DBABM~restricted",
            json!([2_i32]),
            "granting-legacy",
            "1YYN",
        );
        let limits = Limits::new(0x0004_0000, 4096, 64, u32::from(u16::MAX), 0).unwrap();
        assert_eq!(
            normalize(&req, limits).unwrap_err(),
            PrivacyError::GppLimitExceeded
        );
        let too_many = request(
            &format!("DBAA{}", "~".repeat(65)),
            json!([]),
            "grant",
            "1YYN",
        );
        assert_eq!(
            normalize(&too_many, Limits::default()).unwrap_err(),
            PrivacyError::GppLimitExceeded
        );
    }

    #[test]
    fn normalization_does_not_change_input_or_authorization_parse_tcf() {
        let req = request("DBABM~opaque-tcf", json!([2_i32]), "", "invalid-usp");
        let before = serde_json::to_vec(&req).unwrap();
        let sources = normalize(&req, Limits::default()).unwrap();
        assert_eq!(sources.tcf().raw(), Some("opaque-tcf"));
        assert_eq!(sources.active_tcf(), Some("opaque-tcf"));
        assert!(sources.clear_legacy_usp());
        let warnings = sources.into_warnings(None, None);
        assert_eq!(warnings.len(), 1, "no inactive or premature TCF decoding");
        assert_eq!(serde_json::to_vec(&req).unwrap(), before);
    }

    #[test]
    fn source_selection_matches_reference_except_explicit_inactive_tcf_decision() {
        let fixture: Value =
            serde_json::from_str(include_str!("data/privacy/reference.json")).unwrap();
        let rows = fixture["selection"].as_array().unwrap();
        assert_eq!(rows.len(), 36);
        for row in rows {
            assert_eq!(row["kind"], "source-selection-only");
            let req = request(
                row["gpp"].as_str().unwrap(),
                row["sid"].clone(),
                row["legacy_tcf"].as_str().unwrap(),
                row["legacy_usp"].as_str().unwrap(),
            );
            let sources = normalize(&req, Limits::default()).unwrap();
            let scope = match sources.wire_signal() {
                Signal::Yes => 1_i32,
                Signal::No => 0_i32,
                Signal::Ambiguous => -1_i32,
            };
            assert_eq!(scope, row["scope"], "{row}");
            let active_tcf = row["sid"]
                .as_array()
                .is_some_and(|ids| ids.contains(&json!(2_i32)));
            let expected_tcf = if active_tcf {
                &row["selected_tcf"]
            } else {
                // Approved M4 decision: ignore section 2 when SID does not activate it.
                &row["legacy_tcf"]
            };
            assert_eq!(sources.tcf().raw(), expected_tcf.as_str(), "{row}");
            let expected_usp = Usp::parse(row["selected_usp"].as_str()).ok().flatten();
            assert_eq!(sources.usp(), expected_usp, "{row}");
            let warnings = sources.into_warnings(None, None);
            assert_eq!(
                warnings
                    .iter()
                    .any(|w| w.message.contains("section 6") && w.message.contains("conflict")),
                row["usp_conflict"].as_bool().unwrap(),
                "{row}"
            );
        }
    }

    #[test]
    fn wire_and_envelope_warnings_precede_legacy_usp_and_authenticity() {
        let mut req = request("bad-envelope", json!([2_i32, 6_i32]), "legacy", "bad-usp");
        req.regs.as_mut().unwrap().gdpr = Some(0_i32);
        let sources = normalize(&req, Limits::default()).unwrap();
        let malformed = sources.tcf().malformed_warning();
        let warnings = sources.into_warnings(Some(malformed), None);
        assert_eq!(
            warnings
                .iter()
                .map(|w| w.scope.as_deref())
                .collect::<Vec<_>>(),
            vec![
                Some("regs.gpp_sid"),
                Some("regs.gpp"),
                Some("regs.us_privacy"),
                Some("user.consent")
            ]
        );
        let absent_consent = request("DBAA", json!([2_i32]), "", "");
        let normalized_absence = normalize(&absent_consent, Limits::default()).unwrap();
        let stale_warnings = normalized_absence.into_warnings(None, Some(gcl_stale()));
        assert_eq!(stale_warnings.len(), 1);
        assert_eq!(stale_warnings[0].scope, None);
        assert_eq!(stale_warnings[0].code.as_u32(), 10019);
    }

    #[test]
    fn invalid_legacy_usp_is_always_cleared_without_duplicating_selected_validation() {
        for (gpp, sid) in [
            ("", json!(null)),
            ("DBABTA~", json!([6_i32])),
            ("DBABTA~invalid", json!([7_i32])),
        ] {
            let req = request(gpp, sid, "", "invalid");
            let sources = normalize(&req, Limits::default()).unwrap();
            assert!(sources.clear_legacy_usp());
            assert_eq!(sources.usp(), None);
            let warnings = sources.into_warnings(None, None);
            assert_eq!(warnings.len(), 1);
            assert_eq!(warnings[0].scope.as_deref(), Some("regs.us_privacy"));
        }
    }

    proptest::proptest! {
        #[test]
        fn arbitrary_envelopes_never_panic_or_expand_warning_count(raw in ".{0,256}") {
            let req = request(&raw, json!([2_i32, 6_i32]), "legacy", "invalid-usp");
            let before = serde_json::to_vec(&req).unwrap();
            if let Ok(sources) = normalize(&req, Limits::default()) {
                proptest::prop_assert!(sources.into_warnings(None, None).len() <= 5);
            }
            proptest::prop_assert_eq!(serde_json::to_vec(&req).unwrap(), before);
        }
    }
}
