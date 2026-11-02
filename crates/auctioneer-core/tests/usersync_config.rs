//! Runtime user-sync config is resolved over baked defaults without losing
//! account identity or the distinction between absent and explicit values.

#[cfg(test)]
mod tests {
    use auctioneer_bidder::BidderName;
    use auctioneer_config::types::{AccountConfig, HostConfig, TriState};
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::resolve::{
        AccountSelection, resolve_host, resolve_selected_account,
    };
    use auctioneer_runtime::aux_work::MAX_CATALOG_IDENTITIES;

    fn app(source: &str) -> AuctioneerAppConfig {
        toml::from_str(source).expect("runtime TOML parses")
    }

    fn compiled_names() -> [BidderName; 3] {
        ["fake_aaa", "fake_bbb", "fake_pp"].map(BidderName::new)
    }

    #[test]
    fn host_overrides_are_per_field_and_preserve_explicit_values() {
        let mut baked = HostConfig::default();
        baked.host_cookie.cookie_name = "host_id".into();
        baked.host_cookie.family = "family".into();
        baked.host_cookie.max_cookie_size_bytes = 1000;
        baked.user_sync.cooperative.enabled_by_default = true;
        baked.user_sync.priority_groups = vec![vec!["fake_aaa".into()]];
        baked.user_sync.allowed_origins = vec!["https://old.example".into()];
        let cfg = app("[host_cookie]\nmax_cookie_size_bytes = 0\n\
         [user_sync]\npriority_groups = []\nallowed_origins = []\n\
         [user_sync.cooperative]\nenabled_by_default = false\n\
         [gdpr.tcf2.purpose_one_treatment]\naccess_allowed = false\n");
        let host = resolve_host(&cfg, &baked);
        assert_eq!(host.host_cookie.cookie_name, "host_id");
        assert_eq!(host.host_cookie.family, "family");
        assert_eq!(host.host_cookie.max_cookie_size_bytes, 0);
        assert!(host.user_sync.priority_groups.is_empty());
        assert!(host.user_sync.allowed_origins.is_empty());
        assert!(!host.user_sync.cooperative.enabled_by_default);
        assert!(host.gdpr.tcf2.purpose_one_treatment.enabled);
        assert!(!host.gdpr.tcf2.purpose_one_treatment.access_allowed);
    }

    #[test]
    fn account_priority_empty_replaces_host_and_none_inherits() {
        let mut host = HostConfig::default();
        host.user_sync.priority_groups = vec![vec!["fake_bbb".into()]];
        let cfg = app("[accounts.empty.cookie_sync]\npriority_groups = []\n");
        let empty = resolve_selected_account(
            AccountSelection::Named("empty".into()),
            &cfg,
            &AccountConfig::default(),
        );
        let inherited = resolve_selected_account(
            AccountSelection::Named("other".into()),
            &cfg,
            &AccountConfig::default(),
        );
        assert_eq!(
            empty.config.cookie_sync.priority_groups.as_ref(),
            Some(&Vec::new())
        );
        assert!(inherited.config.cookie_sync.priority_groups.is_none());
        assert!(empty.effective_priority_groups(&host).is_empty());
        assert_eq!(
            inherited.effective_priority_groups(&host),
            host.user_sync.priority_groups
        );
    }

    #[test]
    fn default_selection_does_not_lookup_literal_unknown() {
        let cfg = app("[accounts.unknown]\ndisabled = true\n");
        let baked = AccountConfig::default();
        let default =
            resolve_selected_account(AccountSelection::from_requested(None), &cfg, &baked);
        let empty =
            resolve_selected_account(AccountSelection::from_requested(Some("")), &cfg, &baked);
        let directly_empty =
            resolve_selected_account(AccountSelection::Named(String::new()), &cfg, &baked);
        let literal = resolve_selected_account(
            AccountSelection::from_requested(Some("unknown")),
            &cfg,
            &baked,
        );
        let unconfigured = resolve_selected_account(
            AccountSelection::from_requested(Some("\u{e9}-42")),
            &cfg,
            &baked,
        );
        assert_eq!(default.selection, AccountSelection::Default);
        assert_eq!(empty.selection, AccountSelection::Default);
        assert_eq!(directly_empty.selection, AccountSelection::Default);
        assert!(!default.config.disabled);
        assert!(literal.config.disabled);
        assert_eq!(literal.selection, AccountSelection::Named("unknown".into()));
        assert_eq!(
            unconfigured.selection,
            AccountSelection::Named("\u{e9}-42".into())
        );
        assert!(!unconfigured.config.disabled);
    }

    #[test]
    fn account_web_channel_overlay_is_preserved_for_privacy() {
        let cfg = app("[accounts.named.gdpr.channel_enabled]\nweb = 'disabled'\n");
        let selected = resolve_selected_account(
            AccountSelection::Named("named".into()),
            &cfg,
            &AccountConfig::default(),
        );
        assert_eq!(selected.config.gdpr.channel_enabled.web, TriState::Disabled);
    }

    #[test]
    fn account_treatment_overlay_keeps_unset_for_privacy_fallback() {
        let cfg = app("[accounts.named.gdpr.purpose_one_treatment]\naccess_allowed = false\n");
        let selected = resolve_selected_account(
            AccountSelection::Named("named".into()),
            &cfg,
            &AccountConfig::default(),
        );
        assert_eq!(selected.config.gdpr.purpose_one_treatment.enabled, None);
        assert_eq!(
            selected.config.gdpr.purpose_one_treatment.access_allowed,
            Some(false)
        );
    }

    #[test]
    fn merged_config_validation_rejects_invalid_effective_values_and_namespaces() {
        let mut invalid_host = HostConfig::default();
        invalid_host.host_cookie.cookie_name = "uids".into();
        assert!(
            AuctioneerAppConfig::default()
                .validate_effective_with_catalog(
                    &invalid_host,
                    &AccountConfig::default(),
                    &compiled_names()
                )
                .is_err()
        );

        let baked = HostConfig::default();
        let account = AccountConfig::default();
        assert!(
            app("[host_cookie]\ncookie_name = 'uids'\n")
                .validate_effective_with_catalog(&baked, &account, &compiled_names())
                .is_err()
        );
        assert!(
            app("[user_sync]\nexternal_url = 'http://bad.example'\n")
                .validate_effective_with_catalog(&baked, &account, &compiled_names())
                .is_err()
        );
        assert!(
            app("[aliases]\nfake_Foo = 'fake_pp'\nfake_foo = 'fake_pp'\n")
                .validate_effective_with_catalog(&baked, &account, &compiled_names())
                .is_err()
        );
        assert!(
            app("[aliases]\nfake_Pp = 'fake_bbb'\n")
                .validate_effective_with_catalog(&baked, &account, &compiled_names())
                .is_err()
        );
        assert!(
            app("[bidders.fake_Foo]\nusersync_enabled = false\n[bidders.fake_foo]\nenabled = false\n")
                .validate_effective_with_catalog(&baked, &account, &compiled_names())
                .is_err()
        );
        assert!(
            app("[bidders.fake_Pp]\nusersync_enabled = false\n")
                .validate_effective_with_catalog(&baked, &account, &compiled_names())
                .is_err()
        );
        app("[aliases]\nfake_pp = 'fake_bbb'\n")
            .validate_effective_with_catalog(&baked, &account, &compiled_names())
            .unwrap();
    }

    #[test]
    fn runtime_alias_merge_enforces_catalog_name_ceiling() {
        let catalog_limit = 512_usize;
        assert_eq!(MAX_CATALOG_IDENTITIES, 512);
        let base_count = compiled_names().len();
        assert!(base_count < catalog_limit);
        let mut baked = HostConfig::default();
        for index in 0..(catalog_limit - base_count - 1) {
            baked
                .aliases
                .insert(format!("fake_sync_alias_{index}"), "fake_pp".into());
        }
        let account = AccountConfig::default();
        AuctioneerAppConfig::default()
            .validate_effective_with_catalog(&baked, &account, &compiled_names())
            .unwrap();
        let runtime = app("[aliases]\nfake_overflow = 'fake_pp'\n");
        runtime
            .validate_effective_with_catalog(&baked, &account, &compiled_names())
            .unwrap();
        baked
            .aliases
            .insert("fake_last_baked".into(), "fake_pp".into());
        AuctioneerAppConfig::default()
            .validate_effective_with_catalog(&baked, &account, &compiled_names())
            .unwrap();
        assert_eq!(
            runtime.validate_effective_with_catalog(&baked, &account, &compiled_names()),
            Err("user-sync catalog exceeds 512 names".into()),
        );
    }

    #[test]
    fn bidder_override_cannot_shadow_effective_alias_with_different_case() {
        let baked = HostConfig::default();
        let account = AccountConfig::default();
        let colliding =
            app("[aliases]\nfake_Foo = 'fake_pp'\n[bidders.fake_foo]\nusersync_enabled = false\n");
        assert!(
            colliding
                .validate_effective_with_catalog(&baked, &account, &compiled_names())
                .is_err()
        );

        let exact =
            app("[aliases]\nfake_Foo = 'fake_pp'\n[bidders.fake_Foo]\nusersync_enabled = false\n");
        exact
            .validate_effective_with_catalog(&baked, &account, &compiled_names())
            .unwrap();

        let mut baked_alias = HostConfig::default();
        baked_alias
            .aliases
            .insert("fake_Foo".into(), "fake_pp".into());
        let runtime = app("[bidders.fake_foo]\nusersync_enabled = false\n");
        assert!(
            runtime
                .validate_effective_with_catalog(&baked_alias, &account, &compiled_names())
                .is_err()
        );
    }

    #[test]
    fn overlay_rejects_wrong_types_without_silent_defaulting() {
        toml::from_str::<AuctioneerAppConfig>("[accounts.a.cookie_sync]\ndefault_limit = '2'")
            .unwrap_err();
        serde_json::from_value::<AuctioneerAppConfig>(serde_json::json!({
            "user_sync": {"cooperative": {"enabled_by_default": 0_i32}}
        }))
        .unwrap_err();
    }
}
