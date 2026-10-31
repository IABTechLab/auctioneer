//! Shared effective activity schema and validation.

#[cfg(test)]
mod tests {
    use auctioneer_config::activity::{
        Activity, ActivityConfig, ComponentType, Condition, Rule, validate_activities,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn effective_objects_are_revalidated_even_when_constructed_without_serde() {
        let mut activities = BTreeMap::from([(
            Activity::FetchBids,
            ActivityConfig {
                default: None,
                rules: vec![
                    Rule {
                        allow: true,
                        condition: None
                    };
                    129
                ]
                .into(),
            },
        )]);
        assert!(validate_activities(&activities).is_err());
        activities
            .get_mut(&Activity::FetchBids)
            .expect("activity")
            .rules = vec![Rule {
            allow: true,
            condition: Some(Condition {
                component_name: vec!["fake_x".to_owned(); 128],
                component_type: vec![ComponentType::Bidder],
            }),
        }]
        .into();
        assert!(validate_activities(&activities).is_err());
        activities
            .get_mut(&Activity::FetchBids)
            .expect("activity")
            .rules = vec![Rule {
            allow: true,
            condition: Some(Condition {
                component_name: vec!["\u{e9}".repeat(65)],
                component_type: vec![],
            }),
        }]
        .into();
        assert!(validate_activities(&activities).is_err());
    }

    #[test]
    fn component_types_canonicalize_simple_fold_and_names_remain_original() {
        let cfg: ActivityConfig =
            serde_json::from_value(json!({"rules":[{"allow":false,"condition":{
                "componentName":["fake_FUTURE\u{212a}", "fake_\u{130}", "fake_\u{df}", ""],
                "componentType":["BIDDER", "ANALYTIC\u{17f}", "RTD", "GENERAL"]
            }}]}))
            .expect("simple-fold component types");
        let encoded = serde_json::to_value(&cfg).expect("encode");
        assert_eq!(encoded["default"], serde_json::Value::Null);
        assert_eq!(
            encoded["rules"][0]["condition"]["componentType"],
            json!(["bidder", "analytics", "rtd", "general"])
        );
        assert_eq!(
            encoded["rules"][0]["condition"]["componentName"],
            json!(["fake_FUTURE\u{212a}", "fake_\u{130}", "fake_\u{df}", ""])
        );
        assert!(!cfg.rules[0].allow);
        assert!(ActivityConfig::default().default.unwrap_or(true));
    }
}
