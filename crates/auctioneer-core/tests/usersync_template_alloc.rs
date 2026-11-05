//! URL expansion must be refused before it outgrows the charged stage buffers.

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track;
    use auctioneer_config_data::usersync::{SyncType, SyncerEndpoint, UserSyncConfig};
    use auctioneer_core::usersync::template::{
        SyncPrivacyMacros, SyncTemplateInput, TemplateError, build_sync_url,
    };

    const URL_CAP: usize = 32 * 1024;
    const URL_STAGE_BUDGET: u64 = 3 * 32 * 1024;

    fn privacy(consent: &str) -> SyncPrivacyMacros<'_> {
        SyncPrivacyMacros {
            gdpr: "0",
            gdpr_consent: consent,
            us_privacy: "",
            gpp: "",
            gpp_sid: "",
        }
    }

    fn assert_bounded_refusal(consent: &str, account: Option<&str>) {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: Some("$UID".to_owned()),
        };
        // Fixture allocations are excluded, matching the request's URL-stage charge.
        let baseline = alloc_track::reset_peak();
        let result = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_aaa",
            bidder_name: "fake_aaa",
            sync_type: SyncType::Redirect,
            account,
            privacy: privacy(consent),
        });
        let peak = alloc_track::peak_since(baseline);
        assert!(matches!(result, Err(TemplateError::OutputTooLong)));
        assert!(
            peak <= URL_STAGE_BUDGET,
            "URL expansion retained {peak} bytes against the {URL_STAGE_BUDGET}-byte stage budget"
        );
    }

    #[test]
    fn nested_callback_expansion_is_rejected_before_oversized_allocation() {
        assert_bounded_refusal(&"<".repeat(10_000), None);
    }

    #[test]
    fn consent_expansion_is_rejected_before_oversized_allocation() {
        assert_bounded_refusal(&"<".repeat(30_000), None);
    }

    #[test]
    fn account_expansion_is_rejected_before_oversized_allocation() {
        assert_bounded_refusal("", Some(&"<".repeat(30_000)));
    }

    #[test]
    fn unescaped_endpoint_value_fits_exact_url_limit_but_not_one_more_byte() {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            redirect_url: Some(
                "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&uid={{.UserMacro}}".to_owned(),
            ),
            ..UserSyncConfig::default()
        };
        let prefix = "https://bidder.example.com/sync?consent=";
        let endpoint = SyncerEndpoint {
            url: format!("{prefix}{{{{.GDPRConsent}}}}"),
            user_macro: Some("$UID".to_owned()),
        };
        for size in [URL_CAP, URL_CAP + 1] {
            let consent = "a".repeat(size - prefix.len());
            let result = build_sync_url(SyncTemplateInput {
                host: &host,
                endpoint: &endpoint,
                callback_bidder: "fake_aaa",
                bidder_name: "fake_aaa",
                sync_type: SyncType::Redirect,
                account: None,
                privacy: privacy(&consent),
            });
            if size == URL_CAP {
                let built = result.expect("unescaped value at the exact limit fits");
                assert_eq!(built.url, format!("{prefix}{consent}"));
                assert!(built.url.capacity() <= URL_CAP);
            } else {
                assert!(matches!(result, Err(TemplateError::OutputTooLong)));
            }
        }
    }
}
