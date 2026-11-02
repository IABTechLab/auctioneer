//! Hostile GPP normalization and authorization, beyond the parser's unit cap tests.

#[cfg(test)]
mod tests {
    use auctioneer_config::types::{AccountConfig, HostConfig, PurposeEnforcement, TriState};
    use auctioneer_openrtb::{NonFatalWarning, types::bid_request::BidRequest};
    use auctioneer_privacy::gcl::{CmpRecord, Gcl};
    use auctioneer_privacy::gpp::Limits;
    use auctioneer_privacy::gvl::{Gvl, GvlStore};
    use auctioneer_privacy::sources::{ConsentSource, EnvelopeState, normalize};
    use auctioneer_privacy::tcf::parse_tcf_v2;
    use auctioneer_privacy::{
        AuctionPermissions, PrivacyContext, PrivacyError, ResolveOptions, ResolvedPrivacy,
    };
    use proptest::bool::ANY as ANY_BOOL;
    use proptest::prelude::{ProptestConfig, prop_assert, prop_assert_eq, proptest};
    use serde_json::json;
    use time::macros::datetime;

    const GRANTING: &str = include_str!("data/tcf_v2/valid_full_consent.txt");
    const RESTRICTIVE: &str = include_str!("data/tcf_v2/valid_purpose_1_only.txt");
    const DENIED: AuctionPermissions = AuctionPermissions::Partial {
        allow_bid_request: false,
        pass_id: false,
        pass_geo: true,
    };
    const TCF_CONFLICT: &str = "regs.gpp section 2 and user.consent conflict; using section 2";
    const USP_CONFLICT: &str = "regs.gpp section 6 and regs.us_privacy conflict; using section 6";

    struct Harness {
        host: HostConfig,
        gvl: GvlStore,
        gcl: Gcl,
    }

    impl Harness {
        fn new() -> Self {
            let consent = parse_tcf_v2(GRANTING.trim()).expect("valid granting TCF");
            let gvl = Gvl::load_from_json(
                &json!({
                    "gvlSpecificationVersion": 3_i32, "tcfPolicyVersion": 5_i32,
                    "vendorListVersion": consent.vendor_list_version,
                    "lastUpdated": "2026-07-01T00:00:00Z",
                    "purposes": {
                        "1": {"id": 1_i32, "name": "Storage"},
                        "2": {"id": 2_i32, "name": "Basic ads"}
                    },
                    "specialPurposes": {}, "features": {}, "specialFeatures": {},
                    "vendors": {"8": {"id": 8_i32, "name": "Test vendor",
                        "purposes": [1_i32, 2_i32], "legIntPurposes": [],
                        "flexiblePurposes": [], "specialFeatures": []}}
                })
                .to_string(),
                None,
            )
            .expect("test GVL");
            let mut host = HostConfig::default();
            host.gdpr.enabled = true;
            host.gdpr.tcf2.enabled = true;
            host.gdpr.default_value = "0".into();
            host.gdpr.tcf2.purposes.insert(
                2,
                PurposeEnforcement {
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Enabled,
                    ..PurposeEnforcement::default()
                },
            );
            host.ccpa.enforce = true;
            let harness = Self {
                host,
                gvl: GvlStore::for_test(vec![(3, u32::from(consent.vendor_list_version), gvl)]),
                gcl: Gcl::for_test(
                    vec![CmpRecord {
                        id: consent.cmp_id,
                        deleted_date: None,
                    }],
                    datetime!(2027-01-01 00:00 UTC),
                ),
            };
            let control = request("", &[2_i32, 6_i32]);
            let mut resolved = harness.resolve(&control, false).unwrap();
            assert_eq!(
                resolved.context.permissions("fake_core", "fake_ax"),
                AuctionPermissions::AllowAll,
                "legacy consent must actually grant before testing fallback denial"
            );
            assert!(resolved.context.take_warnings().is_empty());
            harness
        }

        fn resolve<'a>(
            &self,
            request: &'a BidRequest,
            gdpr_exempt: bool,
        ) -> Result<ResolvedPrivacy<'a>, PrivacyError> {
            PrivacyContext::resolve(
                request,
                &AccountConfig::default(),
                &self.host,
                &self.gvl,
                &self.gcl,
                datetime!(2026-09-01 00:00 UTC),
                ResolveOptions {
                    gdpr_exempt,
                    ..ResolveOptions::default()
                },
            )
        }
    }

    fn request(gpp: &str, sid: &[i32]) -> BidRequest {
        serde_json::from_value(json!({
            "id": "gpp-hostile", "imp": [],
            "regs": {"gpp": gpp, "gpp_sid": sid, "us_privacy": "1YNN"},
            "user": {"consent": GRANTING.trim()},
            "ext": {"prebid": {"aliasgvlids": {"fake_ax": 8_i32}}}
        }))
        .expect("typed request fixture")
    }

    fn envelope64(tcf: &str, usp: &str, opaque: &str) -> String {
        let mut sections = vec![opaque; 64];
        sections[1] = tcf;
        sections[5] = usp;
        // Pinned go-gpp IDs 1..=64 header from src/gpp/tests/headers.rs.
        format!("DBAB4Rg~{}", sections.join("~"))
    }

    fn assert_warnings(warnings: &[NonFatalWarning], expected: &[(&str, &str)]) {
        assert_eq!(warnings.len(), expected.len(), "bounded warning count");
        for (warning, &(scope, message)) in warnings.iter().zip(expected) {
            assert_eq!(warning.code.as_u32(), 10_001);
            assert_eq!(warning.scope.as_deref(), Some(scope));
            assert_eq!(warning.message, message, "diagnostic text must be fixed");
            assert!(!warning.message.contains("private-"));
            assert!(!warning.message.contains(GRANTING.trim()));
        }
    }

    #[test]
    fn section_65_is_terminal_with_unchanged_restrictive_tcf_even_when_gdpr_bypasses() {
        parse_tcf_v2(RESTRICTIVE.trim()).expect("restrictive TCF is valid, not malformed");
        let bounded = envelope64(RESTRICTIVE.trim(), "1YNN", "private-opaque:\0=/+");
        let over = format!("{bounded}~private-section-65");
        for (mode, enabled, exempt, sid) in [
            ("enforced", true, false, &[2_i32, 6_i32][..]),
            ("disabled", false, false, &[2_i32, 6_i32][..]),
            ("nonapplicable", true, false, &[6_i32][..]),
            ("nsp", true, true, &[2_i32, 6_i32][..]),
        ] {
            let mut harness = Harness::new();
            harness.host.gdpr.enabled = enabled;
            let before = request(&bounded, sid);
            let sources = normalize(&before, Limits::default()).unwrap();
            assert_eq!(sources.envelope_state(), EnvelopeState::Valid);
            let active = sid.contains(&2_i32);
            assert_eq!(sources.active_tcf(), active.then_some(RESTRICTIVE.trim()));
            let mut resolved = harness.resolve(&before, exempt).unwrap();
            assert_eq!(
                resolved.context.permissions("fake_core", "fake_ax"),
                if mode == "enforced" {
                    DENIED
                } else {
                    AuctionPermissions::AllowAll
                },
                "within-limit control for {mode}"
            );
            assert_warnings(
                &resolved.context.take_warnings(),
                if active {
                    &[("regs.gpp", TCF_CONFLICT)]
                } else {
                    &[]
                },
            );

            let after = request(&over, sid);
            assert_eq!(
                normalize(&after, Limits::default()).unwrap_err(),
                PrivacyError::GppLimitExceeded,
                "normalization cannot downgrade the 65th slot to a syntax warning"
            );
            let error = harness.resolve(&after, exempt).unwrap_err();
            assert_eq!(error, PrivacyError::GppLimitExceeded, "{mode}");
            assert_eq!(error.to_string(), "regs.gpp exceeds privacy parsing limits");
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 64, failure_persistence: None, ..ProptestConfig::default()
        })]

        #[test]
        fn malformed_unknown_payloads_cannot_poison_valid_active_2_and_6(
            payload in "[^~]{0,512}"
        ) {
            let harness = Harness::new();
            let opaque = format!("private-opaque:\0\u{e9}=/+.{payload}");
            let gpp = envelope64(GRANTING.trim(), "1YYN", &opaque);
            // Unsupported sections are opaque even when SID names them.
            let req = request(&gpp, &[64_i32, 7_i32, 6_i32, 2_i32, 7_i32]);
            let original = serde_json::to_vec(&req).unwrap();
            let sources = normalize(&req, Limits::default()).unwrap();
            prop_assert_eq!(sources.envelope_state(), EnvelopeState::Valid);
            prop_assert_eq!(sources.tcf().source(), ConsentSource::Gpp);
            prop_assert_eq!(sources.tcf().raw(), Some(GRANTING.trim()));
            prop_assert!(sources.active_usp().unwrap().sale_opt_out());
            assert_warnings(&sources.into_warnings(None, None), &[("regs.gpp", USP_CONFLICT)]);

            let mut resolved = harness.resolve(&req, false).unwrap();
            prop_assert_eq!(resolved.context.permissions("fake_core", "fake_ax"), AuctionPermissions::AllowAll);
            prop_assert!(resolved.context.bidder_decision("fake_core", "fake_ax").ccpa);
            prop_assert_eq!(resolved.projection.consent, Some(GRANTING.trim()));
            prop_assert!(resolved.projection.usp.unwrap().sale_opt_out());
            assert_warnings(&resolved.context.take_warnings(), &[("regs.gpp", USP_CONFLICT)]);
            prop_assert!(resolved.context.take_warnings().is_empty());
            prop_assert_eq!(serde_json::to_vec(&req).unwrap(), original);
        }

        #[test]
        fn authoritative_invalid_section_2_never_grants_or_leaks_payload_in_warnings(
            payload in "[^~]{0,512}", valid_prefix in ANY_BOOL
        ) {
            let harness = Harness::new();
            let selected = if valid_prefix {
                format!("{}.\0private-selected:{payload}", GRANTING.trim())
            } else {
                format!("private-selected:\0{payload}")
            };
            let usp = format!("private-selected-usp:\0{payload}");
            let gpp = envelope64(&selected, &usp, &format!("private-unused:{payload}"));
            let mut req = request(&gpp, &[2_i32, 6_i32]);
            req.regs.as_mut().unwrap().us_privacy = Some(format!("private-legacy-usp:{payload}"));
            let original = serde_json::to_vec(&req).unwrap();
            let sources = normalize(&req, Limits::default()).unwrap();
            prop_assert_eq!(sources.envelope_state(), EnvelopeState::Valid);
            prop_assert_eq!(sources.tcf().source(), ConsentSource::Gpp);
            prop_assert_eq!(sources.tcf().raw(), Some(selected.as_str()));
            prop_assert!(sources.usp().is_none());

            let mut resolved = harness.resolve(&req, false).unwrap();
            prop_assert_eq!(resolved.context.permissions("fake_core", "fake_ax"), DENIED);
            prop_assert_eq!(resolved.projection.consent, Some(selected.as_str()));
            assert_warnings(&resolved.context.take_warnings(), &[
                ("regs.us_privacy", "regs.us_privacy is not a valid USP value; clearing legacy value"),
                ("regs.gpp", TCF_CONFLICT),
                ("regs.gpp", USP_CONFLICT),
                ("regs.gpp", "regs.gpp section 2 is not a valid TCF v2 string; treating as no consent"),
                ("regs.gpp", "regs.gpp section 6 is not a valid USP value; no usable USP"),
            ]);
            prop_assert!(resolved.context.take_warnings().is_empty());
            prop_assert_eq!(serde_json::to_vec(&req).unwrap(), original);
        }
    }
}
