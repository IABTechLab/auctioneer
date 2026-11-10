//! Local SDK traits for the shared model without repeating its fields.

use super::AuctioneerAppConfig;
use super::validation::{
    BidderConfigurationError, CompiledPolicyError, PolicyValidationError, validate_compiled_policy,
};
use edgezero_core::app_config::{AppConfigMeta, AppConfigRoot, SecretField};
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
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
        validate_compiled_policy(
            &self.0,
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        )
        .map_err(|error| {
            let errors = validation_error(&error, &self.0);
            // The SDK extractor retains only the field path. Report the safe
            // application-owned reason here, before that redaction, for operators.
            log::error!("operator configuration validation failed: {errors}");
            errors
        })
    }
}

fn validation_error(error: &CompiledPolicyError, config: &AuctioneerAppConfig) -> ValidationErrors {
    let (code, message) = match error {
        CompiledPolicyError::Registry(_) => (
            "registry_build_failed",
            Cow::Borrowed("compiled bidder registry is invalid; rebuild the CLI and serving guest"),
        ),
        CompiledPolicyError::Policy(reason) => (
            "invalid_effective_config",
            policy_diagnostic(*reason, config),
        ),
    };
    let mut errors = ValidationErrors::new();
    errors.add("runtime", ValidationError::new(code).with_message(message));
    errors
}

fn policy_diagnostic(
    reason: PolicyValidationError,
    config: &AuctioneerAppConfig,
) -> Cow<'static, str> {
    if matches!(
        reason,
        PolicyValidationError::Bidder(BidderConfigurationError::SettingsBinding)
    ) {
        for settings in config
            .bidders
            .values()
            .filter_map(|entry| entry.settings.as_ref())
        {
            let declaration = auctioneer_bidder::bidder_declarations()
                .iter()
                .find(|row| row.key == settings.implementation_key());
            if let Some(missing) = declaration.filter(|row| !row.has_settings)
                && let Some(feature) = missing.required_feature
            {
                // Both identifiers come from compiled declarations, never the
                // configured alias name, endpoint, credential or rejected value.
                return Cow::Owned(format!(
                    "bidder implementation '{}' is unavailable in this build; rebuild both the CLI and serving guest with Cargo feature '{feature}'",
                    missing.key,
                ));
            }
        }
    }
    // PolicyValidationError contains only fixed categories and reviewed static
    // bidder field/reason identifiers. Never format a raw validator error here.
    Cow::Owned(reason.to_string())
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
        let config = AuctioneerAppConfig::default();
        let registry = super::validation_error(
            &super::CompiledPolicyError::Registry(
                auctioneer_bidder::RegistryBuildError::MissingBinding {
                    bidder: "fake_secret_canary",
                },
            ),
            &config,
        );
        let malformed = super::validation_error(
            &super::CompiledPolicyError::Policy(super::PolicyValidationError::UserSync),
            &config,
        );
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
    fn sdk_boundary_checks_resolved_sync_urls_and_priority_groups_without_leaking_values() {
        let valid = json!({
            "schema_version": 1_u8,
            "host": {"user_sync": {
                "external_url": "https://sync.example.test",
                "priority_groups": vec![vec!["fake_sync"; 1]; 32],
            }},
        });
        let accepted: SdkAuctioneerAppConfig = serde_json::from_value(valid.clone()).unwrap();
        accepted.validate().unwrap();
        for (field, rejected) in [
            ("external_url", json!("fake_private_url_canary")),
            (
                "priority_groups",
                json!(vec![vec!["fake_private_priority_canary"; 1]; 33]),
            ),
        ] {
            let mut document = valid.clone();
            document["host"]["user_sync"][field] = rejected;
            let model: AuctioneerAppConfig = serde_json::from_value(document).unwrap();
            model
                .validate()
                .expect("raw models establish structure, not effective sync safety");
            let error = SdkAuctioneerAppConfig(model).validate().unwrap_err();
            assert_eq!(
                serde_json::to_value(&error).unwrap(),
                json!({"runtime": [{"code": "invalid_effective_config", "message": "operator policy user sync invalid", "params": {}}]}),
                "the real SDK boundary must reject {field}",
            );
            for diagnostic in [error.to_string(), format!("{error:?}")] {
                assert!(!diagnostic.contains("fake_private_"));
            }
        }
    }

    #[test]
    fn sdk_validation_explains_the_safe_policy_refusal() {
        let config: SdkAuctioneerAppConfig = serde_json::from_value(json!({
            "schema_version": 1_u8,
            "host": {"user_sync": {"external_url": "fake_private_url_canary"}},
        }))
        .unwrap();
        let errors = config.validate().unwrap_err();
        assert!(
            errors
                .to_string()
                .contains("operator policy user sync invalid")
        );
        assert!(!format!("{errors:?}").contains("fake_private_url_canary"));
    }

    #[test]
    fn compiled_settings_owners_are_available_without_development_transport() {
        let mut checked = 0_usize;
        for declaration in auctioneer_bidder::bidder_declarations() {
            if !declaration.has_settings {
                continue;
            }
            let config = serde_json::from_value::<SdkAuctioneerAppConfig>(json!({
                "schema_version": 1_u8,
                "bidders": {(declaration.key): {
                    "enabled": false, "implementation": declaration.key, "settings": {},
                }},
            }))
            .unwrap();
            config.validate().unwrap();
            checked += 1;
        }
        assert!(checked > 0, "exercise compiled typed settings owners");
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
