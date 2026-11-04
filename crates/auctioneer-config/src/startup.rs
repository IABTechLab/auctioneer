//! Staged startup source intent; decoding is not domain or resource admission.

use std::fmt;

use crate::model_validation::{deserialize_optional_value, structural_validation_result};
use crate::object::deserialize_object;
use serde::{Deserialize, Deserializer, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

/// Typed startup source selection, kept separate from replaceable policy.
/// This staged model accepts source intent only. Resource/health settings and
/// deployment resolution remain separate reviewed startup work; validation
/// here does not establish input bounds, funding or a usable store binding.
#[derive(Debug, Default, Serialize, Validate)]
pub struct StartupConfig {
    /// Omission requests the standalone store source, never a fallback mode.
    #[validate(nested)]
    pub config: PolicySource,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupFields {
    #[serde(default, deserialize_with = "deserialize_object")]
    config: PolicySource,
}

impl<'de> Deserialize<'de> for StartupConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields: StartupFields = deserialize_object(deserializer)?;
        Ok(Self {
            config: fields.config,
        })
    }
}

/// Exactly one policy owner for a future immutable configuration domain.
/// If the config section is supplied, its discriminator is required. Only
/// omission of the whole section selects the standalone store default.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
#[expect(
    clippy::empty_enum_variants_with_brackets,
    reason = "Serde unit variants ignore unknown fields; struct variants reject cross-mode options"
)]
pub enum PolicySource {
    /// Read from a deployment-approved logical binding and key.
    Store(StorePolicySource),
    /// Use a snapshot supplied by the embedding host.
    Host {},
    /// Use product defaults only when this mode is explicitly selected.
    ProductDefaults {},
}

impl Default for PolicySource {
    fn default() -> Self {
        Self::Store(StorePolicySource::default())
    }
}

/// Store-source requests; omitted values require explicit deployment resolution.
/// Structural validation preserves SDK identifier/key rules; adapter-specific
/// constraints, binding membership and timeout funding are not established here.
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorePolicySource {
    /// Logical config binding, not a physical provider location.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub store: Option<String>,
    /// Key subject to the selected adapter's binding constraints.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub key: Option<String>,
    /// Extraction-wide timeout intent, resolved and installed before reads.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_config_fetch_ms: Option<u32>,
}

impl Validate for PolicySource {
    fn validate(&self) -> Result<(), ValidationErrors> {
        match self {
            Self::Store(store) => store.validate(),
            Self::Host {} | Self::ProductDefaults {} => Ok(()),
        }
    }
}

impl fmt::Debug for StorePolicySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StorePolicySource(<redacted>)")
    }
}

impl Validate for StorePolicySource {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let view = StoreSourceValidation {
            store: self.store.as_deref(),
            key: self.key.as_deref(),
            max_config_fetch_ms: self.max_config_fetch_ms,
        };
        match view.validate() {
            Ok(()) => Ok(()),
            Err(errors) => structural_validation_result(errors),
        }
    }
}

#[derive(Validate)]
struct StoreSourceValidation<'a> {
    #[validate(length(min = 1_u64))]
    #[validate(custom(function = "validate_logical_store_id"))]
    store: Option<&'a str>,
    #[validate(length(min = 1_u64))]
    #[validate(custom(function = "validate_config_key"))]
    key: Option<&'a str>,
    // EdgeZero's existing extraction deadline ceiling is 168 hours, not a
    // newly selected workload or memory reservation.
    #[validate(range(min = 1_u32, max = 604_800_000_u32))]
    max_config_fetch_ms: Option<u32>,
}

fn validate_logical_store_id(value: &str) -> Result<(), ValidationError> {
    if !value.contains("__")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        Ok(())
    } else {
        Err(ValidationError::new("logical_store_id"))
    }
}

fn validate_config_key(value: &str) -> Result<(), ValidationError> {
    // Empty input has its own derived length error. Accepted keys are not
    // trimmed or restricted to the logical store identifier's grammar.
    if value.is_empty() || (!value.trim().is_empty() && !value.chars().any(char::is_control)) {
        Ok(())
    } else {
        Err(ValidationError::new("config_key"))
    }
}

#[cfg(test)]
mod tests {
    use super::{PolicySource, StartupConfig};
    use serde_json::{Value, error::Category, json};
    use validator::Validate as _;

    fn decode(document: Value) -> StartupConfig {
        serde_json::from_value(document).expect("the startup fixture has valid types")
    }

    #[test]
    fn absent_source_requests_store_without_selecting_deployment_values() {
        let startup = decode(json!({}));
        startup.validate().unwrap();
        let PolicySource::Store(store) = &startup.config else {
            panic!("standalone omission must select store, never defaults mode");
        };
        assert_eq!(store.store, None);
        assert_eq!(store.key, None);
        assert_eq!(store.max_config_fetch_ms, None);
        assert_eq!(
            serde_json::to_value(&startup).unwrap(),
            json!({"config": {"source": "store"}})
        );
        let from_toml: StartupConfig = toml::from_str("").unwrap();
        assert_eq!(
            serde_json::to_value(from_toml).unwrap(),
            json!({"config": {"source": "store"}})
        );
    }

    #[test]
    fn source_modes_round_trip_json_and_toml_without_policy_io() {
        let documents = [
            json!({"config": {"source": "store", "store": "fake_config", "key": "fake_key", "max_config_fetch_ms": 2_000_u32}}),
            json!({"config": {"source": "host"}}),
            json!({"config": {"source": "product_defaults"}}),
        ];
        assert_eq!(documents.len(), 3);
        for document in documents {
            let startup = decode(document.clone());
            startup.validate().unwrap();
            assert_eq!(serde_json::to_value(&startup).unwrap(), document);
            let encoded = toml::to_string(&startup).unwrap();
            let restored: StartupConfig = toml::from_str(&encoded).unwrap();
            assert_eq!(serde_json::to_value(restored).unwrap(), document);
        }
    }

    #[test]
    fn source_specific_fields_cannot_cross_modes() {
        let mut checked = 0_usize;
        for mode in ["host", "product_defaults"] {
            for (field, value) in [
                ("store", json!("fake_config")),
                ("key", json!("fake_key")),
                ("max_config_fetch_ms", json!(2_000_u32)),
            ] {
                for supplied in [value, json!(null)] {
                    let document = json!({"config": {"source": mode, field: supplied}});
                    let error = serde_json::from_value::<StartupConfig>(document.clone())
                        .expect_err("non-store modes must not retain store-source options");
                    assert_eq!(error.classify(), Category::Data);
                    assert!(
                        error
                            .to_string()
                            .contains(&format!("unknown field `{field}`"))
                    );
                    // TOML has no null value; lexical null controls stay in JSON.
                    if !supplied.is_null() {
                        let text = toml::to_string(&document).unwrap();
                        let toml_error = toml::from_str::<StartupConfig>(&text).unwrap_err();
                        assert!(
                            toml_error
                                .to_string()
                                .contains(&format!("unknown field `{field}`"))
                        );
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 12);
    }

    #[test]
    fn explicit_source_table_requires_its_discriminator() {
        for document in [
            json!({"config": {}}),
            json!({"config": {"key": "fake_key"}}),
        ] {
            let error = serde_json::from_value::<StartupConfig>(document).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
            assert!(error.to_string().contains("missing field `source`"));
        }
    }

    #[test]
    fn startup_and_source_sections_never_accept_positional_arrays() {
        for document in [
            json!([]),
            json!([{"source": "store"}]),
            json!({"config": ["store"]}),
            json!({"config": ["host"]}),
            json!({"config": ["product_defaults"]}),
        ] {
            let error = serde_json::from_value::<StartupConfig>(document).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
            assert!(error.to_string().contains("invalid type"));
        }
        for text in [
            "config = []",
            "config = [\"store\"]",
            "config = [\"host\"]",
            "config = [\"product_defaults\"]",
        ] {
            let error = toml::from_str::<StartupConfig>(text).unwrap_err();
            assert!(error.to_string().contains("invalid type"));
        }
    }

    #[test]
    fn startup_rejects_unknown_and_unapproved_fields() {
        for document in [
            json!({"baked": {}}),
            json!({"resources": {"baseline_bytes": 1_u64}}),
            json!({"config": {"source": "store", "max_json_depth": 128_u32}}),
            json!({"config": {"source": "store", "unknown": true}}),
        ] {
            let error = serde_json::from_value::<StartupConfig>(document).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
            assert!(error.to_string().contains("unknown field"));
        }
    }

    #[test]
    fn explicit_null_never_means_inherited_startup_input() {
        let mut checked = 0_usize;
        for field in ["store", "key", "max_config_fetch_ms"] {
            let document = json!({"config": {"source": "store", field: null}});
            let error = serde_json::from_value::<StartupConfig>(document).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
            checked += 1;
        }
        let error = serde_json::from_value::<StartupConfig>(json!({"config": null})).unwrap_err();
        assert_eq!(error.classify(), Category::Data);
        assert_eq!(checked, 3);
    }

    #[test]
    fn store_source_validates_each_supplied_value_with_fixed_errors() {
        for (field, value, code) in [
            ("store", json!(""), "length"),
            ("key", json!(""), "length"),
            ("max_config_fetch_ms", json!(0_u32), "range"),
            ("store", json!("fake__store"), "logical_store_id"),
            ("store", json!("fake-store"), "logical_store_id"),
            ("store", json!("fake.store"), "logical_store_id"),
            ("store", json!("fake/store"), "logical_store_id"),
            ("store", json!("fak\u{e9}"), "logical_store_id"),
            ("key", json!("  \t"), "config_key"),
            ("key", json!("fake\u{0}key"), "config_key"),
            ("key", json!("fake\u{85}key"), "config_key"),
        ] {
            let startup = decode(json!({"config": {"source": "store", field: value}}));
            let errors = startup
                .validate()
                .expect_err("invalid explicit values must refuse");
            assert_eq!(
                serde_json::to_value(errors).unwrap(),
                json!({"config": {field: [{"code": code, "message": null, "params": {}}]}})
            );
        }
    }

    #[test]
    fn logical_ids_do_not_impose_their_grammar_on_store_keys() {
        for store in ["fake_store", "1fake", "_fake", "FAKE"] {
            let startup = decode(
                json!({"config": {"source": "store", "store": store, "key": " fake/key:\u{e9}__value "}}),
            );
            startup.validate().unwrap();
            let PolicySource::Store(source) = startup.config else {
                panic!("the explicit store source must be preserved");
            };
            assert_eq!(source.store.as_deref(), Some(store));
            assert_eq!(source.key.as_deref(), Some(" fake/key:\u{e9}__value "));
        }
    }

    #[test]
    fn config_timeout_obeys_the_existing_sdk_deadline_ceiling() {
        let cases = [
            (0_u32, false),
            (1, true),
            (604_800_000, true),
            (604_800_001, false),
            (u32::MAX, false),
        ];
        assert_eq!(cases.len(), 5);
        for (milliseconds, valid) in cases {
            let startup =
                decode(json!({"config": {"source": "store", "max_config_fetch_ms": milliseconds}}));
            let result = startup.validate();
            if valid {
                result.unwrap();
            } else {
                assert_eq!(
                    serde_json::to_value(result.unwrap_err()).unwrap(),
                    json!({"config": {"max_config_fetch_ms": [{"code": "range", "message": null, "params": {}}]}})
                );
            }
        }
    }

    #[test]
    fn startup_debug_does_not_disclose_selector_values() {
        let startup = decode(
            json!({"config": {"source": "store", "store": "private_prefix_store_suffix", "key": "private_prefix_key_suffix"}}),
        );
        for debug in [format!("{startup:?}"), format!("{startup:#?}")] {
            for secret in ["private", "prefix", "suffix", "store_suffix", "key_suffix"] {
                assert!(
                    !debug.contains(secret),
                    "startup Debug exposed an input fragment"
                );
            }
            assert!(debug.contains("<redacted>"));
        }
    }
}
