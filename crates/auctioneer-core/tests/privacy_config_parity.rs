//! ACT-2 parity between baked configuration and runtime overlays.
//! The ignored child probe is executed by auctioneer-config's hermetic build harness
//! with a nondefault TOML fixture compiled into the actual generated Rust.

#[cfg(test)]
mod tests {
    use auctioneer_config::activity::{
        Activity, ActivityConfig, ComponentType, Condition, Rule, validate_activities,
    };
    use auctioneer_config::types::{AccountConfig, AccountPrivacyConfig, HostConfig};
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::resolve::{resolve_account, resolve_host};
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::{env, fs};
    use validator::Validate as _;

    const ACTIVITIES: [(&str, Activity, Option<bool>); 8_usize] = [
        ("fetchBids", Activity::FetchBids, None),
        ("transmitUfpd", Activity::TransmitUfpd, Some(true)),
        (
            "transmitPreciseGeo",
            Activity::TransmitPreciseGeo,
            Some(false),
        ),
        ("transmitTid", Activity::TransmitTid, Some(true)),
        ("syncUser", Activity::SyncUser, Some(false)),
        ("reportAnalytics", Activity::ReportAnalytics, None),
        ("enrichUfpd", Activity::EnrichUfpd, Some(true)),
        (
            "transmitUniqueRequestIds",
            Activity::TransmitUniqueRequestIds,
            Some(false),
        ),
    ];

    fn fixture() -> Value {
        let mut activities = json!({});
        for (key, _, default) in ACTIVITIES {
            let mut config = json!({"rules": [
                {"allow": true, "condition": {
                    "componentName": ["fake_Future\u{212a}", "fake_Alias/\u{df}~", "fake_Future\u{212a}", format!("fake_{key}")],
                    "componentType": ["BIDDER", "aNaLyTiCs", "RtD", "GENERAL"]
                }},
                {"allow": false, "condition": {}},
                {"allow": true}
            ]});
            if let Some(value) = default {
                config["default"] = json!(value);
            }
            activities[key] = config;
        }
        json!({
            "ccpa": {"enforce": true}, "lmt": {"enforce": false},
            "account": {
                "ccpa": {"enabled": "enabled", "channel_enabled": {
                    "web": "enabled", "app": "disabled", "amp": "unset",
                    "video": "disabled", "dooh": "enabled"
                }},
                "privacy": {"allowactivities": activities}
            }
        })
    }

    fn shared_activities(value: Value) -> Result<BTreeMap<Activity, ActivityConfig>, String> {
        let activities = serde_json::from_value(value).map_err(|error| error.to_string())?;
        validate_activities(&activities).map_err(str::to_owned)?;
        Ok(activities)
    }

    fn shared_configs(value: &Value) -> (HostConfig, AccountConfig) {
        // build_codegen uses these ActivityConfig/AccountCcpaConfig DTOs and
        // validate_activities. This branch never calls the runtime merge helper.
        let activities = shared_activities(value["account"]["privacy"]["allowactivities"].clone())
            .expect("valid shared activity DTO and bounds");
        let account = AccountConfig {
            ccpa: serde_json::from_value(value["account"]["ccpa"].clone())
                .expect("valid shared CCPA DTO"),
            privacy: AccountPrivacyConfig {
                allowactivities: Arc::new(activities),
                ..AccountPrivacyConfig::default()
            },
            ..AccountConfig::default()
        };
        let host = HostConfig {
            ccpa: serde_json::from_value(value["ccpa"].clone()).expect("effective host CCPA"),
            lmt: serde_json::from_value(value["lmt"].clone()).expect("effective host LMT"),
            ..HostConfig::default()
        };
        (host, account)
    }

    fn runtime_input(value: &Value) -> Value {
        let mut input = json!({"accounts": {"m4-parity": value["account"]}});
        for key in ["ccpa", "lmt"] {
            if let Some(block) = value.get(key) {
                input[key] = block.clone();
            }
        }
        input
    }

    fn runtime_configs(
        value: &Value,
        host: &HostConfig,
        account: &AccountConfig,
    ) -> (HostConfig, AccountConfig) {
        let app: AuctioneerAppConfig =
            serde_json::from_value(runtime_input(value)).expect("valid runtime DTO");
        app.validate().expect("valid runtime bounds");
        let body = serde_json::to_vec(&json!({"site": {"publisher": {"id": "m4-parity"}}}))
            .expect("valid request body");
        let resolved =
            resolve_account(&body, &app, account, u32::MAX).expect("resolve configured account");
        assert_eq!(resolved.id, "m4-parity");
        (resolve_host(&app, host), resolved.config)
    }

    fn assert_equivalent(
        expected: &(HostConfig, AccountConfig),
        actual: &(HostConfig, AccountConfig),
    ) {
        assert_eq!(actual.0.ccpa.enforce, expected.0.ccpa.enforce);
        assert_eq!(actual.0.lmt.enforce, expected.0.lmt.enforce);
        assert_eq!(
            actual.1.privacy.allowactivities,
            expected.1.privacy.allowactivities
        );
        assert_eq!(
            serde_json::to_value(&actual.1.ccpa).expect("runtime CCPA"),
            serde_json::to_value(&expected.1.ccpa).expect("shared CCPA")
        );
    }

    #[test]
    #[ignore = "run by auctioneer-config's build_failure_harness with a baked TOML override"]
    fn emitted_baked_config_matches_runtime_fixture() {
        let path = env::var_os("AUCTIONEER__DEFAULTS__PATH_OVERRIDE")
            .expect("the parent harness must supply its compiled-in TOML fixture");
        let toml_source: toml::Value =
            toml::from_str(&fs::read_to_string(path).expect("read the child build's fixture"))
                .expect("valid TOML source");
        let source = serde_json::to_value(toml_source).expect("fixture as JSON");
        let host = &source["baked"]["host"];
        let account = &source["baked"]["account"];
        let value = json!({
            "ccpa": host["ccpa"], "lmt": host["lmt"],
            "account": {
                "ccpa": account["ccpa"],
                "privacy": {"allowactivities": account["privacy"]["allowactivities"]}
            }
        });
        let generated = (
            auctioneer_config::host_default().clone(),
            auctioneer_config::account_default().clone(),
        );
        // Resolve from neutral defaults, not the generated defaults: otherwise
        // a missing runtime field could inherit the same codegen mistake.
        let runtime = runtime_configs(&value, &HostConfig::default(), &AccountConfig::default());
        assert_equivalent(&generated, &runtime);
        assert_equivalent(&shared_configs(&value), &generated);
        assert!(generated.0.ccpa.enforce);
        assert!(generated.0.lmt.enforce);
        assert_eq!(generated.1.privacy.allowactivities.len(), 8_usize);
    }

    #[test]
    fn same_fixture_lowers_all_activities_and_host_switches_equivalently() {
        for (ccpa, lmt) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut value = fixture();
            value["ccpa"]["enforce"] = json!(ccpa);
            value["lmt"]["enforce"] = json!(lmt);
            let expected = shared_configs(&value);
            let mut base_host = expected.0.clone();
            base_host.ccpa.enforce = !ccpa;
            base_host.lmt.enforce = !lmt;
            let actual = runtime_configs(&value, &base_host, &AccountConfig::default());
            assert_equivalent(&expected, &actual);
            assert_eq!(actual.0.ccpa.enforce, ccpa);
            assert_eq!(actual.0.lmt.enforce, lmt);
            assert_eq!(actual.1.privacy.allowactivities.len(), 8_usize);
            for (key, activity, default) in ACTIVITIES {
                let config = &actual.1.privacy.allowactivities[&activity];
                assert_eq!(config.default, default, "{key}");
                assert_eq!(
                    config.rules.as_ref(),
                    &[
                        Rule {
                            allow: true,
                            condition: Some(Condition {
                                component_name: vec![
                                    "fake_Future\u{212a}".to_owned(),
                                    "fake_Alias/\u{df}~".to_owned(),
                                    "fake_Future\u{212a}".to_owned(),
                                    format!("fake_{key}"),
                                ],
                                component_type: vec![
                                    ComponentType::Bidder,
                                    ComponentType::Analytics,
                                    ComponentType::Rtd,
                                    ComponentType::General,
                                ],
                            })
                        },
                        Rule {
                            allow: false,
                            condition: Some(Condition::default())
                        },
                        Rule {
                            allow: true,
                            condition: None
                        },
                    ],
                    "{key}"
                );
            }
            assert_eq!(
                serde_json::to_value(&actual.1.ccpa).expect("effective CCPA"),
                value["account"]["ccpa"]
            );
        }
    }

    #[test]
    fn partial_runtime_overlay_matches_independently_completed_shared_fixture() {
        let mut value = fixture();
        value["lmt"]["enforce"] = json!(true);
        let base = shared_configs(&value);
        let replacement = json!([
            {"allow": false, "condition": {"componentName": ["fake_Alias/\u{df}~"]}},
            {"allow": true, "condition": {"componentType": ["rTd"]}}
        ]);
        let overlay = json!({
            "ccpa": {"enforce": false}, "lmt": {},
            "account": {
                "ccpa": {"enabled": "unset", "channel_enabled": {
                    "web": "unset", "app": "enabled", "dooh": "disabled"
                }},
                "privacy": {"allowactivities": {
                    "fetchBids": {"default": false},
                    "transmitUfpd": {"rules": []},
                    "transmitPreciseGeo": {"rules": replacement},
                    "transmitTid": {}
                }}
            }
        });
        // Spell out the effective fixture without using merge_activities or
        // resolving an overlay on this side of the comparison.
        let mut completed = value.clone();
        completed["ccpa"]["enforce"] = json!(false);
        completed["account"]["ccpa"] = json!({
            "enabled": "unset", "channel_enabled": {
                "web": "unset", "app": "enabled", "amp": "unset",
                "video": "disabled", "dooh": "disabled"
            }
        });
        let activities = &mut completed["account"]["privacy"]["allowactivities"];
        activities["fetchBids"]["default"] = json!(false);
        activities["transmitUfpd"]["rules"] = json!([]);
        activities["transmitPreciseGeo"]["rules"] = replacement;
        let expected = shared_configs(&completed);
        let actual = runtime_configs(&overlay, &base.0, &base.1);
        assert_equivalent(&expected, &actual);
        // An empty override must retain both the entire map and CCPA channels.
        let inherited = runtime_configs(
            &json!({"ccpa": {}, "account": {"ccpa": {}, "privacy": {"allowactivities": {}}}}),
            &base.0,
            &base.1,
        );
        assert_equivalent(&base, &inherited);
        assert_equivalent(&shared_configs(&value), &base);
    }

    #[test]
    fn every_ccpa_tristate_replaces_each_field_with_shared_dto_parity() {
        for (before, after) in [
            ("disabled", "enabled"),
            ("unset", "disabled"),
            ("enabled", "unset"),
        ] {
            let mut initial = fixture();
            initial["account"]["ccpa"] = json!({"enabled": before, "channel_enabled": {
                "web": before, "app": before, "amp": before, "video": before, "dooh": before
            }});
            let base = shared_configs(&initial);
            let mut completed = initial.clone();
            completed["account"]["ccpa"] = json!({"enabled": after, "channel_enabled": {
                "web": after, "app": after, "amp": after, "video": after, "dooh": after
            }});
            let overlay = json!({"account": {"ccpa": completed["account"]["ccpa"]}});
            let actual = runtime_configs(&overlay, &base.0, &base.1);
            assert_equivalent(&shared_configs(&completed), &actual);
            assert_eq!(
                serde_json::to_value(&actual.1.ccpa).expect("effective CCPA"),
                completed["account"]["ccpa"]
            );
        }
    }

    #[test]
    fn shared_activity_validation_and_runtime_validation_agree_at_boundaries() {
        let cases = [
            (
                "128 rules",
                true,
                json!({"rules": vec![json!({"allow": true}); 128_usize]}),
            ),
            (
                "129 rules",
                false,
                json!({"rules": vec![json!({"allow": true}); 129_usize]}),
            ),
            (
                "128 UTF-8 bytes",
                true,
                json!({"rules": [{"allow": true, "condition": {
                    "componentName": [format!("fake_a{}", "\u{e9}".repeat(61_usize))]
                }}]}),
            ),
            (
                "130 UTF-8 bytes",
                false,
                json!({"rules": [{"allow": true, "condition": {
                    "componentName": ["\u{e9}".repeat(65_usize)]
                }}]}),
            ),
            (
                "128 combined entries",
                true,
                json!({"rules": [{"allow": true, "condition": {
                    "componentName": vec!["fake_x"; 124_usize],
                    "componentType": ["BIDDER", "ANALYTICS", "RTD", "GENERAL"]
                }}]}),
            ),
            (
                "129 combined entries",
                false,
                json!({"rules": [{"allow": true, "condition": {
                    "componentName": vec!["fake_x"; 125_usize],
                    "componentType": ["BIDDER", "ANALYTICS", "RTD", "GENERAL"]
                }}]}),
            ),
            ("missing allow", false, json!({"rules": [{}]})),
            (
                "unknown condition key",
                false,
                json!({"rules": [{"allow": true,
                    "condition": {"componentname": []}
                }]}),
            ),
            (
                "unknown component type",
                false,
                json!({"rules": [{"allow": true,
                    "condition": {"componentType": ["module"]}
                }]}),
            ),
            ("incorrect default type", false, json!({"default": 1_i32})),
        ];
        for (label, accepted, config) in cases {
            let mut value = fixture();
            let activities = json!({"fetchBids": config});
            value["account"]["privacy"]["allowactivities"] = activities.clone();
            assert_eq!(
                shared_activities(activities).is_ok(),
                accepted,
                "shared: {label}"
            );
            let runtime = serde_json::from_value::<AuctioneerAppConfig>(runtime_input(&value));
            assert_eq!(
                runtime.is_ok_and(|app| app.validate().is_ok()),
                accepted,
                "runtime: {label}"
            );
            if accepted {
                assert_equivalent(
                    &shared_configs(&value),
                    &runtime_configs(&value, &HostConfig::default(), &AccountConfig::default()),
                );
            }
        }
    }
}
