//! Scoped response-budget resolution before envelope clamping.

use crate::config::AuctioneerAppConfig;
use auctioneer_bidder::BidderRegistry;
use auctioneer_config::types::HostConfig;
use auctioneer_exchange::{AccountSource, BidderBudgets, ResolvedAliases, ResolvedBudgets};

/// Resolve both caps per field: observable bidder, alias base, selected
/// account, runtime host, baked host, then the historic response cap.
/// These are requested values; exchange lookup clamps to the funded
/// boot envelope and emits warnings under the dispatched bidder's name.
#[must_use]
pub fn resolve_budgets(
    app: &AuctioneerAppConfig,
    baked: &HostConfig,
    account_id: &str,
    account_source: AccountSource,
    registry: &BidderRegistry,
    aliases: &ResolvedAliases,
) -> ResolvedBudgets {
    let limits = &baked.resource_limits;
    let legacy = u64::try_from(
        app.auction
            .max_bidder_response_bytes
            .unwrap_or(limits.max_bidder_response_bytes),
    )
    .unwrap_or(u64::MAX);
    let legacy_override = app.auction.max_bidder_response_bytes.map(|_| legacy);
    let host = BidderBudgets {
        max_transport_bytes: app
            .auction
            .max_transport_bytes
            .or(legacy_override)
            .unwrap_or(if limits.max_transport_bytes == 0 {
                legacy
            } else {
                limits.max_transport_bytes
            }),
        max_decoded_bytes: app.auction.max_decoded_bytes.or(legacy_override).unwrap_or(
            if limits.max_decoded_bytes == 0 {
                legacy
            } else {
                limits.max_decoded_bytes
            },
        ),
    };
    let account = match account_source {
        AccountSource::Configured => app.accounts.get(account_id),
        AccountSource::DefaultFallback | AccountSource::Provided => None,
    };
    let defaults = BidderBudgets {
        max_transport_bytes: account
            .and_then(|row| row.max_transport_bytes)
            .unwrap_or(host.max_transport_bytes),
        max_decoded_bytes: account
            .and_then(|row| row.max_decoded_bytes)
            .unwrap_or(host.max_decoded_bytes),
    };
    let mut budgets = ResolvedBudgets::new();
    // Populate only registry identities and resolved aliases, not arbitrary
    // bidder override keys that have no route in this auction.
    for name in registry
        .base_names()
        .into_iter()
        .chain(aliases.map.bases.keys().cloned())
    {
        let own = app.bidders.get(name.as_str());
        let base = aliases
            .map
            .bases
            .get(&name)
            .and_then(|base| app.bidders.get(base.as_str()));
        budgets.insert(
            name,
            BidderBudgets {
                max_transport_bytes: own
                    .and_then(|row| row.max_transport_bytes)
                    .or(base.and_then(|row| row.max_transport_bytes))
                    .unwrap_or(defaults.max_transport_bytes),
                max_decoded_bytes: own
                    .and_then(|row| row.max_decoded_bytes)
                    .or(base.and_then(|row| row.max_decoded_bytes))
                    .unwrap_or(defaults.max_decoded_bytes),
            },
        );
    }
    budgets
}

#[cfg(test)]
mod tests {
    #[test]
    fn fallback_identity_cannot_select_the_unknown_publisher_override() {
        use crate::config::resolve::resolve_account_from_value;
        use auctioneer_config::types::AccountConfig;

        let app: AuctioneerAppConfig = toml::from_str(
            "[accounts.unknown]\nmax_transport_bytes = 1024\nmax_decoded_bytes = 2048\n",
        )
        .unwrap();
        let publisherless = serde_json::json!({"site": {}});
        let fallback =
            resolve_account_from_value(Some(&publisherless), &app, &AccountConfig::default())
                .unwrap();
        assert_eq!(fallback.source, AccountSource::DefaultFallback);
        let budgets = resolve_budgets(
            &app,
            &host(),
            &fallback.id,
            fallback.source,
            registry(),
            &ResolvedAliases::empty(),
        );
        assert_eq!(
            budgets[&BidderName::new("fake_pp")],
            BidderBudgets {
                max_transport_bytes: 0x0001_0000,
                max_decoded_bytes: 0x0001_0000
            }
        );
        let named_publisher = serde_json::json!({"site": {"publisher": {"id": "unknown"}}});
        let configured =
            resolve_account_from_value(Some(&named_publisher), &app, &AccountConfig::default())
                .unwrap();
        assert_eq!(configured.source, AccountSource::Configured);
        let configured_budgets = resolve_budgets(
            &app,
            &host(),
            &configured.id,
            configured.source,
            registry(),
            &ResolvedAliases::empty(),
        );
        assert_eq!(
            configured_budgets[&BidderName::new("fake_pp")],
            BidderBudgets {
                max_transport_bytes: 1_024,
                max_decoded_bytes: 2_048
            }
        );
    }

    use super::{AccountSource, resolve_budgets};
    use crate::bidder_fixture::registry;
    use crate::config::AuctioneerAppConfig;
    use crate::handlers::request_disclosure_audit::construction_costs;
    use auctioneer_bidder::BidderName;
    use auctioneer_config::types::HostConfig;
    use auctioneer_exchange::{
        AliasMap, BidderBudgets, BootEnvelope, ResolvedAliases, resolve_budget,
    };

    fn host() -> HostConfig {
        let mut host = HostConfig::default();
        host.resource_limits.max_bidder_response_bytes = 0x0001_0000;
        host
    }

    fn aliases() -> ResolvedAliases {
        let mut map = AliasMap::default();
        map.bases
            .insert(BidderName::new("fake_alias"), BidderName::new("fake_aaa"));
        ResolvedAliases::wrap(map)
    }

    #[test]
    fn unset_response_caps_default_to_the_legacy_cap() {
        let budgets = resolve_budgets(
            &AuctioneerAppConfig::default(),
            &host(),
            "publisher",
            AccountSource::Configured,
            registry(),
            &aliases(),
        );
        assert!(budgets.len() > 1);
        for budget in budgets.values() {
            assert_eq!(
                *budget,
                BidderBudgets {
                    max_transport_bytes: 0x0001_0000,
                    max_decoded_bytes: 0x0001_0000
                }
            );
        }
    }

    #[test]
    fn response_caps_follow_baked_runtime_account_and_bidder_precedence_per_field() {
        let mut baked = host();
        baked.resource_limits.max_transport_bytes = 0x8000;
        baked.resource_limits.max_decoded_bytes = 0xC000;
        let stages = [
            ("", (0x8000, 0xC000)),
            (
                "[auction]\nmax_transport_bytes = 8192\nmax_decoded_bytes = 16384\n",
                (0x2000, 0x4000),
            ),
            (
                "[auction]\nmax_transport_bytes = 8192\nmax_decoded_bytes = 16384\n[accounts.publisher]\nmax_transport_bytes = 24576\n",
                (0x6000, 0x4000),
            ),
            (
                "[auction]\nmax_transport_bytes = 8192\nmax_decoded_bytes = 16384\n[accounts.publisher]\nmax_transport_bytes = 24576\n[bidders.fake_aaa]\nmax_decoded_bytes = 32768\n",
                (0x6000, 0x8000),
            ),
        ];
        for (toml, expected) in stages {
            let app: AuctioneerAppConfig = toml::from_str(toml).unwrap();
            let budgets = resolve_budgets(
                &app,
                &baked,
                "publisher",
                AccountSource::Configured,
                registry(),
                &aliases(),
            );
            for name in ["fake_aaa", "fake_alias"] {
                let budget = budgets[&BidderName::new(name)];
                assert_eq!(
                    (budget.max_transport_bytes, budget.max_decoded_bytes),
                    expected
                );
            }
        }
    }

    #[test]
    fn alias_own_overrides_fall_back_to_the_base_per_field() {
        for (own, expected) in [
            ("max_transport_bytes = 4096", (4_096, 0x8000)),
            ("max_decoded_bytes = 8192", (0x4000, 0x2000)),
            (
                "max_transport_bytes = 4096\nmax_decoded_bytes = 8192",
                (4_096, 0x2000),
            ),
        ] {
            let app: AuctioneerAppConfig = toml::from_str(&format!(
                "[bidders.fake_aaa]\nmax_transport_bytes = 16384\nmax_decoded_bytes = 32768\n[bidders.fake_alias]\n{own}\n"
            )).unwrap();
            let budgets = resolve_budgets(
                &app,
                &host(),
                "publisher",
                AccountSource::Configured,
                registry(),
                &aliases(),
            );
            let budget = budgets[&BidderName::new("fake_alias")];
            assert_eq!(
                (budget.max_transport_bytes, budget.max_decoded_bytes),
                expected
            );
            assert_eq!(
                budgets[&BidderName::new("fake_aaa")],
                BidderBudgets {
                    max_transport_bytes: 0x4000,
                    max_decoded_bytes: 0x8000
                }
            );
        }
    }

    #[test]
    fn selected_account_and_each_bidder_keep_distinct_caps() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[accounts.publisher]\nmax_transport_bytes = 8192\nmax_decoded_bytes = 16384\n[accounts.other]\nmax_transport_bytes = 1024\n[bidders.fake_aaa]\nmax_decoded_bytes = 32768\n[bidders.fake_pp]\nmax_transport_bytes = 4096\n"
        ).unwrap();
        let budgets = resolve_budgets(
            &app,
            &host(),
            "publisher",
            AccountSource::Configured,
            registry(),
            &aliases(),
        );
        assert_eq!(
            budgets[&BidderName::new("fake_aaa")],
            BidderBudgets {
                max_transport_bytes: 0x2000,
                max_decoded_bytes: 0x8000
            }
        );
        assert_eq!(
            budgets[&BidderName::new("fake_pp")],
            BidderBudgets {
                max_transport_bytes: 4_096,
                max_decoded_bytes: 0x4000
            }
        );
        assert_eq!(
            budgets[&BidderName::new("fake_bbb")],
            BidderBudgets {
                max_transport_bytes: 0x2000,
                max_decoded_bytes: 0x4000
            }
        );
        let defaults = resolve_budgets(
            &app,
            &host(),
            "missing",
            AccountSource::DefaultFallback,
            registry(),
            &ResolvedAliases::empty(),
        );
        assert_eq!(
            defaults[&BidderName::new("fake_bbb")].max_transport_bytes,
            0x0001_0000
        );
    }

    #[test]
    fn a_scoped_override_can_raise_the_host_default_but_not_the_boot_envelope() {
        use auctioneer_openrtb::WarningCode;

        let app: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_transport_bytes = 1024\nmax_decoded_bytes = 2048\n[accounts.publisher]\nmax_transport_bytes = 32768\n[bidders.fake_aaa]\nmax_decoded_bytes = 65536\n"
        ).unwrap();
        let budgets = resolve_budgets(
            &app,
            &host(),
            "publisher",
            AccountSource::Configured,
            registry(),
            &aliases(),
        );
        let mut warnings = Vec::new();
        let budget = resolve_budget(
            &budgets,
            &aliases(),
            &BidderName::new("fake_alias"),
            Some(BootEnvelope {
                max_transport_bytes: 0x4000,
                max_decoded_bytes: 0x0002_0000,
                construction_costs: construction_costs(),
            }),
            &mut warnings,
        )
        .unwrap();
        assert_eq!(
            budget,
            BidderBudgets {
                max_transport_bytes: 0x4000,
                max_decoded_bytes: 0x0001_0000
            }
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, WarningCode::OutboundBudgetClamp.as_u32());
    }

    #[test]
    fn legacy_runtime_override_sets_only_caps_without_independent_overrides() {
        let mut baked = host();
        baked.resource_limits.max_transport_bytes = 0x8000;
        baked.resource_limits.max_decoded_bytes = 0xC000;
        let app: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_bidder_response_bytes = 8192\nmax_decoded_bytes = 16384\n",
        )
        .unwrap();
        let budgets = resolve_budgets(
            &app,
            &baked,
            "publisher",
            AccountSource::Configured,
            registry(),
            &ResolvedAliases::empty(),
        );
        assert_eq!(
            budgets[&BidderName::new("fake_aaa")],
            BidderBudgets {
                max_transport_bytes: 0x2000,
                max_decoded_bytes: 0x4000
            }
        );
    }
}
