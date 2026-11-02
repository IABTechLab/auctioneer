//! Coverage requirements for usersync roles in the real compiled catalog.

#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

#[cfg(test)]
mod tests {
    use super::live_catalog;
    use auctioneer_config::usersync::SyncType;
    use std::ptr::eq;

    #[test]
    fn compiled_catalog_retains_required_usersync_roles() {
        let redirect = live_catalog::redirect();
        let iframe = live_catalog::iframe();
        let unsyncable = live_catalog::unsyncable();
        for role in [redirect, iframe] {
            assert!(
                auctioneer_config::bidders()
                    .iter()
                    .any(|info| eq(info, role.info))
            );
            assert!(!role.info.disabled);
            assert!(role.info.user_sync.enabled);
        }
        assert_eq!(redirect.kind, SyncType::Redirect);
        assert_eq!(iframe.kind, SyncType::Iframe);
        assert_ne!(redirect.name(), redirect.family());
        assert_ne!(redirect.family(), iframe.family());
        assert!(!redirect.user_macro().is_empty());
        assert!(iframe.user_macro().is_empty());
        assert!(!redirect.endpoint.url.contains("{{.GDPRConsent}}"));
        assert!(iframe.endpoint.url.contains("{{.GDPRConsent}}"));
        assert!(unsyncable.user_sync.iframe.is_none());
        assert!(unsyncable.user_sync.redirect.is_none());
    }
}
