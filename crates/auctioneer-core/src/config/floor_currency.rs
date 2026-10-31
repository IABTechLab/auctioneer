//! Root validation without copying rejected snapshot tables into error params.

use auctioneer_config::FloorCurrencyConfig;
use validator::ValidationError;

pub(super) fn validate(snapshot: &FloorCurrencyConfig) -> Result<(), ValidationError> {
    snapshot.validate().map_err(ValidationError::new)
}

#[cfg(test)]
mod tests {
    use super::super::AuctioneerAppConfig;
    use serde_json::{Value, json};
    use validator::Validate as _;

    fn config_with_snapshot(snapshot: &Value) -> AuctioneerAppConfig {
        serde_json::from_value(json!({"floor_currency": snapshot})).unwrap()
    }

    #[test]
    fn floor_currency_accepts_operator_snapshot_through_existing_runtime_config() {
        let parsed = serde_json::from_value::<AuctioneerAppConfig>(json!({
            "floor_currency": {
                "expires_at": 2_000_i32,
                "usd_per_unit": {"QAA": 2.5_f64}
            }
        }));
        assert!(
            parsed.is_ok(),
            "operator floor-currency snapshot must be accepted"
        );
        let config = parsed.unwrap();
        config.validate().unwrap();
        config.validate_effective_defaults().unwrap();
        assert_eq!(
            serde_json::to_value(&config).unwrap()["floor_currency"],
            json!({"expires_at": 2_000_i32, "usd_per_unit": {"QAA": 2.5_f64}})
        );
    }

    #[test]
    fn floor_currency_absence_keeps_existing_runtime_config_valid() {
        let config: AuctioneerAppConfig = serde_json::from_value(json!({})).unwrap();
        config.validate().unwrap();
        assert!(serde_json::to_value(config).unwrap()["floor_currency"].is_null());
    }

    #[test]
    fn floor_currency_root_validation_rejects_bad_codes_rates_and_usd_overrides() {
        for rates in [
            json!({"qaa": 2.5_f64}),
            json!({"QA": 2.5_f64}),
            json!({"Q1A": 2.5_f64}),
            json!({"QAA": 0.0_f64}),
            json!({"QAA": -1.0_f64}),
            json!({"USD": 1.0_f64}),
        ] {
            let config =
                config_with_snapshot(&json!({"expires_at": 2_000_i32, "usd_per_unit": rates}));
            let errors = config
                .validate()
                .expect_err("invalid floor snapshot must fail validation");
            assert!(errors.field_errors().contains_key("floor_currency"));
            assert!(config.validate_effective_defaults().is_err());
        }
    }

    #[test]
    fn floor_currency_root_validation_rejects_nonpositive_expiry() {
        for expires_at in [-1_i32, 0_i32] {
            let config = config_with_snapshot(&json!({
                "expires_at": expires_at,
                "usd_per_unit": {"QAA": 2.5_f64}
            }));
            assert!(config.validate().is_err());
        }
    }

    #[test]
    fn floor_currency_root_shape_requires_explicit_expiry_and_rejects_feeds() {
        for snapshot in [
            json!({"usd_per_unit": {"QAA": 2.5_f64}}),
            json!({"expires_at": 2_000_i32}),
            json!({"expires_at": 2_000_i32, "usd_per_unit": {}, "feed_url": "https://example.test/rates"}),
        ] {
            serde_json::from_value::<AuctioneerAppConfig>(json!({
                "floor_currency": snapshot
            }))
            .unwrap_err();
        }
    }

    #[test]
    fn floor_currency_root_parser_rejects_duplicate_rate_keys_not_as_unknown_root_field() {
        // Raw wire JSON is required to retain duplicate and escaped-equivalent keys.
        for wire in [
            r#"{"floor_currency":{"expires_at":2000,"usd_per_unit":{"QAA":1.25,"QAA":2.5}}}"#,
            r#"{"floor_currency":{"expires_at":2000,"usd_per_unit":{"QAA":1.25,"\u0051AA":2.5}}}"#,
        ] {
            let error = serde_json::from_str::<AuctioneerAppConfig>(wire)
                .expect_err("duplicate floor-currency keys must fail at config ingestion");
            assert!(
                error.to_string().contains("duplicate floor currency code"),
                "duplicates must be rejected by the rate-map parser, not unknown-root-field rejection"
            );
        }
    }

    #[test]
    fn floor_currency_root_count_cap_is_independent_of_global_config_caps() {
        let mut rates = serde_json::Map::new();
        for index in 0_i32..256_i32 {
            let code = String::from_utf8(vec![
                b'Q',
                b'A'.checked_add(u8::try_from(index.checked_div(26).unwrap()).unwrap())
                    .unwrap(),
                b'A'.checked_add(u8::try_from(index.checked_rem(26).unwrap()).unwrap())
                    .unwrap(),
            ])
            .unwrap();
            rates.insert(code, json!(1.25_f64));
        }
        assert_eq!(rates.len(), 256);
        let mut config =
            config_with_snapshot(&json!({"expires_at": 2_000_i32, "usd_per_unit": rates}));
        assert!(
            config.validate().is_ok(),
            "the 256-entry boundary must be valid"
        );
        rates.insert("QZZ".to_owned(), json!(1.25_f64));
        assert_eq!(rates.len(), 257);
        serde_json::from_value::<AuctioneerAppConfig>(json!({
            "floor_currency": {"expires_at": 2_000_i32, "usd_per_unit": rates}
        }))
        .unwrap_err();
        config
            .floor_currency
            .as_mut()
            .unwrap()
            .usd_per_unit
            .insert("QZZ".into(), 1.25_f64);
        let errors = config
            .validate()
            .expect_err("257 rates must be refused independently");
        assert_eq!(
            errors.field_errors()["floor_currency"][0].code,
            "floor_currency_too_many_rates"
        );
    }

    #[test]
    fn floor_currency_validation_diagnostics_never_copy_rejected_rate_tables() {
        let rejected_code = "private_config_canary".repeat(1_024);
        let config = config_with_snapshot(&json!({
            "expires_at": 2_000_i32,
            "usd_per_unit": {(rejected_code.clone()): 2.5_f64}
        }));
        let errors = config
            .validate()
            .expect_err("an oversized rate key must be refused");
        let diagnostic = errors.to_string();
        assert!(diagnostic.len() < 256);
        assert!(!diagnostic.contains("private_config_canary"));
        assert!(errors.field_errors()["floor_currency"][0].params.is_empty());
    }

    #[test]
    fn floor_currency_static_validation_does_not_substitute_its_own_clock() {
        let config = config_with_snapshot(&json!({
            "expires_at": 1_i32,
            "usd_per_unit": {"QAA": 2.5_f64}
        }));
        assert!(
            config.validate().is_ok(),
            "freshness uses the handler-supplied time, not config IO"
        );
    }
}
