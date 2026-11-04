//! Local SDK traits for the shared model without repeating its fields.

use super::{AuctioneerAppConfig, EffectiveConfigError, validate_effective_defaults};
use edgezero_core::app_config::{AppConfigMeta, AppConfigRoot, SecretField};
use serde::{Deserialize, Deserializer, Serialize};
use std::ops::{Deref, DerefMut};
use validator::{Validate, ValidateArgs, ValidationError, ValidationErrors};

/// Transparent SDK boundary; ordinary engine APIs use the contained model.
#[derive(Debug, Default, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct SdkAuctioneerAppConfig(pub AuctioneerAppConfig);

impl<'de> Deserialize<'de> for SdkAuctioneerAppConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <AuctioneerAppConfig as Deserialize>::deserialize(deserializer).map(Self)
    }

    fn deserialize_in_place<D: Deserializer<'de>>(
        deserializer: D,
        place: &mut Self,
    ) -> Result<(), D::Error> {
        <AuctioneerAppConfig as Deserialize>::deserialize_in_place(deserializer, &mut place.0)
    }
}

impl Deref for SdkAuctioneerAppConfig {
    type Target = AuctioneerAppConfig;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SdkAuctioneerAppConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Validate for SdkAuctioneerAppConfig {
    fn validate(&self) -> Result<(), ValidationErrors> {
        validate_effective_defaults(&self.0).map_err(|error| validation_error(&error))
    }
}

fn validation_error(error: &EffectiveConfigError) -> ValidationErrors {
    let code = match error {
        EffectiveConfigError::Registry(_) => "registry_build_failed",
        EffectiveConfigError::Malformed(_) => "invalid_effective_config",
    };
    let mut errors = ValidationErrors::new();
    errors.add("runtime", ValidationError::new(code));
    errors
}

impl ValidateArgs<'_> for SdkAuctioneerAppConfig {
    type Args = ();

    fn validate_with_args(&self, (): Self::Args) -> Result<(), ValidationErrors> {
        self.validate()
    }
}

impl AppConfigMeta for SdkAuctioneerAppConfig {
    fn secret_fields() -> Vec<SecretField> {
        // The original root's derive had neither secret annotations nor nested
        // metadata. Credentials retain bidder-owned reference binding instead.
        Vec::new()
    }
}

impl AppConfigRoot for SdkAuctioneerAppConfig {}

#[cfg(test)]
mod tests {
    use super::{AppConfigMeta as _, AuctioneerAppConfig, SdkAuctioneerAppConfig};
    use serde::Deserialize as _;
    use serde_json::json;
    use std::mem::{align_of, size_of};
    use validator::Validate as _;

    #[test]
    fn sdk_boundary_accepts_the_direct_runtime_policy_without_other_config_scopes() {
        let document = json!({
            "schema_version": 1_u8,
            "host": {"status_response": "fake_runtime_only"},
            "account_defaults": {"targeting_prefix": "fake"}
        });
        let config: SdkAuctioneerAppConfig = serde_json::from_value(document.clone()).unwrap();
        config.validate().unwrap();
        let emitted = serde_json::to_value(&config).unwrap();
        assert_eq!(emitted["schema_version"], document["schema_version"]);
        assert_eq!(emitted["host"]["status_response"], "fake_runtime_only");
        assert_eq!(emitted["account_defaults"]["targeting_prefix"], "fake");
        for retired in ["baked", "startup", "runtime"] {
            assert!(emitted.get(retired).is_none());
            let mut invalid = document.clone();
            invalid[retired] = json!({});
            serde_json::from_value::<SdkAuctioneerAppConfig>(invalid)
                .expect_err("retired root sections must be rejected by the SDK policy wrapper");
        }
    }

    #[test]
    fn sdk_validation_keeps_registry_and_operator_refusals_distinct_without_input_values() {
        let registry = super::validation_error(&super::EffectiveConfigError::Registry(
            auctioneer_bidder::RegistryBuildError::MissingBinding {
                bidder: "fake_secret_canary",
            },
        ));
        let malformed = super::validation_error(&super::EffectiveConfigError::Malformed(
            "fake_secret_canary".to_owned(),
        ));
        assert_eq!(
            registry.field_errors()["runtime"][0].code,
            "registry_build_failed"
        );
        assert_eq!(
            malformed.field_errors()["runtime"][0].code,
            "invalid_effective_config"
        );
        for errors in [registry, malformed] {
            assert!(!format!("{errors:?}").contains("fake_secret_canary"));
            assert!(errors.field_errors()["runtime"][0].params.is_empty());
        }
    }

    #[test]
    fn sdk_wrapper_keeps_model_layout_and_existing_empty_secret_metadata() {
        assert_eq!(
            size_of::<SdkAuctioneerAppConfig>(),
            size_of::<AuctioneerAppConfig>()
        );
        assert_eq!(
            align_of::<SdkAuctioneerAppConfig>(),
            align_of::<AuctioneerAppConfig>()
        );
        assert!(SdkAuctioneerAppConfig::secret_fields().is_empty());
    }

    #[test]
    fn sdk_boundary_keeps_object_only_decoding_at_each_policy_scope() {
        for document in [
            json!([1_u8, {}, {}, {}, {}, {}]),
            json!({"schema_version": 1_u8, "host": []}),
            json!({"schema_version": 1_u8, "account_defaults": []}),
            json!({"schema_version": 1_u8, "accounts": {"fake_publisher": []}}),
        ] {
            serde_json::from_value::<AuctioneerAppConfig>(document.clone())
                .expect_err("the canonical policy must reject array-valued policy scopes");
            serde_json::from_value::<SdkAuctioneerAppConfig>(document)
                .expect_err("the SDK wrapper must reject array-valued policy scopes");
        }
    }

    #[test]
    fn sdk_wrapper_preserves_wire_shape_field_assignment_and_validation() {
        let mut wrapper = SdkAuctioneerAppConfig(AuctioneerAppConfig::default());
        wrapper.host.resource_limits.default_tmax_ms = Some(700);
        wrapper.validate().unwrap();
        let document = serde_json::to_value(&wrapper.0).unwrap();
        assert_eq!(serde_json::to_value(&wrapper).unwrap(), document);
        let parsed_json: SdkAuctioneerAppConfig = serde_json::from_value(document.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed_json.0).unwrap(), document);
        let parsed_toml: SdkAuctioneerAppConfig =
            toml::from_str(&toml::to_string(&wrapper).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(parsed_toml.0).unwrap(), document);
        wrapper.host.resource_limits.default_tmax_ms = Some(0);
        assert!(wrapper.validate().is_err());
    }

    #[test]
    fn sdk_wrapper_forwards_in_place_deserialization_to_the_shared_model() {
        let mut wrapper = SdkAuctioneerAppConfig(AuctioneerAppConfig::default());
        wrapper.host.resource_limits.default_tmax_ms = Some(700);
        let document =
            json!({"schema_version": 1_u8, "account_defaults": {"targeting_prefix": "pb"}});
        let wire = serde_json::to_string(&document).unwrap();
        let mut deserializer = serde_json::Deserializer::from_str(&wire);
        SdkAuctioneerAppConfig::deserialize_in_place(&mut deserializer, &mut wrapper).unwrap();
        let direct: AuctioneerAppConfig = serde_json::from_value(document).unwrap();
        assert_eq!(
            serde_json::to_value(&wrapper).unwrap(),
            serde_json::to_value(direct).unwrap()
        );
    }
}
