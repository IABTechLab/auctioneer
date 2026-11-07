//! Synthetic identities and capabilities for shared core tests.

pub use auctioneer_bidder::test_support::fake_bidder;

pub use fixtures::registry;

pub mod fixtures {
    use super::fake_bidder::FakeBidder;
    use auctioneer_bidder::BidderRegistry;
    use auctioneer_config_data::types::{BidderInfo, Capabilities, MediaType};
    use auctioneer_config_data::usersync::{SyncerConfig, SyncerEndpoint};
    use std::sync::OnceLock;

    /// Shared registry for synthetic handler fixtures.
    ///
    /// # Panics
    /// Panics if the fixture declares duplicate bidder names.
    pub fn registry() -> &'static BidderRegistry {
        static REGISTRY: OnceLock<BidderRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            let mut registry = BidderRegistry::new();
            for (name, family, vendor, iframe, redirect) in [
                ("fake_aaa", "fam_a", 17, false, true),
                ("fake_bbb", "fam_b", 29, true, true),
                ("fake_pp", "fam_c", 43, false, false),
            ] {
                let endpoint = || SyncerEndpoint {
                    url: "https://sync.bidder.example/sync?back={{.RedirectURL}}".into(),
                    user_macro: Some("$UID".into()),
                };
                let info = Box::leak(Box::new(BidderInfo {
                    name: name.into(),
                    endpoint: "https://bidder.example/auction".into(),
                    uses_https: true,
                    gvl_vendor_id: Some(vendor),
                    capabilities: Capabilities {
                        site: vec![MediaType::Banner],
                        ..Capabilities::default()
                    },
                    user_sync: SyncerConfig {
                        key: Some(family.into()),
                        iframe: iframe.then(endpoint),
                        redirect: redirect.then(endpoint),
                        ..SyncerConfig::default()
                    },
                    ..BidderInfo::default()
                }));
                registry
                    .register_with_implementation(
                        info,
                        "fake_protocol",
                        Box::<FakeBidder>::default(),
                    )
                    .expect("unique test bidder");
            }
            registry
        })
    }
}
