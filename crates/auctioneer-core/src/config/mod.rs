//! Concrete configuration composition, semantic admission and SDK integration.
//! Shared source/policy declarations and pure validation belong to auctioneer-config.

#[cfg(any(test, feature = "test-support"))]
use auctioneer_bidder::BidderName;
use auctioneer_bidder::BidderSettings;
use auctioneer_config::policy::OperatorPolicy as SharedAppConfig;
use auctioneer_config::source::BidderOverrides as SharedBidderOverrides;
use auctioneer_config::types::{AccountConfig, HostConfig, MAX_BIDDER_NAME_BYTES};
#[cfg(any(test, feature = "test-support"))]
use auctioneer_config::usersync::{
    validate_account_cookie_sync, validate_catalog_size, validate_host_cookie, validate_user_sync,
};
#[cfg(any(test, feature = "test-support"))]
use std::collections::BTreeSet;
#[cfg(any(test, feature = "test-support"))]
use validator::Validate as _;

pub mod bidder_settings;
pub mod budgets;
#[cfg(test)]
mod floor_currency {
    #[cfg(test)]
    mod tests {
        use super::super::{AuctioneerAppConfig, validate_effective_defaults};
        use serde_json::{Value, json};
        use validator::Validate as _;

        fn config_with_snapshot(snapshot: &Value) -> AuctioneerAppConfig {
            serde_json::from_value(json!({"schema_version": 1_u8, "floor_currency": snapshot}))
                .unwrap()
        }

        #[test]
        fn floor_currency_accepts_operator_snapshot_through_existing_runtime_config() {
            let parsed = serde_json::from_value::<AuctioneerAppConfig>(json!({
                "schema_version": 1_u8,
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
            validate_effective_defaults(&config).unwrap();
            assert_eq!(
                serde_json::to_value(&config).unwrap()["floor_currency"],
                json!({"expires_at": 2_000_i32, "usd_per_unit": {"QAA": 2.5_f64}})
            );
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
                assert!(validate_effective_defaults(&config).is_err());
            }
        }
    }
}
pub mod override_value;
pub mod policy;
pub mod policy_account;
pub mod policy_host;
pub mod resolve;
pub mod runtime;
mod sdk;
pub mod validation;

pub use auctioneer_config::source::*;
pub use sdk::SdkAuctioneerAppConfig;

/// Shared source composed with this build's discovered, bidder-owned settings.
pub type AuctioneerAppConfig = SharedAppConfig<BidderSettings>;
/// A shared override composed with this build's discovered settings.
pub type BidderOverrides = SharedBidderOverrides<BidderSettings>;

/// Structural registry failures carried by [`EffectiveConfigError::Registry`].
pub use auctioneer_bidder::RegistryBuildError;

/// Invalid runtime data is distinct from an immutable registry build failure.
#[derive(Debug, thiserror::Error)]
pub enum EffectiveConfigError {
    #[error("{0}")]
    Malformed(String),
    #[error(transparent)]
    Registry(#[from] RegistryBuildError),
}

/// Shared push/load validation against this build's baked defaults.
///
/// # Errors
/// Returns a malformed-config error for unsafe effective configuration,
/// or a distinct fatal error if this build's registry cannot initialize.
pub fn validate_effective_defaults(
    config: &AuctioneerAppConfig,
) -> Result<(), EffectiveConfigError> {
    validate_effective(
        config,
        auctioneer_config_data::host_default(),
        auctioneer_config_data::account_default(),
    )
}

/// Validate the merged values used by both push and runtime extraction.
/// Account sync settings have no cross-field host dependency, so each
/// account validates its own replacement list without cloning inherited
/// account privacy maps or host priority groups.
///
/// # Errors
/// Returns a malformed-config error for an invalid overlay, host, account,
/// or namespace; registry initialization failures remain distinct and fatal.
pub fn validate_effective(
    config: &AuctioneerAppConfig,
    baked_host: &HostConfig,
    baked_account: &AccountConfig,
) -> Result<(), EffectiveConfigError> {
    validation::validate_compiled_operator_policy(config, baked_host, baked_account)
}

/// Test-only namespace and sync controls with synthetic catalog names.
/// Production authoring and extraction use [`validate_effective`] and the
/// discovered bidder validators, never this names-only fixture boundary.
///
/// # Errors
/// Returns the same effective-config and namespace errors as
/// [`validate_effective`], using the supplied catalog for each check.
#[cfg(any(test, feature = "test-support"))]
pub fn validate_effective_with_catalog(
    config: &AuctioneerAppConfig,
    baked_host: &HostConfig,
    baked_account: &AccountConfig,
    compiled_names: &[BidderName],
) -> Result<(), String> {
    config.validate().map_err(|err| err.to_string())?;
    let host = resolve::resolve_host(config, baked_host);
    validate_host_cookie(&host.host_cookie)?;
    match &host.user_sync {
        Some(sync) => validate_user_sync(sync, cfg!(feature = "insecure-bidders"))?,
        None if matches!(
            config.host.user_sync,
            override_value::FieldOverride::Value(_)
        ) =>
        {
            return Err("user_sync requires an explicit external origin".to_owned());
        }
        None => {}
    }
    validate_catalog_size(compiled_names.len(), host.aliases.len())?;
    let defaults = resolve::resolved_account(config, baked_account);
    validate_account_cookie_sync(&defaults.cookie_sync)?;
    for overlay in config.accounts.values() {
        let mut account = defaults.clone();
        policy_account::merge_policy_account(&mut account, overlay);
        validate_account_cookie_sync(&account.cookie_sync)?;
    }
    let mut folded_aliases = BTreeSet::new();
    for (alias, base) in &host.aliases {
        if alias.is_empty() {
            return Err("host alias name must be nonempty".to_owned());
        }
        if alias.len() > 128 {
            return Err("host alias name exceeds 128 UTF-8 bytes".to_owned());
        }
        if !folded_aliases.insert(alias.to_ascii_lowercase()) {
            return Err(format!("alias namespace collides at {alias:?}"));
        }
        if !compiled_names.iter().any(|name| name.as_str() == base) {
            return Err(format!(
                "host alias {alias:?} references unknown base {base:?}"
            ));
        }
        if compiled_names
            .iter()
            .any(|name| name.as_str().eq_ignore_ascii_case(alias) && name.as_str() != alias)
        {
            return Err(format!(
                "alias {alias:?} shadows a differently cased base bidder"
            ));
        }
    }
    let mut folded_bidders = BTreeSet::new();
    for name in config.bidders.keys() {
        let folded = name.to_ascii_lowercase();
        if !folded_bidders.insert(folded.clone()) {
            return Err(format!("bidder override namespace collides at {name:?}"));
        }
        if folded_aliases.contains(&folded) && !host.aliases.contains_key(name) {
            return Err(format!(
                "bidder override {name:?} shadows a differently cased alias"
            ));
        }
        if compiled_names
            .iter()
            .any(|base| base.as_str().eq_ignore_ascii_case(name) && base.as_str() != name)
        {
            return Err(format!(
                "bidder override {name:?} shadows a differently cased base bidder"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AuctioneerAppConfig, validate_effective_with_catalog};
    use auctioneer_bidder::BidderName;
    use auctioneer_config::types::{AccountConfig, HostConfig};
    use std::collections::BTreeMap;

    #[test]
    fn empty_runtime_alias_name_is_rejected_by_effective_config() {
        let app = AuctioneerAppConfig {
            host: super::policy::HostPolicy {
                aliases: Some(BTreeMap::from([(String::new(), "fake_aaa".to_owned())])),
                ..super::policy::HostPolicy::default()
            },
            ..AuctioneerAppConfig::default()
        };
        let error = validate_effective_with_catalog(
            &app,
            &HostConfig::default(),
            &AccountConfig::default(),
            &[BidderName::new("fake_aaa")],
        )
        .expect_err("empty alias must fail before callback construction");
        assert_eq!(error, "host alias name must be nonempty");
    }

    #[test]
    fn empty_inherited_alias_name_is_rejected_by_effective_config() {
        let host = HostConfig {
            aliases: BTreeMap::from([(String::new(), "fake_aaa".to_owned())]),
            ..HostConfig::default()
        };
        let error = validate_effective_with_catalog(
            &AuctioneerAppConfig::default(),
            &host,
            &AccountConfig::default(),
            &[BidderName::new("fake_aaa")],
        )
        .expect_err("inherited empty alias must fail before callback construction");
        assert_eq!(error, "host alias name must be nonempty");
    }

    #[test]
    fn runtime_alias_over_64_bytes_is_rejected_without_priority_groups() {
        for alias in ["a".repeat(65), format!("{}a", "\u{e9}".repeat(32))] {
            let app = AuctioneerAppConfig {
                host: super::policy::HostPolicy {
                    aliases: Some(BTreeMap::from([(alias, "fake_aaa".to_owned())])),
                    ..super::policy::HostPolicy::default()
                },
                ..AuctioneerAppConfig::default()
            };
            let error = validate_effective_with_catalog(
                &app,
                &HostConfig::default(),
                &AccountConfig::default(),
                &[BidderName::new("fake_aaa")],
            )
            .expect_err("oversized alias must fail config validation, not a later callback");
            assert!(error.contains("alias"), "{error}");
            assert!(error.contains("host_alias_name_too_long"), "{error}");
        }
    }

    #[test]
    fn inherited_alias_over_128_bytes_is_rejected_without_priority_groups() {
        let host = HostConfig {
            aliases: BTreeMap::from([("a".repeat(129), "fake_aaa".to_owned())]),
            ..HostConfig::default()
        };
        let error = validate_effective_with_catalog(
            &AuctioneerAppConfig::default(),
            &host,
            &AccountConfig::default(),
            &[BidderName::new("fake_aaa")],
        )
        .expect_err("inherited aliases obey the same name limit");
        assert!(error.contains("alias"), "{error}");
        assert!(error.contains("128"), "{error}");
    }

    #[test]
    fn runtime_aliases_at_64_utf8_bytes_are_accepted() {
        for alias in [
            format!("fake_{}", "a".repeat(59)),
            format!("fake_a{}", "\u{e9}".repeat(29)),
        ] {
            let app = AuctioneerAppConfig {
                host: super::policy::HostPolicy {
                    aliases: Some(BTreeMap::from([(alias, "fake_aaa".to_owned())])),
                    ..super::policy::HostPolicy::default()
                },
                ..AuctioneerAppConfig::default()
            };
            validate_effective_with_catalog(
                &app,
                &HostConfig::default(),
                &AccountConfig::default(),
                &[BidderName::new("fake_aaa")],
            )
            .expect("runtime alias at the limit fits");
        }
    }

    #[test]
    fn inherited_aliases_at_128_utf8_bytes_are_accepted() {
        for alias in [
            format!("fake_{}", "a".repeat(123)),
            format!("fake_a{}", "\u{e9}".repeat(61)),
        ] {
            let host = HostConfig {
                aliases: BTreeMap::from([(alias, "fake_aaa".to_owned())]),
                ..HostConfig::default()
            };
            validate_effective_with_catalog(
                &AuctioneerAppConfig::default(),
                &host,
                &AccountConfig::default(),
                &[BidderName::new("fake_aaa")],
            )
            .expect("inherited alias at the limit fits");
        }
    }

    #[test]
    fn unconfigured_sync_requires_an_explicit_valid_origin_before_activation() {
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        assert!(host.user_sync.is_none());
        let omitted = AuctioneerAppConfig::default();
        assert_eq!(
            validate_effective_with_catalog(&omitted, host, account, &[]),
            Ok(())
        );
        assert!(
            super::resolve::resolve_host(&omitted, host)
                .user_sync
                .is_none()
        );

        for source in [
            "schema_version = 1\n[host.user_sync]\npriority_groups = []\n",
            "schema_version = 1\n[host.user_sync.cooperative]\nenabled_by_default = true\n",
        ] {
            let error = toml::from_str::<AuctioneerAppConfig>(source).unwrap_err();
            assert!(error.message().contains("missing field `external_url`"));
        }

        let configured: AuctioneerAppConfig = toml::from_str(
            "schema_version = 1\n[host.user_sync]\nexternal_url = 'https://sync.example.test'\n",
        )
        .unwrap();
        assert_eq!(
            validate_effective_with_catalog(&configured, host, account, &[]),
            Ok(())
        );
        assert_eq!(
            super::resolve::resolve_host(&configured, host)
                .user_sync
                .as_ref()
                .unwrap()
                .external_url,
            "https://sync.example.test"
        );

        let invalid: AuctioneerAppConfig = toml::from_str(
            "schema_version = 1\n[host.user_sync]\nexternal_url = 'https:/sync.example.test'\n",
        )
        .unwrap();
        assert_eq!(
            validate_effective_with_catalog(&invalid, host, account, &[]),
            Err("sync URL must have an explicit scheme and authority".to_owned())
        );
    }
}
