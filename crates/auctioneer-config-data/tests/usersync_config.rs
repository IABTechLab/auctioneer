#[cfg(test)]
mod tests {
    use auctioneer_config_data::types::{
        AccountCookieSyncConfig, HostCookieConfig, PurposeOneTreatment, SyncerConfig,
        UserSyncConfig,
    };
    use auctioneer_config_data::usersync::{validate_account_cookie_sync, validate_user_sync};

    #[test]
    fn typed_skip_when_omission_and_explicit_false_empty_stay_distinct_from_true() {
        let omitted: SyncerConfig = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!omitted.skip_when.gdpr);
        assert!(omitted.skip_when.gpp_sid.is_empty());

        let explicit: SyncerConfig = serde_json::from_value(serde_json::json!({
            "enabled": false,
            "skip_when": {"gdpr": false, "gpp_sid": []}
        }))
        .unwrap();
        assert!(!explicit.enabled);
        assert!(!explicit.skip_when.gdpr);
        assert!(explicit.skip_when.gpp_sid.is_empty());

        let active: SyncerConfig = serde_json::from_value(serde_json::json!({
            "skip_when": {"gdpr": true, "GPPSID": ["2,6", "-1"]}
        }))
        .unwrap();
        assert!(active.skip_when.gdpr);
        assert_eq!(active.skip_when.gpp_sid, ["2,6", "-1"]);
        serde_json::from_value::<SyncerConfig>(serde_json::json!({
            "skip_when": ["2,6"]
        }))
        .unwrap_err();
    }

    #[test]
    fn explicit_zero_false_and_empty_sync_defaults_are_preserved() {
        let cookie: HostCookieConfig = toml::from_str(
            "cookie_name = 'auctioneer_uid'\nfamily = 'auctioneer'\nmax_cookie_size_bytes = 0\n",
        )
        .unwrap();
        assert_eq!(cookie.ttl_days, 90);
        assert_eq!(cookie.max_cookie_size_bytes, 0);

        let sync: UserSyncConfig = toml::from_str(
        "external_url = 'https://sync.example.test'\npriority_groups = []\nallowed_origins = []\n[cooperative]\nenabled_by_default = false\n",
    ).unwrap();
        assert!(!sync.cooperative.enabled_by_default);
        assert!(sync.priority_groups.is_empty());
        assert!(sync.allowed_origins.is_empty());
        validate_user_sync(&sync, false).unwrap();

        let account: AccountCookieSyncConfig = toml::from_str(
            "default_limit = 0\nmax_limit = 0\ndefault_coop_sync = false\npriority_groups = []\n",
        )
        .unwrap();
        assert_eq!(account.default_limit, Some(0));
        assert_eq!(account.max_limit, Some(0));
        assert_eq!(account.default_coop_sync, Some(false));
        assert_eq!(account.priority_groups, Some(vec![]));
        validate_account_cookie_sync(&account).unwrap();

        let treatment: PurposeOneTreatment =
            toml::from_str("enabled = false\naccess_allowed = false\n").unwrap();
        assert!(!treatment.enabled);
        assert!(!treatment.access_allowed);
    }

    #[test]
    fn skip_when_keeps_gdpr_scope_separate_from_exact_sid_strings() {
        let syncer: SyncerConfig = serde_json::from_value(serde_json::json!({
            "skip_when": {"gdpr": true, "gpp_sid": ["02,6", "-1"]}
        }))
        .expect("typed skip policy");
        assert!(syncer.skip_when.gdpr);
        assert_eq!(syncer.skip_when.gpp_sid, ["02,6", "-1"]);
    }

    #[test]
    fn baked_sync_defaults_are_emitted_from_the_source_document() {
        let host = auctioneer_config_data::host_default();
        let account = auctioneer_config_data::account_default();
        assert_eq!(host.host_cookie.cookie_name, "auctioneer_uid");
        assert_eq!(host.host_cookie.family, "auctioneer");
        assert_eq!(host.host_cookie.ttl_days, 90);
        assert_eq!(
            host.user_sync.as_ref().unwrap().external_url,
            "https://sync.example.test"
        );
        assert!(
            !host
                .user_sync
                .as_ref()
                .unwrap()
                .cooperative
                .enabled_by_default
        );
        assert!(host.gdpr.tcf2.purpose_one_treatment.enabled);
        assert!(host.gdpr.tcf2.purpose_one_treatment.access_allowed);
        assert_eq!(account.cookie_sync.priority_groups, None);
        assert_eq!(account.gdpr.purpose_one_treatment.enabled, None);
    }

    #[test]
    fn baked_identity_and_callback_origin_cannot_be_omitted() {
        toml::from_str::<HostCookieConfig>("ttl_days = 90").unwrap_err();
        toml::from_str::<UserSyncConfig>("priority_groups = []").unwrap_err();
    }
}
