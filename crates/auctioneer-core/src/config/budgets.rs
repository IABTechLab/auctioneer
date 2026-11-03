//! Scoped response-budget resolution before envelope clamping.

use crate::config::{
    AuctionConfig, AuctioneerAppConfig, BidderOverrides, policy::OperatorPolicy,
    resolve::AccountSelection,
};
use auctioneer_bidder::BidderRegistry;
use auctioneer_config_data::types::HostConfig;
use auctioneer_exchange::{AccountSource, BidderBudgets, ResolvedAliases, ResolvedBudgets};
use std::collections::BTreeMap;

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
    let host = resolve_host_budgets(&app.auction, baked);
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
    resolve_bidder_budgets(&app.bidders, defaults, registry, aliases)
}

/// Compose requested caps per field: observable bidder, alias base, named
/// account, deployment account defaults, runtime host, then product host.
///
/// Default selection never looks up a synthesized account identity. This staged
/// resolver does not validate policy, consult boot state or clamp to capacity;
/// exchange lookup retains installed minima and clamp-warning ownership.
#[must_use]
pub fn resolve_policy_budgets(
    policy: &OperatorPolicy,
    product: &HostConfig,
    selection: &AccountSelection,
    registry: &BidderRegistry,
    aliases: &ResolvedAliases,
) -> ResolvedBudgets {
    let host = resolve_host_budgets(&policy.host.resource_limits, product);
    let account = match selection {
        AccountSelection::Named(id) => policy.accounts.get(id),
        AccountSelection::Default => None,
    };
    let defaults = BidderBudgets {
        max_transport_bytes: account
            .and_then(|row| row.max_transport_bytes)
            .or(policy.account_defaults.max_transport_bytes)
            .unwrap_or(host.max_transport_bytes),
        max_decoded_bytes: account
            .and_then(|row| row.max_decoded_bytes)
            .or(policy.account_defaults.max_decoded_bytes)
            .unwrap_or(host.max_decoded_bytes),
    };
    resolve_bidder_budgets(&policy.bidders, defaults, registry, aliases)
}

fn resolve_host_budgets(requested: &AuctionConfig, product: &HostConfig) -> BidderBudgets {
    let limits = &product.resource_limits;
    let legacy = u64::try_from(
        requested
            .max_bidder_response_bytes
            .unwrap_or(limits.max_bidder_response_bytes),
    )
    .unwrap_or(u64::MAX);
    let legacy_override = requested.max_bidder_response_bytes.map(|_| legacy);
    BidderBudgets {
        max_transport_bytes: requested.max_transport_bytes.or(legacy_override).unwrap_or(
            if limits.max_transport_bytes == 0 {
                legacy
            } else {
                limits.max_transport_bytes
            },
        ),
        max_decoded_bytes: requested.max_decoded_bytes.or(legacy_override).unwrap_or(
            if limits.max_decoded_bytes == 0 {
                legacy
            } else {
                limits.max_decoded_bytes
            },
        ),
    }
}

fn resolve_bidder_budgets(
    bidders: &BTreeMap<String, BidderOverrides>,
    defaults: BidderBudgets,
    registry: &BidderRegistry,
    aliases: &ResolvedAliases,
) -> ResolvedBudgets {
    let mut budgets = ResolvedBudgets::new();
    // Populate only registry identities and resolved aliases, not arbitrary
    // bidder override keys that have no route in this auction.
    for name in registry
        .base_names()
        .into_iter()
        .chain(aliases.map.bases.keys().cloned())
    {
        let own = bidders.get(name.as_str());
        let base = aliases
            .map
            .bases
            .get(&name)
            .and_then(|base| bidders.get(base.as_str()));
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
        use auctioneer_config_data::types::AccountConfig;

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

    use super::{AccountSource, resolve_budgets, resolve_policy_budgets};
    use crate::bidder_fixture::registry;
    use crate::config::{
        AccountOverride, AuctioneerAppConfig, policy::OperatorPolicy, resolve::AccountSelection,
    };
    use crate::handlers::request_disclosure_audit::construction_costs;
    use auctioneer_bidder::BidderName;
    use auctioneer_config_data::types::HostConfig;
    use auctioneer_exchange::{
        AliasMap, BidderBudgets, BootEnvelope, ResolvedAliases, resolve_budget,
    };
    use serde_json::{Value, json};
    use validator::Validate as _;

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

    fn policy(document: Value) -> OperatorPolicy {
        serde_json::from_value(document).expect("typed fake operator policy")
    }

    #[test]
    fn policy_response_caps_follow_all_scopes_independently_per_field() {
        let mut product = host();
        product.resource_limits.max_transport_bytes = 0x8000;
        product.resource_limits.max_decoded_bytes = 49_152;
        let scopes = [
            "/host/resource_limits",
            "/account_defaults",
            "/accounts/fake_account",
            "/bidders/fake_aaa",
            "/bidders/fake_alias",
        ];
        for (field, expected_by_scope) in [
            (
                "max_transport_bytes",
                [0x8000, 8_192, 24_576, 4_096, 0x4000, 1_024],
            ),
            (
                "max_decoded_bytes",
                [49_152, 12_288, 0x7000, 6_144, 20_480, 2_048],
            ),
        ] {
            for (highest, expected_cap) in expected_by_scope.into_iter().enumerate() {
                let mut document = json!({
                    "schema_version": 1_u64,
                    "host": {"resource_limits": {
                        "max_transport_bytes": 8_192_u64, "max_decoded_bytes": 12_288_u64
                    }},
                    "account_defaults": {
                        "max_transport_bytes": 24_576_u64, "max_decoded_bytes": 0x7000_u64
                    },
                    "accounts": {"fake_account": {
                        "max_transport_bytes": 4_096_u64, "max_decoded_bytes": 6_144_u64
                    }},
                    "bidders": {
                        "fake_aaa": {
                            "max_transport_bytes": 0x4000_u64, "max_decoded_bytes": 20_480_u64
                        },
                        "fake_alias": {
                            "max_transport_bytes": 1_024_u64, "max_decoded_bytes": 2_048_u64
                        }
                    }
                });
                for scope in scopes.iter().skip(highest) {
                    document
                        .pointer_mut(scope)
                        .expect("response-cap scope")
                        .as_object_mut()
                        .expect("response-cap object")
                        .remove(field)
                        .expect("higher-priority cap was present");
                }
                let budgets = resolve_policy_budgets(
                    &policy(document),
                    &product,
                    &AccountSelection::Named("fake_account".into()),
                    registry(),
                    &aliases(),
                );
                let expected = if field == "max_transport_bytes" {
                    BidderBudgets {
                        max_transport_bytes: expected_cap,
                        max_decoded_bytes: 2_048,
                    }
                } else {
                    BidderBudgets {
                        max_transport_bytes: 1_024,
                        max_decoded_bytes: expected_cap,
                    }
                };
                assert_eq!(
                    budgets[&BidderName::new("fake_alias")],
                    expected,
                    "{field} at scope {highest}"
                );
            }
        }
    }

    #[test]
    fn policy_response_caps_use_legacy_product_fallback_only_for_zero_fields() {
        for (transport, decoded, expected) in [
            (0, 0, (0x0001_0000, 0x0001_0000)),
            (0, 24_576, (0x0001_0000, 24_576)),
            (0x8000, 0, (0x8000, 0x0001_0000)),
            (0x8000, 24_576, (0x8000, 24_576)),
        ] {
            let mut product = host();
            product.resource_limits.max_transport_bytes = transport;
            product.resource_limits.max_decoded_bytes = decoded;
            let budgets = resolve_policy_budgets(
                &OperatorPolicy::default(),
                &product,
                &AccountSelection::Default,
                registry(),
                &aliases(),
            );
            assert!(!budgets.is_empty());
            for budget in budgets.values() {
                assert_eq!(
                    (budget.max_transport_bytes, budget.max_decoded_bytes),
                    expected
                );
            }
        }
    }

    #[test]
    fn policy_response_caps_keep_runtime_legacy_fallback_below_independent_caps() {
        let mut product = host();
        product.resource_limits.max_transport_bytes = 0x8000;
        product.resource_limits.max_decoded_bytes = 49_152;
        for (limits, expected) in [
            (
                json!({"max_bidder_response_bytes": 8_192_u64}),
                (8_192, 8_192),
            ),
            (
                json!({"max_bidder_response_bytes": 8_192_u64, "max_transport_bytes": 4_096_u64}),
                (4_096, 8_192),
            ),
            (
                json!({"max_bidder_response_bytes": 8_192_u64, "max_decoded_bytes": 0x4000_u64}),
                (8_192, 0x4000),
            ),
            (
                json!({
                    "max_bidder_response_bytes": 8_192_u64,
                    "max_transport_bytes": 4_096_u64, "max_decoded_bytes": 0x4000_u64
                }),
                (4_096, 0x4000),
            ),
        ] {
            let budgets = resolve_policy_budgets(
                &policy(json!({"schema_version": 1_u64, "host": {"resource_limits": limits}})),
                &product,
                &AccountSelection::Default,
                registry(),
                &ResolvedAliases::empty(),
            );
            let budget = budgets[&BidderName::new("fake_aaa")];
            assert_eq!(
                (budget.max_transport_bytes, budget.max_decoded_bytes),
                expected
            );
        }
    }

    #[test]
    fn policy_response_caps_distinguish_default_missing_and_literal_unknown_accounts() {
        for (defaults, expected) in [
            (json!({}), (8_192, 12_288)),
            (json!({"max_transport_bytes": 24_576_u64}), (24_576, 12_288)),
            (json!({"max_decoded_bytes": 0x7000_u64}), (8_192, 0x7000)),
            (
                json!({"max_transport_bytes": 24_576_u64, "max_decoded_bytes": 0x7000_u64}),
                (24_576, 0x7000),
            ),
        ] {
            let mut policy = policy(json!({
                "schema_version": 1_u64,
                "host": {"resource_limits": {
                    "max_transport_bytes": 8_192_u64, "max_decoded_bytes": 12_288_u64
                }},
                "account_defaults": defaults,
                "accounts": {"unknown": {
                    "max_transport_bytes": 1_024_u64, "max_decoded_bytes": 2_048_u64
                }}
            }));
            let named = AccountSelection::Named("unknown".into());
            let budgets = resolve_policy_budgets(&policy, &host(), &named, registry(), &aliases());
            assert_eq!(
                budgets[&BidderName::new("fake_bbb")],
                BidderBudgets {
                    max_transport_bytes: 1_024,
                    max_decoded_bytes: 2_048,
                }
            );
            for selection in [
                AccountSelection::Default,
                AccountSelection::Named("fake_missing".into()),
            ] {
                let fallback_budgets =
                    resolve_policy_budgets(&policy, &host(), &selection, registry(), &aliases());
                let budget = fallback_budgets[&BidderName::new("fake_bbb")];
                assert_eq!(
                    (budget.max_transport_bytes, budget.max_decoded_bytes),
                    expected,
                    "{selection:?}"
                );
            }
            policy
                .accounts
                .remove("unknown")
                .expect("named before-state");
            let removed = resolve_policy_budgets(&policy, &host(), &named, registry(), &aliases());
            let budget = removed[&BidderName::new("fake_bbb")];
            assert_eq!(
                (budget.max_transport_bytes, budget.max_decoded_bytes),
                expected
            );
        }
    }

    #[test]
    fn policy_response_caps_keep_only_registry_and_resolved_alias_identities() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "host": {"aliases": {"fake_unresolved_alias": "fake_aaa"}},
            "account_defaults": {
                "max_transport_bytes": 4_096_u64, "max_decoded_bytes": 8_192_u64
            },
            "accounts": {"fake_account": {"max_transport_bytes": 0x4000_u64}},
            "bidders": {
                "fake_aaa": {"max_decoded_bytes": 0x8000_u64},
                "fake_alias": {"max_transport_bytes": 2_048_u64},
                "fake_pp": {"max_transport_bytes": 1_024_u64},
                "fake_unrouted": {"max_transport_bytes": 1_024_u64}
            }
        }));
        let budgets = resolve_policy_budgets(
            &policy,
            &host(),
            &AccountSelection::Named("fake_account".into()),
            registry(),
            &aliases(),
        );
        assert_eq!(budgets.len(), 4);
        for (name, expected) in [
            ("fake_aaa", (0x4000, 0x8000)),
            ("fake_alias", (2_048, 0x8000)),
            ("fake_bbb", (0x4000, 8_192)),
            ("fake_pp", (1_024, 8_192)),
        ] {
            let budget = budgets[&BidderName::new(name)];
            assert_eq!(
                (budget.max_transport_bytes, budget.max_decoded_bytes),
                expected,
                "{name}"
            );
        }
        assert!(!budgets.contains_key(&BidderName::new("fake_unrouted")));
        assert!(!budgets.contains_key(&BidderName::new("fake_unresolved_alias")));
    }

    #[test]
    fn policy_response_caps_are_clamped_only_by_exchange_lookup() {
        use auctioneer_openrtb::WarningCode;

        let policy = policy(json!({
            "schema_version": 1_u64,
            "host": {"resource_limits": {
                "max_transport_bytes": 1_024_u64, "max_decoded_bytes": 2_048_u64
            }},
            "account_defaults": {
                "max_transport_bytes": 0x8000_u64, "max_decoded_bytes": 0x0001_0000_u64
            },
            "bidders": {
                "fake_aaa": {"max_transport_bytes": 98_304_u64},
                "fake_alias": {"max_decoded_bytes": 0x0002_0000_u64}
            }
        }));
        policy
            .host
            .resource_limits
            .validate()
            .expect("valid host caps");
        for bidder in policy.bidders.values() {
            bidder
                .validate()
                .expect("valid bidder caps before clamping");
        }
        let aliases = aliases();
        let budgets = resolve_policy_budgets(
            &policy,
            &host(),
            &AccountSelection::Default,
            registry(),
            &aliases,
        );
        let name = BidderName::new("fake_alias");
        let requested = BidderBudgets {
            max_transport_bytes: 98_304,
            max_decoded_bytes: 0x0002_0000,
        };
        assert_eq!(budgets[&name], requested);
        let mut warnings = Vec::new();
        assert_eq!(
            resolve_budget(&budgets, &aliases, &name, None, &mut warnings).unwrap(),
            requested
        );
        assert!(warnings.is_empty());
        for (ceilings, expected, warning_count) in [
            ((0x4000, 0x0004_0000), (0x4000, 0x0002_0000), 1),
            ((0x0004_0000, 0x8000), (98_304, 0x8000), 1),
            ((0x4000, 0x8000), (0x4000, 0x8000), 1),
            ((0x0004_0000, 0x0004_0000), (98_304, 0x0002_0000), 0),
        ] {
            let mut clamp_warnings = Vec::new();
            let resolved = resolve_budget(
                &budgets,
                &aliases,
                &name,
                Some(BootEnvelope {
                    max_transport_bytes: ceilings.0,
                    max_decoded_bytes: ceilings.1,
                    construction_costs: construction_costs(),
                }),
                &mut clamp_warnings,
            )
            .unwrap();
            assert_eq!(
                (resolved.max_transport_bytes, resolved.max_decoded_bytes),
                expected
            );
            assert_eq!(clamp_warnings.len(), warning_count);
            assert!(
                clamp_warnings
                    .iter()
                    .all(|warning| warning.code == WarningCode::OutboundBudgetClamp.as_u32())
            );
            assert_eq!(budgets[&name], requested);
        }
    }

    #[test]
    fn policy_response_caps_leave_invalid_requests_to_validation() {
        for scope in [
            "/host/resource_limits",
            "/account_defaults",
            "/accounts/fake_account",
            "/bidders/fake_aaa",
            "/bidders/fake_alias",
        ] {
            for field in ["max_transport_bytes", "max_decoded_bytes"] {
                for cap in [0_u64, 1_023, 104_857_601] {
                    let mut document = json!({
                        "schema_version": 1_u64,
                        "host": {"resource_limits": {}},
                        "account_defaults": {},
                        "accounts": {"fake_account": {}},
                        "bidders": {"fake_aaa": {}, "fake_alias": {}}
                    });
                    document.pointer_mut(scope).expect("cap scope")[field] = json!(cap);
                    // AccountPolicy admission is staged; the existing account
                    // leaf validator still owns the unchanged response-cap range.
                    let account_leaf: AccountOverride =
                        serde_json::from_value(document.pointer(scope).expect("cap leaf").clone())
                            .expect("same typed response-cap leaf");
                    assert!(account_leaf.validate().is_err());
                    let policy = policy(document);
                    match scope.strip_prefix("/bidders/") {
                        Some(name) => assert!(policy.bidders[name].validate().is_err()),
                        None if scope == "/host/resource_limits" => {
                            assert!(policy.host.resource_limits.validate().is_err());
                        }
                        None => {}
                    }
                    let budgets = resolve_policy_budgets(
                        &policy,
                        &host(),
                        &AccountSelection::Named("fake_account".into()),
                        registry(),
                        &aliases(),
                    );
                    let budget = budgets[&BidderName::new("fake_alias")];
                    let actual = if field == "max_transport_bytes" {
                        budget.max_transport_bytes
                    } else {
                        budget.max_decoded_bytes
                    };
                    assert_eq!(actual, cap, "{scope}.{field} must not repair invalid caps");
                }
            }
        }
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
