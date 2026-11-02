//! Auction-only `PrivacyContext::allow_all_no_scrub()` invariants and
//! migration checks for the real sync source normalizer.

#[cfg(test)]
mod tests {
    use auctioneer_config::types::{AccountConfig, HostConfig};
    use auctioneer_privacy::{
        Activity, AuctionPermissions, Component, GdprSignalIn, PrivacyContext, Signal,
        SyncEndpoint, SyncPrivacyError, SyncPrivacyInput, gcl::Gcl, gvl::GvlStore,
        resolve_sync_privacy,
    };
    use time::OffsetDateTime;

    #[test]
    fn auction_stub_allows_every_known_activity_for_every_known_component() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        for activity in [
            Activity::FetchBids,
            Activity::TransmitUfpd,
            Activity::TransmitPreciseGeo,
            Activity::TransmitTid,
            Activity::SyncUser,
            Activity::EnrichUfpd,
            Activity::TransmitUniqueRequestIds,
            Activity::ReportAnalytics,
        ] {
            for component in [
                Component::General("host"),
                Component::Bidder("fake_alpha"),
                Component::Bidder("fake_beta"),
                Component::Bidder("fake_no_gvl"),
                Component::Bidder("fake_delta"),
                Component::Analytics("analytics"),
                Component::Rtd("rtd"),
            ] {
                assert!(
                    ctx.activity_allow(activity, component),
                    "stub denied {activity:?} for {component:?}",
                );
            }
        }
    }

    #[test]
    fn auction_stub_returns_allow_all_for_every_bidder() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        for bidder in [
            "fake_alpha",
            "fake_beta",
            "fake_no_gvl",
            "fake_delta",
            "fake_unknown",
        ] {
            assert_eq!(
                ctx.permissions(bidder, bidder),
                AuctionPermissions::AllowAll
            );
        }
    }

    #[test]
    fn sync_ambiguous_uses_raw_host_no_default() {
        let mut host = HostConfig::default();
        host.gdpr.default_value = "0".into();
        let account = AccountConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let input = SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieAbsent,
            consent: None,
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        let ctx = resolve_sync_privacy(input).expect("ambiguous signal is valid");
        assert_eq!(ctx.parsed_request_signal, Signal::Ambiguous);
        assert_eq!(ctx.normalized_request_signal, Signal::No);
        assert_eq!(ctx.selected_consent(), None);
    }

    #[test]
    fn setuid_explicit_yes_requires_nonempty_consent() {
        let host = HostConfig::default();
        let account = AccountConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let input = SyncPrivacyInput {
            gdpr: GdprSignalIn::SetUidQuery(Some("1")),
            consent: Some(""),
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::SetUid,
            gvl: &gvl,
            gcl: &gcl,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        assert_eq!(
            resolve_sync_privacy(input).err(),
            Some(SyncPrivacyError::MissingRequiredConsent)
        );
    }
}
