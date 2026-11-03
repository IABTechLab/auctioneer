//! privacy enforcement typed config boundaries and overlay semantics.

#[cfg(test)]
mod tests {
    use auctioneer_config::types::AccountConfig;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::resolve::{resolve_account, resolve_host};
    use serde_json::{Value, json};
    use validator::Validate as _;

    fn app(value: Value) -> AuctioneerAppConfig {
        let config: AuctioneerAppConfig = serde_json::from_value(value).expect("valid runtime DTO");
        config.validate().expect("valid runtime bounds");
        config
    }

    fn account(config: &AuctioneerAppConfig, baked: &AccountConfig) -> AccountConfig {
        let body =
            serde_json::to_vec(&json!({"site":{"publisher":{"id":"pub"}}})).expect("request body");
        resolve_account(&body, config, baked, u32::MAX)
            .expect("resolve account")
            .config
    }

    #[test]
    fn host_switches_are_optional_boolean_overlays() {
        let mut baked = auctioneer_config::host_default().clone();
        assert!(!baked.ccpa.enforce);
        assert!(!baked.lmt.enforce);
        baked.ccpa.enforce = true;
        baked.lmt.enforce = true;
        let inherited = resolve_host(&app(json!({"ccpa":{}, "lmt":{}})), &baked);
        assert!(inherited.ccpa.enforce);
        assert!(inherited.lmt.enforce);
        let disabled = resolve_host(&app(json!({"ccpa":{"enforce":false}})), &baked);
        assert!(!disabled.ccpa.enforce);
        assert!(disabled.lmt.enforce);
        let lmt_disabled = resolve_host(&app(json!({"lmt":{"enforce":false}})), &baked);
        assert!(lmt_disabled.ccpa.enforce);
        assert!(!lmt_disabled.lmt.enforce);
        let enabled = resolve_host(
            &app(json!({"ccpa":{"enforce":true}, "lmt":{"enforce":true}})),
            auctioneer_config::host_default(),
        );
        assert!(enabled.ccpa.enforce);
        assert!(enabled.lmt.enforce);
    }

    #[test]
    fn ccpa_unset_replaces_baked_while_omissions_inherit() {
        let mut baked_value =
            serde_json::to_value(auctioneer_config::account_default()).expect("baked");
        baked_value["ccpa"] = json!({"enabled":"enabled", "channel_enabled":{
            "web":"enabled", "app":"disabled", "amp":"enabled", "video":"disabled", "dooh":"enabled"
        }});
        let baked: AccountConfig = serde_json::from_value(baked_value).expect("effective account");
        let config = app(json!({"accounts":{"pub":{"ccpa":{
            "enabled":"unset", "channel_enabled":{"web":"unset", "app":"enabled"}
        }}}}));
        let resolved = serde_json::to_value(account(&config, &baked)).expect("resolved");
        assert_eq!(
            resolved["ccpa"],
            json!({"enabled":"unset", "channel_enabled":{
                "web":"unset", "app":"enabled", "amp":"enabled", "video":"disabled", "dooh":"enabled"
            }})
        );
    }

    #[test]
    fn ccpa_and_host_switch_schema_is_strict() {
        for invalid in [
            json!({"ccpa":{"enforce":"true"}}),
            json!({"lmt":{"enforce":1_i32}}),
            json!({"ccpa":{"Enforce":true}}),
            json!({"lmt":{"enfroce":true}}),
            json!({"accounts":{"pub":{"ccpa":{"enabled":true}}}}),
            json!({"accounts":{"pub":{"ccpa":{"enabled":"ENABLED"}}}}),
            json!({"accounts":{"pub":{"ccpa":{"Enabled":"enabled"}}}}),
            json!({"accounts":{"pub":{"ccpa":{"channel_enabled":{"APP":"enabled"}}}}}),
            json!({"accounts":{"pub":{"ccpa":{"channel_enabled":{"app":"invalid"}}}}}),
        ] {
            serde_json::from_value::<AuctioneerAppConfig>(invalid).expect_err("invalid schema");
        }
        for state in ["enabled", "disabled", "unset"] {
            let config = app(json!({"accounts":{"pub":{"ccpa":{"enabled":state,
                "channel_enabled":{"web":state,"app":state,"video":state,"amp":state,"dooh":state}
            }}}}));
            let encoded =
                serde_json::to_value(account(&config, auctioneer_config::account_default()))
                    .expect("resolved");
            for channel in ["web", "app", "video", "amp", "dooh"] {
                assert_eq!(encoded["ccpa"]["channel_enabled"][channel], state);
            }
        }
    }

    #[test]
    fn activity_merge_is_per_key_per_field_with_whole_rule_replacement() {
        let mut baked_value =
            serde_json::to_value(auctioneer_config::account_default()).expect("baked");
        baked_value["privacy"]["allowactivities"] = json!({
            "fetchBids":{"default":false,"rules":[{"allow":false}]},
            "transmitUfpd":{"default":true,"rules":[{"allow":false}]},
            "transmitTid":{"default":false,"rules":[{"allow":false}]},
            "syncUser":{"default":false,"rules":[{"allow":true}]}
        });
        let baked: AccountConfig = serde_json::from_value(baked_value).expect("effective account");
        let replacement = json!([
            {"allow":true,"condition":{"componentName":["fake_Future\u{212a}","fake_Future\u{212a}"],"componentType":["BIDDER"]}},
            {"allow":false}, {"allow":true}
        ]);
        let config = app(json!({"accounts":{"pub":{"privacy":{"allowactivities":{
            "fetchBids":{"rules":replacement},
            "transmitUfpd":{"default":false},
            "transmitTid":{"rules":[]}
        }}}}}));
        let resolved = serde_json::to_value(account(&config, &baked)).expect("resolved");
        let activities = &resolved["privacy"]["allowactivities"];
        assert_eq!(activities["fetchBids"]["default"], false);
        assert_eq!(
            activities["fetchBids"]["rules"]
                .as_array()
                .expect("rules")
                .len(),
            3
        );
        assert_eq!(
            activities["fetchBids"]["rules"][0]["condition"]["componentName"],
            json!(["fake_Future\u{212a}", "fake_Future\u{212a}"])
        );
        assert_eq!(
            activities["fetchBids"]["rules"][0]["condition"]["componentType"],
            json!(["bidder"])
        );
        assert_eq!(activities["fetchBids"]["rules"][1]["allow"], false);
        assert_eq!(activities["fetchBids"]["rules"][2]["allow"], true);
        assert_eq!(activities["transmitUfpd"]["default"], false);
        assert_eq!(activities["transmitUfpd"]["rules"][0]["allow"], false);
        assert_eq!(activities["transmitTid"]["rules"], json!([]));
        assert_eq!(activities["syncUser"]["default"], false);
    }

    #[test]
    fn all_eight_activity_keys_and_match_all_conditions_are_accepted() {
        for key in [
            "fetchBids",
            "transmitUfpd",
            "transmitPreciseGeo",
            "transmitTid",
            "syncUser",
            "reportAnalytics",
            "enrichUfpd",
            "transmitUniqueRequestIds",
        ] {
            let config = app(json!({"accounts":{"pub":{"privacy":{"allowactivities":{
                key:{"rules":[{"allow":false},{"allow":true,"condition":{}},{"allow":false,"condition":{"componentName":[],"componentType":[]}}]}
            }}}}}));
            let resolved =
                serde_json::to_value(account(&config, auctioneer_config::account_default()))
                    .expect("resolved");
            assert_eq!(
                resolved["privacy"]["allowactivities"][key]["default"],
                Value::Null
            );
        }
    }

    #[test]
    fn invalid_activity_schema_and_bounds_are_rejected() {
        let mut invalid = vec![
            json!({"fetchbids":{}}),
            json!({"FetchBids":{}}),
            json!({"unknown":{}}),
            json!({"fetchBids":{"Default":false}}),
            json!({"fetchBids":{"default":"false"}}),
            json!({"fetchBids":{"rules":[{}]}}),
            json!({"fetchBids":{"rules":[{"allow":null}]}}),
            json!({"fetchBids":{"rules":[{"allow":true,"other":1_i32}]}}),
            json!({"fetchBids":{"rules":[{"allow":true,"condition":{"componentname":[]}}]}}),
            json!({"fetchBids":{"rules":[{"allow":true,"condition":{"componentType":["module"]}}]}}),
            json!({"fetchBids":{"rules":[{"allow":true,"condition":{"componentType":["B\u{130}DDER"]}}]}}),
            json!({"fetchBids":{"rules":vec![json!({"allow":true});129]}}),
            json!({"fetchBids":{"rules":[{"allow":true,"condition":{"componentName":["x".repeat(129)]}}]}}),
            json!({"fetchBids":{"rules":[{"allow":true,"condition":{"componentName":["\u{e9}".repeat(65)]}}]}}),
        ];
        invalid.push(json!({"fetchBids":{"rules":[{"allow":true,"condition":{
            "componentName":vec!["fake_x";128],"componentType":["bidder"]
        }}]}}));
        for activities in invalid {
            let result = serde_json::from_value::<AuctioneerAppConfig>(
                json!({"accounts":{"pub":{"privacy":{"allowactivities":activities}}}}),
            );
            assert!(result.is_err() || result.expect("DTO").validate().is_err());
        }
    }

    #[test]
    fn activity_bounds_are_inclusive() {
        let config = app(json!({"accounts":{"pub":{"privacy":{"allowactivities":{
            "fetchBids":{"rules":vec![json!({"allow":true,"condition":{
                "componentName":vec![format!("fake_a{}", "\u{e9}".repeat(61));124],
                "componentType":["BIDDER","ANALYTICS","RTD","GENERAL"]
            }});128]}
        }}}}}));
        config.validate().expect("inclusive bounds are valid");
    }
}
