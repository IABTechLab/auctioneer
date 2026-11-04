//! The strict versioned operator authoring and runtime payload.
//!
//! Deserialization establishes shape, not semantic or resource admission.
//! Effective policy and resource admission remain the engine's responsibility.
//! Optional fields reject null; nullable effective fields use explicit clears.

use std::{collections::BTreeMap, fmt};

use crate::FloorCurrencyConfig;
use crate::model_validation::{
    deserialize_optional_value, serialize_optional_u8_keyed_map, structural_validation_result,
};
use crate::object::{ObjectFields, deserialize_fields};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::override_value::FieldOverride;
use crate::source::{
    AccountCcpaOverride, AccountPrivacyOverride, AuctionConfig, BidderOverrides,
    ChannelEnabledOverride, CooperativeOverride, FeatureFlags, HostEnforcementOverride,
    HostGdprOverride, PurposeEnforcementOverride, SpecialFeatureEnforcementOverride,
    deserialize_u8_keyed_map, validate_bid_rounding, validate_bidder_map,
    validate_host_alias_overlay, validate_purpose_id_keys, validate_response_cap,
    validate_special_feature_id_keys, validate_status_banner, validate_targeting_prefix,
    validate_targeting_truncation,
};
use crate::types::TriState;

/// The one supported application policy format, independent of the SDK envelope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplicationSchemaVersion;

impl Serialize for ApplicationSchemaVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(1)
    }
}

impl<'de> Deserialize<'de> for ApplicationSchemaVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let version = u64::deserialize(deserializer)
            .map_err(|_error| D::Error::custom("invalid_application_schema_version"))?;
        if version == 1 {
            Ok(Self)
        } else {
            Err(D::Error::custom("unsupported_application_schema_version"))
        }
    }
}

pub use object_models::{AccountPolicy, HostPolicy, OperatorPolicy};

#[expect(
    clippy::same_name_method,
    reason = "Serde remote Self generates field helpers used by the object-guarded trait impls"
)]
mod object_models {
    use super::{
        AccountCcpaOverride, AccountCookieSyncPolicy, AccountGdprPolicy, AccountPrivacyOverride,
        ApplicationSchemaVersion, AuctionConfig, BTreeMap, BidderOverrides, FeatureFlags,
        FieldOverride, FloorCurrencyConfig, HostCookiePolicy, HostEnforcementOverride,
        HostGdprOverride, UserSyncPolicy, deserialize_optional_value, validate_bid_rounding,
        validate_response_cap, validate_targeting_prefix,
    };
    use serde::{Deserialize, Serialize};
    use validator::Validate;

    /// One complete policy publication. Omission inherits only within this document.
    #[derive(Deserialize, Serialize)]
    #[serde(
        remote = "Self",
        deny_unknown_fields,
        bound(deserialize = "Settings: Deserialize<'de>")
    )]
    pub struct OperatorPolicy<Settings: Validate> {
        /// Required on the wire; programmatic defaults select this format, not admission.
        pub schema_version: ApplicationSchemaVersion,
        /// Host policy; never startup bindings, extraction caps or provider facts.
        #[serde(default)]
        pub host: HostPolicy,
        /// Deployment-wide account overrides applied before a selected account.
        #[serde(default)]
        pub account_defaults: AccountPolicy,
        /// Publisher-specific overrides, not independently compiled account defaults.
        #[serde(default)]
        pub accounts: BTreeMap<String, AccountPolicy>,
        /// Shared tunables and generated, implementation-tagged protocol settings.
        #[serde(default)]
        pub bidders: BTreeMap<String, BidderOverrides<Settings>>,
        /// Supported runtime feature requests, not new compiled capabilities.
        #[serde(default)]
        pub features: FeatureFlags,
        /// Operator-provided floor-only conversion snapshot.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub floor_currency: Option<FloorCurrencyConfig>,
    }

    /// Optional host policy requests over product defaults.
    #[derive(Debug, Default, Deserialize, Serialize, Validate)]
    #[serde(remote = "Self", deny_unknown_fields)]
    pub struct HostPolicy {
        /// Requested auction limits; installed capacity remains independently authoritative.
        #[serde(default)]
        #[validate(nested)]
        pub resource_limits: AuctionConfig,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(nested)]
        pub gdpr: Option<HostGdprOverride>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub ccpa: Option<HostEnforcementOverride>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub lmt: Option<HostEnforcementOverride>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub host_cookie: Option<HostCookiePolicy>,
        /// Inherited or cleared means no service unless the explicit parent provides one.
        #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
        pub user_sync: FieldOverride<UserSyncPolicy>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub status_response: Option<String>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub aliases: Option<BTreeMap<String, String>>,
    }

    /// The same strict overlay type is used for account defaults and named accounts.
    #[derive(Debug, Default, Deserialize, Serialize, Validate)]
    #[serde(remote = "Self", deny_unknown_fields)]
    pub struct AccountPolicy {
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(custom(function = "validate_response_cap"))]
        pub max_transport_bytes: Option<u64>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(custom(function = "validate_response_cap"))]
        pub max_decoded_bytes: Option<u64>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub disabled: Option<bool>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub debug_allow: Option<bool>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        pub cookie_sync: Option<AccountCookieSyncPolicy>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(custom(function = "validate_bid_rounding"))]
        pub bid_rounding: Option<String>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(custom(function = "validate_targeting_prefix"))]
        pub targeting_prefix: Option<String>,
        #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
        pub truncate_target_attribute: FieldOverride<usize>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(nested)]
        pub gdpr: Option<AccountGdprPolicy>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(nested)]
        pub ccpa: Option<AccountCcpaOverride>,
        #[serde(
            default,
            deserialize_with = "deserialize_optional_value",
            skip_serializing_if = "Option::is_none"
        )]
        #[validate(nested)]
        pub privacy: Option<AccountPrivacyOverride>,
    }
}

impl<Settings: Validate> Default for OperatorPolicy<Settings> {
    fn default() -> Self {
        Self {
            schema_version: ApplicationSchemaVersion,
            host: HostPolicy::default(),
            account_defaults: AccountPolicy::default(),
            accounts: BTreeMap::new(),
            bidders: BTreeMap::new(),
            features: FeatureFlags::default(),
            floor_currency: None,
        }
    }
}

impl<'de, Settings: Validate + Deserialize<'de>> ObjectFields<'de> for OperatorPolicy<Settings> {
    fn deserialize_fields<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::deserialize(deserializer)
    }
}

impl<'de, Settings: Validate + Deserialize<'de>> Deserialize<'de> for OperatorPolicy<Settings> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_fields(deserializer)
    }
}

impl<Settings: Validate + Serialize> Serialize for OperatorPolicy<Settings> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<Settings: Validate> validator::ValidateArgs<'_> for OperatorPolicy<Settings> {
    type Args = ();

    fn validate_with_args(&self, (): Self::Args) -> Result<(), ValidationErrors> {
        self.validate()
    }
}

/// Shared structural checks only; effective policy admission belongs to the engine.
impl<Settings: Validate> Validate for OperatorPolicy<Settings> {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        errors.merge_self("host", validate_host_policy(&self.host));
        errors.merge_self(
            "account_defaults",
            validate_account_policy(&self.account_defaults),
        );
        if self
            .accounts
            .values()
            .any(|account| validate_account_policy(account).is_err())
        {
            errors.add("accounts", ValidationError::new("account_override_invalid"));
        }
        if let Err(error) = validate_bidder_map(&self.bidders) {
            errors.add("bidders", error);
        }
        if let Some(snapshot) = &self.floor_currency
            && let Err(code) = snapshot.validate()
        {
            errors.add("floor_currency", ValidationError::new(code));
        }
        structural_validation_result(errors)
    }
}

impl<Settings: Validate> fmt::Debug for OperatorPolicy<Settings> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OperatorPolicy(<redacted>)")
    }
}

impl<'de> ObjectFields<'de> for HostPolicy {
    fn deserialize_fields<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::deserialize(deserializer)
    }
}

impl<'de> Deserialize<'de> for HostPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_fields(deserializer)
    }
}

impl Serialize for HostPolicy {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

// Root-only rules keep map keys and clear wrappers out of derive's value params.
fn validate_host_policy(host: &HostPolicy) -> Result<(), ValidationErrors> {
    let mut errors = host.validate().err().unwrap_or_default();
    if let Some(banner) = &host.status_response
        && let Err(error) = validate_status_banner(banner)
    {
        errors.add("status_response", error);
    }
    if let Some(aliases) = &host.aliases
        && let Err(error) = validate_host_alias_overlay(aliases)
    {
        errors.add("aliases", error);
    }
    structural_validation_result(errors)
}

/// Cookie policy with explicit resets only on nullable effective fields.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostCookiePolicy {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub cookie_name: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub domain: FieldOverride<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub ttl_days: Option<i64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_cookie_size_bytes: Option<usize>,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub optout_cookie: FieldOverride<OptoutCookiePolicy>,
}

/// Optional leaves merge independently; only the complete cookie may be cleared.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OptoutCookiePolicy {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub value: Option<String>,
}

fn deserialize_optional_u8_keyed_map<'de, D, V>(
    deserializer: D,
) -> Result<Option<BTreeMap<u8, V>>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    let Some(value) = deserialize_u8_keyed_map(deserializer)? else {
        return Err(D::Error::custom("an enforcement map must not be null"));
    };
    Ok(Some(value))
}

/// An explicitly configured service; it has no example-URL default.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserSyncPolicy {
    /// Required explicit origin, subject to existing effective URL safety checks.
    pub external_url: String,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub redirect_url: FieldOverride<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub cooperative: Option<CooperativeOverride>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub priority_groups: Option<Vec<Vec<String>>>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub allowed_origins: Option<Vec<String>>,
}

impl<'de> ObjectFields<'de> for AccountPolicy {
    fn deserialize_fields<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::deserialize(deserializer)
    }
}

impl<'de> Deserialize<'de> for AccountPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_fields(deserializer)
    }
}

impl Serialize for AccountPolicy {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

fn validate_account_policy(account: &AccountPolicy) -> Result<(), ValidationErrors> {
    let mut errors = account.validate().err().unwrap_or_default();
    if let FieldOverride::Value(value) = &account.truncate_target_attribute
        && let Err(error) = validate_targeting_truncation(*value)
    {
        errors.add("truncate_target_attribute", error);
    }
    structural_validation_result(errors)
}

/// Nullable sync overrides preserve absence, clear, false, zero and empty groups.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountCookieSyncPolicy {
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub default_limit: FieldOverride<i64>,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub max_limit: FieldOverride<i64>,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub default_coop_sync: FieldOverride<bool>,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub priority_groups: FieldOverride<Vec<Vec<String>>>,
}

/// Account privacy enforcement with explicit host-fallback resets for treatment.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountGdprPolicy {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub enabled: Option<TriState>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub purpose_one_treatment: Option<AccountTreatmentPolicy>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub channel_enabled: Option<ChannelEnabledOverride>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_u8_keyed_map",
        serialize_with = "serialize_optional_u8_keyed_map",
        skip_serializing_if = "Option::is_none"
    )]
    pub purposes: Option<BTreeMap<u8, PurposeEnforcementOverride>>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_u8_keyed_map",
        serialize_with = "serialize_optional_u8_keyed_map",
        skip_serializing_if = "Option::is_none"
    )]
    pub special_features: Option<BTreeMap<u8, SpecialFeatureEnforcementOverride>>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_enforcement_vendors: Option<Vec<String>>,
}

impl Validate for AccountGdprPolicy {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        if let Some(purposes) = &self.purposes
            && let Err(error) = validate_purpose_id_keys(purposes)
        {
            errors.add("purposes", error);
        }
        if let Some(features) = &self.special_features
            && let Err(error) = validate_special_feature_id_keys(features)
        {
            errors.add("special_features", error);
        }
        structural_validation_result(errors)
    }
}

/// Clearing either optional boolean restores the host's treatment fallback.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountTreatmentPolicy {
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub enabled: FieldOverride<bool>,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub access_allowed: FieldOverride<bool>,
}

#[cfg(test)]
mod tests {
    use crate::source::test_support::TestSettings;
    type OperatorPolicy = super::OperatorPolicy<TestSettings>;
    use crate::override_value::FieldOverride;
    use serde_json::{Value, json};
    use validator::Validate as _;

    fn assert_no_null(value: &Value) {
        match value {
            Value::Object(fields) => fields.values().for_each(assert_no_null),
            Value::Array(values) => values.iter().for_each(assert_no_null),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                assert!(!value.is_null(), "absent optional leaves must be omitted");
            }
        }
    }

    fn collect_policy_paths(value: &Value, path: &str, paths: &mut Vec<String>) {
        match value {
            Value::Object(fields) => {
                for (field, child_value) in fields {
                    let child_path = format!("{path}/{field}");
                    paths.push(child_path.clone());
                    collect_policy_paths(child_value, &child_path, paths);
                }
            }
            Value::Array(values) => {
                for (index, element) in values.iter().enumerate() {
                    collect_policy_paths(element, &format!("{path}/{index}"), paths);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    #[test]
    fn sparse_nested_policy_serialization_omits_absent_leaves_in_every_scope() {
        let account = json!({
            "cookie_sync": {},
            "gdpr": {"purpose_one_treatment": {}, "channel_enabled": {},
                "purposes": {"1": {}}, "special_features": {"1": {}}},
            "ccpa": {"channel_enabled": {}},
            "privacy": {"ipv4": {}, "ipv6": {}, "allowactivities": {"fetchBids": {}}}
        });
        let document = json!({
            "schema_version": 1_u64,
            "host": {
                "resource_limits": {},
                "gdpr": {"tcf2": {"purpose_one_treatment": {},
                    "purposes": {"1": {}}, "special_features": {"1": {}}}},
                "ccpa": {}, "lmt": {},
                "host_cookie": {"optout_cookie": {}},
                "user_sync": {"external_url": "https://sync.example.test", "cooperative": {}}
            },
            "account_defaults": account,
            "accounts": {"fake_publisher": account},
            "bidders": {"fake_sparse": {}},
            "features": {}
        });
        for input in [
            json!({"schema_version": 1_u64, "host": {"resource_limits": {}},
                "account_defaults": {}, "accounts": {}, "bidders": {}, "features": {}}),
            document,
        ] {
            let policy: OperatorPolicy = serde_json::from_value(input.clone()).unwrap();
            policy.validate().unwrap();
            let emitted = serde_json::to_value(&policy).unwrap();
            assert_no_null(&emitted);
            assert_eq!(emitted, input);
            let json: OperatorPolicy = serde_json::from_value(emitted.clone()).unwrap();
            let toml: OperatorPolicy = toml::from_str(&toml::to_string(&policy).unwrap()).unwrap();
            assert_eq!(serde_json::to_value(json).unwrap(), emitted);
            assert_eq!(serde_json::to_value(toml).unwrap(), emitted);
        }
    }

    #[test]
    fn borrowed_leaves_round_trip_without_emitting_null_or_integer_toml_keys() {
        let document = json!({
            "schema_version": 1_u64,
            "host": {"gdpr": {"tcf2": {
                "purpose_one_treatment": {},
                "purposes": {"1": {}},
                "special_features": {"1": {}}
            }}, "ccpa": {}, "lmt": {}, "user_sync": {
                "external_url": "https://sync.example.test", "cooperative": {}
            }},
            "account_defaults": {"privacy": {"allowactivities": {
                "fetchBids": {},
                "transmitUfpd": {"default": false, "rules": [{"allow": false}, {"allow": true, "condition": {}}]}
            }}, "ccpa": {"channel_enabled": {}}, "gdpr": {
                "purposes": {"1": {}}, "special_features": {"1": {}}
            }},
            "bidders": {"fake": {}}, "features": {}
        });
        let policy: OperatorPolicy = serde_json::from_value(document).unwrap();
        policy.validate().unwrap();
        let canonical = serde_json::to_value(&policy).unwrap();
        let json_restored: OperatorPolicy = serde_json::from_value(canonical.clone()).unwrap();
        assert_eq!(serde_json::to_value(json_restored).unwrap(), canonical);
        let toml_restored: OperatorPolicy =
            toml::from_str(&toml::to_string(&policy).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(toml_restored).unwrap(), canonical);
        assert_eq!(
            canonical["host"]["gdpr"]["tcf2"]["purpose_one_treatment"],
            json!({})
        );
        assert_eq!(
            canonical["account_defaults"]["privacy"]["allowactivities"]["fetchBids"],
            json!({})
        );
        assert_eq!(
            canonical["account_defaults"]["privacy"]["allowactivities"]["transmitUfpd"]["rules"][0],
            json!({"allow": false})
        );
    }

    #[test]
    fn borrowed_leaves_reject_nested_nulls_and_unknown_fields() {
        let base = json!({
            "schema_version": 1_u64,
            "host": {
                "resource_limits": {"default_tmax_ms": 500_u64},
                "gdpr": {"enabled": true, "host_vendor_id": 1_u64, "non_standard_publishers": [], "default_value": "0", "tcf2": {
                    "enabled": false,
                    "purpose_one_treatment": {"enabled": false, "access_allowed": true},
                    "purposes": {"1": {"enforce_algo": "full", "enforce_purpose": "enabled", "enforce_vendors": "unset", "vendor_exceptions": []}},
                    "special_features": {"1": {"enforce": "enabled", "vendor_exceptions": []}}
                }},
                "ccpa": {"enforce": false}, "lmt": {"enforce": true},
                "user_sync": {"external_url": "https://sync.example.test", "cooperative": {"enabled_by_default": false}}
            },
            "account_defaults": {
                "ccpa": {"enabled": "unset", "channel_enabled": {"web": "enabled"}},
                "gdpr": {"channel_enabled": {"app": "disabled"}, "purposes": {"1": {"enforce_purpose": "unset"}}, "special_features": {"1": {"enforce": "enabled"}}},
                "privacy": {"ipv4": {"anon_keep_bits": 24_u8}, "ipv6": {"anon_keep_bits": 64_u8}, "allowactivities": {"fetchBids": {"default": false, "rules": [{"allow": false, "condition": {"componentName": ["fake"], "componentType": ["bidder"]}}]}}}
            },
            "bidders": {"fake": {"enabled": false, "usersync_enabled": true, "timeout_ms": 100_u64, "sample_rate": 0.5_f64, "max_transport_bytes": 1_024_u64, "max_decoded_bytes": 2_048_u64}},
            "features": {"gzip_outbound": false, "returnallbidstatus_default": true}
        });
        serde_json::from_value::<OperatorPolicy>(base.clone())
            .unwrap()
            .validate()
            .unwrap();
        let mut paths = Vec::new();
        collect_policy_paths(&base, "", &mut paths);
        for path in paths {
            let mut null_document = base.clone();
            *null_document.pointer_mut(&path).unwrap() = json!(null);
            assert!(
                serde_json::from_value::<OperatorPolicy>(null_document).is_err(),
                "null at {path}"
            );
            if base.pointer(&path).unwrap().is_object()
                && !matches!(
                    path.as_str(),
                    "/bidders"
                        | "/account_defaults/privacy/allowactivities"
                        | "/host/gdpr/tcf2/purposes"
                        | "/host/gdpr/tcf2/special_features"
                        | "/account_defaults/gdpr/purposes"
                        | "/account_defaults/gdpr/special_features"
                )
            {
                let mut unknown_document = base.clone();
                unknown_document
                    .pointer_mut(&path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("unknown".to_owned(), json!(true));
                let error = serde_json::from_value::<OperatorPolicy>(unknown_document).unwrap_err();
                assert!(
                    error.to_string().contains("unknown field"),
                    "{path}: {error}"
                );
            }
        }
        // The same borrowed leaves apply to a selected account, not just defaults.
        let mut selected = base.clone();
        selected["accounts"] = json!({"fake_publisher": base["account_defaults"].clone()});
        selected["accounts"]["fake_publisher"]["privacy"]["ipv4"]["anon_keep_bits"] = json!(null);
        serde_json::from_value::<OperatorPolicy>(selected)
            .expect_err("selected-account borrowed leaves reject null too");
    }

    fn structural_policy(scope: &str, overlay: Value) -> OperatorPolicy {
        let mut document = json!({
            "schema_version": 1_u64,
            "host": {},
            "account_defaults": {},
            "accounts": {"fake_private_account_canary": {}}
        });
        *document.pointer_mut(scope).unwrap() = overlay;
        serde_json::from_value(document).expect("structural cases must deserialize first")
    }

    fn assert_structural_case(scope: &str, overlay: Value, valid: bool) {
        structural_policy(scope, json!({}))
            .validate()
            .expect("the control passes before introducing one independent input");
        assert_eq!(
            structural_policy(scope, overlay).validate().is_ok(),
            valid,
            "scope: {scope}"
        );
    }

    fn assert_fixed_structural_errors(value: &Value) {
        match value {
            Value::Object(fields) if fields.contains_key("code") => {
                assert!(fields["code"].is_string());
                assert_eq!(fields["params"], json!({}));
                assert_eq!(fields["message"], json!(null));
            }
            Value::Object(fields) => {
                for nested in fields.values() {
                    assert_fixed_structural_errors(nested);
                }
            }
            Value::Array(errors) => {
                for error in errors {
                    assert_fixed_structural_errors(error);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                panic!("unexpected structural diagnostic shape")
            }
        }
    }

    #[test]
    fn structural_host_resource_limits_reuse_existing_scalar_bounds() {
        for (field, minimum, maximum) in [
            ("default_tmax_ms", 1_u64, 30_000_u64),
            ("max_tmax_ms", 1, 30_000),
            ("max_bidders_per_auction", 1, 500),
            ("max_outbound_requests", 1, 1_000),
            ("max_bidder_response_bytes", 1_024, 0x0100_0000),
            ("max_outbound_raw_bytes", 1_024, 0x0100_0000),
            ("max_outbound_request_bytes", 1_024, 0x0100_0000),
            ("max_transport_bytes", 1_024, 104_857_600),
            ("max_decoded_bytes", 1_024, 104_857_600),
            ("max_parsed_nodes", 1, 0x0010_0000),
        ] {
            for (value, valid) in [
                (minimum - 1, false),
                (minimum, true),
                (maximum, true),
                (maximum + 1, false),
            ] {
                assert_structural_case("/host", json!({"resource_limits": {field: value}}), valid);
            }
        }
    }

    #[test]
    fn structural_specialized_checks_keep_literal_field_paths() {
        for (overlay, expected) in [
            (
                json!({"host": {"status_response": "a".repeat(8_193)}}),
                json!({"host": {"status_response": [{"code": "status_banner_bytes", "message": null, "params": {}}]}}),
            ),
            (
                json!({"host": {"aliases": {format!("fake_{}", "a".repeat(60)): "fake_base"}}}),
                json!({"host": {"aliases": [{"code": "host_alias_name_too_long", "message": null, "params": {}}]}}),
            ),
            (
                json!({"account_defaults": {"truncate_target_attribute": 0_usize}}),
                json!({"account_defaults": {"truncate_target_attribute": [{"code": "range", "message": null, "params": {}}]}}),
            ),
        ] {
            let mut document = json!({"schema_version": 1_u64});
            document
                .as_object_mut()
                .unwrap()
                .extend(overlay.as_object().unwrap().clone());
            let policy: OperatorPolicy = serde_json::from_value(document).unwrap();
            let error = policy.validate().expect_err("specialized rule must reject");
            assert_eq!(serde_json::to_value(error).unwrap(), expected);
        }
    }

    #[test]
    fn structural_host_gdpr_checks_vendor_and_default_value_bounds() {
        for (value, valid) in [(0_u32, true), (0xFFFF, true), (0x0001_0000, false)] {
            assert_structural_case("/host", json!({"gdpr": {"host_vendor_id": value}}), valid);
        }
        for (value, valid) in [
            ("0", true),
            ("1", true),
            ("", false),
            ("fake_private_default", false),
        ] {
            assert_structural_case("/host", json!({"gdpr": {"default_value": value}}), valid);
        }
    }

    #[test]
    fn structural_host_gdpr_enforcement_keys_reuse_existing_ranges() {
        for (field, last) in [("purposes", 10_u8), ("special_features", 12_u8)] {
            for (id, valid) in [(0_u8, false), (1, true), (last, true), (last + 1, false)] {
                assert_structural_case(
                    "/host",
                    json!({"gdpr": {"tcf2": {field: {id.to_string(): {}}}}}),
                    valid,
                );
            }
            assert_structural_case("/host", json!({"gdpr": {"tcf2": {field: {}}}}), true);
        }
    }

    #[test]
    fn structural_host_status_limit_counts_utf8_bytes() {
        for (banner, valid) in [
            (String::new(), true),
            ("a".repeat(8_192), true),
            ("a".repeat(8_193), false),
            ("\u{e9}".repeat(4_096), true),
            ("\u{e9}".repeat(4_097), false),
        ] {
            assert_structural_case("/host", json!({"status_response": banner}), valid);
        }
    }

    #[test]
    fn structural_host_alias_count_reuses_existing_limit() {
        for (count, valid) in [(0_usize, true), (64, true), (65, false)] {
            let aliases: serde_json::Map<String, Value> = (0..count)
                .map(|index| (format!("fake_alias_{index}"), json!("fake_bidder")))
                .collect();
            assert_structural_case("/host", json!({"aliases": aliases}), valid);
        }
    }

    #[test]
    fn structural_host_alias_identity_limits_count_utf8_bytes() {
        for (name, valid) in [
            (format!("fake_{}", "a".repeat(59)), true),
            (format!("fake_{}", "a".repeat(60)), false),
            (format!("fake_a{}", "\u{e9}".repeat(29)), true),
            (format!("fake_{}", "\u{e9}".repeat(30)), false),
        ] {
            for aliases in [
                json!({name.clone(): "fake_bidder"}),
                json!({"fake_alias": name}),
            ] {
                assert_structural_case("/host", json!({"aliases": aliases}), valid);
            }
        }
    }

    #[test]
    fn structural_account_caps_are_checked_independently_in_both_scopes() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for field in ["max_transport_bytes", "max_decoded_bytes"] {
                for (value, valid) in [
                    (0_u64, false),
                    (1_023, false),
                    (1_024, true),
                    (104_857_600, true),
                    (104_857_601, false),
                ] {
                    for disabled in [false, true] {
                        assert_structural_case(
                            scope,
                            json!({field: value, "disabled": disabled}),
                            valid,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn structural_account_string_lengths_reuse_character_bounds() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for (field, maximum) in [("bid_rounding", 32_usize), ("targeting_prefix", 8_usize)] {
                for (length, valid) in [
                    (0_usize, false),
                    (1, true),
                    (maximum, true),
                    (maximum + 1, false),
                ] {
                    for unit in ["a", "\u{e9}"] {
                        assert_structural_case(scope, json!({field: unit.repeat(length)}), valid);
                    }
                }
            }
        }
    }

    #[test]
    fn structural_account_truncation_checks_values_but_accepts_inherit_and_clear() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for (value, valid) in [
                (0_usize, false),
                (11, false),
                (12, true),
                (20, true),
                (21, false),
            ] {
                assert_structural_case(scope, json!({"truncate_target_attribute": value}), valid);
            }
            assert_structural_case(
                scope,
                json!({"truncate_target_attribute": {"clear": true}}),
                true,
            );
            assert_structural_case(scope, json!({}), true);
        }
    }

    #[test]
    fn structural_account_gdpr_enforcement_keys_reuse_existing_ranges() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for (field, last) in [("purposes", 10_u8), ("special_features", 12_u8)] {
                for (id, valid) in [(0_u8, false), (1, true), (last, true), (last + 1, false)] {
                    assert_structural_case(
                        scope,
                        json!({"gdpr": {field: {id.to_string(): {}}}}),
                        valid,
                    );
                }
                assert_structural_case(scope, json!({"gdpr": {field: {}}}), true);
            }
        }
    }

    #[test]
    fn structural_account_privacy_ip_bounds_remain_family_specific() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for (family, maximum) in [("ipv4", 32_u8), ("ipv6", 128_u8)] {
                for (bits, valid) in [(0_u8, true), (maximum, true), (maximum + 1, false)] {
                    assert_structural_case(
                        scope,
                        json!({"privacy": {family: {"anon_keep_bits": bits}}}),
                        valid,
                    );
                }
            }
        }
    }

    #[test]
    fn structural_account_activity_rule_count_reuses_existing_limit() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for (count, valid) in [(0_usize, true), (128, true), (129, false)] {
                assert_structural_case(
                    scope,
                    json!({"privacy": {"allowactivities": {
                        "fetchBids": {"default": false, "rules": vec![json!({"allow": false}); count]}
                    }}}),
                    valid,
                );
            }
        }
    }

    #[test]
    fn structural_account_activity_condition_limit_combines_names_and_types() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for (names, kinds, valid) in [
                (128_usize, 0_usize, true),
                (127, 1, true),
                (128, 1, false),
                (0, 129, false),
            ] {
                assert_structural_case(
                    scope,
                    json!({"privacy": {"allowactivities": {
                        "fetchBids": {"rules": [{"allow": false, "condition": {
                            "componentName": vec!["fake_component"; names],
                            "componentType": vec!["bidder"; kinds]
                        }}]}
                    }}}),
                    valid,
                );
            }
        }
    }

    #[test]
    fn structural_account_activity_component_limit_counts_utf8_bytes() {
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            for (name, valid) in [
                (format!("fake_{}", "a".repeat(123)), true),
                (format!("fake_{}", "a".repeat(124)), false),
                (format!("fake_{}", "\u{e9}".repeat(61)), true),
                (format!("fake_{}", "\u{e9}".repeat(62)), false),
            ] {
                assert_structural_case(
                    scope,
                    json!({"privacy": {"allowactivities": {
                        "fetchBids": {"rules": [{"allow": false, "condition": {"componentName": [name]}}]}
                    }}}),
                    valid,
                );
            }
        }
    }

    #[test]
    fn structural_gdpr_diagnostics_keep_existing_codes_without_raw_values() {
        let error = structural_policy(
            "/host",
            json!({"gdpr": {
                "default_value": "fake_private_default_canary",
                "tcf2": {"purposes": {"0": {"vendor_exceptions": ["fake_private_vendor_canary"]}}}
            }}),
        )
        .validate()
        .expect_err("host GDPR checks must recurse");
        let serialized = serde_json::to_value(error).unwrap();
        assert_eq!(
            serialized,
            json!({"host": {"gdpr": {
                "default_value": [{"code": "gdpr_default_value_must_be_0_or_1", "message": null, "params": {}}],
                "tcf2": {"purposes": [{"code": "purpose_id_out_of_range", "message": null, "params": {}}]}
            }}})
        );
        assert_fixed_structural_errors(&serialized);
    }

    #[test]
    fn structural_account_map_diagnostics_do_not_retain_names_values_or_siblings() {
        let policy: OperatorPolicy = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "accounts": {
                "fake_private_account_canary": {"privacy": {"allowactivities": {
                    "fetchBids": {"rules": [{"allow": false, "condition": {
                        "componentName": ["fake_private_component_canary".repeat(8)]
                    }}]}
                }}},
                "fake_private_sibling_canary": {"max_transport_bytes": 1_024_u64}
            }
        }))
        .unwrap();
        let error = policy
            .validate()
            .expect_err("invalid named accounts must be checked");
        let serialized = serde_json::to_value(error).unwrap();
        assert_eq!(
            serialized,
            json!({
                "accounts": [{"code": "account_override_invalid", "message": null, "params": {}}]
            })
        );
        assert_fixed_structural_errors(&serialized);
    }

    #[test]
    fn structural_nested_diagnostics_do_not_retain_rejected_maps_or_strings() {
        for (scope, overlay) in [
            (
                "/host",
                json!({"resource_limits": {"default_tmax_ms": 0_i64}}),
            ),
            (
                "/host",
                json!({"status_response": "fake_private_status_canary".repeat(400)}),
            ),
            (
                "/host",
                json!({"aliases": {"fake_private_alias_canary".repeat(8): "fake_private_base_canary"}}),
            ),
            (
                "/account_defaults",
                json!({"targeting_prefix": "fake_private_prefix_canary"}),
            ),
            (
                "/account_defaults",
                json!({"gdpr": {"special_features": {"13": {"vendor_exceptions": ["fake_private_vendor_canary"]}}}}),
            ),
            (
                "/account_defaults",
                json!({"privacy": {"allowactivities": {
                    "fetchBids": {"rules": [{"allow": false, "condition": {
                        "componentName": ["fake_private_component_canary".repeat(8)]
                    }}]}
                }}}),
            ),
        ] {
            let error = structural_policy(scope, overlay)
                .validate()
                .expect_err("the invalid leaf must be rejected before inspecting diagnostics");
            let serialized = serde_json::to_value(error).unwrap();
            assert_fixed_structural_errors(&serialized);
            assert!(!serialized.to_string().contains("fake_private_"));
        }
    }

    #[test]
    fn structural_validation_does_not_add_effective_policy_rules() {
        assert_structural_case(
            "/host",
            json!({
                "ccpa": {"enforce": false},
                "lmt": {"enforce": false},
                "host_cookie": {"cookie_name": "", "ttl_days": 0_i64, "max_cookie_size_bytes": 0_usize,
                    "domain": {"clear": true}, "optout_cookie": {"clear": true}},
                "user_sync": {"clear": true},
                "status_response": ""
            }),
            true,
        );
        for scope in ["/account_defaults", "/accounts/fake_private_account_canary"] {
            assert_structural_case(
                scope,
                json!({
                    "disabled": false,
                    "debug_allow": false,
                    "bid_rounding": "fake_not_a_rounding_mode",
                    "cookie_sync": {"default_limit": 0_i64, "max_limit": -1_i64,
                        "default_coop_sync": false, "priority_groups": []},
                    "gdpr": {"enabled": "unset", "purpose_one_treatment": {
                        "enabled": {"clear": true}, "access_allowed": false}},
                    "ccpa": {"enabled": "unset"}
                }),
                true,
            );
        }
    }

    fn ordinary_optional_host_cases() -> Vec<(String, Vec<Value>)> {
        [
            (
                "/floor_currency",
                vec![
                    json!({"expires_at": 2_000_i64, "usd_per_unit": {"EUR": 1.25_f64}}),
                    json!({"expires_at": 3_000_i64, "usd_per_unit": {}}),
                ],
            ),
            ("/host/gdpr", vec![json!({"enabled": true}), json!({})]),
            ("/host/ccpa", vec![json!({"enforce": true}), json!({})]),
            ("/host/lmt", vec![json!({"enforce": false}), json!({})]),
            (
                "/host/host_cookie",
                vec![
                    json!({"domain": "example.test", "optout_cookie": {}}),
                    json!({}),
                ],
            ),
            (
                "/host/status_response",
                vec![json!("configured"), json!("")],
            ),
            (
                "/host/aliases",
                vec![json!({"fake_alias": "fake_bidder"}), json!({})],
            ),
            (
                "/host/host_cookie/cookie_name",
                vec![json!("uids"), json!("")],
            ),
            (
                "/host/host_cookie/family",
                vec![json!("fake_family"), json!("")],
            ),
            (
                "/host/host_cookie/ttl_days",
                vec![json!(90_i64), json!(0_i64)],
            ),
            (
                "/host/host_cookie/max_cookie_size_bytes",
                vec![json!(4096_usize), json!(0_usize)],
            ),
            (
                "/host/host_cookie/optout_cookie/name",
                vec![json!("optout"), json!("")],
            ),
            (
                "/host/host_cookie/optout_cookie/value",
                vec![json!("1"), json!("")],
            ),
            (
                "/host/user_sync/cooperative",
                vec![json!({"enabled_by_default": true}), json!({})],
            ),
            (
                "/host/user_sync/priority_groups",
                vec![json!([["fake_bidder"]]), json!([])],
            ),
            (
                "/host/user_sync/allowed_origins",
                vec![json!(["https://publisher.example.test"]), json!([])],
            ),
        ]
        .into_iter()
        .map(|(path, values)| (path.to_owned(), values))
        .collect()
    }

    fn ordinary_optional_policy_cases() -> (Value, Vec<(String, Vec<Value>)>) {
        let mut document = json!({
            "schema_version": 1_u64,
            "host": {"user_sync": {"external_url": "https://sync.example.test"}},
            "account_defaults": {},
            "accounts": {"fake_account": {}}
        });
        let mut cases = ordinary_optional_host_cases();
        for scope in ["account_defaults", "accounts/fake_account"] {
            for (field, values) in [
                (
                    "max_transport_bytes",
                    vec![json!(4096_u64), json!(1024_u64)],
                ),
                ("max_decoded_bytes", vec![json!(8192_u64), json!(2048_u64)]),
                ("disabled", vec![json!(true), json!(false)]),
                ("debug_allow", vec![json!(false), json!(true)]),
                (
                    "cookie_sync",
                    vec![
                        json!({"default_limit": 5_i64, "default_coop_sync": true}),
                        json!({}),
                    ],
                ),
                ("bid_rounding", vec![json!("down"), json!("")]),
                ("targeting_prefix", vec![json!("hb"), json!("")]),
                ("gdpr", vec![json!({}), json!({"enabled": "unset"})]),
                ("ccpa", vec![json!({"enabled": "unset"}), json!({})]),
                (
                    "privacy",
                    vec![json!({"ipv4": {"anon_keep_bits": 24_u8}}), json!({})],
                ),
            ] {
                cases.push((format!("/{scope}/{field}"), values));
            }
            for (field, values) in [
                ("enabled", vec![json!("unset"), json!("disabled")]),
                (
                    "purpose_one_treatment",
                    vec![json!({"enabled": true, "access_allowed": false}), json!({})],
                ),
                (
                    "channel_enabled",
                    vec![json!({"web": "enabled"}), json!({})],
                ),
                ("purposes", vec![json!({"1": {}}), json!({})]),
                ("special_features", vec![json!({"1": {}}), json!({})]),
                (
                    "basic_enforcement_vendors",
                    vec![json!(["fake_bidder"]), json!([])],
                ),
            ] {
                cases.push((format!("/{scope}/gdpr/{field}"), values));
            }
        }

        // Seed every earlier layer and sibling before changing one field at a time.
        for (path, values) in &cases {
            let (parent, field) = path.rsplit_once('/').unwrap();
            document
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.to_owned(), values[0].clone());
        }
        assert_eq!(cases.len(), 48);
        (document, cases)
    }

    #[test]
    fn ordinary_optional_policy_fields_reject_explicit_null() {
        let (base, cases) = ordinary_optional_policy_cases();
        let _valid_before_state: OperatorPolicy = serde_json::from_value(base.clone()).unwrap();
        let mut accepted_nulls = Vec::new();
        for (path, _) in cases {
            assert!(!base.pointer(&path).unwrap().is_null(), "{path}");
            let mut document = base.clone();
            *document.pointer_mut(&path).unwrap() = json!(null);
            match serde_json::from_value::<OperatorPolicy>(document) {
                Ok(_) => accepted_nulls.push(path),
                Err(error) => assert!(error.is_data(), "{path}"),
            }
        }
        assert!(
            accepted_nulls.is_empty(),
            "accepted explicit nulls: {accepted_nulls:?}"
        );
    }

    #[test]
    fn ordinary_optional_policy_fields_omit_absent_values_on_round_trip() {
        let (base, cases) = ordinary_optional_policy_cases();
        let _valid_before_state: OperatorPolicy = serde_json::from_value(base.clone()).unwrap();
        let mut serialized_absences = Vec::new();
        for (path, _) in cases {
            let mut document = base.clone();
            let (parent, field) = path.rsplit_once('/').unwrap();
            let removed = document
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(removed.is_some(), "before-state presence: {path}");
            let policy: OperatorPolicy = serde_json::from_value(document).unwrap();
            let serialized = serde_json::to_value(policy).unwrap();
            if serialized.pointer(&path).is_some() {
                serialized_absences.push(path);
            } else {
                let round_trip: OperatorPolicy =
                    serde_json::from_value(serialized.clone()).unwrap();
                assert_eq!(
                    serde_json::to_value(round_trip).unwrap(),
                    serialized,
                    "{path}"
                );
            }
        }
        assert!(
            serialized_absences.is_empty(),
            "serialized absent fields: {serialized_absences:?}"
        );
    }

    #[test]
    fn ordinary_optional_policy_fields_preserve_concrete_values() {
        let (base, cases) = ordinary_optional_policy_cases();
        let _valid_before_state: OperatorPolicy = serde_json::from_value(base.clone()).unwrap();
        for (path, values) in cases {
            for value in values {
                let mut document = base.clone();
                *document.pointer_mut(&path).unwrap() = value.clone();
                let policy: OperatorPolicy = serde_json::from_value(document).unwrap();
                let serialized = serde_json::to_value(policy).unwrap();
                let actual = serialized.pointer(&path).unwrap();
                assert!(!actual.is_null(), "{path}");
                // Shared serving DTOs may emit their own absent leaves inside objects.
                if !value.is_object() {
                    assert_eq!(actual, &value, "{path}");
                }
                let round_trip: OperatorPolicy =
                    serde_json::from_value(serialized.clone()).unwrap();
                assert_eq!(
                    serde_json::to_value(round_trip).unwrap(),
                    serialized,
                    "{path}"
                );
            }
        }
    }

    #[test]
    fn ordinary_optional_enforcement_maps_keep_string_keys_in_toml() {
        for scope in ["account_defaults", "accounts/fake_account"] {
            for field in ["purposes", "special_features"] {
                for value in [json!({}), json!({"1": {}})] {
                    let (mut document, _) = ordinary_optional_policy_cases();
                    let path = format!("/{scope}/gdpr/{field}");
                    *document.pointer_mut(&path).unwrap() = value.clone();
                    let policy: OperatorPolicy = serde_json::from_value(document).unwrap();
                    let wire = toml::to_string(&policy).unwrap();
                    let round_trip: OperatorPolicy = toml::from_str(&wire).unwrap();
                    let serialized = serde_json::to_value(round_trip).unwrap();
                    assert_eq!(
                        serialized
                            .pointer(&path)
                            .unwrap()
                            .as_object()
                            .unwrap()
                            .keys()
                            .collect::<Vec<_>>(),
                        value.as_object().unwrap().keys().collect::<Vec<_>>(),
                        "{path}"
                    );
                }
            }
        }
    }

    #[test]
    fn application_version_is_required_not_defaulted() {
        let error = serde_json::from_value::<OperatorPolicy>(json!({})).unwrap_err();
        assert!(error.is_data());
        let _missing_toml_version = toml::from_str::<OperatorPolicy>("").unwrap_err();
        let policy: OperatorPolicy =
            serde_json::from_value(json!({"schema_version": 1_u64})).unwrap();
        assert_eq!(
            serde_json::to_value(policy).unwrap()["schema_version"],
            1_u64
        );
    }

    #[test]
    fn application_version_rejects_wrong_values_without_echoing_input() {
        for version in [
            json!(0_i32),
            json!(2_i32),
            json!(-1_i32),
            json!(1.0_f64),
            json!(null),
            json!("private-version"),
            json!({"private-version": 1_i32}),
        ] {
            let error =
                serde_json::from_value::<OperatorPolicy>(json!({"schema_version": version}))
                    .unwrap_err();
            assert!(matches!(
                error.to_string().as_str(),
                "invalid_application_schema_version" | "unsupported_application_schema_version"
            ));
        }
    }

    #[test]
    fn legacy_and_unknown_scopes_are_not_policy_aliases() {
        for key in [
            "baked",
            "startup",
            "runtime",
            "auction",
            "targeting",
            "status",
            "gdpr",
            "aliases",
            "unknown",
        ] {
            let error =
                serde_json::from_value::<OperatorPolicy>(json!({"schema_version": 1_u64, key: {}}))
                    .unwrap_err();
            assert!(error.to_string().contains("unknown field"), "{key}");
        }
        for document in [
            json!({"schema_version": 1_u64, "host": {"unknown": true}}),
            json!({"schema_version": 1_u64, "account_defaults": {"unknown": true}}),
            json!({"schema_version": 1_u64, "accounts": {"fake_account": {"unknown": true}}}),
            json!({"schema_version": 1_u64, "host": {"resource_limits": {"unknown": true}}}),
        ] {
            let error = serde_json::from_value::<OperatorPolicy>(document).unwrap_err();
            assert!(error.to_string().contains("unknown field"));
        }
    }

    #[test]
    fn host_and_account_defaults_keep_their_distinct_scopes() {
        let policy: OperatorPolicy = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "host": {
                "gdpr": {"enabled": true},
                "status_response": "configured",
                "resource_limits": {"default_tmax_ms": 1500_i64},
                "aliases": {"fake_alias": "fake_bidder"}
            },
            "account_defaults": {"debug_allow": false, "targeting_prefix": "ad"},
            "accounts": {"fake_account": {"debug_allow": true, "targeting_prefix": "hb"}}
        }))
        .unwrap();
        assert_eq!(policy.host.gdpr.as_ref().unwrap().enabled, Some(true));
        assert_eq!(policy.host.resource_limits.default_tmax_ms, Some(1500));
        assert_eq!(policy.host.status_response.as_deref(), Some("configured"));
        assert_eq!(
            policy.host.aliases.as_ref().unwrap()["fake_alias"],
            "fake_bidder"
        );
        assert_eq!(policy.account_defaults.debug_allow, Some(false));
        assert_eq!(policy.accounts["fake_account"].debug_allow, Some(true));
        assert_eq!(
            policy.account_defaults.targeting_prefix.as_deref(),
            Some("ad")
        );
    }

    #[test]
    fn nullable_policy_fields_use_explicit_clear_not_null() {
        let policy: OperatorPolicy = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "host": {
                "host_cookie": {"domain": {"clear": true}, "optout_cookie": {"clear": true}},
                "user_sync": {"clear": true}
            },
            "account_defaults": {
                "truncate_target_attribute": {"clear": true},
                "cookie_sync": {"default_limit": {"clear": true}, "max_limit": 0_i64, "default_coop_sync": false, "priority_groups": []},
                "gdpr": {"purpose_one_treatment": {"enabled": {"clear": true}, "access_allowed": false}}
            }
        })).unwrap();
        let cookie = policy.host.host_cookie.as_ref().unwrap();
        assert_eq!(cookie.domain, FieldOverride::Clear);
        assert_eq!(cookie.optout_cookie, FieldOverride::Clear);
        assert!(matches!(policy.host.user_sync, FieldOverride::Clear));
        assert_eq!(
            policy.account_defaults.truncate_target_attribute,
            FieldOverride::Clear
        );
        let sync = policy.account_defaults.cookie_sync.as_ref().unwrap();
        assert_eq!(sync.default_limit, FieldOverride::Clear);
        assert_eq!(sync.max_limit, FieldOverride::Value(0));
        assert_eq!(sync.default_coop_sync, FieldOverride::Value(false));
        assert_eq!(sync.priority_groups, FieldOverride::Value(Vec::new()));
        let treatment = policy
            .account_defaults
            .gdpr
            .as_ref()
            .unwrap()
            .purpose_one_treatment
            .as_ref()
            .unwrap();
        assert_eq!(treatment.enabled, FieldOverride::Clear);
        assert_eq!(treatment.access_allowed, FieldOverride::Value(false));
    }

    #[test]
    fn unconfigured_sync_is_distinct_from_a_configured_explicit_origin() {
        let absent: OperatorPolicy =
            serde_json::from_value(json!({"schema_version": 1_u64})).unwrap();
        assert!(absent.host.user_sync.is_inherit());
        let configured: OperatorPolicy = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "host": {"user_sync": {"external_url": "https://sync.example.test", "redirect_url": {"clear": true}}}
        })).unwrap();
        let FieldOverride::Value(sync) = configured.host.user_sync else {
            panic!("explicit service")
        };
        assert_eq!(sync.external_url, "https://sync.example.test");
        assert_eq!(sync.redirect_url, FieldOverride::Clear);
        for service in [
            json!({}),
            json!(null),
            json!({"redirect_url": "https://sync.example.test"}),
        ] {
            let error = serde_json::from_value::<OperatorPolicy>(
                json!({"schema_version": 1_u64, "host": {"user_sync": service}}),
            )
            .unwrap_err();
            assert!(error.is_data());
        }
    }

    #[test]
    fn explicit_clears_round_trip_through_toml_and_json() {
        let wire = json!({
            "schema_version": 1_u64,
            "host": {"host_cookie": {"domain": {"clear": true}}},
            "accounts": {"fake_account": {"truncate_target_attribute": {"clear": true}}}
        });
        let original: OperatorPolicy = serde_json::from_value(wire).unwrap();
        let json = serde_json::to_value(&original).unwrap();
        let toml = toml::to_string(&original).unwrap();
        let round_trip: OperatorPolicy = toml::from_str(&toml).unwrap();
        assert_eq!(serde_json::to_value(round_trip).unwrap(), json);
        assert_eq!(
            json["host"]["host_cookie"]["domain"],
            json!({"clear": true})
        );
        assert_eq!(
            json["accounts"]["fake_account"]["truncate_target_attribute"],
            json!({"clear": true})
        );
        assert!(json["host"].get("user_sync").is_none());
    }

    #[test]
    fn every_nullable_field_uses_the_same_strict_clear_contract() {
        let base = json!({
            "schema_version": 1_u64,
            "host": {
                "host_cookie": {"domain": "example.test", "optout_cookie": {"name": "optout", "value": "yes"}},
                "user_sync": {"external_url": "https://sync.example.test", "redirect_url": "https://sync.example.test/callback"}
            },
            "account_defaults": {
                "truncate_target_attribute": 16_usize,
                "cookie_sync": {"default_limit": 5_i64, "max_limit": 10_i64, "default_coop_sync": true, "priority_groups": [["fake_bidder"]]},
                "gdpr": {"purpose_one_treatment": {"enabled": true, "access_allowed": false}}
            }
        });
        let _valid_before_state: OperatorPolicy = serde_json::from_value(base.clone()).unwrap();
        let paths = [
            "/host/host_cookie/domain",
            "/host/host_cookie/optout_cookie",
            "/host/user_sync",
            "/host/user_sync/redirect_url",
            "/account_defaults/truncate_target_attribute",
            "/account_defaults/cookie_sync/default_limit",
            "/account_defaults/cookie_sync/max_limit",
            "/account_defaults/cookie_sync/default_coop_sync",
            "/account_defaults/cookie_sync/priority_groups",
            "/account_defaults/gdpr/purpose_one_treatment/enabled",
            "/account_defaults/gdpr/purpose_one_treatment/access_allowed",
        ];
        assert_eq!(paths.len(), 11);
        for path in paths {
            assert!(
                !base.pointer(path).unwrap().is_null(),
                "before-state presence: {path}"
            );
            let mut cleared = base.clone();
            *cleared.pointer_mut(path).unwrap() = json!({"clear": true});
            let policy: OperatorPolicy = serde_json::from_value(cleared).unwrap();
            let serialized = serde_json::to_value(policy).unwrap();
            assert_eq!(
                serialized.pointer(path).unwrap(),
                &json!({"clear": true}),
                "{path}"
            );
            for invalid in [
                json!(null),
                json!({"clear": false}),
                json!({"clear": true, "extra": true}),
            ] {
                let mut document = base.clone();
                *document.pointer_mut(path).unwrap() = invalid;
                let error = serde_json::from_value::<OperatorPolicy>(document).unwrap_err();
                assert!(error.is_data(), "{path}");
            }
        }
    }

    #[test]
    fn required_and_nonnullable_fields_do_not_gain_a_clear_union() {
        for document in [
            json!({"schema_version": {"clear": true}}),
            json!({"schema_version": 1_u64, "host": {"status_response": {"clear": true}}}),
            json!({"schema_version": 1_u64, "host": {"resource_limits": {"default_tmax_ms": {"clear": true}}}}),
            json!({"schema_version": 1_u64, "host": {"user_sync": {"external_url": {"clear": true}}}}),
            json!({"schema_version": 1_u64, "account_defaults": {"debug_allow": {"clear": true}}}),
            json!({"schema_version": 1_u64, "accounts": {"fake_account": {"disabled": {"clear": true}}}}),
            json!({"schema_version": 1_u64, "features": {"gzip_outbound": {"clear": true}}}),
        ] {
            let error = serde_json::from_value::<OperatorPolicy>(document).unwrap_err();
            assert!(error.is_data());
        }
    }

    #[test]
    fn debug_does_not_expose_account_names_or_policy_values() {
        let policy: OperatorPolicy = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "host": {"status_response": "private-status-value"},
            "accounts": {"fake_private_account": {"targeting_prefix": "secret"}}
        }))
        .unwrap();
        assert_eq!(format!("{policy:?}"), "OperatorPolicy(<redacted>)");
        assert_eq!(format!("{policy:#?}"), "OperatorPolicy(<redacted>)");
    }

    #[test]
    fn optout_cookie_keeps_leaf_omission_separate_from_empty() {
        for value in [
            json!({"value": "1"}),
            json!({"name": ""}),
            json!({"name": "optout"}),
            json!({"name": "", "value": ""}),
        ] {
            let document = json!({
                "schema_version": 1_u64,
                "host": {"host_cookie": {"optout_cookie": value}}
            });
            let policy: OperatorPolicy = serde_json::from_value(document).unwrap();
            let serialized = serde_json::to_value(&policy).unwrap();
            assert_eq!(serialized["host"]["host_cookie"]["optout_cookie"], value);
            let toml = toml::to_string(&policy).unwrap();
            let from_toml: OperatorPolicy = toml::from_str(&toml).unwrap();
            assert_eq!(
                serde_json::to_value(from_toml).unwrap()["host"]["host_cookie"]["optout_cookie"],
                value
            );
        }
    }

    #[test]
    fn optout_cookie_leaf_nulls_are_not_inheritance_or_clear() {
        for value in [json!({"name": null}), json!({"value": null})] {
            let error = serde_json::from_value::<OperatorPolicy>(json!({
                "schema_version": 1_u64,
                "host": {"host_cookie": {"optout_cookie": value}}
            }))
            .unwrap_err();
            assert!(error.is_data());
        }
    }
}
