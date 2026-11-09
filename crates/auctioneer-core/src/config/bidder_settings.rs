//! Runtime settings own request-required values; resolving an alias borrows them.

use super::{AuctioneerAppConfig, BidderOverrides};
use auctioneer_bidder::request_info::BidderSettingsSource;
use auctioneer_bidder::{BidderName, BidderSettings};
use auctioneer_exchange::alias::AliasMap;
use std::collections::BTreeMap;
use std::iter::once;

/// Owned settings projected from accepted policy for a request.
pub struct RuntimeBidderSettings(BTreeMap<String, BidderOverrides>);

impl RuntimeBidderSettings {
    /// Copy only settings for requested names and their resolved base identities.
    ///
    /// `policy` must already be accepted operator policy, and `aliases` must be
    /// the resolved request/host map. This does not resolve aliases or copy
    /// unrelated bidder flags, accounts or host policy. Shared bases are copied once.
    #[must_use]
    pub fn project(
        policy: &AuctioneerAppConfig,
        requested: &[BidderName],
        aliases: &AliasMap,
    ) -> Self {
        let mut settings = BTreeMap::new();
        for name in requested
            .iter()
            .flat_map(|name| once(name).chain(aliases.bases.get(name)))
        {
            if settings.contains_key(name.as_str()) {
                continue;
            }
            if let Some(value) = policy
                .bidders
                .get(name.as_str())
                .and_then(|entry| entry.settings.as_ref())
            {
                settings.insert(
                    name.as_str().to_owned(),
                    BidderOverrides {
                        settings: Some(value.clone()),
                        ..BidderOverrides::default()
                    },
                );
            }
        }
        Self(settings)
    }
}

impl BidderSettingsSource for RuntimeBidderSettings {
    fn settings(&self, name: &str) -> Option<&BidderSettings> {
        self.0.get(name)?.settings.as_ref()
    }
}

#[cfg(test)]
mod tests {
    // Nonempty generated settings are covered by the bidder's synthetic child contract.
    use super::{AuctioneerAppConfig, RuntimeBidderSettings};
    use auctioneer_bidder::BidderName;
    use auctioneer_bidder::request_info::BidderSettingsSource as _;
    use auctioneer_exchange::alias::AliasMap;

    #[test]
    fn projecting_settings_omits_settings_free_rows_without_mutating_policy() {
        let config = serde_json::from_value::<AuctioneerAppConfig>(serde_json::json!({
            "schema_version": 1_u8,
            "bidders": {"fake_unrelated": {"enabled": false}},
            "accounts": {"fake_discarded": {"targeting_prefix": "unused"}}
        }))
        .expect("valid settings-free policy");
        let before = serde_json::to_value(&config).unwrap();
        assert!(config.bidders.contains_key("fake_unrelated"));
        assert!(config.accounts.contains_key("fake_discarded"));
        let projected = RuntimeBidderSettings::project(
            &config,
            &[BidderName::new("fake_unrelated")],
            &AliasMap::default(),
        );
        assert!(projected.0.is_empty());
        assert_eq!(serde_json::to_value(&config).unwrap(), before);
    }

    #[test]
    fn projecting_settings_ignores_missing_and_settings_free_entries() {
        let config = serde_json::from_value::<AuctioneerAppConfig>(serde_json::json!({
            "schema_version": 1_u8,
            "bidders": {
                "fake_alias": {"enabled": false, "timeout_ms": 20_u32},
                "fake_base": {"sample_rate": 0.5_f64},
                "fake_unused": {"usersync_enabled": false}
            },
            "accounts": {"fake_discarded": {"targeting_prefix": "discarded"}}
        }))
        .expect("valid settings-free policy");
        let mut aliases = AliasMap::default();
        aliases
            .bases
            .insert(BidderName::new("fake_alias"), BidderName::new("fake_base"));
        let projected = RuntimeBidderSettings::project(
            &config,
            &[
                BidderName::new("fake_alias"),
                BidderName::new("fake_missing"),
            ],
            &aliases,
        );
        assert!(projected.0.is_empty());
        assert!(projected.settings("fake_alias").is_none());
        assert!(projected.settings("fake_base").is_none());
        assert_eq!(config.bidders.len(), 3);
        assert_eq!(config.accounts.len(), 1);
    }
}
