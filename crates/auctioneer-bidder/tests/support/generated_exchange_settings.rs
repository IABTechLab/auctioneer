//! Request-resolved alias bindings, typed host settings and construction isolation.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_bidder::{
        BidderRegistry, BidderRequestInfo, fake_synthetic::FakeBidder,
        request_info::BidderSettingsSource,
    };
    use auctioneer_bidder::{BidderSettings, bidder_settings::fake_integer_probe};
    use auctioneer_config_data::types::{
        AccountConfig, BidderImplementationBinding, BidderInfo, HostConfig, ResourceLimits,
    };
    use auctioneer_exchange::test_support::response_budgets;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, resolve_aliases, run_auction,
        scrub::DisclosureAudit,
    };
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::{Arc, Mutex},
    };

    struct Settings(BTreeMap<String, BidderSettings>);

    impl BidderSettingsSource for Settings {
        fn settings(&self, name: &str) -> Option<&BidderSettings> {
            self.0.get(name)
        }
    }

    type Observation = (String, Option<i64>, Option<i64>);
    type Seen = Arc<Mutex<Vec<Observation>>>;
    fn observer(seen: Seen) -> &'static (dyn Fn(&BidderRequestInfo<'_>) + Send + Sync) {
        Box::leak(Box::new(move |info: &BidderRequestInfo<'_>| {
            let settings = fake_integer_probe::resolve(
                info.settings()
                    .and_then(BidderSettings::as_fake_integer_probe),
                info.base_settings()
                    .and_then(BidderSettings::as_fake_integer_probe),
            );
            seen.lock().unwrap().push((
                info.bidder.as_str().to_owned(),
                settings.primary_number(),
                settings.secondary_number(),
            ));
        }))
    }

    struct NoFanout;
    #[async_trait(?Send)]
    impl BidderFanout for NoFanout {
        async fn send_all(
            &self,
            _calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            panic!("settings observer must never dispatch transport requests");
        }
    }

    fn registry_with_shadowed_name(seen: &Seen, shadowed: &Seen) -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        for (name, observations) in [("fake_fx", seen), ("fake_first", shadowed)] {
            registry
                .register(
                    Box::leak(Box::new(BidderInfo {
                        name: name.into(),
                        endpoint: "https://bidder.example.test/auction".into(),
                        uses_https: true,
                        ..BidderInfo::default()
                    })),
                    Box::new(FakeBidder {
                        construction_observer: Some(observer(Arc::clone(observations))),
                        reject_imp: Some("i"),
                        ..FakeBidder::default()
                    }),
                )
                .expect("unique test bidder");
        }
        registry
    }

    fn drive(
        settings: &Settings,
        binding: Option<&'static str>,
        request_aliases: bool,
    ) -> (Value, Vec<Observation>) {
        let raw = json!({"id":"settings-auction", "site":{"id":"site"},
            "imp":[{"id":"i", "banner":{"w":300_i32,"h":250_i32},
                "ext":{"prebid":{"bidder":{"fake_first":{},"fake_second":{}}}}}],
            "ext":{"prebid":{"aliases": if request_aliases {json!({"fake_first":"fake_fx","fake_second":"fake_fx"})} else {json!({})}}}});
        let request = prepare(
            serde_json::from_value(raw).unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits::default(),
        )
        .unwrap();
        let seen = Seen::default();
        let shadowed = Seen::default();
        let registry = registry_with_shadowed_name(&seen, &shadowed);
        let host = HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: 1000,
                max_tmax_ms: 2000,
                max_bidders_per_auction: 2,
                max_outbound_requests: 2,
                max_outbound_raw_bytes: 4096,
                max_outbound_request_bytes: 4096,
                max_transport_bytes: 4096,
                max_decoded_bytes: 4096,
                max_bidder_response_bytes: 4096,
                max_parsed_nodes: 1024,
            },
            ..HostConfig::default()
        };
        let account_config = AccountConfig::default();
        let account = ResolvedAccount {
            id: "publisher".into(),
            source: AccountSource::DefaultFallback,
            config: &account_config,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let bindings: Vec<_> = binding
            .into_iter()
            .flat_map(|implementation| {
                [
                    BidderImplementationBinding {
                        bidder: "fake_first",
                        implementation: "fake_string_probe",
                    },
                    BidderImplementationBinding {
                        bidder: "fake_fx",
                        implementation,
                    },
                ]
            })
            .collect();
        let mut context = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host,
            &account,
            &registry,
            &NoFanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        context.bidder_settings = Some(settings);
        context.bidder_bindings = &bindings;
        let aliases = if request_aliases {
            BTreeMap::new()
        } else {
            BTreeMap::from([
                ("fake_first".into(), "fake_fx".into()),
                ("fake_second".into(), "fake_fx".into()),
            ])
        };
        let resolved = resolve_aliases(&request, &registry, &BTreeSet::new(), &aliases).unwrap();
        let plan = AuctionPlan::new(
            resolved,
            BTreeSet::new(),
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let response = block_on(run_auction(
            &request,
            &context,
            &plan,
            &mut DisclosureAudit::default(),
        ))
        .unwrap()
        .into_bid_response()
        .unwrap();
        assert!(
            shadowed.lock().unwrap().is_empty(),
            "the shadowed implementation must never construct a request"
        );
        let observed = seen.lock().unwrap().clone();
        (serde_json::to_value(response).unwrap(), observed)
    }

    fn settings(alias: Option<&str>, base: Option<&str>) -> Settings {
        let mut entries = BTreeMap::new();
        if let Some(implementation) = base {
            let values = if implementation == "fake_integer_probe" {
                json!({"primary_number":41_i64,"secondary_number":42_i64})
            } else {
                json!({})
            };
            entries.insert(
                "fake_fx".into(),
                serde_json::from_value(json!({"implementation":implementation,"values":values}))
                    .unwrap(),
            );
        }
        if let Some(implementation) = alias {
            let values = if implementation == "fake_integer_probe" {
                json!({"primary_number":0_i64})
            } else {
                json!({})
            };
            entries.insert(
                "fake_first".into(),
                serde_json::from_value(json!({"implementation":implementation,"values":values}))
                    .unwrap(),
            );
        }
        Settings(entries)
    }

    #[test]
    fn both_request_and_host_aliases_borrow_settings_with_independent_field_precedence() {
        let mut cases = 0;
        for request_aliases in [false, true] {
            let (response, seen) = drive(
                &settings(Some("fake_integer_probe"), Some("fake_integer_probe")),
                Some("fake_integer_probe"),
                request_aliases,
            );
            assert_eq!(
                seen,
                [
                    ("fake_first".into(), Some(0), Some(42)),
                    ("fake_second".into(), Some(41), Some(42))
                ]
            );
            assert!(response["ext"]["errors"]["fake_first"].is_null());
            assert!(response["ext"]["errors"]["fake_second"].is_null());
            cases += 1;
        }
        assert_eq!(cases, 2);
    }

    #[test]
    fn wrong_requested_binding_blocks_only_that_alias_before_construction() {
        let mut cases = 0;
        for request_aliases in [false, true] {
            let (response, seen) = drive(
                &settings(Some("fake_string_probe"), Some("fake_integer_probe")),
                Some("fake_integer_probe"),
                request_aliases,
            );
            assert_eq!(seen, [("fake_second".into(), Some(41), Some(42))]);
            assert_eq!(
                response["ext"]["errors"]["fake_first"],
                json!([{"code":5_u32,"message":"bidder settings implementation mismatch"}])
            );
            assert!(response["ext"]["errors"]["fake_second"].is_null());
            cases += 1;
        }
        assert_eq!(cases, 2);
    }

    #[test]
    fn wrong_base_or_missing_binding_never_falls_back_through_typed_projection() {
        let mut cases = 0;
        for (base, binding) in [
            (Some("fake_string_probe"), Some("fake_integer_probe")),
            (Some("fake_integer_probe"), None),
            (None, None),
        ] {
            let (response, seen) =
                drive(&settings(Some("fake_integer_probe"), base), binding, true);
            let expected = if base.is_none() {
                vec![("fake_second".into(), None, None)]
            } else {
                Vec::new()
            };
            assert_eq!(seen, expected);
            assert_eq!(
                response["ext"]["errors"]["fake_first"],
                json!([{"code":5_u32,"message":"bidder settings implementation mismatch"}])
            );
            if base.is_some() {
                assert_eq!(
                    response["ext"]["errors"]["fake_second"],
                    json!([{"code":5_u32,"message":"bidder settings implementation mismatch"}])
                );
            }
            cases += 1;
        }
        let (response, seen) = drive(&settings(None, None), None, true);
        assert_eq!(
            seen,
            [
                ("fake_first".into(), None, None),
                ("fake_second".into(), None, None)
            ]
        );
        assert!(response["ext"]["errors"].is_null());
        cases += 1;
        assert_eq!(cases, 4);
    }
}
