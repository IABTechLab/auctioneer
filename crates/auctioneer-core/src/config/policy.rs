//! Strict versioned operator policy, staged separately from the serving reader.
//!
//! Deserialization establishes shape, not semantic or resource admission.
//! Only the planned shared validator and funded snapshot constructor may make
//! this document usable by a handler; existing serving still uses its old DTO.
//! Optional fields declared here reject null. Reused serving DTO leaves retain
//! their existing contract pending the required atomic serving cutover.

use std::{borrow::Borrow, collections::BTreeMap, fmt};

use auctioneer_config::FloorCurrencyConfig;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::Error as _,
    ser::{Error as _, SerializeMap as _},
};

use super::{
    AccountCcpaOverride, AccountPrivacyOverride, AuctionConfig, BidderOverrides,
    ChannelEnabledOverride, CooperativeOverride, FeatureFlags, HostEnforcementOverride,
    HostGdprOverride, PurposeEnforcementOverride, SpecialFeatureEnforcementOverride,
    deserialize_u8_keyed_map, override_value::FieldOverride,
};
use auctioneer_config::types::TriState;

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

/// One complete policy publication. Omission inherits only within this document.
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorPolicy {
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
    pub bidders: BTreeMap<String, BidderOverrides>,
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

impl fmt::Debug for OperatorPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OperatorPolicy(<redacted>)")
    }
}

/// Optional host policy requests over product defaults.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostPolicy {
    /// Requested auction limits; installed capacity remains independently authoritative.
    #[serde(default)]
    pub resource_limits: AuctionConfig,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
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

fn deserialize_optional_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
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

fn serialize_optional_u8_keyed_map<S, V>(
    value: impl Borrow<Option<BTreeMap<u8, V>>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: Serialize,
{
    let Some(entries) = value.borrow() else {
        return Err(S::Error::custom(
            "an absent enforcement map must be omitted",
        ));
    };
    let mut map = serializer.serialize_map(Some(entries.len()))?;
    for (key, entry) in entries {
        map.serialize_entry(&EnforcementKey(*key), entry)?;
    }
    map.end()
}

struct EnforcementKey(u8);

impl Serialize for EnforcementKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
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

/// The same strict overlay type is used for account defaults and named accounts.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountPolicy {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_transport_bytes: Option<u64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
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
    pub bid_rounding: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub targeting_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
    pub truncate_target_attribute: FieldOverride<usize>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub gdpr: Option<AccountGdprPolicy>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub ccpa: Option<AccountCcpaOverride>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub privacy: Option<AccountPrivacyOverride>,
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
    use super::OperatorPolicy;
    use crate::config::override_value::FieldOverride;
    use serde_json::{Value, json};

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
