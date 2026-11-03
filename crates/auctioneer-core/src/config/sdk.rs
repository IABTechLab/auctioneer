//! Local SDK traits for the shared model without repeating its fields.

use super::AuctioneerAppConfig;
use edgezero_core::app_config::{AppConfigMeta, AppConfigRoot, SecretField};
use serde::{Deserialize, Deserializer, Serialize};
use std::ops::{Deref, DerefMut};
use validator::{Validate, ValidateArgs, ValidationErrors};

/// Transparent SDK boundary; ordinary engine APIs use the contained model.
#[derive(Debug, Default, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct SdkAuctioneerAppConfig(pub AuctioneerAppConfig);

impl<'de> Deserialize<'de> for SdkAuctioneerAppConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        AuctioneerAppConfig::deserialize(deserializer).map(Self)
    }

    fn deserialize_in_place<D: Deserializer<'de>>(
        deserializer: D,
        place: &mut Self,
    ) -> Result<(), D::Error> {
        AuctioneerAppConfig::deserialize_in_place(deserializer, &mut place.0)
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
        self.0.validate()
    }
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
    fn sdk_wrapper_preserves_wire_shape_field_assignment_and_validation() {
        let mut wrapper = SdkAuctioneerAppConfig(AuctioneerAppConfig::default());
        wrapper.auction.default_tmax_ms = Some(700);
        wrapper.validate().unwrap();
        let document = serde_json::to_value(&wrapper.0).unwrap();
        assert_eq!(serde_json::to_value(&wrapper).unwrap(), document);
        let parsed_json: SdkAuctioneerAppConfig = serde_json::from_value(document.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed_json.0).unwrap(), document);
        let parsed_toml: SdkAuctioneerAppConfig =
            toml::from_str(&toml::to_string(&wrapper).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(parsed_toml.0).unwrap(), document);
        wrapper.auction.default_tmax_ms = Some(0);
        assert!(wrapper.validate().is_err());
    }

    #[test]
    fn sdk_wrapper_forwards_in_place_deserialization_to_the_shared_model() {
        let mut wrapper = SdkAuctioneerAppConfig(AuctioneerAppConfig::default());
        wrapper.auction.default_tmax_ms = Some(700);
        let document = json!({"targeting": {"prefix": "pb"}});
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
