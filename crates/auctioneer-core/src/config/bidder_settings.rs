//! Runtime settings keep their extracted owner; resolving an alias borrows it.

use super::{AuctioneerAppConfig, BidderOverrides};
use auctioneer_bidder::BidderSettings;
use auctioneer_bidder::request_info::BidderSettingsSource;
use std::collections::BTreeMap;

/// The original parsed bidder map, pruned in place without rebuilding settings.
pub struct RuntimeBidderSettings(BTreeMap<String, BidderOverrides>);

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
}
