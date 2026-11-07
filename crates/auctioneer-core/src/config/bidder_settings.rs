//! Runtime settings own request-required values; resolving an alias borrows them.

use super::{AuctioneerAppConfig, BidderOverrides};
use auctioneer_bidder::request_info::BidderSettingsSource;
use auctioneer_bidder::{BidderName, BidderSettings};
use auctioneer_exchange::alias::AliasMap;
use std::collections::BTreeMap;
use std::iter::once;

/// Owned settings projected for a request or moved from an extracted configuration.
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

impl From<AuctioneerAppConfig> for RuntimeBidderSettings {
    fn from(mut config: AuctioneerAppConfig) -> Self {
        // Move funded entries; unrelated accounts and non-settings entries die here.
        config
            .bidders
            .retain(|_name, entry| entry.settings.is_some());
        Self(config.bidders)
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
    fn retaining_settings_drops_all_entries_without_settings() {
        let config = serde_json::from_value::<AuctioneerAppConfig>(serde_json::json!({
            "schema_version": 1_u8,
            "bidders": {"fake_unrelated": {"enabled": false}},
            "accounts": {"fake_discarded": {"targeting_prefix": "discarded"}}
        }))
        .unwrap();
        assert!(RuntimeBidderSettings::from(config).0.is_empty());
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
