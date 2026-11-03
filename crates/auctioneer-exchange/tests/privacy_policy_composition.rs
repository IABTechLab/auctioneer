//! Privacy policy composition at the real prepare -> split -> adapter -> fanout boundary.
//! Only the adapter transport is recorded; no privacy resolver/scrub is mocked.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod auction_support;
mod support;

#[cfg(test)]
mod privacy;

#[cfg(test)]
mod tests {
    use super::auction_support::{
        ACCOUNT_ID, CLIENT_V4, CLIENT_V6, CONSENT, FPD_PATHS, Harness, ID_PATHS, IDENTIFIER_PATHS,
        UNKNOWN_PATHS, absent, coarse_geo, no_raw_ips, request, second_imp, unchanged,
    };
    use auctioneer_bidder::BidderName;
    use auctioneer_config_data::types::TriState;
    use auctioneer_exchange::AuctionError;
    use serde_json::{Value, json};
    use std::time::Duration;

    fn assert_user_fpd(body: &Value, original: &Value, restricted: bool) {
        unchanged(body, original, FPD_PATHS);
        if restricted {
            absent(body, &["/user/ext/context"]);
        } else {
            unchanged(body, original, &["/user/ext/context"]);
        }
    }

    #[test]
    fn classified_alias_scopes_share_audit_without_restoring_denied_data_on_the_wire() {
        for (pass_id, pass_geo) in [(true, true), (false, true), (true, false), (false, false)] {
            let mut harness = Harness::default();
            harness.activities(json!({
                "transmitUfpd": {"rules": [{
                    "condition": {"componentName": ["fake_restricted"]}, "allow": pass_id}]},
                "transmitPreciseGeo": {"rules": [{
                    "condition": {"componentName": ["fake_restricted"]}, "allow": pass_geo}]}
            }));
            let mut req = request(&["fake_restricted", "fake_sibling"]);
            req["ext"]["prebid"]["aliases"] = json!({
                "fake_restricted": "fake_bb", "fake_sibling": "fake_bb"
            });
            let observed = harness.run(req);
            observed.assert_built(&["fake_restricted", "fake_sibling"]);
            assert_eq!(observed.audit.attempted_scopes, 2);
            assert_eq!(observed.audit.failed_scopes, 0);
            assert_eq!(observed.audit.counts() == [0; 8], pass_id && pass_geo);
            unchanged(
                &observed.body("fake_sibling"),
                &observed.original,
                &["/user", "/device", "/source/tid", "/imp/0/ext/tid"],
            );
            let body = observed.body("fake_restricted");
            if pass_id {
                unchanged(&body, &observed.original, IDENTIFIER_PATHS);
                unchanged(&body, &observed.original, FPD_PATHS);
            } else {
                absent(&body, ID_PATHS);
                absent(&body, FPD_PATHS);
            }
            if !pass_id || !pass_geo {
                absent(&body, UNKNOWN_PATHS);
                absent(
                    &body,
                    &[
                        "/user/ext/context",
                        "/user/geo/ext",
                        "/device/geo/ext",
                        "/user/geo/extra_geo",
                        "/device/geo/extra_geo",
                    ],
                );
                for owner in ["user", "device"] {
                    for key in [
                        "country",
                        "city",
                        "metro",
                        "region",
                        "regionfips104",
                        "type",
                        "lastfix",
                        "ipservice",
                        "utcoffset",
                    ] {
                        assert!(observed.original[owner]["geo"].get(key).is_some());
                        assert_eq!(
                            body[owner]["geo"][key],
                            observed.original[owner]["geo"][key]
                        );
                    }
                }
            }
            if pass_geo {
                unchanged(
                    &body,
                    &observed.original,
                    &[
                        "/device/ip",
                        "/device/ipv6",
                        "/user/geo/lat",
                        "/user/geo/lon",
                        "/user/geo/zip",
                        "/user/geo/accuracy",
                        "/device/geo/lat",
                        "/device/geo/lon",
                        "/device/geo/zip",
                        "/device/geo/accuracy",
                    ],
                );
            } else {
                coarse_geo(&body);
            }
            let construction = observed
                .constructions
                .iter()
                .find(|scope| scope.bidder == "fake_restricted")
                .unwrap();
            assert_eq!(body["user"], construction.input["user"]);
            assert_eq!(body["device"], construction.input["device"]);
        }
    }

    #[test]
    fn survivor_fpd_identity_rewrite_fails_before_construction_or_dispatch() {
        for elapsed in [Duration::ZERO, Duration::from_secs(6)] {
            let harness = Harness {
                elapsed,
                ..Harness::default()
            };
            let mut req = request(&["fake_bb"]);
            second_imp(&mut req);
            req["imp"][1]["ext"]["bidder"] = json!({"fake_ccc": {}});
            req["imp"][0]["ext"]["prebid"] = json!({
                "imp": {"fake_bb": {"id": "imp-2"}}
            });
            let observed = harness.run(req);
            assert!(
                matches!(
                    &observed.result,
                    Err(AuctionError::InvalidImpFirstPartyData(_))
                ),
                "FPD cannot move a bid into a sibling's impression scope: {:?}",
                observed.result
            );
            observed.assert_built(&[]);
            assert!(observed.outbound.is_empty());
            assert!(observed.response_hooks.is_empty());
            assert_eq!(observed.audit.attempted_scopes, 0);
        }
    }

    #[test]
    fn identity_preserving_fpd_keeps_bids_in_their_original_participation_scopes() {
        for explicit_id in [false, true] {
            let harness = Harness::default();
            let mut req = request(&["fake_bb"]);
            second_imp(&mut req);
            req["imp"][1]["ext"]["bidder"] = json!({"fake_ccc": {}});
            let mut patch = json!({"banner": {"w": 640_i32}});
            if explicit_id {
                patch["id"] = json!("imp-1");
            }
            req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": patch}});
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            assert_eq!(observed.body("fake_bb")["imp"][0]["banner"]["w"], 640_i32);
            assert_eq!(observed.body("fake_bb")["imp"][0]["id"], "imp-1");
            assert_eq!(observed.body("fake_ccc")["imp"][0]["id"], "imp-2");
            assert_eq!(observed.response().seatbid.len(), 2);
            for (seat, impid) in [("fake_bb", "imp-1"), ("fake_ccc", "imp-2")] {
                let bids = &observed
                    .response()
                    .seatbid
                    .iter()
                    .find(|row| row.seat == seat)
                    .unwrap()
                    .bid;
                assert_eq!(bids.len(), 1);
                assert_eq!(bids[0].impid, impid);
            }
        }
    }

    #[test]
    fn fetch_denied_identity_rewrite_does_not_poison_allowed_sibling() {
        let mut harness = Harness::default();
        harness.activities(json!({"fetchBids": {"rules": [{
            "condition": {"componentName": ["fake_bb"]}, "allow": false
        }]}}));
        let mut req = request(&["fake_bb"]);
        second_imp(&mut req);
        req["imp"][1]["ext"]["bidder"] = json!({"fake_ccc": {}});
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": {"id": "imp-2"}}});
        let observed = harness.run(req);
        observed.assert_built(&["fake_ccc"]);
        observed.assert_blocked("fake_bb", &["imp-1"]);
        assert_eq!(observed.body("fake_ccc")["imp"][0]["id"], "imp-2");
        assert_eq!(observed.response().seatbid[0].bid[0].impid, "imp-2");
    }

    #[test]
    fn expired_allowed_survivor_validates_fpd_before_scrubbing_or_timeout() {
        let mut harness = Harness {
            elapsed: Duration::from_secs(6),
            ..Harness::default()
        };
        harness.activities(json!({"transmitUfpd": {"default": false}}));
        let mut req = request(&["fake_bb"]);
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": {"banner": null}}});
        let observed = harness.run(req);
        assert!(matches!(
            observed.result,
            Err(AuctionError::InvalidImpFirstPartyData(_))
        ));
        assert!(observed.constructions.is_empty());
        assert!(observed.outbound.is_empty());
        assert_eq!(observed.audit.attempted_scopes, 0);
        assert_eq!(observed.audit.counts(), [0; 8]);
    }

    #[test]
    fn expired_valid_survivors_return_timeout_without_attempting_disclosure() {
        let mut harness = Harness {
            elapsed: Duration::from_secs(6),
            ..Harness::default()
        };
        harness.activities(json!({
            "transmitUfpd": {"default": false},
            "transmitPreciseGeo": {"default": false},
            "transmitTid": {"default": false}
        }));
        let observed = harness.run(request(&["fake_bb", "fake_ccc"]));
        observed.response();
        assert!(observed.constructions.is_empty());
        assert!(observed.outbound.is_empty());
        assert_eq!(observed.ext()["errors"]["prebid"][0]["code"], 12_i32);
        assert_eq!(observed.audit.attempted_scopes, 0);
        assert_eq!(observed.audit.failed_scopes, 0);
        assert_eq!(observed.audit.counts(), [0; 8]);
    }

    #[test]
    fn expired_fetch_denied_scope_never_materializes_malformed_fpd() {
        let mut harness = Harness {
            elapsed: Duration::from_secs(6),
            ..Harness::default()
        };
        harness.activities(json!({"fetchBids": {"default": false}}));
        let mut req = request(&["fake_bb"]);
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": {"banner": null}}});
        let observed = harness.run(req);
        observed.response();
        observed.assert_blocked("fake_bb", &["imp-1"]);
        assert!(observed.constructions.is_empty());
        assert!(observed.outbound.is_empty());
        assert_eq!(observed.audit.attempted_scopes, 0);
        assert_eq!(observed.audit.counts(), [0; 8]);
    }

    #[test]
    fn fetch_denial_prevents_construction_and_reports_one_exclusion_even_with_gdpr_denial() {
        for gdpr_denies in [false, true] {
            let mut harness = Harness::default();
            if gdpr_denies {
                harness.enforce_gdpr();
            }
            harness.activities(json!({"fetchBids": {"default": false}}));
            let mut req = request(&["fake_bb", "fake_ccc"]);
            req["regs"]["gdpr"] = json!(1_i32);
            second_imp(&mut req);
            let observed = harness.run(req);
            observed.assert_built(&[]);
            assert!(observed.outbound.is_empty());
            assert!(observed.response().seatbid.is_empty());
            for bidder in ["fake_bb", "fake_ccc"] {
                observed.assert_blocked(bidder, &["imp-1", "imp-2"]);
            }
        }
    }

    #[test]
    fn fetch_first_match_allows_sibling_and_denied_bidder_does_not_consume_cap() {
        let mut harness = Harness::default();
        harness.host.resource_limits.max_bidders_per_auction = 1;
        harness.activities(json!({"fetchBids": {"rules": [
        {"condition": {"componentName": ["fake_CCC"], "componentType": ["BIDDER"]}, "allow": true},
        {"condition": {"componentType": ["bidder"]}, "allow": false}
    ]}}));
        let observed = harness.run(request(&["fake_bb", "fake_ccc"]));
        observed.assert_built(&["fake_ccc"]);
        assert_eq!(observed.outbound.len(), 1);
        observed.assert_blocked("fake_bb", &["imp-1"]);
        assert_eq!(observed.response().seatbid.len(), 1);
        assert_eq!(observed.response().seatbid[0].seat, "fake_ccc");
        assert_eq!(observed.response().seatbid[0].bid.len(), 1);
        assert!(observed.ext()["warnings"].get("fake_ccc").is_none());
    }

    #[test]
    fn privacy_gate_skips_fetch_denied_bidder_fpd_before_projecting_allowed_sibling() {
        let mut harness = Harness::default();
        harness.activities(json!({"fetchBids": {"rules": [
            {"condition": {"componentName": ["fake_ax"]}, "allow": false}
        ]}}));
        let mut req = request(&["fake_ax", "fake_ccc"]);
        req["ext"]["prebid"]["aliases"] = json!({"fake_ax": "fake_bb"});
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_ax": {"banner": null}}});
        second_imp(&mut req);
        req["imp"][1]["ext"]["bidder"] = json!({"fake_ax": {}});

        let observed = harness.run(req);
        assert!(
            observed.result.is_ok(),
            "fetch-denied FPD must not abort the allowed sibling: {:?}",
            observed.result
        );
        observed.assert_built(&["fake_ccc"]);
        observed.assert_blocked("fake_ax", &["imp-1", "imp-2"]);
        assert_eq!(observed.outbound.len(), 1);
        let body = observed.body("fake_ccc");
        assert_eq!(body["imp"].as_array().unwrap().len(), 1);
        assert_eq!(body["imp"][0]["id"], "imp-1");
        unchanged(&body, &observed.original, &["/imp/0/banner"]);
        assert_eq!(observed.response().seatbid.len(), 1);
        assert_eq!(observed.response().seatbid[0].seat, "fake_ccc");
        assert_eq!(observed.response().seatbid[0].bid[0].impid, "imp-1");
    }

    #[test]
    fn privacy_gate_skips_gdpr_denied_bidder_fpd_before_projecting_allowed_sibling() {
        let mut harness = Harness::default();
        harness.enforce_gdpr();
        let mut req = request(&["fake_bb", "fake_ax"]);
        req["ext"]["prebid"]["aliases"] = json!({"fake_ax": "fake_bb"});
        req["ext"]["prebid"]["aliasgvlids"] = json!({"fake_ax": 8_i32});
        req["regs"]["gdpr"] = json!(1_i32);
        req["user"]["consent"] = json!(CONSENT.trim());
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": {"banner": null}}});
        second_imp(&mut req);
        req["imp"][1]["ext"]["bidder"] = json!({"fake_bb": {}});

        let observed = harness.run(req);
        assert!(
            observed.result.is_ok(),
            "GDPR-denied FPD must not abort the allowed sibling: {:?}",
            observed.result
        );
        observed.assert_built(&["fake_ax"]);
        observed.assert_blocked("fake_bb", &["imp-1", "imp-2"]);
        assert_eq!(observed.outbound.len(), 1);
        let body = observed.body("fake_ax");
        assert_eq!(body["imp"].as_array().unwrap().len(), 1);
        assert_eq!(body["imp"][0]["id"], "imp-1");
        unchanged(&body, &observed.original, &["/imp/0/banner"]);
        assert_eq!(observed.response().seatbid.len(), 1);
        assert_eq!(observed.response().seatbid[0].seat, "fake_ax");
        assert_eq!(observed.response().seatbid[0].bid[0].impid, "imp-1");
    }

    #[test]
    fn privacy_gate_keeps_allowed_bidder_malformed_fpd_fatal() {
        let harness = Harness::default();
        let mut req = request(&["fake_bb", "fake_ccc"]);
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": {"banner": null}}});
        let observed = harness.run(req);
        assert!(
            matches!(
                &observed.result,
                Err(AuctionError::InvalidImpFirstPartyData(_))
            ),
            "allowed bidder FPD still requires validation: {:?}",
            observed.result
        );
        observed.assert_built(&[]);
        assert!(observed.outbound.is_empty());
        assert!(observed.response_hooks.is_empty());
    }

    #[test]
    fn activity_uses_requested_alias_identity_and_simple_case_folding() {
        for (rule_name, denied, allowed) in [
            ("fake_bb", "fake_bb", "fake_ax"),
            ("fake_AX", "fake_ax", "fake_bb"),
            ("fake_k", "fake_\u{212a}", "fake_bb"),
        ] {
            let mut harness = Harness::default();
            harness.activities(json!({"fetchBids": {"rules": [
                {"condition": {"componentName": [rule_name]}, "allow": false}
            ]}}));
            let alias = if denied == "fake_\u{212a}" {
                denied
            } else {
                "fake_ax"
            };
            let mut req = request(&["fake_bb", alias]);
            req["ext"]["prebid"]["aliases"] = json!({(alias): "fake_bb"});
            let observed = harness.run(req);
            observed.assert_built(&[allowed]);
            observed.assert_blocked(denied, &["imp-1"]);
            assert_eq!(observed.constructions[0].core, "fake_bb");
            assert_eq!(observed.response().seatbid[0].seat, allowed);
        }
    }

    #[test]
    fn no_sale_matches_exact_requested_identity_without_core_fallback() {
        for usp in ["1YYN", "private-malformed"] {
            for (names, exempt) in [
                (json!([]), vec![]),
                (json!(["fake_bb"]), vec!["fake_bb"]),
                (json!(["fake_ax", "fake_ax"]), vec!["fake_ax"]),
                (json!(["*"]), vec!["fake_bb", "fake_ax"]),
            ] {
                let mut harness = Harness::default();
                harness.host.ccpa.enforce = true;
                let mut req = request(&["fake_bb", "fake_ax"]);
                req["ext"]["prebid"]["aliases"] = json!({"fake_ax": "fake_bb"});
                req["ext"]["prebid"]["nosale"] = names;
                req["regs"]["us_privacy"] = json!(usp);
                let observed = harness.run(req);
                observed.assert_built(&["fake_bb", "fake_ax"]);
                for bidder in ["fake_bb", "fake_ax"] {
                    let body = observed.body(bidder);
                    if exempt.contains(&bidder) {
                        unchanged(&body, &observed.original, &["/user", "/device"]);
                    } else {
                        absent(&body, ID_PATHS);
                        coarse_geo(&body);
                    }
                    assert_user_fpd(&body, &observed.original, !exempt.contains(&bidder));
                }
            }
        }
    }

    #[test]
    fn no_sale_accepts_registered_and_resolved_names_not_participating_in_auction() {
        for ccpa in [false, true] {
            let mut harness = Harness::default();
            harness.host.ccpa.enforce = ccpa;
            harness
                .host
                .aliases
                .insert("fake_host_alias".into(), "fake_bb".into());
            let mut req = request(&["fake_ccc"]);
            req["ext"]["prebid"]["aliases"] = json!({"fake_request_alias": "fake_bb"});
            req["ext"]["prebid"]["nosale"] =
                json!(["fake_aaa", "fake_host_alias", "fake_request_alias"]);
            req["regs"]["us_privacy"] = json!("1YYN");
            let observed = harness.run(req);
            observed.assert_built(&["fake_ccc"]);
            assert_eq!(observed.response().seatbid.len(), 1);
            if ccpa {
                absent(&observed.body("fake_ccc"), ID_PATHS);
            } else {
                unchanged(&observed.body("fake_ccc"), &observed.original, ID_PATHS);
            }
        }
    }

    #[test]
    fn no_sale_accepts_aliases_dropped_for_disablement_with_allowed_sibling() {
        for ccpa in [false, true] {
            for (host_alias, disable_base, baked) in [
                (false, false, false),
                (false, true, false),
                (true, false, false),
                (true, true, false),
                (true, true, true),
            ] {
                let mut harness = Harness::default();
                harness.host.ccpa.enforce = ccpa;
                if baked {
                    harness.baked_disabled.insert("fake_bb".into());
                } else {
                    harness.disabled.insert(BidderName::new(if disable_base {
                        "fake_bb"
                    } else {
                        "fake_ax"
                    }));
                }
                let mut req = request(&["fake_ax", "fake_ccc"]);
                if host_alias {
                    harness
                        .host
                        .aliases
                        .insert("fake_ax".into(), "fake_bb".into());
                } else {
                    req["ext"]["prebid"]["aliases"] = json!({"fake_ax": "fake_bb"});
                }
                req["ext"]["prebid"]["nosale"] = json!(["fake_ax"]);
                req["regs"]["us_privacy"] = json!("1YYN");
                let observed = harness.run(req);
                observed.assert_built(&["fake_ccc"]);
                assert_eq!(observed.outbound.len(), 1);
                assert_eq!(observed.response().seatbid[0].seat, "fake_ccc");
                assert!(
                    observed.ext()["warnings"]["fake_ax"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|warning| warning["code"] == 6_i32)
                );
                if ccpa {
                    absent(&observed.body("fake_ccc"), ID_PATHS);
                }
            }
        }
    }

    #[test]
    fn invalid_no_sale_is_bad_input_before_disabled_policy_and_invalid_usp_shortcuts() {
        for nsp in [false, true] {
            for names in [
                json!(["fake_unknown_secret"]),
                json!(["fake_Bb"]),
                json!(["*", "fake_bb"]),
                json!(["fake_bad_host_alias"]),
            ] {
                let mut harness = Harness::default();
                if nsp {
                    harness.enforce_gdpr();
                    harness.host.gdpr.non_standard_publishers = vec![ACCOUNT_ID.into()];
                }
                harness
                    .host
                    .aliases
                    .insert("fake_bad_host_alias".into(), "fake_not_registered".into());
                let mut req = request(&["fake_bb"]);
                req["regs"] = json!({"gdpr": 1_i32, "us_privacy": "invalid-private-usp"});
                req["ext"]["prebid"]["nosale"] = names;
                harness
                    .run(req)
                    .assert_bad_input("ext.prebid.nosale contains an invalid bidder exemption");
            }
        }
    }

    #[test]
    fn gdpr_off_and_nsp_preserve_each_independent_policy() {
        for nsp in [false, true] {
            for policy in [
                "ccpa",
                "lmt",
                "coppa",
                "transmitUfpd",
                "transmitPreciseGeo",
                "transmitTid",
                "fetchBids",
            ] {
                let mut harness = Harness::default();
                if nsp {
                    harness.enforce_gdpr();
                    harness.host.gdpr.non_standard_publishers = vec![ACCOUNT_ID.into()];
                }
                let mut req = request(&["fake_bb"]);
                req["regs"]["gdpr"] = json!(1_i32);
                req["user"]["consent"] = json!("inactive-invalid-tcf");
                match policy {
                    "ccpa" => {
                        harness.host.ccpa.enforce = true;
                        req["regs"]["us_privacy"] = json!("1YYN");
                    }
                    "lmt" => {
                        harness.host.lmt.enforce = true;
                        req["device"]["lmt"] = json!(1_i32);
                    }
                    "coppa" => req["regs"]["coppa"] = json!(1_i32),
                    activity => harness.activities(json!({(activity): {"default": false}})),
                }
                let observed = harness.run(req);
                assert!(
                    observed.warnings().is_empty(),
                    "inactive GDPR must not parse TCF: {policy}, NSP={nsp}"
                );
                if policy == "fetchBids" {
                    observed.assert_blocked("fake_bb", &["imp-1"]);
                    continue;
                }
                observed.assert_built(&["fake_bb"]);
                let body = observed.body("fake_bb");
                match policy {
                    "ccpa" | "lmt" => {
                        absent(&body, ID_PATHS);
                        coarse_geo(&body);
                    }
                    "coppa" => {
                        absent(&body, ID_PATHS);
                        absent(&body, FPD_PATHS);
                        assert_eq!(body["user"]["geo"], json!({}));
                        assert_eq!(body["device"]["geo"], json!({}));
                    }
                    "transmitUfpd" => {
                        absent(&body, ID_PATHS);
                        absent(&body, FPD_PATHS);
                    }
                    "transmitPreciseGeo" => {
                        coarse_geo(&body);
                        unchanged(&body, &observed.original, IDENTIFIER_PATHS);
                        absent(&body, UNKNOWN_PATHS);
                    }
                    "transmitTid" => absent(&body, &["/source/tid", "/imp/0/ext/tid"]),
                    other => panic!("unexpected independent policy: {other}"),
                }
                if !matches!(policy, "transmitUfpd" | "coppa") {
                    assert_user_fpd(
                        &body,
                        &observed.original,
                        matches!(policy, "ccpa" | "lmt" | "transmitPreciseGeo"),
                    );
                }
            }
        }
    }

    #[test]
    fn no_sale_exemption_never_overrides_other_policies() {
        for policy in ["gdpr", "lmt", "coppa", "fetchBids", "transmitUfpd"] {
            let mut harness = Harness::default();
            harness.host.ccpa.enforce = true;
            let mut req = request(&["fake_bb"]);
            req["regs"]["us_privacy"] = json!("1YYN");
            req["ext"]["prebid"]["nosale"] = json!(["*"]);
            match policy {
                "gdpr" => {
                    harness.enforce_gdpr();
                    req["regs"]["gdpr"] = json!(1_i32);
                }
                "lmt" => {
                    harness.host.lmt.enforce = true;
                    req["device"]["lmt"] = json!(1_i32);
                }
                "coppa" => req["regs"]["coppa"] = json!(1_i32),
                activity => harness.activities(json!({(activity): {"default": false}})),
            }
            let observed = harness.run(req);
            if matches!(policy, "gdpr" | "fetchBids") {
                observed.assert_blocked("fake_bb", &["imp-1"]);
            } else {
                observed.assert_built(&["fake_bb"]);
                let body = observed.body("fake_bb");
                absent(&body, ID_PATHS);
                if policy == "coppa" {
                    assert_eq!(body["user"]["geo"], json!({}));
                    assert_eq!(body["device"]["geo"], json!({}));
                }
                if policy == "lmt" {
                    coarse_geo(&body);
                }
                if matches!(policy, "transmitUfpd" | "coppa") {
                    absent(&body, FPD_PATHS);
                }
            }
        }
    }

    #[test]
    fn ccpa_enablement_uses_inventory_channel_account_host_precedence() {
        for usp in ["1YYN", "private-malformed"] {
            for inventory in ["site", "app", "dooh"] {
                for (channel, account, host, scrub) in [
                    (TriState::Disabled, TriState::Enabled, true, false),
                    (TriState::Enabled, TriState::Disabled, false, true),
                    (TriState::Unset, TriState::Disabled, true, false),
                    (TriState::Unset, TriState::Enabled, false, true),
                    (TriState::Unset, TriState::Unset, true, true),
                    (TriState::Unset, TriState::Unset, false, false),
                ] {
                    let mut harness = Harness::default();
                    harness.host.ccpa.enforce = host;
                    harness.account.ccpa.enabled = account;
                    harness.account.ccpa.channel_enabled.video = TriState::Disabled;
                    match inventory {
                        "app" => harness.account.ccpa.channel_enabled.app = channel,
                        "dooh" => harness.account.ccpa.channel_enabled.dooh = channel,
                        _ => harness.account.ccpa.channel_enabled.web = channel,
                    }
                    let mut req = request(&["fake_bb"]);
                    req.as_object_mut().unwrap().remove("site");
                    req[inventory] =
                        json!({"id": "inventory-context", "ext": {"data": {"section": "news"}}});
                    req["ext"]["prebid"]["channel"] = json!({"name": "video"});
                    req["regs"]["us_privacy"] = json!(usp);
                    let observed = harness.run(req);
                    observed.assert_built(&["fake_bb"]);
                    let body = observed.body("fake_bb");
                    if scrub {
                        absent(&body, ID_PATHS);
                        coarse_geo(&body);
                    } else {
                        unchanged(&body, &observed.original, &["/user", "/device"]);
                    }
                    assert_eq!(body[inventory]["ext"]["data"], json!({"section": "news"}));
                }
            }
        }
    }

    #[test]
    fn all_transmission_scrubs_follow_fpd_projection_and_leave_sibling_and_original_intact() {
        for restricted in ["fake_a", "fake_z"] {
            let mut harness = Harness::default();
            let deny =
                json!({"rules": [{"condition": {"componentName": [restricted]}, "allow": false}]});
            harness.activities(
                json!({"transmitUfpd": deny, "transmitPreciseGeo": deny, "transmitTid": deny}),
            );
            let mut req = request(&["fake_a", "fake_z"]);
            req["ext"]["prebid"]["aliases"] = json!({"fake_a": "fake_bb", "fake_z": "fake_bb"});
            second_imp(&mut req);
            for imp in req["imp"].as_array_mut().unwrap() {
                imp["ext"]["prebid"] = json!({"imp": {
                    "fake_a": {"ext": {"tid": "projected-fake_a-tid", "data": {"projection": "fake_a"}}},
                    "fake_z": {"ext": {"tid": "projected-fake_z-tid", "data": {"projection": "fake_z"}}}
                }});
            }
            let observed = harness.run(req);
            observed.assert_built(&["fake_a", "fake_z"]);
            let body = observed.body(restricted);
            absent(&body, ID_PATHS);
            absent(&body, FPD_PATHS);
            absent(&body, &["/source/tid", "/imp/0/ext/tid", "/imp/1/ext/tid"]);
            coarse_geo(&body);
            unchanged(
                &body,
                &observed.original,
                &[
                    "/id",
                    "/imp/0/id",
                    "/imp/1/id",
                    "/site/ext/data",
                    "/source/ext/schain",
                ],
            );
            absent(&body, &["/user/ext/context", "/device/ext/context"]);
            for imp in body["imp"].as_array().unwrap() {
                assert_eq!(imp["ext"]["data"]["projection"], restricted);
                assert_eq!(imp["ext"]["data"]["slot"], "top");
            }
            let input = &observed
                .constructions
                .iter()
                .find(|build| build.bidder == restricted)
                .unwrap()
                .input;
            absent(input, ID_PATHS);
            absent(input, FPD_PATHS);
            absent(input, &["/source/tid", "/imp/0/ext/tid", "/imp/1/ext/tid"]);
            let sibling = if restricted == "fake_a" {
                "fake_z"
            } else {
                "fake_a"
            };
            let sibling_body = observed.body(sibling);
            unchanged(
                &sibling_body,
                &observed.original,
                &["/user", "/device", "/source"],
            );
            for imp in sibling_body["imp"].as_array().unwrap() {
                assert_eq!(imp["ext"]["tid"], format!("projected-{sibling}-tid"));
            }
        }
    }

    fn response_hook_request(coppa: bool) -> Value {
        let mut req = request(&["fake_legacy"]);
        req.as_object_mut().unwrap().remove("tmax");
        req["ext"]["prebid"]["aliases"] =
            json!({"fake_legacy": "fake_bb", "fake_capable": "fake_ccc"});
        second_imp(&mut req);
        let mut sibling_imp = req["imp"][0].clone();
        sibling_imp["id"] = json!("sibling-only-imp");
        sibling_imp["ext"]["bidder"] = json!({"fake_capable": {}});
        req["imp"].as_array_mut().unwrap().push(sibling_imp);
        for imp in req["imp"].as_array_mut().unwrap() {
            let owner = if imp["id"] == "sibling-only-imp" {
                "fake_capable"
            } else {
                "fake_legacy"
            };
            imp["ext"]["prebid"] = json!({"imp": {(owner): {
                "ext": {"tid": "projected-private-tid", "data": {"owner": owner}}
            }}});
        }
        req["regs"] = json!({
            "gpp": "DBACNY~opaque-tcf~1YYN", "gpp_sid": [2_i32,6_i32],
            "us_privacy": "invalid-private-usp", "coppa": i32::from(coppa)
        });
        req
    }

    #[test]
    fn response_hook_receives_the_same_sanitized_projected_request_as_construction() {
        for (coppa, multi_request) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut harness = Harness {
                multi_request,
                ..Harness::default()
            };
            harness.host.resource_limits.default_tmax_ms = 733;
            harness.host.ccpa.enforce = true;
            harness.activities(json!({
                "transmitUfpd": {"default": false},
                "transmitPreciseGeo": {"default": false},
                "transmitTid": {"default": false}
            }));
            let observed = harness.run(response_hook_request(coppa));
            observed.assert_built(&["fake_legacy", "fake_capable"]);
            assert_eq!(observed.response().seatbid.len(), 2);
            assert_eq!(observed.outbound.len(), if multi_request { 3 } else { 2 });
            assert_eq!(observed.response_hooks.len(), observed.outbound.len());
            absent(
                &observed.original,
                &["/tmax", "/user/consent", "/regs/gdpr"],
            );
            for build in &observed.constructions {
                let input = &build.input;
                absent(input, ID_PATHS);
                absent(input, FPD_PATHS);
                absent(input, &["/source/tid", "/imp/0/ext/tid", "/imp/1/ext/tid"]);
                assert_eq!(input["tmax"], 733_i64);
                if coppa {
                    assert_eq!(input["user"]["geo"], json!({}));
                    assert_eq!(input["device"]["geo"], json!({}));
                } else {
                    coarse_geo(input);
                }
                let expected_ids = if build.bidder == "fake_legacy" {
                    assert_eq!(input["user"]["consent"], "opaque-tcf");
                    assert_eq!(input["regs"]["gdpr"], 1_i32);
                    assert_eq!(input["regs"]["us_privacy"], "1YYN");
                    vec!["imp-1", "imp-2"]
                } else {
                    absent(input, &["/user/consent", "/regs/gdpr", "/regs/us_privacy"]);
                    vec!["sibling-only-imp"]
                };
                let imps = input["imp"].as_array().unwrap();
                let actual_ids: Vec<&str> =
                    imps.iter().map(|imp| imp["id"].as_str().unwrap()).collect();
                assert_eq!(
                    actual_ids, expected_ids,
                    "only this bidder's impressions reach either hook"
                );
                for imp in imps {
                    assert_eq!(imp["ext"]["data"]["owner"], build.bidder);
                    absent(imp, &["/ext/prebid/imp", "/ext/prebid/bidder"]);
                }
                unchanged(
                    input,
                    &observed.original,
                    &["/regs/gpp", "/regs/gpp_sid", "/site"],
                );
                let hooks: Vec<_> = observed
                    .response_hooks
                    .iter()
                    .filter(|hook| hook.bidder == build.bidder)
                    .collect();
                assert_eq!(
                    hooks.len(),
                    if multi_request { expected_ids.len() } else { 1 }
                );
                let outgoing = observed.requests(&build.bidder);
                assert_eq!(hooks.len(), outgoing.len());
                for (hook, outbound) in hooks.into_iter().zip(outgoing) {
                    assert_eq!(
                        hook.covered_imp_ids,
                        outbound.covered_imp_ids().collect::<Vec<_>>(),
                        "each response hook must receive its own outbound request coverage"
                    );
                    assert_eq!(
                        &hook.input, input,
                        "make_bids must receive the same bidder-local request as make_requests: {}, COPPA={coppa}, multi={multi_request}",
                        build.bidder
                    );
                }
            }
            no_raw_ips(&observed.outbound);
        }
    }

    #[test]
    fn ufpd_denial_composes_with_ccpa_lmt_and_coppa_instead_of_skipping_geo_scrubs() {
        for policy in ["ccpa", "lmt", "coppa"] {
            let mut harness = Harness::default();
            harness.activities(
                json!({"transmitUfpd": {"default": false}, "transmitTid": {"default": false}}),
            );
            let mut req = request(&["fake_bb"]);
            match policy {
                "ccpa" => {
                    harness.host.ccpa.enforce = true;
                    req["regs"]["us_privacy"] = json!("1YYN");
                }
                "lmt" => {
                    harness.host.lmt.enforce = true;
                    req["device"]["lmt"] = json!(1_i32);
                }
                _ => req["regs"]["coppa"] = json!(1_i32),
            }
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb"]);
            let body = observed.body("fake_bb");
            absent(&body, ID_PATHS);
            absent(&body, FPD_PATHS);
            absent(&body, &["/source/tid", "/imp/0/ext/tid"]);
            if policy == "coppa" {
                assert_eq!(body["user"]["geo"], json!({}));
                assert_eq!(body["device"]["geo"], json!({}));
            } else {
                coarse_geo(&body);
            }
            no_raw_ips(&observed.outbound);
        }
    }

    #[test]
    fn coppa_clears_full_existing_geo_but_does_not_create_missing_objects() {
        for missing_user in [false, true] {
            let harness = Harness::default();
            let mut req = request(&["fake_bb"]);
            req["regs"]["coppa"] = json!(1_i32);
            if missing_user {
                req.as_object_mut().unwrap().remove("user");
                req.as_object_mut().unwrap().remove("source");
                req["device"].as_object_mut().unwrap().remove("geo");
            }
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb"]);
            let body = observed.body("fake_bb");
            absent(&body, ID_PATHS);
            if missing_user {
                absent(&body, &["/user", "/source", "/device/geo"]);
            } else {
                assert_eq!(body["user"]["geo"], json!({}));
                assert_eq!(body["device"]["geo"], json!({}));
                absent(&body, FPD_PATHS);
                absent(&body, &["/user/ext/context"]);
            }
            no_raw_ips(&observed.outbound);
        }
    }

    #[test]
    fn header_filled_ip_is_masked_before_adapter_and_never_restored_in_outbound_headers() {
        for raw in [CLIENT_V4, CLIENT_V6] {
            let mut harness = Harness::default();
            harness.activities(json!({"transmitPreciseGeo": {"default": false}}));
            harness.headers.true_client_ip = Some(raw);
            harness.headers.x_forwarded_for = Some(raw);
            harness.headers.x_real_ip = Some(raw);
            harness.headers.remote_addr = Some(raw);
            harness.headers.sec_gpc = Some("1");
            let mut req = request(&["fake_bb"]);
            req["device"].as_object_mut().unwrap().remove("ip");
            req["device"].as_object_mut().unwrap().remove("ipv6");
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb"]);
            let field = if raw == CLIENT_V4 { "ip" } else { "ipv6" };
            let expected = if raw == CLIENT_V4 {
                "8.8.4.0"
            } else {
                "2001:4860:4860::"
            };
            assert_eq!(
                observed.original["device"][field], raw,
                "prepare must actually fill the raw header IP"
            );
            assert_eq!(observed.body("fake_bb")["device"][field], expected);
            assert_eq!(observed.constructions[0].input["device"][field], expected);
            no_raw_ips(&observed.outbound);
            assert!(
                observed.outbound[0]
                    .headers()
                    .any(|(name, value)| name.eq_ignore_ascii_case("Sec-GPC") && value == "1")
            );
        }
    }

    #[test]
    fn precise_geo_denial_uses_account_keep_bits_without_removing_ids_fpd_or_tids() {
        let mut harness = Harness::default();
        harness.account.privacy.ipv4.anon_keep_bits = 16;
        harness.account.privacy.ipv6.anon_keep_bits = 32;
        harness.activities(json!({"transmitPreciseGeo": {"default": false}}));
        let observed = harness.run(request(&["fake_bb"]));
        observed.assert_built(&["fake_bb"]);
        let body = observed.body("fake_bb");
        assert_eq!(body["device"]["ip"], "8.8.0.0");
        assert_eq!(body["device"]["ipv6"], "2001:4860::");
        for owner in ["user", "device"] {
            assert_eq!(body[owner]["geo"]["lat"], json!(48.86_f64));
            assert_eq!(body[owner]["geo"]["lon"], json!(-1.12_f64));
            assert_eq!(body[owner]["geo"]["country"], "USA");
        }
        unchanged(&body, &observed.original, IDENTIFIER_PATHS);
        absent(&body, UNKNOWN_PATHS);
        unchanged(&body, &observed.original, FPD_PATHS);
        unchanged(
            &body,
            &observed.original,
            &["/source/tid", "/imp/0/ext/tid"],
        );
        no_raw_ips(&observed.outbound);
    }

    #[test]
    fn every_outbound_request_from_a_multi_request_adapter_obeys_the_same_privacy_decision() {
        let mut harness = Harness {
            multi_request: true,
            ..Harness::default()
        };
        harness.host.ccpa.enforce = true;
        harness.activities(
            json!({"transmitUfpd": {"default": false}, "transmitTid": {"default": false}}),
        );
        let mut req = request(&["fake_bb"]);
        req["regs"]["us_privacy"] = json!("1YYN");
        second_imp(&mut req);
        let observed = harness.run(req);
        observed.assert_built(&["fake_bb"]);
        assert_eq!(observed.outbound.len(), 2);
        for (idx, outbound) in observed.outbound.iter().enumerate() {
            let body: Value = serde_json::from_slice(outbound.body()).unwrap();
            absent(&body, ID_PATHS);
            absent(&body, FPD_PATHS);
            absent(&body, &["/source/tid", "/imp/0/ext/tid"]);
            coarse_geo(&body);
            assert_eq!(body["imp"].as_array().unwrap().len(), 1);
            assert_eq!(body["imp"][0]["id"], format!("imp-{}", idx + 1));
        }
        no_raw_ips(&observed.outbound);
        assert_eq!(observed.response().seatbid[0].bid.len(), 2);
    }

    #[test]
    fn att_mapping_is_app_only_case_insensitive_and_precedes_lmt_enforcement() {
        for (app, os, att, supplied_lmt, expected_lmt, enforce, scrub) in [
            (true, "iOS", Some(0_i32), 0_i32, 1_i32, true, true),
            (true, "IOS", Some(1_i32), 0_i32, 1_i32, true, true),
            (true, "iOs", Some(2_i32), 0_i32, 1_i32, true, true),
            (true, "ios", Some(3_i32), 1_i32, 0_i32, true, false),
            (true, "ios", Some(4_i32), 1_i32, 1_i32, true, true),
            (true, "ios", Some(4_i32), 0_i32, 0_i32, true, false),
            (true, "ios", None, 0_i32, 0_i32, true, false),
            (true, "ios", None, 1_i32, 1_i32, true, true),
            (true, "Android", Some(1_i32), 0_i32, 0_i32, true, false),
            (false, "iOS", Some(1_i32), 0_i32, 0_i32, true, false),
            (false, "iOS", Some(3_i32), 1_i32, 1_i32, true, true),
            (true, "iOS", Some(1_i32), 0_i32, 1_i32, false, false),
        ] {
            let mut harness = Harness::default();
            harness.host.lmt.enforce = enforce;
            let mut req = request(&["fake_bb"]);
            if app {
                req.as_object_mut().unwrap().remove("site");
                req["app"] = json!({"id": "app-context", "ext": {"data": {"genre": "news"}}});
            }
            req["device"]["os"] = json!(os);
            req["device"]["lmt"] = json!(supplied_lmt);
            if let Some(status) = att {
                req["device"]["ext"]["atts"] = json!(status);
            }
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb"]);
            let body = observed.body("fake_bb");
            assert_eq!(
                body["device"]["lmt"], expected_lmt,
                "app={app}, os={os}, ATT={att:?}"
            );
            if scrub {
                absent(&body, ID_PATHS);
                coarse_geo(&body);
            } else {
                unchanged(&body, &observed.original, ID_PATHS);
            }
            assert_user_fpd(&body, &observed.original, scrub);
            if app {
                assert_eq!(body["app"]["ext"]["data"], json!({"genre": "news"}));
            }
        }
    }

    #[test]
    fn legacy_projection_uses_resolved_bidder_metadata_and_never_overwrites_populated_values() {
        for legacy in ["missing", "empty", "populated"] {
            let harness = Harness::default();
            let mut req = request(&["fake_bb", "fake_ccc", "fake_gpp_alias"]);
            req["ext"]["prebid"]["aliases"] = json!({"fake_gpp_alias": "fake_ccc"});
            // Keep TCF authorization disabled: raw consent backfill must not parse it.
            let gpp = "DBADNbA~opaque-selected-tcf~1YNN~opaque-unused-payload!";
            let sid = json!([7_i32, 6_i32, 2_i32, 6_i32, 999_i32]);
            req["regs"]["gpp"] = json!(gpp);
            req["regs"]["gpp_sid"] = sid.clone();
            match legacy {
                "empty" => {
                    req["user"]["consent"] = json!("");
                    req["regs"]["us_privacy"] = json!("");
                }
                "populated" => {
                    req["regs"]["gdpr"] = json!(0_i32);
                    req["user"]["consent"] = json!("existing-legacy-consent");
                    req["regs"]["us_privacy"] = json!("1NNY");
                }
                _ => {}
            }
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc", "fake_gpp_alias"]);
            let legacy_body = observed.body("fake_bb");
            let populated = legacy == "populated";
            assert_eq!(legacy_body["regs"]["gdpr"], i32::from(!populated));
            assert_eq!(
                legacy_body["user"]["consent"],
                if populated {
                    "existing-legacy-consent"
                } else {
                    "opaque-selected-tcf"
                }
            );
            assert_eq!(
                legacy_body["regs"]["us_privacy"],
                if populated { "1NNY" } else { "1YNN" }
            );
            for bidder in ["fake_bb", "fake_ccc", "fake_gpp_alias"] {
                let body = observed.body(bidder);
                assert_eq!(body["regs"]["gpp"], gpp);
                assert_eq!(
                    body["regs"]["gpp_sid"], sid,
                    "wire SID order, duplicates, and unknown IDs survive"
                );
                unchanged(&body, &observed.original, ID_PATHS);
                if bidder != "fake_bb" {
                    for path in ["/regs/gdpr", "/user/consent", "/regs/us_privacy"] {
                        assert_eq!(
                            body.pointer(path),
                            observed.original.pointer(path),
                            "GPP-capable {bidder} does not downgrade {path}"
                        );
                    }
                }
            }
            if legacy != "populated" {
                assert!(
                    observed.warnings().is_empty(),
                    "inactive TCF and unused section payload are not authorization errors"
                );
            }
        }
    }

    #[test]
    fn legacy_backfill_after_scrubbing_adds_only_privacy_signals_including_when_user_was_absent() {
        for missing_user in [false, true] {
            let mut harness = Harness::default();
            harness.activities(
                json!({"transmitUfpd": {"default": false}, "transmitTid": {"default": false}}),
            );
            let mut req = request(&["fake_bb", "fake_ccc"]);
            if missing_user {
                req.as_object_mut().unwrap().remove("user");
            }
            req["regs"] =
                json!({"coppa": 1_i32, "gpp": "DBACNY~opaque-tcf~1YYN", "gpp_sid": [2_i32,6_i32]});
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            for bidder in ["fake_bb", "fake_ccc"] {
                let body = observed.body(bidder);
                absent(&body, ID_PATHS);
                absent(&body, FPD_PATHS);
                absent(&body, &["/source/tid", "/imp/0/ext/tid"]);
                assert_eq!(body["device"]["geo"], json!({}));
                if bidder == "fake_bb" {
                    assert_eq!(body["user"]["consent"], "opaque-tcf");
                    assert_eq!(body["regs"]["us_privacy"], "1YYN");
                    assert_eq!(body["regs"]["gdpr"], 1_i32);
                } else {
                    absent(&body, &["/user/consent", "/regs/us_privacy", "/regs/gdpr"]);
                }
                if missing_user {
                    absent(&body, &["/user/geo"]);
                    if bidder == "fake_ccc" {
                        absent(&body, &["/user"]);
                    }
                } else {
                    assert_eq!(body["user"]["geo"], json!({}));
                }
                unchanged(&body, &observed.original, &["/regs/gpp", "/regs/gpp_sid"]);
            }
            no_raw_ips(&observed.outbound);
        }
    }

    #[test]
    fn missing_empty_and_inactive_sid_lists_do_not_backfill_inactive_sections() {
        for (sid, expected_gdpr, expected_usp) in [
            (None, None, None),
            (Some(json!([])), Some(0_i32), None),
            (Some(json!([7_i32, 999_i32])), Some(0_i32), None),
            (Some(json!([6_i32])), Some(0_i32), Some("1YNN")),
        ] {
            let harness = Harness::default();
            let mut req = request(&["fake_bb", "fake_ccc"]);
            req["regs"]["gpp"] = json!("DBACNY~opaque-tcf~1YNN");
            if let Some(wire_sid) = &sid {
                req["regs"]["gpp_sid"] = wire_sid.clone();
            }
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            let body = observed.body("fake_bb");
            assert_eq!(
                body["regs"].get("gdpr"),
                expected_gdpr.map(|value| json!(value)).as_ref()
            );
            assert_eq!(
                body["regs"].get("us_privacy"),
                expected_usp.map(|value| json!(value)).as_ref()
            );
            absent(&body, &["/user/consent"]);
            let capable = observed.body("fake_ccc");
            absent(
                &capable,
                &["/regs/gdpr", "/user/consent", "/regs/us_privacy"],
            );
            for outbound_body in [&body, &capable] {
                assert_eq!(outbound_body["regs"].get("gpp_sid"), sid.as_ref());
                assert_eq!(
                    outbound_body["regs"]["gpp"],
                    observed.original["regs"]["gpp"]
                );
            }
            assert!(observed.warnings().is_empty());
        }
    }

    #[test]
    fn empty_sid_retains_legacy_gdpr_scope_while_downgrading_missing_gdpr_to_zero() {
        let mut harness = Harness::default();
        harness.enforce_gdpr();
        let mut req = request(&["fake_bb"]);
        req["regs"] = json!({"gdpr": 1_i32, "gpp_sid": [], "gpp": "DBABM~"});
        let denied = harness.run(req.clone());
        denied.assert_blocked("fake_bb", &["imp-1"]);

        req["regs"].as_object_mut().unwrap().remove("gdpr");
        let allowed = harness.run(req);
        allowed.assert_built(&["fake_bb"]);
        assert_eq!(allowed.body("fake_bb")["regs"]["gdpr"], 0_i32);
        unchanged(&allowed.body("fake_bb"), &allowed.original, ID_PATHS);
    }

    #[test]
    fn malformed_selected_usp_scrub_obeys_enablement_and_exact_alias_exemptions() {
        for regs in [
            json!({"us_privacy":"private-malformed"}),
            json!({"gpp":"DBABTA~private-malformed","gpp_sid":[6_i32],"us_privacy":"1YNN"}),
            json!({"gpp":"DBABTA~ ","gpp_sid":[6_i32]}),
        ] {
            for enabled in [false, true] {
                for (names, exempt) in [
                    (json!([]), [false, false]),
                    (json!(["fake_restricted"]), [true, false]),
                    (json!(["*"]), [true, true]),
                ] {
                    let mut harness = Harness::default();
                    harness.host.ccpa.enforce = enabled;
                    let mut req = request(&["fake_restricted", "fake_sibling"]);
                    req["ext"]["prebid"]["aliases"] =
                        json!({"fake_restricted":"fake_bb","fake_sibling":"fake_bb"});
                    req["ext"]["prebid"]["nosale"] = names;
                    req["regs"] = regs.clone();
                    let observed = harness.run(req);
                    observed.assert_built(&["fake_restricted", "fake_sibling"]);
                    for (index, bidder) in
                        ["fake_restricted", "fake_sibling"].into_iter().enumerate()
                    {
                        let body = observed.body(bidder);
                        let restricted = enabled && !exempt[index];
                        if restricted {
                            absent(&body, ID_PATHS);
                            coarse_geo(&body);
                        } else {
                            unchanged(&body, &observed.original, &["/user", "/device"]);
                        }
                        assert_user_fpd(&body, &observed.original, restricted);
                        unchanged(
                            &body,
                            &observed.original,
                            &["/source/tid", "/imp/0/ext/tid"],
                        );
                        if regs.get("gpp").is_some() {
                            unchanged(&body, &observed.original, &["/regs/gpp", "/regs/gpp_sid"]);
                        } else {
                            absent(&body, &["/regs/us_privacy"]);
                        }
                    }
                    assert!(!observed.ext().to_string().contains("private-malformed"));
                    if enabled && exempt == [false, false] {
                        no_raw_ips(&observed.outbound);
                    }
                }
            }
        }
    }

    #[test]
    fn empty_active_usp_falls_back_to_legacy_but_invalid_nonempty_selection_never_does() {
        for (selected, scrub, warning_count) in [
            ("", true, 0),
            (" ", true, 2),
            ("invalid-selected-usp", true, 2),
        ] {
            let mut harness = Harness::default();
            harness.host.ccpa.enforce = true;
            let mut req = request(&["fake_bb", "fake_ccc"]);
            let gpp = format!("DBABTA~{selected}");
            req["regs"] = json!({"gpp": gpp, "gpp_sid": [6_i32], "us_privacy": "1YYN"});
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            for bidder in ["fake_bb", "fake_ccc"] {
                let body = observed.body(bidder);
                assert_eq!(body["regs"]["gpp"], gpp);
                assert_eq!(body["regs"]["gpp_sid"], json!([6_i32]));
                assert_eq!(body["regs"]["us_privacy"], "1YYN");
                if scrub {
                    absent(&body, ID_PATHS);
                    coarse_geo(&body);
                    assert_user_fpd(&body, &observed.original, true);
                } else {
                    unchanged(&body, &observed.original, &["/user", "/device"]);
                }
            }
            assert_eq!(
                observed.warnings().len(),
                warning_count,
                "warnings normalize once, not once per bidder"
            );
            assert!(
                observed
                    .warnings()
                    .iter()
                    .all(|warning| warning["code"] == 10_001_i32)
            );
        }
    }

    #[test]
    fn empty_or_invalid_active_usp_is_never_injected_into_missing_legacy_fields() {
        for selected in ["", " ", "invalid-selected-usp"] {
            let mut harness = Harness::default();
            harness.host.ccpa.enforce = true;
            let mut req = request(&["fake_bb", "fake_ccc"]);
            req["regs"] = json!({"gpp": format!("DBABTA~{selected}"), "gpp_sid": [6_i32]});
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            for bidder in ["fake_bb", "fake_ccc"] {
                let body = observed.body(bidder);
                absent(&body, &["/regs/us_privacy"]);
                unchanged(&body, &observed.original, &["/regs/gpp", "/regs/gpp_sid"]);
                if selected.is_empty() {
                    unchanged(&body, &observed.original, &["/user", "/device"]);
                } else {
                    absent(&body, ID_PATHS);
                    coarse_geo(&body);
                    assert_user_fpd(&body, &observed.original, true);
                }
            }
            assert_eq!(observed.warnings().len(), usize::from(!selected.is_empty()));
        }
    }

    #[test]
    fn active_gpp_usp_drives_scrubbing_even_when_populated_legacy_projection_disagrees() {
        for (selected, legacy, scrub) in [("1YYN", "1YNN", true), ("1YNN", "1YYN", false)] {
            let mut harness = Harness::default();
            harness.host.ccpa.enforce = true;
            let mut req = request(&["fake_bb", "fake_ccc"]);
            req["regs"] = json!({"gpp": format!("DBABTA~{selected}"), "gpp_sid": [6_i32], "us_privacy": legacy});
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            for bidder in ["fake_bb", "fake_ccc"] {
                let body = observed.body(bidder);
                assert_eq!(body["regs"]["us_privacy"], legacy);
                if scrub {
                    absent(&body, ID_PATHS);
                    coarse_geo(&body);
                } else {
                    unchanged(&body, &observed.original, &["/user", "/device"]);
                }
            }
            assert_eq!(observed.warnings().len(), 1);
            assert_eq!(observed.warnings()[0]["code"], 10_001_i32);
        }
    }

    #[test]
    fn invalid_legacy_usp_is_sanitized_even_when_ccpa_is_disabled() {
        for legacy in [
            "1yyN",
            " 1YYN",
            "1YYN ",
            "1YYN-extra",
            "2YYN",
            "private-usp",
        ] {
            let harness = Harness::default();
            let mut req = request(&["fake_bb", "fake_ccc"]);
            req["regs"]["us_privacy"] = json!(legacy);
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            for bidder in ["fake_bb", "fake_ccc"] {
                let body = observed.body(bidder);
                absent(&body, &["/regs/us_privacy"]);
                unchanged(&body, &observed.original, &["/user", "/device"]);
            }
            assert_eq!(
                observed.warnings(),
                vec![json!({"code": 10_001_i32,
            "message": "regs.us_privacy is not a valid USP value; clearing legacy value"})]
            );
            assert!(!observed.ext().to_string().contains(legacy));
        }
    }

    #[test]
    fn legacy_usp_sanitation_precedes_valid_gpp_backfill_for_noncapable_bidder_only() {
        let harness = Harness::default();
        let mut req = request(&["fake_bb", "fake_ccc"]);
        req["regs"] =
            json!({"gpp": "DBABTA~1YYN", "gpp_sid": [6_i32], "us_privacy": "private-invalid-usp"});
        let observed = harness.run(req);
        observed.assert_built(&["fake_bb", "fake_ccc"]);
        assert_eq!(observed.body("fake_bb")["regs"]["us_privacy"], "1YYN");
        absent(&observed.body("fake_ccc"), &["/regs/us_privacy"]);
        assert_eq!(
            observed.warnings().len(),
            2,
            "legacy validation plus source conflict, neither duplicated per bidder"
        );
        for bidder in ["fake_bb", "fake_ccc"] {
            unchanged(
                &observed.body(bidder),
                &observed.original,
                &["/regs/gpp", "/regs/gpp_sid"],
            );
        }
    }

    #[test]
    fn invalid_legacy_with_empty_active_usp_is_cleared_without_conflict_or_fallback_grant() {
        let mut harness = Harness::default();
        harness.host.ccpa.enforce = true;
        let mut req = request(&["fake_bb", "fake_ccc"]);
        req["regs"] = json!({"gpp": "DBABTA~", "gpp_sid": [6_i32], "us_privacy": "bad-legacy-usp"});
        let observed = harness.run(req);
        observed.assert_built(&["fake_bb", "fake_ccc"]);
        for bidder in ["fake_bb", "fake_ccc"] {
            let body = observed.body(bidder);
            absent(&body, &["/regs/us_privacy"]);
            absent(&body, ID_PATHS);
            coarse_geo(&body);
            assert_user_fpd(&body, &observed.original, true);
            assert_eq!(body["regs"]["gpp"], "DBABTA~");
        }
        assert_eq!(observed.warnings().len(), 1);
        assert!(
            observed.warnings()[0]["message"]
                .as_str()
                .unwrap()
                .contains("regs.us_privacy")
        );
    }

    #[test]
    fn selected_tcf_controls_live_fetch_and_authoritative_empty_or_invalid_tcf_cannot_retry_legacy()
    {
        for (gpp, allow) in [
            (format!("DBABM~{}", CONSENT.trim()), true),
            ("DBABM~".into(), false),
            ("DBABM~invalid-selected-tcf".into(), false),
            ("malformed-envelope".into(), true),
            ("DBABTA~1YNN".into(), true),
        ] {
            let mut harness = Harness::default();
            harness.enforce_gdpr();
            let mut req = request(&["fake_ax"]);
            req["ext"]["prebid"]["aliases"] = json!({"fake_ax": "fake_bb"});
            req["ext"]["prebid"]["aliasgvlids"] = json!({"fake_ax": 8_i32});
            req["user"]["consent"] = json!(CONSENT.trim());
            req["regs"] = json!({"gpp": gpp, "gpp_sid": [2_i32]});
            let observed = harness.run(req);
            if allow {
                observed.assert_built(&["fake_ax"]);
                assert_eq!(observed.response().seatbid[0].seat, "fake_ax");
                assert_eq!(observed.body("fake_ax")["user"]["consent"], CONSENT.trim());
                unchanged(&observed.body("fake_ax"), &observed.original, ID_PATHS);
            } else {
                observed.assert_blocked("fake_ax", &["imp-1"]);
                assert!(observed.outbound.is_empty());
            }
            let diagnostics = observed.ext().to_string();
            assert!(!diagnostics.contains(CONSENT.trim()));
            assert!(!diagnostics.contains("invalid-selected-tcf"));
        }
    }

    #[test]
    fn simultaneous_legacy_and_selected_usp_failures_warn_once_in_source_order_without_raw_values()
    {
        let harness = Harness::default();
        let mut req = request(&["fake_bb", "fake_ccc"]);
        req["user"]["consent"] = json!("private-legacy-tcf");
        req["regs"] = json!({"gpp": "DBACNY~private-gpp-tcf~private-gpp-usp",
        "gpp_sid": [2_i32,6_i32], "us_privacy": "private-legacy-usp"});
        let observed = harness.run(req);
        observed.assert_built(&["fake_bb", "fake_ccc"]);
        let expected = [
            "regs.us_privacy is not a valid USP value; clearing legacy value",
            "regs.gpp section 2 and user.consent conflict; using section 2",
            "regs.gpp section 6 and regs.us_privacy conflict; using section 6",
            "regs.gpp section 6 is not a valid USP value; no usable USP",
        ];
        assert_eq!(
            observed.warnings(),
            expected
                .iter()
                .map(|message| json!({"code": 10_001_i32, "message": message}))
                .collect::<Vec<_>>()
        );
        for raw in [
            "private-legacy-tcf",
            "private-gpp-tcf",
            "private-gpp-usp",
            "private-legacy-usp",
            CLIENT_V4,
            CLIENT_V6,
        ] {
            assert!(
                !observed.ext().to_string().contains(raw),
                "diagnostics exposed {raw}"
            );
        }
        for bidder in ["fake_bb", "fake_ccc"] {
            let body = observed.body(bidder);
            absent(&body, &["/regs/us_privacy"]);
            unchanged(
                &body,
                &observed.original,
                &["/user/consent", "/regs/gpp", "/regs/gpp_sid"],
            );
        }
    }

    #[test]
    fn gpp_limits_are_hard_bad_input_with_zero_adapter_calls_even_when_policies_are_disabled() {
        for gpp in [
            format!("DBABM~{}", "x".repeat(0x0003_FFFB)),
            "A".repeat(4097),
            format!("!{}", "~opaque".repeat(65)),
            "DBBB".into(),
        ] {
            for nsp in [false, true] {
                let mut harness = Harness::default();
                if nsp {
                    harness.enforce_gdpr();
                    harness.host.gdpr.non_standard_publishers = vec![ACCOUNT_ID.into()];
                }
                let mut req = request(&["fake_bb", "fake_ccc"]);
                req["user"]["consent"] = json!(CONSENT.trim());
                req["regs"] = json!({"gpp": gpp, "gpp_sid": [999_i32], "us_privacy": "1YNN"});
                harness
                    .run(req)
                    .assert_bad_input("regs.gpp exceeds privacy parsing limits");
            }
        }
    }

    #[test]
    fn adding_a_65th_opaque_section_is_a_hard_error_not_fallback_to_granting_legacy_tcf() {
        let mut harness = Harness::default();
        harness.enforce_gdpr();
        let mut req = request(&["fake_ax"]);
        req["ext"]["prebid"]["aliases"] = json!({"fake_ax": "fake_bb"});
        req["ext"]["prebid"]["aliasgvlids"] = json!({"fake_ax": 8_i32});
        req["user"]["consent"] = json!(CONSENT.trim());
        // Pinned go-gpp header for IDs 1..=64, also used by the envelope qualification fixtures.
        let mut sections = vec!["opaque"; 64];
        sections[1] = ""; // Authoritative empty section 2 denies despite granting legacy TCF.
        let bounded = format!("DBAB4Rg~{}", sections.join("~"));
        req["regs"] = json!({"gpp": bounded, "gpp_sid": [2_i32]});
        harness
            .run(req.clone())
            .assert_blocked("fake_ax", &["imp-1"]);
        req["regs"]["gpp"] = json!(format!("{bounded}~opaque-65"));
        harness
            .run(req)
            .assert_bad_input("regs.gpp exceeds privacy parsing limits");
    }
}
