//! Compiled only in the owned, fake-only settings workspace.

use auctioneer_runtime::test_support::alloc_track;
mod bounded_store;
mod config_funding_support;
use config_funding_support as config_funding;
mod fanout_limits;
mod generated_exchange_settings;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

mod allocation_meter {
    use super::alloc_track;
    use std::hint::black_box;

    #[test]
    fn canonical_allocator_tracks_allocations_without_refunding_deallocations() {
        let before = alloc_track::allocated();
        let buffer = black_box(Vec::<u8>::with_capacity(4096));
        let allocated = alloc_track::allocated();
        assert!(allocated - before >= 4096);
        drop(buffer);
        assert_eq!(alloc_track::allocated(), allocated);
    }
}

#[allow(
    dead_code,
    reason = "the once-owner adds a test entrypoint only for resource variants"
)]
mod sdk_validation_allocation {
    use super::alloc_track;
    use auctioneer_bidder::{
        BidderConfigInput, compiled_bidders, fake_synthetic::factory_calls,
        settings_observations::take,
    };
    use auctioneer_core::config::SdkAuctioneerAppConfig;
    use auctioneer_runtime::{
        budget::{
            AUX_MANIFEST_BYTES, derive_aux_work_for_profile,
            derive_isolate_budget_with_baked_config, min_aux_route_peak_bytes,
        },
        serialized_json_bytes,
    };
    use edgezero_core::{blob_envelope::BlobEnvelope, config_store::ConfigExtractionLimits};
    use serde_json::{Value, json};
    use std::hint::black_box;
    use validator::Validate as _;

    fn nodes(value: &Value) -> u32 {
        match value {
            Value::Object(fields) => fields
                .values()
                .map(nodes)
                .sum::<u32>()
                .checked_add(1)
                .unwrap(),
            Value::Array(values) => values
                .iter()
                .map(nodes)
                .sum::<u32>()
                .checked_add(1)
                .unwrap(),
            _ => 1,
        }
    }

    fn full_config_charge(caps: &ConfigExtractionLimits, product_bytes: u64) -> u64 {
        let mut profile = auctioneer_core::test_support::boot_parity_fixture::profile_for_adapter(
            "unequal_explicit_caps",
            "axum",
        );
        profile.limits.max_runtime_config_bytes = caps.max_backend_bytes;
        profile.limits.max_runtime_config_blob_bytes = caps.max_blob_bytes;
        profile.limits.max_config_json_nodes = caps.max_json_nodes;
        let isolate = derive_isolate_budget_with_baked_config(
            &profile.limits,
            &profile.target,
            profile.outbound_meta_bytes,
            profile.construction_costs,
            product_bytes,
            profile.catalog_costs,
        )
        .unwrap();
        let aux =
            derive_aux_work_for_profile(&profile.limits, &profile.target, product_bytes).unwrap();
        let mut without_config_nodes = profile.limits;
        without_config_nodes.max_config_json_nodes = 0;
        let aux_without_config_nodes =
            derive_aux_work_for_profile(&without_config_nodes, &profile.target, product_bytes)
                .unwrap();
        // Aux nodes also include independent body/cookie work; select only the
        // config-node delta, without charging unrelated aux terms to validation.
        let config_nodes = aux
            .charges()
            .nodes
            .checked_sub(aux_without_config_nodes.charges().nodes)
            .unwrap();
        assert_eq!(
            aux.charges().config.checked_add(config_nodes).unwrap(),
            isolate.runtime_config_term,
        );
        isolate.runtime_config_term
    }

    pub(super) fn check() {
        // Resident metadata/defaults have separate funding. No transient plan
        // or SDK validation is prepared before either measured call.
        let definitions = auctioneer_config_data::bidders().len();
        assert!(
            auctioneer_config_data::bidders()
                .iter()
                .all(|info| info.name.len() == 64)
        );
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        let product_bytes = serialized_json_bytes(&(host, account)).unwrap();
        assert_eq!(
            product_bytes, 1_447,
            "product-default allowance is independent of catalog size"
        );
        assert!(matches!(definitions, 1 | 256 | 512));
        assert_eq!(
            auctioneer_config_data::bidder_implementation_bindings().len(),
            definitions
        );
        assert_eq!(
            compiled_bidders().len(),
            2,
            "catalog growth must not add factories"
        );
        let factories_before = factory_calls();

        // Prove the observer is live without constructing a plan or bidder.
        let _ = take();
        let callback = compiled_bidders()
            .iter()
            .find(|row| row.key == "fake_integer_probe")
            .unwrap()
            .config_validator
            .unwrap();
        callback(BidderConfigInput {
            requested: None,
            base: None,
            definition: None,
            enabled: false,
        })
        .unwrap();
        let control = take();
        assert_eq!(control.len(), 1);
        assert_eq!(control[0].owner, "fake_integer_probe");
        drop(control);

        let mut failures = Vec::new();
        for (
            case,
            data,
            payload_bytes,
            raw_cap,
            node_cap,
            expected_component,
            expected_charge,
            valid,
        ) in [
            (
                "minimal",
                json!({"schema_version": 1}),
                20_usize,
                155_u64,
                6_u32,
                43_573_u64,
                228_789_u64,
                true,
            ),
            (
                "invalid-tmax",
                json!({"schema_version": 1, "host": {"resource_limits": {"default_tmax_ms": 0}}}),
                69,
                204,
                9,
                58_035,
                243_251,
                false,
            ),
        ] {
            let wire = serde_json::to_string(&data).unwrap();
            assert_eq!(wire.len(), payload_bytes);
            let envelope =
                serde_json::to_value(BlobEnvelope::new(data, "2026-10-06T00:00:00Z".into()))
                    .unwrap();
            assert_eq!(
                u64::try_from(serde_json::to_vec(&envelope).unwrap().len()).unwrap(),
                raw_cap
            );
            assert_eq!(nodes(&envelope), node_cap);
            let caps = ConfigExtractionLimits {
                max_backend_bytes: raw_cap,
                max_blob_bytes: raw_cap,
                max_total_bytes: raw_cap,
                max_secret_bytes: 1,
                max_json_nodes: node_cap,
                timeout: auctioneer_core::config::runtime::CONFIG_FETCH_TIMEOUT,
                ..ConfigExtractionLimits::default()
            };
            caps.validate().unwrap();
            // Earlier 43,573/58,035-byte comparisons covered this component
            // only, omitting existing product-default clone funding. They did
            // not establish underfunding of the complete production term.
            let component_charge = min_aux_route_peak_bytes(
                caps.max_backend_bytes,
                caps.max_blob_bytes,
                u64::from(caps.max_json_nodes),
            )
            .checked_sub(AUX_MANIFEST_BYTES)
            .unwrap();
            assert_eq!(component_charge, expected_component);
            let charge = full_config_charge(&caps, product_bytes);
            assert_eq!(
                charge, expected_charge,
                "the complete existing config charge must not grow with the catalog"
            );
            assert_eq!(charge.checked_sub(component_charge).unwrap(), 185_216);
            assert!(take().is_empty());

            let allocated_before = alloc_track::allocated();
            let baseline = alloc_track::reset_peak();
            let sdk: SdkAuctioneerAppConfig = serde_json::from_str(black_box(&wire)).unwrap();
            let result = black_box(&sdk).validate();
            let peak = alloc_track::peak_since(baseline);
            let retained = alloc_track::live().checked_sub(baseline).unwrap();
            let cumulative = alloc_track::allocated()
                .checked_sub(allocated_before)
                .unwrap();
            let observed = take();

            eprintln!(
                "sdk-validation-allocation: definitions={definitions} name_bytes=64 case={case} payload={payload_bytes} backend_cap={} blob_cap={} nodes={} product_bytes={product_bytes} component_charge={component_charge} peak={peak} retained={retained} cumulative={cumulative} charge={charge} callbacks={}",
                caps.max_backend_bytes,
                caps.max_blob_bytes,
                caps.max_json_nodes,
                observed.len()
            );
            assert!(
                peak > 0,
                "the actual SDK validation must remain inside the meter"
            );
            assert!(
                observed.is_empty(),
                "absent disabled settings, including invalid policy, must not invoke an owner"
            );
            if valid {
                result.unwrap();
            } else {
                assert_eq!(
                    serde_json::to_value(result.unwrap_err()).unwrap(),
                    json!({"runtime": [{"code": "invalid_effective_config", "message": null, "params": {}}]})
                );
            }
            assert_eq!(factory_calls(), factories_before);
            // Compare the actual measured peak, not the outer report-parser's
            // synthetic records. Check every population; catalog growth changes
            // this scratch allocation even though the config charge stays fixed.
            if peak > charge {
                failures.push(format!("definitions={definitions} case={case}: peak={peak} exceeds complete unchanged config charge={charge} (product_bytes={product_bytes}, component_charge={component_charge})"));
            }
        }
        assert!(
            failures.is_empty(),
            "underfunded SDK validation: {failures:?}"
        );
    }
}

mod request_binding {
    use auctioneer_bidder::{BidderName, BidderRequestInfo, EntryPoint};
    use auctioneer_bidder::{BidderSettings, bidder_settings::fake_integer_probe};
    use serde_json::json;
    use std::ptr;

    fn settings_info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("fake_alias"),
            BidderName::new("fake_base"),
            "account",
            None,
            "https://bidder.example/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn settings_binding_refuses_each_wrong_or_missing_implementation_before_projection() {
        let mut cases = 0;
        let integer: BidderSettings =
            serde_json::from_value(json!({"implementation":"fake_integer_probe","values":{}}))
                .unwrap();
        let string: BidderSettings =
            serde_json::from_value(json!({"implementation":"fake_string_probe","values":{}}))
                .unwrap();
        for (expected, requested, base) in [
            (Some("fake_integer_probe"), Some(&string), None),
            (Some("fake_integer_probe"), None, Some(&string)),
            (Some("fake_integer_probe"), Some(&integer), Some(&string)),
            (Some("fake_integer_probe"), Some(&string), Some(&integer)),
            (Some("fake_alias"), Some(&integer), None),
            (None, Some(&integer), None),
            (None, None, Some(&integer)),
        ] {
            let mut info = settings_info();
            let error = info.set_settings(expected, requested, base).unwrap_err();
            assert_eq!(
                error,
                auctioneer_bidder::BidderError::failed_to_request_bids(
                    "bidder settings implementation mismatch"
                )
            );
            assert!(info.settings().is_none());
            assert!(info.base_settings().is_none());
            cases += 1;
        }
        assert_eq!(cases, 7);
    }

    #[test]
    fn settings_binding_borrows_both_entries_and_preserves_fieldwise_zero_precedence() {
        let base: BidderSettings = serde_json::from_value(
            json!({"implementation":"fake_integer_probe","values":{"primary_number":41_i64,"secondary_number":42_i64}}),
        )
        .unwrap();
        let alias: BidderSettings = serde_json::from_value(
            json!({"implementation":"fake_integer_probe","values":{"primary_number":0_i64}}),
        )
        .unwrap();
        let mut info = settings_info();
        info.set_settings(Some("fake_integer_probe"), Some(&alias), Some(&base))
            .unwrap();
        assert!(ptr::eq(info.settings().unwrap(), &raw const alias));
        assert!(ptr::eq(info.base_settings().unwrap(), &raw const base));
        let resolved = fake_integer_probe::resolve(
            info.settings()
                .and_then(BidderSettings::as_fake_integer_probe),
            info.base_settings()
                .and_then(BidderSettings::as_fake_integer_probe),
        );
        assert_eq!(resolved.primary_number(), Some(0));
        assert_eq!(resolved.secondary_number(), Some(42));
        info.set_settings(None, None, None).unwrap();
        assert!(info.settings().is_none());
        assert!(info.base_settings().is_none());
    }

    #[test]
    fn settings_binding_enforces_limits_for_programmatic_callers_and_clears_previous_binding() {
        let mut cases = 0;
        let small: BidderSettings = serde_json::from_value(
            json!({"implementation":"fake_string_probe","values":{"token":"host"}}),
        )
        .unwrap();
        let large: BidderSettings = serde_json::from_value(json!({"implementation":"fake_string_probe","values":{"token":"private-marker".repeat(2048)}})).unwrap();
        for (requested, base) in [
            (Some(&large), None),
            (None, Some(&large)),
            (Some(&small), Some(&large)),
        ] {
            let mut info = settings_info();
            info.set_settings(Some("fake_string_probe"), Some(&small), None)
                .unwrap();
            let error = info
                .set_settings(Some("fake_string_probe"), requested, base)
                .unwrap_err();
            assert_eq!(
                error,
                auctioneer_bidder::BidderError::failed_to_request_bids(
                    "bidder settings exceed limits"
                )
            );
            assert!(info.settings().is_none());
            assert!(info.base_settings().is_none());
            cases += 1;
        }
        assert_eq!(cases, 3);
    }
}

mod retention {
    use super::alloc_track;
    use auctioneer_bidder::{BidderName, request_info::BidderSettingsSource as _};
    use auctioneer_core::config::{AuctioneerAppConfig, bidder_settings::RuntimeBidderSettings};
    use auctioneer_exchange::alias::AliasMap;
    use serde_json::json;
    use std::hint::black_box;

    fn retention_policy(
        count: usize,
        width: usize,
        unrelated: usize,
        accounts: usize,
    ) -> AuctioneerAppConfig {
        let mut bidders: serde_json::Map<_, _> = (0..count)
            .map(|index| {
                (format!("fake_bidder_{index}"), json!({
                    "settings": {"implementation": "fake_string_probe", "values": {"token": "x".repeat(width)}}
                }))
            })
            .collect();
        bidders.insert("fake_base".to_owned(), json!({
            "settings": {"implementation": "fake_string_probe", "values": {"token": "b".repeat(width)}}
        }));
        for index in 0..unrelated {
            bidders.insert(format!("fake_unrelated_{index}"), json!({
                "settings": {"implementation": "fake_string_probe", "values": {"token": "u".repeat(4_096)}}
            }));
        }
        let account_rows: serde_json::Map<_, _> = (0..accounts)
            .map(|index| {
                (
                    format!("fake_account_{index}"),
                    json!({
                        "targeting_prefix": "unused"
                    }),
                )
            })
            .collect();
        serde_json::from_value(json!({
            "schema_version": 1, "bidders": bidders, "accounts": account_rows
        }))
        .unwrap()
    }

    fn assert_retention_populations(
        policy: &AuctioneerAppConfig,
        count: usize,
        unrelated: usize,
        accounts: usize,
    ) {
        assert_eq!(policy.bidders.len(), count + 1 + unrelated);
        assert_eq!(policy.accounts.len(), accounts);
        for index in 0..unrelated {
            assert!(
                policy.bidders[&format!("fake_unrelated_{index}")]
                    .settings
                    .is_some()
            );
        }
    }

    fn assert_projected_copies_and_exclusions(
        policy: &AuctioneerAppConfig,
        projected: &RuntimeBidderSettings,
        requested: &[BidderName],
        width: usize,
        unrelated: usize,
    ) {
        for name in requested {
            let original = policy.bidders[name.as_str()]
                .settings
                .as_ref()
                .unwrap()
                .as_fake_string_probe()
                .unwrap()
                .token
                .as_ref()
                .unwrap();
            let supply = projected
                .settings(name.as_str())
                .unwrap()
                .as_fake_string_probe()
                .unwrap()
                .token
                .as_ref()
                .unwrap();
            assert_eq!(supply, original);
            assert_eq!(supply.len(), width);
            assert_ne!(supply.as_ptr(), original.as_ptr());
        }
        for index in 0..unrelated {
            assert!(
                projected
                    .settings(&format!("fake_unrelated_{index}"))
                    .is_none()
            );
        }
    }

    #[test]
    fn projecting_settings_copies_selected_strings_once_independent_of_unrelated_populations() {
        let before = alloc_track::reset_peak();
        let allocated = alloc_track::allocated();
        let control = black_box(Vec::<u8>::with_capacity(4_096));
        assert_eq!(alloc_track::peak_since(before), 4_096);
        assert_eq!(alloc_track::live() - before, 4_096);
        assert_eq!(alloc_track::allocated() - allocated, 4_096);
        drop(control);
        assert_eq!(alloc_track::live(), before);

        let mut cases = 0;
        for count in [1_usize, 8] {
            let mut requested: Vec<_> = (0..count)
                .map(|index| BidderName::new(format!("fake_bidder_{index}")))
                .collect();
            let mut aliases = AliasMap::default();
            for name in &requested {
                aliases
                    .bases
                    .insert(name.clone(), BidderName::new("fake_base"));
            }
            requested.push(BidderName::new("fake_base"));
            requested.push(BidderName::new("fake_bidder_0"));
            let mut previous_width_cost: Option<(usize, (u64, u64, u64))> = None;
            for width in [1, 64, 4_096] {
                let mut width_cost = None;
                for unrelated in [0_usize, 8] {
                    for accounts in [0_usize, 8] {
                        let policy = retention_policy(count, width, unrelated, accounts);
                        let before_policy = serde_json::to_value(&policy).unwrap();
                        assert_retention_populations(&policy, count, unrelated, accounts);

                        let before = alloc_track::reset_peak();
                        let allocated = alloc_track::allocated();
                        let projected =
                            RuntimeBidderSettings::project(&policy, &requested, &aliases);
                        let cost = (
                            alloc_track::peak_since(before),
                            u64::try_from(alloc_track::live() - before).unwrap(),
                            alloc_track::allocated() - allocated,
                        );
                        let selected_bytes = u64::try_from((count + 1) * width).unwrap();
                        assert!(cost.0 >= selected_bytes);
                        assert!(cost.1 >= selected_bytes);
                        assert!(cost.2 >= selected_bytes);
                        if let Some(expected) = width_cost {
                            assert_eq!(
                                cost, expected,
                                "unrelated bidders={unrelated}, accounts={accounts}"
                            );
                        } else {
                            width_cost = Some(cost);
                        }
                        assert_projected_copies_and_exclusions(
                            &policy, &projected, &requested, width, unrelated,
                        );
                        drop(projected);
                        assert_eq!(alloc_track::live(), before);
                        assert_eq!(serde_json::to_value(&policy).unwrap(), before_policy);
                        cases += 1;
                    }
                }
                let cost = width_cost.unwrap();
                if let Some((previous_width, previous_cost)) = previous_width_cost {
                    // Map/key storage is unchanged; only the selected string bytes grow.
                    let copied_bytes =
                        u64::try_from((count + 1) * (width - previous_width)).unwrap();
                    assert_eq!(cost.0 - previous_cost.0, copied_bytes);
                    assert_eq!(cost.1 - previous_cost.1, copied_bytes);
                    assert_eq!(cost.2 - previous_cost.2, copied_bytes);
                }
                previous_width_cost = Some((width, cost));
            }
        }
        assert_eq!(cases, 24);
    }
}

mod retained_owner {
    #[cfg(test)]
    mod tests {
        use auctioneer_bidder::request_info::BidderSettingsSource as _;
        use auctioneer_bidder::{BidderSettings, bidder_settings::fake_string_probe};
        use auctioneer_core::config::{
            AuctioneerAppConfig, bidder_settings::RuntimeBidderSettings,
        };
        use serde_json::json;

        fn projection_policy() -> AuctioneerAppConfig {
            serde_json::from_value(json!({
                "schema_version": 1,
                "host": {"aliases": {"fake_alias": "fake_base"}},
                "bidders": {
                    "fake_alias": {
                        "enabled": false,
                        "timeout_ms": 20,
                        "settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 0}}
                    },
                    "fake_base": {"settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 42, "secondary_number": 7}}},
                    "fake_second_alias": {"settings": {"implementation": "fake_integer_probe", "values": {"secondary_number": 9}}},
                    "fake_unused_alias": {"settings": {"implementation": "fake_string_probe", "values": {"token": "unused-alias"}}},
                    "fake_unused_base": {"settings": {"implementation": "fake_string_probe", "values": {"token": "unused-base"}}},
                    "fake_unrelated": {"settings": {"implementation": "fake_string_probe", "values": {"token": "unrelated"}}},
                    "fake_no_settings": {"enabled": false}
                },
                "accounts": {"fake_discarded": {"targeting_prefix": "discarded"}}
            }))
            .expect("actual generated settings")
        }

        #[test]
        fn projecting_settings_copies_only_requested_and_resolved_base_values() {
            use auctioneer_bidder::{BidderName, bidder_settings::fake_integer_probe};
            use auctioneer_exchange::alias::AliasMap;

            let policy = projection_policy();
            let mut aliases = AliasMap::default();
            for (alias, base) in [
                ("fake_alias", "fake_base"),
                ("fake_second_alias", "fake_base"),
                ("fake_missing_alias", "fake_base"),
                ("fake_unused_alias", "fake_unused_base"),
            ] {
                aliases
                    .bases
                    .insert(BidderName::new(alias), BidderName::new(base));
            }
            let before_policy = serde_json::to_value(&policy).unwrap();
            for name in ["fake_unused_alias", "fake_unused_base", "fake_unrelated"] {
                assert!(policy.bidders[name].settings.is_some());
            }
            let names = [
                "fake_alias",
                "fake_base",
                "fake_second_alias",
                "fake_unused_alias",
                "fake_unused_base",
                "fake_unrelated",
                "fake_no_settings",
                "fake_missing",
                "fake_missing_alias",
            ];
            let cases: &[(&[&str], &[&str])] = &[
                (&[], &[]),
                (&["fake_base"], &["fake_base"]),
                (&["fake_alias"], &["fake_alias", "fake_base"]),
                (&["fake_missing_alias"], &["fake_base"]),
                (&["fake_no_settings", "fake_missing"], &[]),
                (&["fake_unrelated"], &["fake_unrelated"]),
                (
                    &["fake_alias", "fake_second_alias", "fake_base", "fake_alias"],
                    &["fake_alias", "fake_base", "fake_second_alias"],
                ),
            ];
            let mut cases_run = 0;
            for (requested, expected) in cases {
                let requested: Vec<_> = requested
                    .iter()
                    .map(|name| BidderName::new(*name))
                    .collect();
                let projected = RuntimeBidderSettings::project(&policy, &requested, &aliases);
                let retained: Vec<_> = names
                    .into_iter()
                    .filter(|name| projected.settings(name).is_some())
                    .collect();
                assert_eq!(retained, *expected);
                for name in *expected {
                    assert_eq!(
                        serde_json::to_value(projected.settings(name).unwrap()).unwrap(),
                        serde_json::to_value(policy.bidders[*name].settings.as_ref().unwrap())
                            .unwrap()
                    );
                }
                cases_run += 1;
            }
            assert_eq!(cases_run, 7);
            assert_eq!(serde_json::to_value(&policy).unwrap(), before_policy);

            let no_aliases = RuntimeBidderSettings::project(
                &policy,
                &[BidderName::new("fake_alias")],
                &AliasMap::default(),
            );
            assert!(no_aliases.settings("fake_alias").is_some());
            assert!(no_aliases.settings("fake_base").is_none());

            let projected = RuntimeBidderSettings::project(
                &policy,
                &[
                    BidderName::new("fake_alias"),
                    BidderName::new("fake_second_alias"),
                    BidderName::new("fake_unrelated"),
                ],
                &aliases,
            );
            drop(policy);
            for (name, expected_primary, expected_secondary) in
                [("fake_alias", 0, 7), ("fake_second_alias", 42, 9)]
            {
                let resolved = fake_integer_probe::resolve(
                    projected
                        .settings(name)
                        .and_then(BidderSettings::as_fake_integer_probe),
                    projected
                        .settings("fake_base")
                        .and_then(BidderSettings::as_fake_integer_probe),
                );
                assert_eq!(resolved.primary_number(), Some(expected_primary));
                assert_eq!(resolved.secondary_number(), Some(expected_secondary));
            }
            assert_eq!(
                projected
                    .settings("fake_unrelated")
                    .and_then(BidderSettings::as_fake_string_probe)
                    .and_then(|settings| settings.token.as_deref()),
                Some("unrelated")
            );
        }

        fn string_projection_policy(width: usize) -> AuctioneerAppConfig {
            serde_json::from_value(json!({
                "schema_version": 1,
                "bidders": {
                    "fake_alias": {"settings": {"implementation": "fake_string_probe", "values": {"token": "a".repeat(width)}}},
                    "fake_fallback": {"settings": {"implementation": "fake_string_probe", "values": {}}},
                    "fake_base": {"settings": {"implementation": "fake_string_probe", "values": {"token": "b".repeat(width)}}},
                    "fake_unrelated": {"settings": {"implementation": "fake_string_probe", "values": {"token": "unrelated-host"}}},
                    "fake_no_settings": {"enabled": false}
                },
                "accounts": {"fake_discarded": {"targeting_prefix": "unused"}}
            }))
            .unwrap()
        }

        fn assert_owned_projection_strings(
            policy: &AuctioneerAppConfig,
            projected: &RuntimeBidderSettings,
        ) {
            for name in ["fake_alias", "fake_base"] {
                let original = policy.bidders[name]
                    .settings
                    .as_ref()
                    .unwrap()
                    .as_fake_string_probe()
                    .unwrap()
                    .token
                    .as_deref()
                    .unwrap();
                let owned = projected
                    .settings(name)
                    .unwrap()
                    .as_fake_string_probe()
                    .unwrap()
                    .token
                    .as_deref()
                    .unwrap();
                assert_eq!(owned, original);
                assert_ne!(owned.as_ptr(), original.as_ptr());
            }
            assert!(projected.settings("fake_unrelated").is_none());
            assert!(projected.settings("fake_no_settings").is_none());
            assert!(projected.settings("fake_missing").is_none());
        }

        #[test]
        fn projected_settings_alias_and_base_lookups_borrow_owned_strings_without_allocating() {
            use super::super::alloc_track;
            use auctioneer_bidder::BidderName;
            use auctioneer_exchange::alias::AliasMap;
            use std::hint::black_box;

            let mut cases = 0;
            for width in [1_usize, 64, 4_096] {
                for lookups in [1, 8] {
                    let policy = string_projection_policy(width);
                    let before_policy = serde_json::to_value(&policy).unwrap();
                    assert!(policy.bidders["fake_unrelated"].settings.is_some());
                    assert!(policy.accounts.contains_key("fake_discarded"));
                    let mut aliases = AliasMap::default();
                    for name in ["fake_alias", "fake_fallback"] {
                        aliases
                            .bases
                            .insert(BidderName::new(name), BidderName::new("fake_base"));
                    }
                    let projected = RuntimeBidderSettings::project(
                        &policy,
                        &[
                            BidderName::new("fake_alias"),
                            BidderName::new("fake_fallback"),
                            BidderName::new("fake_base"),
                            BidderName::new("fake_alias"),
                        ],
                        &aliases,
                    );
                    assert_owned_projection_strings(&policy, &projected);
                    assert_eq!(serde_json::to_value(&policy).unwrap(), before_policy);
                    drop(policy);

                    let alias = projected
                        .settings("fake_alias")
                        .unwrap()
                        .as_fake_string_probe()
                        .unwrap()
                        .token
                        .as_deref()
                        .unwrap();
                    let base = projected
                        .settings("fake_base")
                        .unwrap()
                        .as_fake_string_probe()
                        .unwrap()
                        .token
                        .as_deref()
                        .unwrap();
                    let before = alloc_track::reset_peak();
                    let allocated = alloc_track::allocated();
                    let control = black_box(alias.to_owned());
                    assert_eq!(
                        alloc_track::peak_since(before),
                        u64::try_from(width).unwrap()
                    );
                    assert_eq!(alloc_track::live() - before, i64::try_from(width).unwrap());
                    assert_eq!(
                        alloc_track::allocated() - allocated,
                        u64::try_from(width).unwrap()
                    );
                    drop(control);
                    assert_eq!(alloc_track::live(), before);

                    let before = alloc_track::reset_peak();
                    let allocated = alloc_track::allocated();
                    for _ in 0..lookups {
                        for (name, expected) in [
                            ("fake_alias", alias),
                            ("fake_fallback", base),
                            ("fake_base", base),
                        ] {
                            let view = fake_string_probe::resolve(
                                projected
                                    .settings(name)
                                    .and_then(BidderSettings::as_fake_string_probe),
                                projected
                                    .settings("fake_base")
                                    .and_then(BidderSettings::as_fake_string_probe),
                            );
                            let token = black_box(view.token().unwrap());
                            assert_eq!(token, expected);
                            assert_eq!(token.len(), width);
                            assert_eq!(token.as_ptr(), expected.as_ptr());
                        }
                    }
                    assert_eq!(alloc_track::allocated() - allocated, 0);
                    assert_eq!(alloc_track::peak_since(before), 0);
                    assert_eq!(alloc_track::live(), before);
                    cases += 1;
                }
            }
            assert_eq!(cases, 6);
        }
    }
}

mod config_limits {
    use auctioneer_bidder::BidderSettings;
    use serde_json::{Value, json};

    fn decode(value: Value) -> Result<BidderSettings, serde_json::Error> {
        serde_json::from_value(value)
    }

    #[test]
    fn aggregate_validation_delegates_structural_limits_with_fixed_redacted_errors() {
        use validator::Validate as _;
        let policy = auctioneer_config::policy::OperatorPolicy::<BidderSettings>::default();
        policy.validate().unwrap();
        let valid = decode(
            json!({"implementation":"fake_string_probe","values":{"token":"private-marker"}}),
        )
        .unwrap();
        valid.validate().unwrap();
        let invalid = decode(json!({"implementation":"fake_string_probe","values":{"token":"private-marker".repeat(2048)}})).unwrap();
        let errors = invalid.validate().unwrap_err();
        let fields = errors.field_errors();
        assert_eq!(fields.len(), 1);
        let errors = fields["values"];
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "bidder settings exceed byte limit");
        assert!(errors[0].params.is_empty());
        assert!(errors[0].message.is_none());
    }

    #[test]
    fn typed_settings_byte_ceiling_includes_the_envelope_and_json_escaping() {
        use auctioneer_bidder::bidder_settings::ensure_limits;
        let empty =
            decode(json!({"implementation":"fake_string_probe","values":{"token":""}})).unwrap();
        let overhead = auctioneer_runtime::serialized_json_bytes(&empty).unwrap();
        let width = usize::try_from(0x4000_u64 - overhead).unwrap();
        let mut cases = 0;
        for (extra, expected) in [(0, Ok(())), (1, Err("bidder settings exceed byte limit"))] {
            let settings = decode(json!({"implementation":"fake_string_probe","values":{"token":"x".repeat(width + extra)}})).unwrap();
            assert_eq!(ensure_limits(&settings), expected);
            cases += 1;
        }
        assert_eq!(cases, 2);
        let escaped = decode(
            json!({"implementation":"fake_string_probe","values":{"token":"\n".repeat(width)}}),
        )
        .unwrap();
        assert_eq!(
            ensure_limits(&escaped),
            Err("bidder settings exceed byte limit")
        );
    }
}

mod runtime {
    use super::{bounded_store::bounded_over_get, config_funding};
    use async_trait::async_trait;
    use auctioneer_core::{
        config::runtime::{RuntimeConfigError, resolve_runtime_config},
        handlers::TestFanoutHandle,
        host_primitives::{TestClockHandle, TestRngHandle},
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, TimeoutCause,
    };
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::{
        blob_envelope::BlobEnvelope,
        body::Body,
        config_store::{
            BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
        },
        context::RequestContext,
        http::StatusCode,
        store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry},
        time::{Deadline, MonotonicClock},
    };
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::{
        sync::{Arc, Mutex, PoisonError},
        time::Duration,
    };
    use time::OffsetDateTime;

    const STORE_ID: &str = "auctioneer_runtime";
    const WIDE_CAP: usize = 1 << 20;

    struct SettingsStore(serde_json::Value);

    #[async_trait(?Send)]
    impl ConfigStore for SettingsStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            Ok(Some(
                serde_json::to_string(&BlobEnvelope::new(
                    self.0.clone(),
                    "1970-01-01T00:00:00Z".into(),
                ))
                .unwrap()
                .into(),
            ))
        }
        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    #[test]
    fn malformed_generated_settings_fail_closed_without_payload_text_in_diagnostics() {
        use serde_json::json;
        const CANARY: &str = "private-settings-canary";
        let mut cases = 0;
        for settings in [
            json!({"implementation":CANARY,"values":{}}),
            json!({"implementation":"fake_integer_probe","values":{"primary_number":CANARY}}),
            json!({"implementation":"fake_integer_probe","values":{(CANARY):true}}),
            json!({"implementation":"fake_string_probe","values":{"token":CANARY.repeat(2048)}}),
        ] {
            let fanout = Arc::new(RecordingFanout::default());
            let ctx = context_with(
                Arc::new(SettingsStore(
                    json!({"schema_version": 1, "bidders":{"fake_bidder":{"settings":settings}}}),
                )),
                Arc::clone(&fanout),
            );
            let error = block_on(resolve_runtime_config(&ctx)).unwrap_err();
            assert!(matches!(error, RuntimeConfigError::MalformedHost { .. }));
            assert!(!format!("{error:?}").contains(CANARY));
            let response = error.into_response().unwrap();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let bytes = block_on(response.into_body().into_bytes_bounded(WIDE_CAP)).unwrap();
            assert_eq!(bytes.as_ref(), b"Malformed host configuration");
            assert_eq!(*fanout.calls.lock().unwrap(), 0);
            cases += 1;
        }
        assert_eq!(cases, 4);
    }

    fn registry_of(store: Arc<dyn ConfigStore>) -> ConfigRegistry {
        StoreRegistry::single_id(
            STORE_ID.to_owned(),
            ConfigStoreBinding {
                default_key: STORE_ID.to_owned(),
                handle: ConfigStoreHandle::new(store),
            },
        )
    }

    #[derive(Default)]
    struct RecordingFanout {
        calls: Mutex<usize>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut seen = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
            *seen = seen.saturating_add(calls.len());
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    fn context_with(store: Arc<dyn ConfigStore>, fanout: Arc<RecordingFanout>) -> RequestContext {
        config_funding::install();
        let handler_fanout: Arc<dyn BidderFanout + Send + Sync> = fanout;
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(
                serde_json::to_vec(&serde_json::json!({"id":"fake_settings", "imp":[]})).unwrap(),
            ))
            .expect("request builds");
        request.extensions_mut().insert(registry_of(store));
        request
            .extensions_mut()
            .insert(TestFanoutHandle(handler_fanout));
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        auctioneer_core::test_support::config_store::context(request)
    }
}

mod generated_endpoints {
    use auctioneer_bidder::bidder_endpoint::fake_string_probe::Values;
    use auctioneer_bidder::{
        EndpointBuilder, META_BIDDER_REGION_BYTES, META_BYTES_CAPACITY, OutboundRequestMetaBuilder,
        SinkAbort,
    };

    fn typed_info<'a>(
        endpoint: &'a str,
        macros: &'a [&'a str],
    ) -> auctioneer_bidder::BidderRequestInfo<'a> {
        auctioneer_bidder::BidderRequestInfo::new(
            auctioneer_bidder::BidderName::new("fake_requested_alias"),
            auctioneer_bidder::BidderName::new("fake_base"),
            "account",
            None,
            endpoint,
            macros,
            None,
            None,
            auctioneer_bidder::EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn generated_bindings_encode_values_through_the_existing_capped_builder() {
        let request_info = typed_info("https://bidder.example.test/{{.ZuluRoute}}", &["ZuluRoute"]);
        let values = Values {
            a_route: "a b/&?=#+%",
            z_token: "unused",
        };
        let pairs = values.as_pairs();
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
        meta.set_method("POST").unwrap();
        EndpointBuilder::from_request(&request_info, &pairs)
            .expand_into(&mut meta)
            .unwrap();
        assert_eq!(
            meta.finish().url(),
            "https://bidder.example.test/a%20b%2F%26%3F%3D%23%2B%25"
        );
    }

    #[test]
    fn generated_bindings_cannot_relax_template_membership_or_traversal_refusals() {
        let mut cases = 0;
        for (macros, template, value) in [
            (
                &[][..],
                "https://bidder.example.test/{{.ZuluRoute}}",
                "safe",
            ),
            (
                &["ZuluRoute"][..],
                "https://bidder.example.test/{{.Other}}",
                "safe",
            ),
            (
                &["ZuluRoute"][..],
                "https://bidder.example.test/{{.ZuluRoute}}",
                "../private",
            ),
            (
                &["ZuluRoute"][..],
                "https://bidder.example.test/{{.ZuluRoute}}",
                "%252e%252e%252fprivate",
            ),
        ] {
            let request_info = typed_info(template, macros);
            let pairs = Values {
                a_route: value,
                z_token: "unused",
            }
            .as_pairs();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            assert_eq!(
                EndpointBuilder::from_request(&request_info, &pairs).expand_into(&mut meta),
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap()
                })
            );
            assert_eq!(meta.written(), 4);
            meta.set_url("https://recovery.example.test/").unwrap();
            assert_eq!(meta.finish().url(), "https://recovery.example.test/");
            cases += 1;
        }
        assert_eq!(cases, 4);
    }

    #[test]
    fn query_set_validates_borrowed_macros_even_in_values_it_would_remove() {
        let request_info = typed_info(
            "https://bidder.example.test/path?member_id={{.ZuluRoute}}",
            &["ZuluRoute"],
        );
        let mut cases = 0;
        for value in ["../private", "%252e%252e%252fprivate"] {
            let pairs = Values {
                a_route: value,
                z_token: "unused",
            }
            .as_pairs();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            assert_eq!(
                EndpointBuilder::from_request(&request_info, &pairs).expand_with_query_set_into(
                    &mut meta,
                    "member_id",
                    "42"
                ),
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                })
            );
            assert_eq!(meta.written(), 4);
            cases += 1;
        }
        assert_eq!(cases, 2);
    }
}

mod compiled_sdk {
    use auctioneer_bidder::{
        BidderConfigError, ValidatedRegistryPlan,
        settings_observations::{Observation, take},
    };
    use auctioneer_core::config::{
        AuctioneerAppConfig, EffectiveConfigError, SdkAuctioneerAppConfig,
        validation::{
            BidderConfigurationError, PolicyValidationError, validate_compiled_operator_policy,
            validate_effective_operator_policy,
        },
    };
    use serde_json::{Value, json};
    use validator::Validate as _;

    fn document() -> Value {
        json!({"schema_version": 1,
            "host": {"aliases": {"fake_host_alias": "fake_wire_leaf"}},
            "bidders": {
                "fake_wire_base": {"enabled": true, "settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 7, "secondary_number": 11}}},
                "fake_wire_parent": {"settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 3, "secondary_number": 5}}},
                "fake_wire_leaf": {"settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 0, "secondary_number": 23}}},
                "fake_host_alias": {"settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 0}}},
                "fake_wire_text": {"settings": {"implementation": "fake_string_probe", "values": {"token": "fake_safe"}}},
                "fake_future": {"enabled": true, "settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 17, "secondary_number": 19}}}
            }
        })
    }

    fn integer(
        definition: Option<&str>,
        enabled: bool,
        primary: Option<i64>,
        secondary: Option<i64>,
    ) -> Observation {
        Observation {
            owner: "fake_integer_probe",
            definition: definition.map(str::to_owned),
            enabled,
            primary,
            secondary,
        }
    }

    fn successful_dispatch() -> Vec<Observation> {
        vec![
            integer(None, false, Some(17), Some(19)),
            integer(Some("fake_wire_leaf"), true, Some(0), Some(23)),
            integer(Some("fake_wire_base"), true, Some(7), Some(11)),
            integer(Some("fake_wire_leaf"), true, Some(0), Some(23)),
            integer(Some("fake_wire_parent"), true, Some(3), Some(5)),
            Observation {
                owner: "fake_string_probe",
                definition: Some("fake_wire_text".into()),
                enabled: false,
                primary: None,
                secondary: None,
            },
        ]
    }

    fn refusal(field: &'static str, reason: &'static str) -> PolicyValidationError {
        PolicyValidationError::Bidder(BidderConfigurationError::Bidder(BidderConfigError {
            field,
            reason,
        }))
    }

    fn assert_parity(
        document: &Value,
        expected: Result<(), PolicyValidationError>,
        observed: &[Observation],
    ) {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let _ = take();
        let catalog = ValidatedRegistryPlan::compiled().unwrap();
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        let typed: AuctioneerAppConfig = serde_json::from_value(document.clone()).unwrap();
        assert_eq!(
            validate_effective_operator_policy(&typed, host, account, &catalog),
            expected
        );
        assert_eq!(take(), observed);
        let compiled = validate_compiled_operator_policy(&typed, host, account);
        match expected {
            Ok(()) => compiled.unwrap(),
            Err(expected) => match compiled.unwrap_err() {
                EffectiveConfigError::Malformed(error) => assert_eq!(error, expected.to_string()),
                error @ EffectiveConfigError::Registry(_) => {
                    panic!("valid synthetic compiled catalog refused: {error}")
                }
            },
        }
        assert_eq!(take(), observed);
        for sdk in [
            serde_json::from_value::<SdkAuctioneerAppConfig>(document.clone()).unwrap(),
            toml::from_str::<SdkAuctioneerAppConfig>(&toml::to_string(document).unwrap()).unwrap(),
        ] {
            match (expected, sdk.validate()) {
                (Ok(()), Ok(())) => {}
                (Err(_), Err(errors)) => assert_eq!(
                    serde_json::to_value(errors).unwrap(),
                    json!({"runtime": [{"code": "invalid_effective_config", "message": null, "params": {}}]})
                ),
                (expected, actual) => {
                    panic!("SDK/typed parity: expected {expected:?}, got {actual:?}")
                }
            }
            assert_eq!(
                take(),
                observed,
                "SDK must run the discovered callback union exactly once"
            );
        }
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn sdk_and_typed_paths_dispatch_discovered_callbacks_once_with_real_compiled_aliases() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let catalog = ValidatedRegistryPlan::compiled().unwrap();
        assert_eq!(catalog.definitions().len(), 4);
        for (name, parent) in [
            ("fake_wire_parent", "fake_wire_base"),
            ("fake_wire_leaf", "fake_wire_parent"),
        ] {
            let alias = catalog.definition(name).unwrap();
            assert_eq!(alias.source_parent(), Some(parent));
            assert_eq!(alias.canonical_base(), "fake_wire_base");
            assert_eq!(alias.implementation(), "fake_integer_probe");
            assert!(alias.config_validator().is_some());
        }
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
        let mut input = document();
        assert_parity(&input, Ok(()), &successful_dispatch());
        input["floor_currency"] = json!({"expires_at": 1, "usd_per_unit": {"QAA": 1.25}});
        assert_parity(&input, Ok(()), &successful_dispatch());
    }

    #[test]
    fn sdk_and_typed_paths_reject_bindings_before_any_discovered_callback() {
        let mut cases = 0;
        for name in [
            "fake_wire_base",
            "fake_wire_parent",
            "fake_wire_leaf",
            "fake_host_alias",
        ] {
            let mut input = document();
            input["bidders"]["fake_early"] = json!({"settings": {"implementation": "fake_integer_probe", "values": {"primary_number": -1}}});
            input["bidders"][name]["settings"] = json!({"implementation": "fake_string_probe", "values": {"token": "fake_private_credential_canary"}});
            assert_parity(
                &input,
                Err(PolicyValidationError::Bidder(
                    BidderConfigurationError::SettingsBinding,
                )),
                &[],
            );
            cases += 1;
        }
        assert_eq!(cases, 4);
    }

    #[test]
    fn sdk_and_typed_paths_check_disabled_and_shadowed_supplied_settings() {
        let mut cases = 0;
        for field in ["primary_number", "secondary_number"] {
            for bad_base in [false, true] {
                let mut input = document();
                input["bidders"]["fake_wire_base"]["enabled"] = json!(false);
                input["bidders"]["fake_host_alias"]["settings"]["values"]["secondary_number"] =
                    json!(29);
                let name = if bad_base {
                    "fake_wire_leaf"
                } else {
                    "fake_host_alias"
                };
                input["bidders"][name]["settings"]["values"][field] = json!(-1);
                let primary = if !bad_base && field == "primary_number" {
                    -1
                } else {
                    0
                };
                let secondary = if !bad_base && field == "secondary_number" {
                    -1
                } else {
                    29
                };
                assert_parity(
                    &input,
                    Err(refusal(field, "invalid")),
                    &[
                        integer(None, false, Some(17), Some(19)),
                        integer(
                            Some("fake_wire_leaf"),
                            false,
                            Some(primary),
                            Some(secondary),
                        ),
                    ],
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 4);
    }

    #[test]
    fn sdk_and_typed_paths_preserve_compiled_ancestry_and_per_identity_requiredness() {
        assert_parity(&json!({"schema_version": 1}), Ok(()), &[]);
        let input = json!({"schema_version": 1, "bidders": {
            "fake_wire_base": {"enabled": true, "settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 7, "secondary_number": 11}}},
            "fake_wire_leaf": {"settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 0}}}
        }});
        assert_parity(
            &input,
            Err(refusal("secondary_number", "required")),
            &[
                integer(Some("fake_wire_base"), true, Some(7), Some(11)),
                integer(Some("fake_wire_leaf"), true, Some(0), None),
            ],
        );
        let disabled_parent = json!({"schema_version": 1,
        "host": {"aliases": {"fake_host_alias": "fake_wire_leaf"}},
        "bidders": {
            "fake_wire_base": {"enabled": true, "settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 7, "secondary_number": 11}}},
            "fake_wire_parent": {"enabled": false}, "fake_wire_leaf": {"enabled": true}
        }});
        assert_parity(
            &disabled_parent,
            Ok(()),
            &[
                integer(Some("fake_wire_leaf"), false, None, None),
                integer(Some("fake_wire_base"), true, Some(7), Some(11)),
            ],
        );
    }
}

mod config_callbacks {
    use auctioneer_bidder::BidderSettings;
    use auctioneer_bidder::{
        BidderConfigError, BidderConfigInput, BidderConfigValidator, BidderDeclaration,
        BidderRegistration, RegistryInputs, ValidatedRegistryPlan, bidder_declarations,
        compiled_bidders,
    };
    use auctioneer_config_data::types::{
        BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
    };
    use auctioneer_core::config::{
        BidderOverrides,
        validation::{
            BidderConfigurationError, ConfigValidationContext, PolicyValidationError,
            validate_bidder_configuration, validate_operator_policy,
        },
    };
    use serde_json::{Value, json};
    use std::{cell::RefCell, collections::BTreeMap, mem};

    #[derive(Debug, PartialEq, Eq)]
    struct Observation {
        implementation: &'static str,
        definition: Option<String>,
        enabled: bool,
        primary: Option<i64>,
        secondary: Option<i64>,
    }

    thread_local! {
        static OBSERVATIONS: RefCell<Vec<Observation>> = const { RefCell::new(Vec::new()) };
    }

    fn observations() -> Vec<Observation> {
        OBSERVATIONS.with_borrow_mut(mem::take)
    }

    fn observe_integer(input: BidderConfigInput<'_>) -> Result<(), BidderConfigError> {
        let resolved = auctioneer_bidder::bidder_settings::fake_integer_probe::resolve(
            input
                .requested
                .and_then(BidderSettings::as_fake_integer_probe),
            input.base.and_then(BidderSettings::as_fake_integer_probe),
        );
        OBSERVATIONS.with_borrow_mut(|seen| {
            seen.push(Observation {
                implementation: "fake_integer_probe",
                definition: input.definition.map(|info| info.name.clone()),
                enabled: input.enabled,
                primary: resolved.primary_number(),
                secondary: resolved.secondary_number(),
            });
        });
        callback("fake_integer_probe")(input)
    }

    fn observe_string(input: BidderConfigInput<'_>) -> Result<(), BidderConfigError> {
        OBSERVATIONS.with_borrow_mut(|seen| {
            seen.push(Observation {
                implementation: "fake_string_probe",
                definition: input.definition.map(|info| info.name.clone()),
                enabled: input.enabled,
                primary: None,
                secondary: None,
            });
        });
        callback("fake_string_probe")(input)
    }

    fn public_plan(enabled: bool, disabled: bool) -> ValidatedRegistryPlan {
        let infos = Box::leak(Box::new([
            BidderInfo {
                name: "fake_base".into(),
                default_enabled: Some(enabled),
                disabled,
                ..BidderInfo::default()
            },
            BidderInfo {
                name: "fake_text".into(),
                default_enabled: Some(false),
                ..BidderInfo::default()
            },
            BidderInfo {
                name: "fake_plain".into(),
                ..BidderInfo::default()
            },
        ]));
        let bindings = [
            BidderImplementationBinding {
                bidder: "fake_base",
                implementation: "fake_integer_probe",
            },
            BidderImplementationBinding {
                bidder: "fake_text",
                implementation: "fake_string_probe",
            },
            BidderImplementationBinding {
                bidder: "fake_plain",
                implementation: "fake_plain",
            },
        ];
        let metadata = bindings.map(|row| BidderMetadataDeclaration {
            bidder: row.bidder,
            implementation: row.implementation,
            required_feature: None,
        });
        let mut factories: Vec<_> = compiled_bidders()
            .iter()
            .map(|row| BidderRegistration {
                key: row.key,
                factory: row.factory,
                config_validator: match row.key {
                    "fake_integer_probe" => Some(observe_integer),
                    "fake_string_probe" => Some(observe_string),
                    _ => row.config_validator,
                },
            })
            .collect();
        factories.push(BidderRegistration {
            key: "fake_plain",
            factory: auctioneer_bidder::fake_synthetic::create_bidder,
            config_validator: None,
        });
        let mut declarations: Vec<_> = bidder_declarations()
            .iter()
            .map(|row| BidderDeclaration {
                key: row.key,
                required_feature: row.required_feature,
                has_settings: row.has_settings,
            })
            .collect();
        declarations.push(BidderDeclaration {
            key: "fake_plain",
            required_feature: None,
            has_settings: false,
        });
        ValidatedRegistryPlan::validate(RegistryInputs {
            infos,
            bindings: &bindings,
            metadata_declarations: &metadata,
            factory_declarations: &declarations,
            factories: &factories,
            aliases: &[],
        })
        .unwrap()
    }

    fn overrides(values: Value) -> BTreeMap<String, BidderOverrides> {
        serde_json::from_value(values).unwrap()
    }

    #[test]
    fn complete_policy_validation_preserves_generated_settings_and_owner_refusals() {
        assert!(observations().is_empty());
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let catalog = public_plan(true, false);
        let limits = auctioneer_core::test_support::boot_parity_fixture::profile_for_adapter(
            "unequal_explicit_caps",
            "axum",
        )
        .limits;
        let context = ConfigValidationContext {
            product_host: auctioneer_config_data::product_host_default(),
            product_account: auctioneer_config_data::product_account_default(),
            catalog: &catalog,
            limits: &limits,
            now_unix_seconds: 1000,
        };
        let valid: auctioneer_core::config::policy::OperatorPolicy =
            serde_json::from_value(json!({
                "schema_version": 1_u64,
                "host": {"aliases": {"fake_alias": "fake_base"}},
                "bidders": {
                    "fake_base": {"settings": {"implementation": "fake_integer_probe", "values": {
                        "primary_number": 7_i64, "secondary_number": 11_i64
                    }}},
                    "fake_alias": {"settings": {"implementation": "fake_integer_probe", "values": {
                        "primary_number": 0_i64
                    }}},
                    "fake_future": {"settings": {"implementation": "fake_integer_probe", "values": {
                        "primary_number": 17_i64, "secondary_number": 19_i64
                    }}}
                }
            }))
            .unwrap();
        assert_eq!(validate_operator_policy(&valid, &context), Ok(()));
        let mut seen = observations();
        seen.sort_by_key(|row| row.primary);
        assert_eq!(
            seen,
            vec![
                Observation {
                    implementation: "fake_integer_probe",
                    definition: Some("fake_base".into()),
                    enabled: true,
                    primary: Some(0),
                    secondary: Some(11)
                },
                Observation {
                    implementation: "fake_integer_probe",
                    definition: Some("fake_base".into()),
                    enabled: true,
                    primary: Some(7),
                    secondary: Some(11)
                },
                Observation {
                    implementation: "fake_integer_probe",
                    definition: None,
                    enabled: false,
                    primary: Some(17),
                    secondary: Some(19)
                },
            ]
        );
        let invalid: auctioneer_core::config::policy::OperatorPolicy = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "host": {"aliases": {"fake_alias": "fake_base"}},
            "bidders": {
                "fake_base": {"enabled": false, "settings": {"implementation": "fake_integer_probe", "values": {"primary_number": -1_i64}}},
                "fake_alias": {"enabled": false, "settings": {"implementation": "fake_integer_probe", "values": {"primary_number": 0_i64}}}
            }
        })).unwrap();
        assert_eq!(
            validate_operator_policy(&invalid, &context),
            Err(PolicyValidationError::Bidder(
                BidderConfigurationError::Bidder(BidderConfigError {
                    field: "primary_number",
                    reason: "invalid"
                })
            ))
        );
        assert_eq!(observations().len(), 1);
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn shared_admission_visits_each_identity_once_with_fieldwise_alias_fallback() {
        assert!(observations().is_empty());
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let catalog = public_plan(true, false);
        let bidders = overrides(json!({
            "fake_base": {"settings": {"implementation":"fake_integer_probe", "values":{
                "primary_number":7_i64, "secondary_number":11_i64
            }}},
            "fake_alias": {"settings": {"implementation":"fake_integer_probe", "values":{
                "primary_number":0_i64
            }}},
            "fake_future": {"enabled":true, "settings": {"implementation":"fake_integer_probe", "values":{
                "primary_number":17_i64, "secondary_number":19_i64
            }}},
            "fake_text": {"settings": {"implementation":"fake_string_probe", "values":{"token":"safe"}}}
        }));
        let aliases = BTreeMap::from([("fake_alias".into(), "fake_base".into())]);
        validate_bidder_configuration(&bidders, &aliases, &catalog).unwrap();
        let mut seen = observations();
        seen.sort_by_key(|row| (row.implementation, row.primary));
        assert_eq!(
            seen,
            vec![
                Observation {
                    implementation: "fake_integer_probe",
                    definition: Some("fake_base".into()),
                    enabled: true,
                    primary: Some(0),
                    secondary: Some(11)
                },
                Observation {
                    implementation: "fake_integer_probe",
                    definition: Some("fake_base".into()),
                    enabled: true,
                    primary: Some(7),
                    secondary: Some(11)
                },
                Observation {
                    implementation: "fake_integer_probe",
                    definition: None,
                    enabled: false,
                    primary: Some(17),
                    secondary: Some(19)
                },
                Observation {
                    implementation: "fake_string_probe",
                    definition: Some("fake_text".into()),
                    enabled: false,
                    primary: None,
                    secondary: None
                },
            ]
        );
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn shared_admission_checks_disabled_and_shadowed_supplied_values() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let catalog = public_plan(false, false);
        let aliases = BTreeMap::from([("fake_alias".into(), "fake_base".into())]);
        let mut cases = 0_usize;
        for field in ["primary_number", "secondary_number"] {
            for bad_base in [false, true] {
                let good = json!({"primary_number":0_i64, "secondary_number":23_i64});
                let bad = json!({(field):-1_i64});
                let (base, alias) = if bad_base { (bad, good) } else { (good, bad) };
                let bidders = overrides(json!({
                    "fake_base": {"enabled":false, "settings":{"implementation":"fake_integer_probe", "values":base}},
                    "fake_alias": {"enabled":false, "settings":{"implementation":"fake_integer_probe", "values":alias}}
                }));
                assert!(observations().is_empty());
                assert_eq!(
                    validate_bidder_configuration(&bidders, &aliases, &catalog),
                    Err(BidderConfigurationError::Bidder(BidderConfigError {
                        field,
                        reason: "invalid"
                    }))
                );
                let seen = observations();
                assert_eq!(seen.len(), 1);
                assert!(!seen[0].enabled);
                cases += 1;
            }
        }
        assert_eq!(cases, 4);
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn shared_admission_enforces_requiredness_only_for_known_enabled_identities() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let aliases = BTreeMap::new();
        for override_enabled in [None, Some(true)] {
            let catalog = public_plan(override_enabled.is_none(), false);
            let mut supplied = json!({"fake_base": {}});
            if let Some(enabled) = override_enabled {
                supplied["fake_base"]["enabled"] = json!(enabled);
            }
            let bidders = overrides(supplied);
            assert!(observations().is_empty());
            assert_eq!(
                validate_bidder_configuration(&bidders, &aliases, &catalog),
                Err(BidderConfigurationError::Bidder(BidderConfigError {
                    field: "primary_number",
                    reason: "required"
                }))
            );
            let seen = observations();
            assert_eq!(seen.len(), 1);
            assert!(seen[0].enabled);
        }
        let catalog = public_plan(true, false);
        let bidders = overrides(
            json!({"fake_base":{"settings":{"implementation":"fake_integer_probe", "values":{"primary_number":0_i64}}}}),
        );
        assert_eq!(
            validate_bidder_configuration(&bidders, &aliases, &catalog),
            Err(BidderConfigurationError::Bidder(BidderConfigError {
                field: "secondary_number",
                reason: "required"
            }))
        );
        assert_eq!(observations().len(), 1);

        for (enabled, disabled, value) in [
            (true, false, json!({"fake_base":{"enabled":false}})),
            (false, false, json!({})),
            (true, true, json!({"fake_base":{"enabled":true}})),
        ] {
            let catalog = public_plan(enabled, disabled);
            validate_bidder_configuration(&overrides(value), &aliases, &catalog).unwrap();
            assert!(observations().is_empty());
        }
        let catalog = public_plan(false, false);
        let bidders = overrides(
            json!({"fake_future":{"enabled":true, "settings":{"implementation":"fake_integer_probe", "values":{}}}}),
        );
        validate_bidder_configuration(&bidders, &aliases, &catalog).unwrap();
        assert_eq!(
            observations(),
            vec![Observation {
                implementation: "fake_integer_probe",
                definition: None,
                enabled: false,
                primary: None,
                secondary: None,
            }]
        );
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn shared_admission_rejects_bindings_before_any_callback() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let catalog = public_plan(true, false);
        let aliases = BTreeMap::from([("fake_alias".into(), "fake_base".into())]);
        for name in ["fake_alias", "fake_base", "fake_plain"] {
            let mut bidders = overrides(json!({
                "fake_base":{"settings":{"implementation":"fake_integer_probe", "values":{"primary_number":3_i64,"secondary_number":5_i64}}},
                "fake_early":{"settings":{"implementation":"fake_integer_probe", "values":{"primary_number":7_i64,"secondary_number":11_i64}}}
            }));
            bidders.insert(
                name.into(),
                BidderOverrides {
                    settings: Some(settings("fake_string_probe", json!({"token":"safe"}))),
                    ..BidderOverrides::default()
                },
            );
            let error = validate_bidder_configuration(&bidders, &aliases, &catalog).unwrap_err();
            assert_eq!(error, BidderConfigurationError::SettingsBinding);
            assert!(
                observations().is_empty(),
                "binding errors must precede callback dispatch"
            );
        }
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn shared_admission_rejects_settings_for_unlinked_factories() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let mut declarations: Vec<_> = bidder_declarations()
            .iter()
            .map(|row| BidderDeclaration {
                key: row.key,
                required_feature: if row.key == "fake_integer_probe" {
                    Some("fake_disabled_feature")
                } else {
                    row.required_feature
                },
                has_settings: row.has_settings,
            })
            .collect();
        let factories: Vec<_> = compiled_bidders()
            .iter()
            .filter(|row| row.key != "fake_integer_probe")
            .map(|row| BidderRegistration {
                key: row.key,
                factory: row.factory,
                config_validator: row.config_validator,
            })
            .collect();
        let invalid = RegistryInputs {
            infos: &[],
            bindings: &[],
            metadata_declarations: &[],
            factory_declarations: &declarations,
            factories: &factories,
            aliases: &[],
        };
        assert_eq!(
            ValidatedRegistryPlan::validate(invalid).unwrap_err(),
            auctioneer_bidder::RegistryBuildError::ConfigValidatorMismatch {
                implementation: "fake_integer_probe",
            }
        );
        let inactive = declarations
            .iter_mut()
            .find(|row| row.key == "fake_integer_probe")
            .unwrap();
        assert!(inactive.has_settings);
        inactive.has_settings = false;
        let catalog = ValidatedRegistryPlan::validate(RegistryInputs {
            infos: &[],
            bindings: &[],
            metadata_declarations: &[],
            factory_declarations: &declarations,
            factories: &factories,
            aliases: &[],
        })
        .unwrap();
        assert!(catalog.config_validator("fake_integer_probe").is_none());
        assert_eq!(
            catalog.factory_keys().collect::<Vec<_>>(),
            vec!["fake_string_probe"]
        );
        for enabled in [false, true] {
            let bidders = overrides(json!({
                "fake_future":{"enabled":enabled, "settings":{
                    "implementation":"fake_integer_probe", "values":{"primary_number":0_i64}
                }}
            }));
            assert_eq!(
                validate_bidder_configuration(&bidders, &BTreeMap::new(), &catalog),
                Err(BidderConfigurationError::SettingsBinding)
            );
            assert!(observations().is_empty());
        }
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn shared_admission_rejects_settings_limits_before_any_callback() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let catalog = public_plan(true, false);
        let bidders = overrides(json!({
            "fake_base":{"settings":{"implementation":"fake_integer_probe", "values":{"primary_number":3_i64,"secondary_number":5_i64}}},
            "fake_text":{"enabled":false, "settings":{"implementation":"fake_string_probe", "values":{"token":"private_canary".repeat(2048)}}}
        }));
        let error =
            validate_bidder_configuration(&bidders, &BTreeMap::new(), &catalog).unwrap_err();
        assert_eq!(error, BidderConfigurationError::Structure);
        assert!(observations().is_empty());
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn shared_admission_redacts_rejected_values_and_credential_references() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let catalog = public_plan(false, false);
        let canary = "fake_private_credential_location";
        let aliases = BTreeMap::from([("fake_alias".into(), "fake_text".into())]);
        let bidders = overrides(json!({
            "fake_text":{"settings":{"implementation":"fake_string_probe", "values":{"token":canary}}},
            "fake_alias":{"settings":{"implementation":"fake_string_probe", "values":{"token":""}}}
        }));
        let error = validate_bidder_configuration(&bidders, &aliases, &catalog).unwrap_err();
        assert_eq!(
            error,
            BidderConfigurationError::Bidder(BidderConfigError {
                field: "token",
                reason: "invalid"
            })
        );
        let diagnostic = format!("{error:?} {error}");
        for forbidden in [canary, "private", "credential", "fake_alias", "fake_text"] {
            assert!(
                !diagnostic.contains(forbidden),
                "unexpected input in fixed diagnostic"
            );
        }
        assert_eq!(observations().len(), 1);
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    fn plan(factories: &[BidderRegistration]) -> ValidatedRegistryPlan {
        ValidatedRegistryPlan::validate(RegistryInputs {
            infos: &[],
            bindings: &[],
            metadata_declarations: &[],
            factory_declarations: bidder_declarations(),
            factories,
            aliases: &[],
        })
        .unwrap()
    }

    fn settings(implementation: &str, values: Value) -> BidderSettings {
        serde_json::from_value(json!({"implementation":implementation, "values":values})).unwrap()
    }

    fn integer(values: Value) -> BidderSettings {
        settings("fake_integer_probe", values)
    }

    fn callback(key: &str) -> BidderConfigValidator {
        plan(compiled_bidders()).config_validator(key).unwrap()
    }

    fn check(
        callback: BidderConfigValidator,
        requested: Option<&BidderSettings>,
        base: Option<&BidderSettings>,
        definition: Option<&BidderInfo>,
        enabled: bool,
    ) -> Result<(), BidderConfigError> {
        callback(BidderConfigInput {
            requested,
            base,
            definition,
            enabled,
        })
    }

    #[test]
    fn supplied_values_are_independent_even_when_disabled_or_overridden() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let validate = callback("fake_integer_probe");
        let good = integer(json!({"primary_number":17,"secondary_number":19}));
        let mut cases = 0;
        for field in ["primary_number", "secondary_number"] {
            let bad = integer(json!({(field):-1}));
            for enabled in [false, true] {
                for (requested, base) in [
                    (Some(&bad), Some(&good)),
                    (Some(&good), Some(&bad)),
                    (Some(&bad), None),
                    (None, Some(&bad)),
                ] {
                    assert_eq!(
                        check(validate, requested, base, None, enabled),
                        Err(BidderConfigError {
                            field,
                            reason: "invalid"
                        })
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 16);
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn effective_requiredness_and_fieldwise_fallback_use_the_same_callback() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let validate = callback("fake_integer_probe");
        let definition = BidderInfo::default();
        let primary = integer(json!({"primary_number":0}));
        let secondary = integer(json!({"secondary_number":23}));
        assert_eq!(
            check(validate, None, None, Some(&definition), true),
            Err(BidderConfigError {
                field: "primary_number",
                reason: "required"
            })
        );
        assert_eq!(
            check(validate, Some(&primary), None, Some(&definition), true),
            Err(BidderConfigError {
                field: "secondary_number",
                reason: "required"
            })
        );
        for (requested, base) in [
            (Some(&primary), Some(&secondary)),
            (Some(&secondary), Some(&primary)),
        ] {
            assert_eq!(
                check(validate, requested, base, Some(&definition), true),
                Ok(())
            );
        }
        assert_eq!(
            check(validate, None, None, Some(&definition), false),
            Ok(())
        );
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    #[test]
    fn missing_null_zero_empty_and_request_local_context_remain_distinct() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        let validate = callback("fake_integer_probe");
        let definition = BidderInfo::default();
        let base = integer(json!({"primary_number":31,"secondary_number":37}));
        for values in [
            json!({}),
            json!({"primary_number":null,"secondary_number":null}),
            json!({"primary_number":0}),
        ] {
            let requested = integer(values);
            assert_eq!(
                check(
                    validate,
                    Some(&requested),
                    Some(&base),
                    Some(&definition),
                    true
                ),
                Ok(())
            );
        }
        assert_eq!(check(validate, None, None, None, false), Ok(()));
        let token_validate = callback("fake_string_probe");
        let empty = settings("fake_string_probe", json!({"token":""}));
        assert_eq!(
            check(token_validate, Some(&empty), None, None, false),
            Err(BidderConfigError {
                field: "token",
                reason: "invalid"
            })
        );
        let null = settings("fake_string_probe", json!({"token":null}));
        assert_eq!(
            check(token_validate, Some(&null), None, None, false),
            Ok(())
        );
        let canary = settings(
            "fake_string_probe",
            json!({"token":"fake_private_credential_location"}),
        );
        let error = check(token_validate, Some(&empty), Some(&canary), None, false).unwrap_err();
        assert_eq!(
            error,
            BidderConfigError {
                field: "token",
                reason: "invalid"
            }
        );
        assert!(!format!("{error:?} {error}").contains("fake_private_credential_location"));
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }

    fn assert_callbacks(plan: &ValidatedRegistryPlan) {
        let integer = integer(json!({"primary_number":-1}));
        let token = settings("fake_string_probe", json!({"token":""}));
        assert_eq!(
            check(
                plan.config_validator("fake_integer_probe").unwrap(),
                Some(&integer),
                None,
                None,
                false
            ),
            Err(BidderConfigError {
                field: "primary_number",
                reason: "invalid"
            })
        );
        assert_eq!(
            check(
                plan.config_validator("fake_string_probe").unwrap(),
                Some(&token),
                None,
                None,
                false
            ),
            Err(BidderConfigError {
                field: "token",
                reason: "invalid"
            })
        );
    }

    #[test]
    fn swapped_callbacks_fail_the_behavioral_contract_without_factories() {
        let before = auctioneer_bidder::fake_synthetic::factory_calls();
        assert_callbacks(&plan(compiled_bidders()));
        let integer = callback("fake_integer_probe");
        let token = callback("fake_string_probe");
        let swapped: Vec<_> = compiled_bidders()
            .iter()
            .map(|row| BidderRegistration {
                key: row.key,
                factory: row.factory,
                config_validator: match row.key {
                    "fake_integer_probe" => Some(token),
                    "fake_string_probe" => Some(integer),
                    _ => row.config_validator,
                },
            })
            .collect();
        let wrong = plan(&swapped);
        assert!(
            std::panic::catch_unwind(|| assert_callbacks(&wrong)).is_err(),
            "swapped callbacks escaped behavioral checks"
        );
        assert_eq!(auctioneer_bidder::fake_synthetic::factory_calls(), before);
    }
}
