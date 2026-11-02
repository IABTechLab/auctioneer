//! Rule contents remain shared across request-time account overlays.

#[cfg(test)]
mod tests {
    use auctioneer_config::activity::{Activity, ActivityConfig, Rule, validate_activities};
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::resolve::resolve_account;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn account_clone_and_runtime_overlay_share_rule_storage() {
        let mut baked = auctioneer_config::account_default().clone();
        Arc::make_mut(&mut baked.privacy.allowactivities).insert(
            Activity::FetchBids,
            ActivityConfig {
                default: Some(false),
                rules: vec![
                    Rule {
                        allow: false,
                        condition: None
                    };
                    128
                ]
                .into(),
            },
        );
        Arc::make_mut(&mut baked.privacy.allowactivities).insert(
            Activity::TransmitTid,
            ActivityConfig {
                default: Some(false),
                rules: vec![Rule {
                    allow: false,
                    condition: None,
                }]
                .into(),
            },
        );
        let cloned = baked.clone();
        assert!(Arc::ptr_eq(
            &baked.privacy.allowactivities,
            &cloned.privacy.allowactivities
        ));
        let config: AuctioneerAppConfig = toml::from_str(
            "
            [accounts.pub.privacy.allowactivities.fetchBids]
            default = true
            [accounts.pub.privacy.allowactivities.transmitUfpd]
            rules = [{allow = false}, {allow = true}]
        ",
        )
        .expect("runtime DTO");
        let overlay_rules = config.accounts["pub"]
            .privacy
            .as_ref()
            .expect("privacy")
            .allowactivities
            .as_ref()
            .expect("activities")[&Activity::TransmitUfpd]
            .rules
            .as_ref()
            .expect("rules");
        let body =
            serde_json::to_vec(&json!({"site":{"publisher":{"id":"pub"}}})).expect("request body");
        for _ in 0_usize..2_usize {
            let resolved = resolve_account(&body, &config, &baked, u32::MAX)
                .expect("resolve")
                .config;
            let activities = &resolved.privacy.allowactivities;
            assert!(Arc::ptr_eq(
                &activities[&Activity::FetchBids].rules,
                &baked.privacy.allowactivities[&Activity::FetchBids].rules
            ));
            assert!(Arc::ptr_eq(
                &activities[&Activity::TransmitTid].rules,
                &baked.privacy.allowactivities[&Activity::TransmitTid].rules
            ));
            assert!(Arc::ptr_eq(
                &activities[&Activity::TransmitUfpd].rules,
                overlay_rules
            ));
            assert_eq!(activities[&Activity::FetchBids].default, Some(true));
            validate_activities(activities).expect("valid resolved activities");
        }
        assert_eq!(
            baked.privacy.allowactivities[&Activity::FetchBids].default,
            Some(false)
        );
        let empty_body = serde_json::to_vec(&json!({})).expect("empty request");
        let inherited = resolve_account(&empty_body, &config, &baked, u32::MAX)
            .expect("fallback")
            .config;
        assert!(Arc::ptr_eq(
            &inherited.privacy.allowactivities,
            &baked.privacy.allowactivities
        ));
    }
}
