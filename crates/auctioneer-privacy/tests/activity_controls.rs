#[cfg(test)]
mod tests {
    use auctioneer_config::activity::{ActivityConfig, ActivityConfigs};
    use auctioneer_config::types::{AccountConfig, HostConfig};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_privacy::{
        Activity, Component, PrivacyContext, ResolveOptions, activity_allowed,
    };
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

    fn allowed(config: Value, component: Component<'_>) -> bool {
        let cfg: ActivityConfig = serde_json::from_value(config).unwrap();
        activity_allowed(
            &BTreeMap::from([(Activity::FetchBids, cfg)]),
            Activity::FetchBids,
            component,
        )
    }

    #[test]
    fn first_match_and_default_are_not_deny_override() {
        for (rules, expected) in [
            (json!([{"allow":true},{"allow":false}]), true),
            (json!([{"allow":false},{"allow":true}]), false),
        ] {
            assert_eq!(
                allowed(
                    json!({"default":!expected,"rules":rules}),
                    Component::Bidder("fake_alias")
                ),
                expected
            );
        }
        assert!(allowed(json!({}), Component::Bidder("fake_alias")));
        assert!(!allowed(
            json!({"default":false}),
            Component::Bidder("fake_alias")
        ));
        assert!(activity_allowed(
            &BTreeMap::new(),
            Activity::FetchBids,
            Component::Bidder("fake_alias")
        ));
    }

    #[test]
    fn conditions_are_and_across_clauses_and_or_within_each_clause() {
        let cfg = json!({"default":true,"rules":[{"allow":false,"condition":{
            "componentName":["fake_alias","another"],"componentType":["bidder","rtd"]
        }}]});
        assert!(!allowed(cfg.clone(), Component::Bidder("fake_alias")));
        assert!(!allowed(cfg.clone(), Component::Rtd("another")));
        assert!(allowed(cfg.clone(), Component::Analytics("fake_alias")));
        assert!(allowed(cfg, Component::Bidder("fake_different")));
        for condition in [
            json!({}),
            json!({"componentName":[]}),
            json!({"componentType":[]}),
        ] {
            assert!(!allowed(
                json!({"rules":[{"allow":false,"condition":condition}]}),
                Component::General("named")
            ));
        }
    }

    #[test]
    fn unicode_alias_matching_is_simple_fold_without_core_identity_fallback() {
        let cfg = json!({"default":true,"rules":[{"allow":false,"condition":{
            "componentName":["fake_k","fake_strasse","fake_core"]
        }}]});
        assert!(!allowed(cfg.clone(), Component::Bidder("fake_\u{212a}")));
        assert!(!allowed(cfg.clone(), Component::Bidder("fake_K")));
        assert!(allowed(cfg.clone(), Component::Bidder("fake_stra\u{df}e")));
        assert!(allowed(cfg, Component::Bidder("fake_alias")));
        let allow_rule = json!({"default":false,"rules":[{"allow":true,"condition":{"componentName":["fake_k"]}}]});
        assert!(allowed(
            allow_rule.clone(),
            Component::Bidder("fake_\u{212a}")
        ));
        assert!(!allowed(allow_rule, Component::Bidder("fake_unknown")));
    }

    #[test]
    fn all_eight_activities_match_864_pinned_reference_rows() {
        let fixture: Value =
            serde_json::from_str(include_str!("data/privacy/reference.json")).unwrap();
        let rows = fixture["activities"].as_array().unwrap();
        assert_eq!(rows.len(), 864);
        for row in rows {
            let config: ActivityConfigs =
                serde_json::from_value(row["config"]["allowactivities"].clone()).unwrap();
            let activity: Activity = serde_json::from_value(row["activity"].clone()).unwrap();
            let name = row["component_name"].as_str().unwrap();
            let component = match row["component_type"].as_str().unwrap() {
                "bidder" => Component::Bidder(name),
                "analytics" => Component::Analytics(name),
                "rtd" => Component::Rtd(name),
                "general" => Component::General(name),
                kind => panic!("unknown reference component: {kind}"),
            };
            assert_eq!(
                activity_allowed(&config, activity, component),
                row["allow"].as_bool().unwrap(),
                "{row}"
            );
        }
    }

    #[test]
    fn resolved_activity_config_applies_even_with_gdpr_disabled() {
        let mut account = AccountConfig::default();
        account.privacy.allowactivities =
            serde_json::from_value(json!({"fetchBids":{"default":false}})).unwrap();
        let now = OffsetDateTime::UNIX_EPOCH;
        let ctx = PrivacyContext::resolve(
            &BidRequest::default(),
            &account,
            &HostConfig::default(),
            &GvlStore::for_test(vec![]),
            &Gcl::for_test(vec![], now),
            now,
            ResolveOptions::default(),
        )
        .unwrap()
        .context;
        assert!(!ctx.activity_allow(Activity::FetchBids, Component::Bidder("fake_alias")));
        assert!(ctx.activity_allow(Activity::TransmitUfpd, Component::Bidder("fake_alias")));
    }
}
