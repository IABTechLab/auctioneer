//! Operator-provided rate snapshots for floor conversion only.

use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, btree_map::Entry};
use std::fmt;

/// At most 256 foreign floor rates per snapshot, independently of the global
/// extracted-config byte/node caps, which still apply to the original owner.
pub const MAX_FLOOR_CURRENCY_RATES: usize = 256;

/// USD-per-unit floor rates with an explicit exclusive Unix-second expiry.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FloorCurrencyConfig {
    /// Exclusive Unix-second expiry; checked against the handler's supplied time.
    pub expires_at: i64,
    /// Three uppercase ASCII letters -> finite, strictly positive USD per unit.
    /// USD is implicit identity and cannot be overridden by this map.
    #[serde(deserialize_with = "deserialize_unique_rates")]
    pub usd_per_unit: BTreeMap<String, f64>,
}

impl FloorCurrencyConfig {
    /// Validate this owner's shape without cloning rates or consulting a clock.
    /// Freshness is checked separately by the borrowed floor-conversion view.
    ///
    /// # Errors
    /// Returns a fixed code for an invalid expiry, count, currency code or rate.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.usd_per_unit.len() > MAX_FLOOR_CURRENCY_RATES {
            return Err("floor_currency_too_many_rates");
        }
        if self.expires_at <= 0 {
            return Err("floor_currency_invalid_expiry");
        }
        for (code, rate) in &self.usd_per_unit {
            if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
                return Err("floor_currency_invalid_code");
            }
            if code == "USD" {
                return Err("floor_currency_usd_override");
            }
            if !rate.is_finite() || *rate <= 0.0_f64 {
                return Err("floor_currency_invalid_rate");
            }
        }
        Ok(())
    }
}

fn deserialize_unique_rates<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, f64>, D::Error> {
    struct UniqueRates;

    impl<'de> Visitor<'de> for UniqueRates {
        type Value = BTreeMap<String, f64>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a floor rate map with unique currency codes")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut rates = BTreeMap::new();
            // Compare decoded keys before reading the second value: escaped
            // equivalents are duplicates too, never operator-order overrides.
            while let Some(code) = map.next_key::<String>()? {
                if rates.len() >= MAX_FLOOR_CURRENCY_RATES {
                    return Err(A::Error::custom("too many floor currency rates"));
                }
                match rates.entry(code) {
                    Entry::Vacant(entry) => {
                        entry.insert(map.next_value::<f64>()?);
                    }
                    Entry::Occupied(_) => {
                        return Err(A::Error::custom("duplicate floor currency code"));
                    }
                }
            }
            Ok(rates)
        }
    }

    deserializer.deserialize_map(UniqueRates)
}

#[cfg(test)]
mod tests {
    use super::{FloorCurrencyConfig, MAX_FLOOR_CURRENCY_RATES};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn snapshot() -> FloorCurrencyConfig {
        serde_json::from_value(json!({
            "expires_at": 2_000_i32,
            "usd_per_unit": {"QAA": 2.5_f64, "QAB": 0.25_f64}
        }))
        .unwrap()
    }

    fn rates(count: usize) -> BTreeMap<String, f64> {
        (0..count)
            .map(|index| {
                let code = String::from_utf8(vec![
                    b'Q',
                    b'A'.checked_add(u8::try_from(index.checked_div(26).unwrap()).unwrap())
                        .unwrap(),
                    b'A'.checked_add(u8::try_from(index.checked_rem(26).unwrap()).unwrap())
                        .unwrap(),
                ])
                .unwrap();
                (code, 1.25_f64)
            })
            .collect()
    }

    #[test]
    fn floor_currency_accepts_and_round_trips_explicit_operator_snapshot() {
        let config = snapshot();
        assert_eq!(config.validate(), Ok(()));
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            json!({"expires_at": 2_000_i32, "usd_per_unit": {"QAA": 2.5_f64, "QAB": 0.25_f64}})
        );
    }

    #[test]
    fn floor_currency_count_cap_is_independently_exactly_256() {
        assert_eq!(MAX_FLOOR_CURRENCY_RATES, 256);
        let mut config = snapshot();
        config.usd_per_unit = rates(256);
        assert_eq!(config.usd_per_unit.len(), 256);
        assert_eq!(config.validate(), Ok(()));
        config.usd_per_unit = rates(257);
        assert_eq!(config.usd_per_unit.len(), 257);
        assert_eq!(config.validate(), Err("floor_currency_too_many_rates"));
    }

    #[test]
    fn floor_currency_rate_count_is_bounded_during_deserialization() {
        let within = json!({"expires_at": 2_000_i32, "usd_per_unit": rates(256)});
        let config: FloorCurrencyConfig = serde_json::from_value(within).unwrap();
        assert_eq!(config.usd_per_unit.len(), 256);
        let over = json!({"expires_at": 2_000_i32, "usd_per_unit": rates(257)});
        let error = serde_json::from_value::<FloorCurrencyConfig>(over).unwrap_err();
        assert!(error.to_string().contains("too many floor currency rates"));
    }

    #[test]
    fn floor_currency_rejects_codes_other_than_three_uppercase_ascii_letters() {
        for code in [
            "", "QA", "QAAA", "qaa", "Qaa", "Q1A", " QAA", "QAA ", "Q\u{e9}A",
        ] {
            let mut config = snapshot();
            config.usd_per_unit = BTreeMap::from([(code.to_owned(), 1.0_f64)]);
            assert_eq!(
                config.validate(),
                Err("floor_currency_invalid_code"),
                "accepted malformed code {code:?}"
            );
        }
    }

    #[test]
    fn floor_currency_rejects_even_identity_usd_override() {
        for rate in [1.0_f64, 2.0_f64] {
            let mut config = snapshot();
            config.usd_per_unit.insert("USD".to_owned(), rate);
            assert_eq!(config.validate(), Err("floor_currency_usd_override"));
        }
    }

    #[test]
    fn floor_currency_rejects_nonpositive_and_nonfinite_programmatic_rates() {
        for rate in [
            0.0_f64,
            -0.0_f64,
            -1.0_f64,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let mut config = snapshot();
            config.usd_per_unit.insert("QAA".to_owned(), rate);
            assert_eq!(config.validate(), Err("floor_currency_invalid_rate"));
        }
    }

    #[test]
    fn floor_currency_accepts_every_finite_strictly_positive_rate() {
        for rate in [f64::from_bits(1), f64::MIN_POSITIVE, 1.0_f64, f64::MAX] {
            let mut config = snapshot();
            config.usd_per_unit.insert("QAA".to_owned(), rate);
            assert_eq!(config.validate(), Ok(()));
        }
    }

    #[test]
    fn floor_currency_expiry_is_positive_but_freshness_belongs_to_the_caller() {
        let mut config = snapshot();
        for expires_at in [i64::MIN, -1, 0] {
            config.expires_at = expires_at;
            assert_eq!(config.validate(), Err("floor_currency_invalid_expiry"));
        }
        for expires_at in [1, i64::MAX] {
            config.expires_at = expires_at;
            assert_eq!(config.validate(), Ok(()));
        }
    }

    #[test]
    fn floor_currency_empty_explicit_snapshot_is_valid() {
        let mut config = snapshot();
        config.usd_per_unit.clear();
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn floor_currency_rejects_duplicate_rate_keys_at_deserialization() {
        // A Value/json! object cannot retain duplicate wire keys.
        let wire = r#"{"expires_at":2000,"usd_per_unit":{"QAA":1.25,"QAA":2.5}}"#;
        let error = serde_json::from_str::<FloorCurrencyConfig>(wire)
            .expect_err("duplicate floor-currency rate keys must be refused");
        assert!(error.to_string().contains("duplicate floor currency code"));
    }

    #[test]
    fn floor_currency_rejects_duplicate_escaped_equivalent_rate_keys_at_deserialization() {
        let wire = r#"{"expires_at":2000,"usd_per_unit":{"QAA":1.25,"\u0051AA":2.5}}"#;
        let error = serde_json::from_str::<FloorCurrencyConfig>(wire)
            .expect_err("escaped-equivalent floor-currency rate keys must be refused");
        assert!(error.to_string().contains("duplicate floor currency code"));
    }

    #[test]
    fn floor_currency_serde_requires_expiry_and_rates_and_rejects_unknown_fields() {
        for document in [
            json!({}),
            json!({"usd_per_unit": {"QAA": 1.0_f64}}),
            json!({"expires_at": 2_000_i32}),
            json!({"expires_at": null, "usd_per_unit": {}}),
            json!({"expires_at": 2_000_i32, "usd_per_unit": null}),
            json!({"expires_at": "2000", "usd_per_unit": {}}),
            json!({"expires_at": 2_000.5_f64, "usd_per_unit": {}}),
            json!({"expires_at": 2_000_i32, "usd_per_unit": {"QAA": "2.5"}}),
            json!({"expires_at": 2_000_i32, "usd_per_unit": {"QAA": null}}),
            json!({"expires_at": 2_000_i32, "usd_per_unit": {}, "rates": {}}),
            json!({"expires_at": 2_000_i32, "usd_per_unit": {}, "feed_url": "https://example.test/rates"}),
        ] {
            assert!(
                serde_json::from_value::<FloorCurrencyConfig>(document.clone()).is_err(),
                "accepted invalid snapshot shape {document}"
            );
        }
    }
}

#[cfg(test)]
mod source_tests {
    use crate::{policy::OperatorPolicy as GenericAppConfig, source::test_support::TestSettings};
    type AuctioneerAppConfig = GenericAppConfig<TestSettings>;
    use serde_json::{Value, json};
    use validator::Validate as _;

    fn config_with_snapshot(snapshot: &Value) -> AuctioneerAppConfig {
        serde_json::from_value(json!({"schema_version": 1_u8, "floor_currency": snapshot})).unwrap()
    }

    #[test]
    fn floor_currency_absence_keeps_existing_runtime_config_valid() {
        let config: AuctioneerAppConfig =
            serde_json::from_value(json!({"schema_version": 1_u8})).unwrap();
        config.validate().unwrap();
        assert!(serde_json::to_value(config).unwrap()["floor_currency"].is_null());
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
                "schema_version": 1_u8,
                "floor_currency": snapshot
            }))
            .unwrap_err();
        }
    }

    #[test]
    fn floor_currency_root_parser_rejects_duplicate_rate_keys_not_as_unknown_root_field() {
        // Raw wire JSON is required to retain duplicate and escaped-equivalent keys.
        for wire in [
            r#"{"schema_version":1,"floor_currency":{"expires_at":2000,"usd_per_unit":{"QAA":1.25,"QAA":2.5}}}"#,
            r#"{"schema_version":1,"floor_currency":{"expires_at":2000,"usd_per_unit":{"QAA":1.25,"\u0051AA":2.5}}}"#,
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
            "schema_version": 1_u8,
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
