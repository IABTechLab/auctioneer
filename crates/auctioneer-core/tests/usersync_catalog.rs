//! Sync catalog identity is derived from a synthetic registry and effective host aliases.

#[cfg(test)]
pub use auctioneer_core::test_support::bidder_fixture;

#[cfg(test)]
mod tests {
    use super::bidder_fixture::registry;
    use auctioneer_config::types::HostConfig;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::usersync::catalog::{Catalog, CatalogError};

    fn app(source: &str) -> AuctioneerAppConfig {
        toml::from_str(source).expect("runtime overlay parses")
    }

    #[test]
    fn synthetic_capabilities_and_wire_identity_lengths_are_independent() {
        let (first, _) = registry().get("fake_aaa").unwrap();
        let (second, _) = registry().get("fake_bbb").unwrap();
        let (unsyncable, _) = registry().get("fake_pp").unwrap();
        assert_eq!(first.name.len(), 8);
        assert_eq!(second.name.len(), 8);
        assert_eq!(unsyncable.name.len(), 7);
        assert_eq!(first.user_sync.key.as_ref().unwrap().len(), 5);
        assert_ne!(first.gvl_vendor_id, second.gvl_vendor_id);
        assert_ne!(first.user_sync.key, second.user_sync.key);
        assert!(first.user_sync.iframe.is_none());
        assert!(first.user_sync.redirect.is_some());
        assert!(second.user_sync.iframe.is_some());
        assert!(second.user_sync.redirect.is_some());
        assert!(unsyncable.user_sync.iframe.is_none());
        assert!(unsyncable.user_sync.redirect.is_none());
    }

    #[test]
    fn alias_preserves_requested_spelling_and_base_metadata() {
        let mut host = HostConfig::default();
        host.aliases
            .insert("fake_MY_Alias".into(), "fake_aaa".into());
        let catalog =
            Catalog::from_registry(&host, &AuctioneerAppConfig::default(), registry()).unwrap();
        let candidate = catalog.lookup("fake_My_Alias").expect("alias is known");
        assert_eq!(candidate.requested_name, "fake_My_Alias");
        assert_eq!(candidate.catalog_name, "fake_MY_Alias");
        assert_eq!(candidate.normalized_name, "fake_my_alias");
        assert_eq!(candidate.base_name, "fake_aaa");
        assert_eq!(candidate.family, "fam_a");
        assert_eq!(candidate.bidder.gvl_vendor_id, Some(17));
        assert!(candidate.configured);
    }

    #[test]
    fn exact_alias_override_wins_over_base_lookup() {
        let mut host = HostConfig::default();
        host.aliases.insert("fake_bbb".into(), "fake_aaa".into());
        let catalog =
            Catalog::from_registry(&host, &AuctioneerAppConfig::default(), registry()).unwrap();
        let candidate = catalog.lookup("fake_Bbb").expect("alias is known");
        assert_eq!(candidate.catalog_name, "fake_bbb");
        assert_eq!(candidate.base_name, "fake_aaa");
        assert_eq!(candidate.family, "fam_a");
    }

    #[test]
    fn disabled_entries_remain_lookupable_and_auction_tunables_do_not_gate_sync() {
        let mut host = HostConfig::default();
        host.aliases.insert("fake_alias".into(), "fake_aaa".into());
        let disabled = app("[bidders.fake_aaa]\nusersync_enabled = false\n\
         [bidders.fake_alias]\nusersync_enabled = true\n");
        let catalog = Catalog::from_registry(&host, &disabled, registry()).unwrap();
        let candidate = catalog
            .lookup("fake_alias")
            .expect("disabled alias stays known");
        assert!(candidate.configured);
        assert!(!candidate.enabled);

        let sampled = app("[bidders.fake_aaa]\nsample_rate = 0.0\ntimeout_ms = 1\n");
        let sampled_catalog = Catalog::from_registry(&host, &sampled, registry()).unwrap();
        assert!(sampled_catalog.lookup("fake_alias").unwrap().enabled);
    }

    #[test]
    fn available_names_are_sorted_configured_and_do_not_pre_filter_disabled() {
        let mut host = HostConfig::default();
        host.aliases
            .insert("fake_z_alias".into(), "fake_aaa".into());
        let app = app("[bidders.fake_aaa]\nusersync_enabled = false\n");
        let catalog = Catalog::from_registry(&host, &app, registry()).unwrap();
        assert_eq!(
            catalog.available_names(),
            vec!["fake_aaa", "fake_bbb", "fake_z_alias"]
        );
        assert!(!catalog.lookup("fake_aaa").unwrap().enabled);
        assert!(catalog.lookup("fake_pp").is_some());
        assert!(!catalog.lookup("fake_pp").unwrap().configured);
    }

    #[test]
    fn invalid_alias_base_is_an_error_and_never_falls_back_to_a_base() {
        let mut host = HostConfig::default();
        host.aliases
            .insert("fake_bbb".into(), "fake_not_compiled".into());
        assert!(matches!(
            Catalog::from_registry(&host, &AuctioneerAppConfig::default(), registry()),
            Err(CatalogError::UnknownAliasBase)
        ));
    }

    #[test]
    fn catalog_build_does_not_mutate_host_or_runtime_config() {
        let mut host = HostConfig::default();
        host.aliases.insert("fake_alias".into(), "fake_bbb".into());
        let app = app("[bidders.fake_alias]\nusersync_enabled = false\n");
        let host_before = serde_json::to_value(&host).unwrap();
        let app_before = serde_json::to_value(&app).unwrap();
        let _catalog = Catalog::from_registry(&host, &app, registry()).unwrap();
        assert_eq!(serde_json::to_value(&host).unwrap(), host_before);
        assert_eq!(serde_json::to_value(&app).unwrap(), app_before);
    }
}
