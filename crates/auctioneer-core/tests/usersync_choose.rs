//! Synthetic user-sync selection and locally authored policy controls.

#[cfg(test)]
pub use auctioneer_core::test_support::bidder_fixture;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::bidder_fixture::registry;
    use auctioneer_config_data::types::{AccountConfig, HostConfig, SyncType};
    use auctioneer_core::{
        config::AuctioneerAppConfig,
        usersync::{
            catalog::Catalog,
            choose::{
                ChooseInput, ChooseResult, FilterSettings, Rejection, candidate_order, choose,
            },
            cookie::UidCookie,
            limits::CookieLimits,
            wire::FilterSettingsWire,
        },
    };
    use auctioneer_privacy::{
        GdprSignalIn, SyncEndpoint, SyncPrivacyInput, gcl::Gcl, gvl::GvlStore, resolve_sync_privacy,
    };
    use auctioneer_runtime::{Rng, SeededRng};
    use serde_json::{Value, json};
    use time::{OffsetDateTime, macros::datetime};

    const NOW: OffsetDateTime = datetime!(2026-09-30 12:00 UTC);

    struct IdentityRng;

    impl Rng for IdentityRng {
        fn next_u64(&self) -> u64 {
            u64::MAX
        }
    }

    fn catalog(host: &HostConfig) -> Catalog<'_> {
        Catalog::from_registry(host, &AuctioneerAppConfig::default(), registry()).unwrap()
    }

    fn select(
        host: &HostConfig,
        account: &AccountConfig,
        catalog: &Catalog<'_>,
        requested: &[String],
        filters: &FilterSettings,
        cookie: &UidCookie,
        limit: Option<i64>,
    ) -> ChooseResult {
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], NOW);
        let mut privacy = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(0),
            consent: None,
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account,
            host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now: NOW,
        })
        .unwrap();
        choose(
            ChooseInput {
                catalog,
                requested,
                cooperative: false,
                priority_groups: &[],
                filters,
                cookie,
                now: NOW,
                limit,
                debug: true,
            },
            &mut privacy,
            &IdentityRng,
        )
        .unwrap()
    }

    #[test]
    fn four_requested_and_cooperative_order_cases_match_reference_groups() {
        let requested = vec!["fake_bbb".into(), "fake_aaa".into()];
        let groups = vec![vec!["fake_cc".into()], vec!["fake_bbb".into()]];
        let available = ["fake_aaa", "fake_bbb", "fake_cc"];
        let names = |requested_names: &[String], cooperative| {
            candidate_order(
                requested_names,
                cooperative,
                &groups,
                &available,
                &SeededRng::new(17),
            )
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
        };
        assert_eq!(names(&requested, false).len(), 2);
        assert_eq!(names(&requested, true).len(), 7);
        assert_eq!(names(&[], false).len(), 3);
        assert_eq!(names(&[], true).len(), 5);
        let cooperative = names(&requested, true);
        assert_eq!(cooperative[..2].iter().collect::<BTreeSet<_>>().len(), 2);
        assert_eq!(cooperative[2], "fake_cc");
        assert_eq!(cooperative[3], "fake_bbb");
        assert_eq!(names(&requested, true), cooperative);
        assert_eq!(requested, vec!["fake_bbb", "fake_aaa"]);
    }

    #[test]
    fn identity_shuffle_matches_four_reference_order_fixtures() {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../auctioneer-bidder/tests/conformance/fixtures/usersync/chooser.json"
        ))
        .unwrap();
        let cases = corpus["cases"].as_array().unwrap();
        let mut checked = 0_usize;
        for case in cases
            .iter()
            .filter(|case| case["operation"] == "bidder_order")
        {
            let input = &case["input"];
            let requested = strings(&input["requested"]);
            let groups = input["priority_groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(strings)
                .collect::<Vec<_>>();
            let available = strings(&input["available"]);
            let available_names = available.iter().map(String::as_str).collect::<Vec<_>>();
            let actual = candidate_order(
                &requested,
                input["cooperative"].as_bool().unwrap(),
                &groups,
                &available_names,
                &IdentityRng,
            );
            assert_eq!(
                actual,
                strings(&case["expected"]["bidders"]),
                "{}",
                case["id"]
            );
            checked += 1;
        }
        assert_eq!(checked, 4);
    }

    fn strings(value: &Value) -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn alias_family_is_reserved_before_a_later_filter_denies_it() {
        let mut host = HostConfig::default();
        host.aliases
            .insert("fake_A_Alias".into(), "fake_aaa".into());
        let catalog = catalog(&host);
        let requested = vec!["fake_A_Alias".into(), "fake_aaa".into()];
        let filters = FilterSettings::parse(Some(&json!({
            "image": {"filter": "include", "bidders": ["fake_aaa"]},
            "iframe": {"filter": "exclude", "bidders": "*"}
        })))
        .unwrap();
        let result = select(
            &host,
            &AccountConfig::default(),
            &catalog,
            &requested,
            &filters,
            &UidCookie::default(),
            None,
        );
        assert!(result.selected.is_empty());
        assert_eq!(result.debug[1].reason, Rejection::SharedFamily);
    }

    #[test]
    fn only_successful_selections_count_toward_limit_and_iframe_is_preferred() {
        let host = HostConfig::default();
        let catalog = catalog(&host);
        let requested = vec!["fake_unknown".into(), "fake_bbb".into(), "fake_aaa".into()];
        let result = select(
            &host,
            &AccountConfig::default(),
            &catalog,
            &requested,
            &FilterSettings::default(),
            &UidCookie::default(),
            Some(1),
        );
        assert_eq!(result.selected.len(), 1);
        assert_eq!(result.selected[0].name, "fake_bbb");
        assert_eq!(result.selected[0].sync_type, SyncType::Iframe);
        assert_eq!(result.debug[0].reason, Rejection::UnknownBidder);
    }

    #[test]
    fn runtime_disabled_candidate_stays_in_order_but_is_not_selected() {
        let host = HostConfig::default();
        let app: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_aaa]\nusersync_enabled = false\n").unwrap();
        let catalog = Catalog::from_registry(&host, &app, registry()).unwrap();
        assert!(catalog.available_names().contains(&"fake_aaa"));
        let requested = vec!["fake_aaa".into()];
        let result = select(
            &host,
            &AccountConfig::default(),
            &catalog,
            &requested,
            &FilterSettings::default(),
            &UidCookie::default(),
            None,
        );
        assert!(result.selected.is_empty());
        assert_eq!(result.debug[0].reason, Rejection::Disabled);
    }

    #[test]
    fn disabled_automatic_sync_selection_matches_its_declared_policy_difference() {
        // This locally authored policy row is explicitly not an upstream capture.
        let divergences: Value = serde_json::from_str(include_str!(
            "../../auctioneer-bidder/tests/conformance/fixtures/usersync/divergences.json"
        ))
        .unwrap();
        let row = divergences["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "disabled_automatic_sync_candidates")
            .unwrap();
        let input = &row["input"];
        let host = HostConfig {
            aliases: serde_json::from_value(input["aliases"].clone()).unwrap(),
            ..HostConfig::default()
        };
        let app: AuctioneerAppConfig =
            toml::from_str(input["app_config"].as_str().unwrap()).unwrap();
        // This frozen policy corpus names its own identities; keep its inputs and
        // expectations verbatim instead of projecting them into the fake namespace.
        let mut reference_registry = auctioneer_bidder::BidderRegistry::new();
        for (source_name, fixture_name) in [
            ("bidder_a", "fake_aaa"),
            ("bidder_b", "fake_bbb"),
            ("plain_a", "fake_pp"),
        ] {
            let (template, _) = registry().get(fixture_name).unwrap();
            let mut info = template.clone();
            info.name = source_name.to_owned();
            reference_registry
                .register(
                    Box::leak(Box::new(info)),
                    Box::<super::bidder_fixture::fake_bidder::FakeBidder>::default(),
                )
                .expect("unique test bidder");
        }
        let catalog = Catalog::from_registry(&host, &app, &reference_registry).unwrap();
        assert!(!catalog.lookup("aaa-disabled").unwrap().enabled);
        assert!(catalog.lookup("bidder_a").unwrap().enabled);
        assert_eq!(
            catalog.lookup("aaa-disabled").unwrap().family,
            catalog.lookup("bidder_a").unwrap().family
        );
        let priority_groups: Vec<Vec<String>> =
            serde_json::from_value(input["priority_groups"].clone()).unwrap();
        let cases = input["cases"].as_array().unwrap();
        let expected_cases = row["expected"]["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 3);
        assert_eq!(expected_cases.len(), 3);
        let account = AccountConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], NOW);
        for (case, expected) in cases.iter().zip(expected_cases) {
            let requested: Vec<String> = serde_json::from_value(case["requested"].clone()).unwrap();
            let mut privacy = resolve_sync_privacy(SyncPrivacyInput {
                gdpr: GdprSignalIn::CookieNumber(0),
                consent: None,
                us_privacy: None,
                gpp: None,
                gpp_sid: None,
                account: &account,
                host: &host,
                endpoint: SyncEndpoint::CookieSync,
                gvl: &gvl,
                gcl: &gcl,
                now: NOW,
            })
            .unwrap();
            let result = choose(
                ChooseInput {
                    catalog: &catalog,
                    requested: &requested,
                    cooperative: case["cooperative"].as_bool().unwrap(),
                    priority_groups: &priority_groups,
                    filters: &FilterSettings::default(),
                    cookie: &UidCookie::default(),
                    now: NOW,
                    limit: None,
                    debug: true,
                },
                &mut privacy,
                &IdentityRng,
            )
            .unwrap();
            let actual = json!({
                "selected": result.selected.iter().map(|sync| sync.name.as_str()).collect::<Vec<_>>(),
                "debug": result.debug.iter().map(|diagnostic| json!({
                    "name": diagnostic.name,
                    "reason": format!("{:?}", diagnostic.reason),
                })).collect::<Vec<_>>(),
            });
            assert_eq!(&actual, expected, "{case:?}");
        }
    }

    #[test]
    fn activity_denial_comes_from_privacy_before_runtime_disable() {
        let host = HostConfig::default();
        let app: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_aaa]\nusersync_enabled = false\n").unwrap();
        let catalog = Catalog::from_registry(&host, &app, registry()).unwrap();
        let mut account = AccountConfig::default();
        account.privacy.allowactivities =
            serde_json::from_value(json!({"syncUser":{"default":false}})).unwrap();
        let requested = vec!["fake_aaa".into()];
        let result = select(
            &host,
            &account,
            &catalog,
            &requested,
            &FilterSettings::default(),
            &UidCookie::default(),
            None,
        );
        assert!(result.selected.is_empty());
        assert_eq!(result.debug[0].reason, Rejection::ActivityDenied);
    }

    #[test]
    fn live_uid_blocks_family_and_unlimited_selection_is_stable() {
        let host = HostConfig::default();
        let catalog = catalog(&host);
        let mut cookie = UidCookie::default();
        cookie
            .set_uid("fam_a", "uid", NOW, &CookieLimits::default())
            .unwrap();
        let requested = vec!["fake_aaa".into(), "fake_bbb".into()];
        let run = || {
            select(
                &host,
                &AccountConfig::default(),
                &catalog,
                &requested,
                &FilterSettings::default(),
                &cookie,
                None,
            )
        };
        let first = run();
        assert_eq!(first.selected.len(), 1);
        assert_eq!(first.selected[0].name, "fake_bbb");
        assert_eq!(first.debug[0].reason, Rejection::AlreadySynced);
        assert_eq!(
            first
                .selected
                .iter()
                .map(|sync| sync.name.as_str())
                .collect::<Vec<_>>(),
            run()
                .selected
                .iter()
                .map(|sync| sync.name.as_str())
                .collect::<Vec<_>>()
        );
        assert!(cookie.is_live("fam_a", NOW));
        assert_eq!(requested, vec!["fake_aaa", "fake_bbb"]);
    }

    #[test]
    fn zero_and_maximum_logical_limits_do_not_truncate_a_bounded_catalog() {
        let host = HostConfig::default();
        let catalog = catalog(&host);
        let requested = vec!["fake_aaa".into(), "fake_bbb".into()];
        for limit in [Some(0), Some(i64::MAX), None] {
            let result = select(
                &host,
                &AccountConfig::default(),
                &catalog,
                &requested,
                &FilterSettings::default(),
                &UidCookie::default(),
                limit,
            );
            assert_eq!(result.selected.len(), 2);
        }
    }

    #[test]
    fn filters_are_strict_bounded_and_ascii_case_insensitive() {
        let filters = FilterSettings::parse(Some(&json!({
            "image": {"filter": "include", "bidders": ["fake_AAA"]},
            "iframe": {"filter": "exclude", "bidders": "*"}
        })))
        .unwrap();
        let host = HostConfig::default();
        let catalog = catalog(&host);
        let requested = vec!["fake_Aaa".into(), "fake_bbb".into()];
        let result = select(
            &host,
            &AccountConfig::default(),
            &catalog,
            &requested,
            &filters,
            &UidCookie::default(),
            None,
        );
        assert_eq!(result.selected.len(), 1);
        assert_eq!(result.selected[0].name, "fake_Aaa");
        assert_eq!(result.selected[0].sync_type, SyncType::Redirect);
        FilterSettings::parse(Some(
            &json!({"redirect": {"filter":"include","bidders":"*"}}),
        ))
        .unwrap_err();
        FilterSettings::parse(Some(&json!({"image": {"filter":"include","bidders":[]}}))).unwrap();
        FilterSettings::parse(Some(&json!({"image": {"filter":"other","bidders":"*"}})))
            .unwrap_err();
        FilterSettings::parse(Some(
            &json!({"image": {"filter":"include","bidders":vec!["fake_x"; 257]}}),
        ))
        .unwrap_err();
    }

    #[test]
    fn typed_wire_filters_feed_the_chooser_without_reparsing_json() {
        let wire: FilterSettingsWire = serde_json::from_value(json!({
            "image": {"filter":"include","bidders":["fake_AAA"]},
            "iframe": {"filter":"exclude","bidders":"*"}
        }))
        .unwrap();
        let filters = FilterSettings::from_wire(&wire).unwrap();
        let host = HostConfig::default();
        let catalog = catalog(&host);
        let requested = vec!["fake_Aaa".into(), "fake_bbb".into()];
        let result = select(
            &host,
            &AccountConfig::default(),
            &catalog,
            &requested,
            &filters,
            &UidCookie::default(),
            None,
        );
        assert_eq!(result.selected.len(), 1);
        assert_eq!(result.selected[0].name, "fake_Aaa");
        assert_eq!(result.selected[0].sync_type, SyncType::Redirect);

        let invalid: FilterSettingsWire =
            serde_json::from_value(json!({"image":{"bidders":"*"}})).unwrap();
        FilterSettings::from_wire(&invalid).unwrap_err();
    }
}
