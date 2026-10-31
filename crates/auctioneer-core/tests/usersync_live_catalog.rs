//! Coverage requirements and independent checks for the real-App role helper.

#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

#[cfg(test)]
mod tests {
    use super::live_catalog::{self, SyncRole};
    use auctioneer_config::{
        types::BidderInfo,
        usersync::{SyncType, SyncerEndpoint},
    };
    use std::ptr::eq;
    use url::{Url, form_urlencoded};

    fn role(url: &str, user_macro: &str) -> SyncRole {
        SyncRole {
            info: Box::leak(Box::new(BidderInfo {
                name: "fake_fixture_bidder".to_owned(),
                ..BidderInfo::default()
            })),
            endpoint: Box::leak(Box::new(SyncerEndpoint {
                url: url.to_owned(),
                user_macro: Some(user_macro.to_owned()),
            })),
            kind: SyncType::Redirect,
        }
    }

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

    #[test]
    fn callback_extraction_handles_bare_query_and_named_query_value() {
        let callback = "https://callback.example/setuid?bidder=fake_fixture_bidder&uid=$PARTNER";
        let bare = role(
            "https://sync.partner.example/get?{{.RedirectURL}}",
            "$PARTNER",
        );
        let encoded: String = form_urlencoded::byte_serialize(callback.as_bytes()).collect();
        assert_eq!(
            bare.callback_uri(
                &format!("https://sync.partner.example/get?{encoded}"),
                "partner-user"
            ),
            "/setuid?bidder=fake_fixture_bidder&uid=partner-user"
        );

        let named = role(
            "https://sync.partner.example/get?back={{.RedirectURL}}&mode=1",
            "",
        );
        let mut outer = Url::parse("https://sync.partner.example/get?mode=1").unwrap();
        outer.query_pairs_mut().append_pair(
            "back",
            "https://callback.example/setuid?bidder=fake_fixture_bidder&uid=",
        );
        assert_eq!(
            named.callback_uri(outer.as_str(), "appended-user"),
            "/setuid?bidder=fake_fixture_bidder&uid=appended-user"
        );
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn callback_extraction_rejects_a_changed_partner_destination() {
        let selected = role(
            "https://sync.partner.example/get?{{.RedirectURL}}",
            "$PARTNER",
        );
        drop(selected.callback(
            "https://different.partner.example/get?https%3A%2F%2Fcallback.example%2Fsetuid",
        ));
    }
}
