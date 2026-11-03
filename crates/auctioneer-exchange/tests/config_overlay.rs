//! End-to-end test that changes to `auctioneer.toml`'s auction limits and
//! targeting prefix flow through `resolved_host()` / `resolved_account()`
//! into the auction driver — proving `auctioneer config push` will
//! actually change live behavior (not just the pushed blob).

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
#[expect(
    dead_code,
    reason = "shared recording harness covers more than config overlays"
)]
mod auction_support;
mod support;

#[cfg(test)]
mod tests {
    use super::auction_support::{Harness, request, second_imp};
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{BidderHttpResponse, BidderName, BidderRegistry};
    use auctioneer_config::types::{
        AccountConfig, BidderInfo, Capabilities, Compression, HostConfig, MediaType, ResourceLimits,
    };
    use auctioneer_exchange::construction_costs;
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AliasMap, AuctionContext, AuctionDeadline, AuctionError, AuctionPlan,
        BidderCall, BidderCallResult, BidderFanout, ExchangeFeatures, FanoutError,
        FanoutInternalError, ResolvedAccount, ResolvedAliases, TimeoutCause, resolve_aliases,
        run_auction, split_by_bidder,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, BidRequestExt, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};
    use std::mem;
    use std::time::Duration;

    #[test]
    fn disabled_colliding_alias_never_reaches_construction_or_fanout() {
        for (host_alias, disable_base, baked) in [
            (false, true, false),
            (true, true, false),
            (false, false, false),
            (true, false, false),
            (true, true, true),
        ] {
            for bidder_ext in [
                json!({"bidder": {"fake_ccc": {}, "fake_bb": {}}}),
                json!({"prebid": {"bidder": {"fake_ccc": {}, "fake_bb": {}}}}),
                json!({"fake_ccc": {}, "fake_bb": {}}),
            ] {
                let mut harness = Harness::default();
                if baked {
                    harness.baked_disabled.insert("fake_aaa".into());
                } else {
                    harness.disabled.insert(BidderName::new(if disable_base {
                        "fake_aaa"
                    } else {
                        "fake_ccc"
                    }));
                }
                let mut req = request(&["fake_ccc", "fake_bb"]);
                req["imp"][0]["ext"] = bidder_ext;
                second_imp(&mut req);
                if host_alias {
                    harness
                        .host
                        .aliases
                        .insert("fake_ccc".into(), "fake_aaa".into());
                } else {
                    req["ext"]["prebid"]["aliases"] = json!({"fake_ccc": "fake_aaa"});
                }
                req["ext"]["prebid"]["nosale"] = json!(["fake_ccc"]);
                let observed = harness.run(req);
                observed.assert_built(&["fake_bb"]);
                assert_eq!(observed.outbound.len(), 1);
                assert!(observed.requests("fake_ccc").is_empty());
                assert_eq!(observed.response().seatbid.len(), 1);
                assert_eq!(observed.response().seatbid[0].seat, "fake_bb");
                assert_eq!(
                    observed.ext()["warnings"]["fake_ccc"],
                    json!([{"code": 6_i32, "message": "bidder disabled by overlay: fake_ccc"}]),
                    "retain exactly the resolver's disabled warning, including for baked disablement"
                );
            }
        }
    }

    #[test]
    fn enabled_colliding_alias_still_uses_its_declared_base() {
        for host_alias in [false, true] {
            let mut harness = Harness::default();
            let mut req = request(&["fake_ccc", "fake_bb"]);
            if host_alias {
                harness
                    .host
                    .aliases
                    .insert("fake_ccc".into(), "fake_aaa".into());
            } else {
                req["ext"]["prebid"]["aliases"] = json!({"fake_ccc": "fake_aaa"});
            }
            let observed = harness.run(req);
            observed.assert_built(&["fake_bb", "fake_ccc"]);
            assert_eq!(observed.requests("fake_ccc").len(), 1);
            let alias = observed
                .constructions
                .iter()
                .find(|build| build.bidder == "fake_ccc")
                .unwrap();
            assert_eq!(alias.core, "fake_aaa");
        }
    }

    #[test]
    fn privacy_warning_is_emitted_once_without_unrequested_status_rows() {
        for return_status in [false, true] {
            for expired in [false, true] {
                let mut harness = Harness::default();
                harness.activities(json!({"fetchBids": {"default": false}}));
                if expired {
                    harness.elapsed = Duration::from_secs(2);
                }
                let mut req = request(&["fake_bb"]);
                req["ext"]["prebid"]["returnallbidstatus"] = json!(return_status);
                second_imp(&mut req);
                let observed = harness.run(req);
                observed.assert_built(&[]);
                assert!(observed.outbound.is_empty());
                assert_eq!(
                    observed.ext()["warnings"]["fake_bb"],
                    json!([{
                        "code": 10017_i32,
                        "message": "bidder blocked by privacy settings: fake_bb"
                    }]),
                );
                if return_status {
                    observed.assert_blocked("fake_bb", &["imp-1", "imp-2"]);
                } else {
                    assert!(observed.ext()["prebid"].get("seatnonbid").is_none());
                }
            }
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "sealed dispatch assertions and request-scoped cap setup share one fixture"
    )]
    fn sealed_alias_calls_carry_independent_caps_and_one_clamp_warning() {
        use auctioneer_exchange::{BidderBudgets, BootEnvelope};
        use std::sync::Mutex;

        #[derive(Default)]
        struct CaptureCaps(Mutex<Vec<(u64, u64)>>);

        #[async_trait(?Send)]
        impl BidderFanout for CaptureCaps {
            async fn send_all(
                &self,
                calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                Ok(calls
                    .into_iter()
                    .map(|call| {
                        self.0
                            .lock()
                            .unwrap()
                            .push((call.max_transport_bytes, call.max_decoded_bytes));
                        assert_eq!(
                            call.request
                                .headers()
                                .filter(|&(name, _)| name.eq_ignore_ascii_case("Content-Encoding"))
                                .collect::<Vec<_>>(),
                            vec![("Content-Encoding", "gzip")],
                        );
                        assert_eq!(
                            call.request
                                .headers()
                                .filter(|&(name, _)| name.eq_ignore_ascii_case("Sec-GPC"))
                                .collect::<Vec<_>>(),
                            vec![("Sec-GPC", "1")],
                        );
                        BidderCallResult {
                            result: Err(FanoutError::BadServerResponse),
                            elapsed: Duration::ZERO,
                        }
                    })
                    .collect())
            }
        }

        let mut registry = BidderRegistry::new();
        let info = Box::leak(Box::new(BidderInfo {
            endpoint_compression: Some(Compression::Gzip),
            ..leak("fake_bb", "https://demo.example.test/openrtb").clone()
        }));
        registry
            .register(info, Box::new(FakeBidder::default()))
            .expect("unique test bidder");
        let normalized = prepare(
            serde_json::from_value(json!({
                "id": "caps", "site": {"id": "s"}, "regs": {"ext": {"gpc": "1"}},
                "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"bidder": {"fake_bz": {}}}}],
                "ext": {"prebid": {"aliases": {"fake_bz": "fake_bb"}}}
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &IMP_CAP_NOT_UNDER_TEST,
        )
        .unwrap();
        let aliases =
            resolve_aliases(&normalized, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap();
        let plan = AuctionPlan::new(
            aliases,
            BTreeSet::new(),
            BTreeMap::from([(
                BidderName::new("fake_bz"),
                BidderBudgets {
                    max_transport_bytes: 0x8000,
                    max_decoded_bytes: 0x4000,
                },
            )]),
            BTreeMap::new(),
        )
        .with_boot_envelope(BootEnvelope {
            construction_costs: construction_costs(),
            max_transport_bytes: 0x2000,
            max_decoded_bytes: 0x6000,
        });
        let mut host = baked_host();
        host.resource_limits.max_transport_bytes = 1_024;
        host.resource_limits.max_decoded_bytes = 1_024;
        let account = baked_account();
        let resolved = ResolvedAccount {
            id: "test".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let fanout = CaptureCaps::default();
        let timeouts = BTreeMap::new();
        let ctx = AuctionContext::with_request_start_ext(
            clock.now_monotonic(),
            &host,
            &resolved,
            &registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
            ExchangeFeatures::default(),
            &timeouts,
        );
        let mut audit = DisclosureAudit::default();
        let response = block_on(run_auction(&normalized, &ctx, &plan, &mut audit)).unwrap();
        assert_eq!(*fanout.0.lock().unwrap(), vec![(0x2000, 0x4000)]);
        let ext = response.ext.as_ref().unwrap();
        assert_eq!(ext["warnings"]["fake_bz"].as_array().unwrap().len(), 1);
        assert_eq!(
            ext["warnings"]["fake_bz"][0]["code"],
            auctioneer_openrtb::WarningCode::OutboundBudgetClamp.as_u32(),
        );
        assert!(ext["warnings"].get("fake_bb").is_none());
    }

    struct CannedFanout {
        body: Vec<u8>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for CannedFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(200, vec![], self.body.clone())),
                    elapsed: Duration::from_millis(10),
                })
                .collect())
        }
    }

    fn leak(name: &'static str, endpoint: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: endpoint.into(),
            uses_https: true,
            capabilities: Capabilities {
                site: vec![MediaType::Banner],
                app: vec![MediaType::Banner],
                dooh: vec![],
            },
            ..BidderInfo::default()
        }))
    }

    fn baked_host() -> HostConfig {
        HostConfig {
            status_response: String::new(),
            resource_limits: ResourceLimits {
                default_tmax_ms: 1000,
                max_tmax_ms: 2000,
                max_bidder_response_bytes: 0x0004_0000,
                max_bidders_per_auction: 50,
                max_outbound_requests: 100,
                max_outbound_raw_bytes: 0x0004_0000,
                max_outbound_request_bytes: 0x0004_0000,
                max_transport_bytes: 0x0004_0000,
                max_decoded_bytes: 0x0004_0000,
                max_parsed_nodes: 0x0001_0000,
            },
            ..HostConfig::default()
        }
    }

    fn baked_account() -> AccountConfig {
        AccountConfig {
            targeting_prefix: "hb".into(),
            bid_rounding: "down".into(),
            ..AccountConfig::default()
        }
    }

    fn base_request() -> BidRequest {
        let mut bidders = serde_json::Map::new();
        bidders.insert("fake_bb".into(), serde_json::json!({}));
        BidRequest {
            id: "auc-overlay".into(),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            cur: vec!["USD".into()],
            ..BidRequest::default()
        }
    }

    async fn drive_with(
        host: &HostConfig,
        account: &AccountConfig,
        features: ExchangeFeatures,
        response_body: &[u8],
    ) -> BidResponse {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_bb", "https://demo.example.test/openrtb"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        let resolved = ResolvedAccount {
            id: "test".into(),
            source: AccountSource::DefaultFallback,
            config: account,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let normalized = prepare(
            base_request(),
            &RequestHeaders::default(),
            &IMP_CAP_NOT_UNDER_TEST,
        )
        .unwrap();
        let fanout = CannedFanout {
            body: response_body.to_vec(),
        };
        let empty_timeouts = BTreeMap::new();
        let ctx = AuctionContext::with_request_start_ext(
            clock.now_monotonic(),
            host,
            &resolved,
            &registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
            features,
            &empty_timeouts,
        );
        let plan = AuctionPlan::new(
            ResolvedAliases::empty(),
            BTreeSet::new(),
            response_budgets(ctx.bidders, ctx.host),
            BTreeMap::new(),
        );
        let mut audit = DisclosureAudit::default();
        run_auction(&normalized, &ctx, &plan, &mut audit)
            .await
            .unwrap()
    }

    async fn drive(host: &HostConfig, account: &AccountConfig) -> BidResponse {
        drive_with(
            host,
            account,
            ExchangeFeatures::default(),
            // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
            // `serde_json::json!` yields a `Value`, so converting would mean
            // `.to_string().into_bytes()` at the call site for no gain.
            br#"{"id":"r","seatbid":[{"seat":"fake_bb","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#,
        )
        .await
    }

    fn tmaxrequest(resp: &BidResponse) -> u32 {
        resp.ext
            .as_ref()
            .and_then(|e| e.get("tmaxrequest"))
            .and_then(serde_json::Value::as_u64)
            .map_or(0_u32, |n| u32::try_from(n).unwrap_or(u32::MAX))
    }

    #[test]
    fn overlay_lowering_default_tmax_shortens_bidder_visible_tmax() {
        block_on(async {
            // Simulate the overlay: baked=1000, auctioneer.toml pushes 400.
            let mut host = baked_host();
            host.resource_limits.default_tmax_ms = 400;
            let account = baked_account();
            let resp = drive(&host, &account).await;
            assert_eq!(tmaxrequest(&resp), 400_u32);
        });
    }

    #[test]
    fn overlay_capping_max_bidders_drops_extras() {
        block_on(async {
            // Register two bidders and target both in a single imp,
            // then push `max_bidders_per_auction = 1`. Registry order
            // is lexicographic (`fake_bb` < `fake_ccc`), so `fake_bb`
            // survives and `fake_ccc` gets the code-10999 drop warning.
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    leak("fake_bb", "https://demo.example.test/openrtb"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");
            registry
                .register(
                    leak("fake_ccc", "https://demo.example.test/fake_ccc"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");

            let mut host = baked_host();
            host.resource_limits.max_bidders_per_auction = 1;
            let account = baked_account();
            let resolved = ResolvedAccount {
                id: "test".into(),
                source: AccountSource::DefaultFallback,
                config: &account,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let empty_timeouts = BTreeMap::new();

            // Request targets both bidders in imp.ext.bidder.
            let mut bidders = serde_json::Map::new();
            bidders.insert("fake_bb".into(), serde_json::json!({}));
            bidders.insert("fake_ccc".into(), serde_json::json!({}));
            let req = BidRequest {
                id: "auc-cap".into(),
                imp: vec![Imp {
                    id: "imp-1".into(),
                    banner: Some(Banner {
                        w: Some(300_i32),
                        h: Some(250_i32),
                        ..Banner::default()
                    }),
                    ext: Some(ImpExt {
                        bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                        ..ImpExt::default()
                    }),
                    ..Imp::default()
                }],
                cur: vec!["USD".into()],
                ..BidRequest::default()
            };
            let normalized =
                prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
            let fanout = CannedFanout {
                // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
                // `serde_json::json!` yields a `Value`, so converting would mean
                // `.to_string().into_bytes()` at the call site for no gain.
                body: br#"{"id":"r","seatbid":[{"seat":"fake_bb","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#.to_vec(),
            };
            let ctx = AuctionContext::with_request_start_ext(
                clock.now_monotonic(),
                &host,
                &resolved,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
                ExchangeFeatures::default(),
                &empty_timeouts,
            );
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                response_budgets(ctx.bidders, ctx.host),
                BTreeMap::new(),
            );
            let mut audit = DisclosureAudit::default();
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .unwrap();

            // Only `fake_bb` produced a seat row.
            let seats: Vec<&str> = resp.seatbid.iter().map(|s| s.seat.as_str()).collect();
            assert_eq!(seats, vec!["fake_bb"], "fake_bb must survive the cap");

            // `fake_ccc` was dropped before fanout — its cap warning
            // must appear under ext.warnings.fake_ccc (code 10999).
            let ext_json = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
            assert!(
                ext_json.contains("\"fake_ccc\""),
                "fake_ccc must appear as a dropped-bidder key, got {ext_json}",
            );
            assert!(
                ext_json.contains("10999") && ext_json.contains("max_bidders_per_auction"),
                "expected code-10999 max_bidders_per_auction warning, got {ext_json}",
            );
        });
    }

    #[test]
    fn overlay_targeting_prefix_reaches_hb_keys() {
        block_on(async {
            let host = baked_host();
            let mut account = baked_account();
            account.targeting_prefix = "myp".into();
            let resp = drive(&host, &account).await;
            let keys: Vec<String> = resp
                .seatbid
                .iter()
                .flat_map(|s| s.bid.iter())
                .filter_map(|b| b.ext.as_ref())
                .filter_map(|e| e.prebid.as_ref())
                .flat_map(|p| p.targeting.keys().cloned())
                .collect();
            assert!(
                keys.iter().any(|k| k.starts_with("myp_")),
                "expected account overlay prefix `myp_*`, got {keys:?}",
            );
        });
    }

    /// auction core unconditionally drops non-USD bids (no fx path; response
    /// `cur` is hardcoded to "USD"). Regression guard so a future
    /// re-introduction of `strict_currency_usd_only` cannot silently
    /// admit mixed-currency bids into a USD-labelled response.
    #[test]
    fn non_usd_response_bids_are_dropped() {
        block_on(async {
            let host = baked_host();
            let account = baked_account();
            // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
            // `serde_json::json!` yields a `Value`, so converting would mean
            // `.to_string().into_bytes()` at the call site for no gain.
            let eur_body = br#"{"id":"r","seatbid":[{"seat":"fake_bb","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"EUR"}"#;
            let resp = drive_with(&host, &account, ExchangeFeatures::default(), eur_body).await;
            assert!(
                resp.seatbid.is_empty(),
                "EUR bid must be dropped (auction core has no fx path)",
            );
            let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
            assert!(
                raw.contains("10004"),
                "expected code-10004 currency warning, got {raw}",
            );
        });
    }

    /// `returnallbidstatus_default = true` → when the request omits
    /// the flag AND a bidder fails, the response carries
    /// `ext.prebid.seatnonbid`. Same auction with the flag `false`
    /// (default) omits it. Uses an error-returning fanout so
    /// seatnonbid rows are actually generated.
    #[test]
    fn feature_toggle_returnallbidstatus_default_true_emits_seatnonbid() {
        block_on(async {
            struct TimeoutFanout;
            #[async_trait(?Send)]
            impl BidderFanout for TimeoutFanout {
                async fn send_all(
                    &self,
                    calls: Vec<BidderCall>,
                    _deadline: AuctionDeadline,
                ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                    Ok(calls
                        .iter()
                        .map(|_| BidderCallResult {
                            result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                            elapsed: Duration::from_millis(10),
                        })
                        .collect())
                }
            }

            async fn drive_with_fanout(
                features: ExchangeFeatures,
                fanout: &TimeoutFanout,
            ) -> BidResponse {
                let mut registry = BidderRegistry::new();
                registry
                    .register(
                        leak("fake_bb", "https://demo.example.test/openrtb"),
                        Box::new(FakeBidder::default()),
                    )
                    .expect("unique test bidder");
                let host = baked_host();
                let account = baked_account();
                let resolved = ResolvedAccount {
                    id: "test".into(),
                    source: AccountSource::DefaultFallback,
                    config: &account,
                };
                let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
                let rng = SeededRng::new(42);
                let normalized = prepare(
                    base_request(),
                    &RequestHeaders::default(),
                    &IMP_CAP_NOT_UNDER_TEST,
                )
                .unwrap();
                let empty_timeouts = BTreeMap::new();
                let ctx = AuctionContext::with_request_start_ext(
                    clock.now_monotonic(),
                    &host,
                    &resolved,
                    &registry,
                    fanout,
                    &clock,
                    &rng,
                    GvlStore::baked(),
                    Gcl::baked(),
                    clock.now(),
                    false,
                    features,
                    &empty_timeouts,
                );
                let plan = AuctionPlan::new(
                    ResolvedAliases::empty(),
                    BTreeSet::new(),
                    response_budgets(ctx.bidders, ctx.host),
                    BTreeMap::new(),
                );
                let mut audit = DisclosureAudit::default();
                run_auction(&normalized, &ctx, &plan, &mut audit)
                    .await
                    .unwrap()
            }

            let fanout = TimeoutFanout;

            // Default (returnallbidstatus_default = false): NO seatnonbid
            // in the response.
            let default_resp = drive_with_fanout(ExchangeFeatures::default(), &fanout).await;
            let default_raw = serde_json::to_string(default_resp.ext.as_ref().unwrap()).unwrap();
            assert!(
                !default_raw.contains("seatnonbid"),
                "default returnallbidstatus should suppress seatnonbid, got {default_raw}",
            );

            // Toggle on: seatnonbid present.
            let toggled = ExchangeFeatures {
                returnallbidstatus_default: true,
                ..ExchangeFeatures::default()
            };
            let resp = drive_with_fanout(toggled, &fanout).await;
            let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
            assert!(
                raw.contains("seatnonbid"),
                "returnallbidstatus_default = true must emit seatnonbid on fanout errors, \
                 got {raw}",
            );
        });
    }

    /// The `HostConfig.resource_limits.default_tmax_ms > max_tmax_ms`
    /// misconfiguration must NOT let the effective deadline exceed
    /// `max_tmax_ms`. The response's `tmaxrequest` reflects the
    /// adapter-visible tmax (which the spec says passes through
    /// verbatim), so we assert the DEADLINE math instead via
    /// `resolve_tmax`.
    #[test]
    fn default_tmax_clamped_to_max_when_request_omits_tmax() {
        // Direct exercise of resolve_tmax rather than the auction
        // driver — the response builder doesn't expose the effective
        // deadline value directly, but resolve_tmax is the shared
        // source of truth. `tmaxrequest` shows bidder-visible (750);
        // `deadline_at - request_start` should be clamped to 500.
        use auctioneer_exchange::resolve_tmax;
        let mut host = baked_host();
        host.resource_limits.default_tmax_ms = 750;
        host.resource_limits.max_tmax_ms = 500;
        let start = auctioneer_runtime::MonotonicInstant::now();
        let result = resolve_tmax(None, &host, start).expect("a bounded budget resolves");
        assert_eq!(result.bidder_visible_tmax_ms, 750);
        assert_eq!(
            result.deadline.deadline_at,
            start
                .checked_add(Duration::from_millis(500))
                .expect("no overflow"),
            "effective deadline must clamp to max_tmax_ms even when default > max",
        );
    }

    // -----------------------------------------------------------------
    // `[bidders.<name>] enabled = false` registry overlay. The handler
    // folds the overlay into a `BTreeSet<BidderName>` (what
    // `disabled_from_overlay` produces in `auctioneer-core`) before it ever
    // reaches the exchange, so these tests build that set by hand and
    // drive the split/alias functions that actually enforce it.
    // -----------------------------------------------------------------

    /// A request naming every bidder in `names` on one imp, with a
    /// stub `site` populated — `split_by_bidder` re-validates each
    /// bidder-scoped imp after FPD merge, and `BidRequest::validate()`
    /// requires exactly one of `site` / `app` / `dooh`. Used to drive
    /// `split_by_bidder` directly (bypassing `prepare()`, which is
    /// what supplies that invariant on the handler path).
    fn overlay_test_request(names: &[&str]) -> BidRequest {
        use auctioneer_openrtb::types::inventory::Site;
        let mut bidders = serde_json::Map::new();
        for name in names {
            bidders.insert((*name).to_owned(), serde_json::json!({}));
        }
        BidRequest {
            id: "auc-overlay-disabled".into(),
            site: Some(Site {
                id: Some("stub".into()),
                ..Site::default()
            }),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            cur: vec!["USD".into()],
            ..BidRequest::default()
        }
    }

    fn two_bidder_registry() -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_bb", "https://demo.example.test/openrtb"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        registry
            .register(
                leak("fake_ccc", "https://demo.example.test/fake_ccc"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        registry
    }

    #[test]
    fn overlay_disabled_bidder_dropped_before_fanout_and_emits_code_6() {
        let registry = two_bidder_registry();
        let req = overlay_test_request(&["fake_bb", "fake_ccc"]);
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_bb"));

        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry,
            &disabled,
            &BTreeSet::new(),
        )
        .expect("fake_ccc must still land even though fake_bb is overlay-disabled");

        let landed: Vec<&str> = split
            .per_bidder
            .iter()
            .map(|group| group.bidder.as_str())
            .collect();
        assert_eq!(
            landed,
            vec!["fake_ccc"],
            "overlay-disabled fake_bb must be dropped before fan-out",
        );

        assert_eq!(
            split.disabled_bidder_warnings.len(),
            1,
            "exactly one code-6 warning must be emitted for the disabled bidder",
        );
        let (name, warning) = &split.disabled_bidder_warnings[0];
        assert_eq!(name.as_str(), "fake_bb");
        assert_eq!(warning.code, 6, "BidderTemporarilyDisabled is code 6");
        assert!(
            warning.message.contains("fake_bb") && warning.message.contains("overlay"),
            "expected an overlay-disabled message naming fake_bb, got {}",
            warning.message,
        );
    }

    #[test]
    fn overlay_enabled_true_remains_in_registry() {
        let registry = two_bidder_registry();
        let req = overlay_test_request(&["fake_bb", "fake_ccc"]);
        // Only fake_ccc is overlay-disabled; fake_bb is left enabled
        // (whether via an explicit `enabled = true` overlay entry or
        // by omission, neither adds a name to the disabled set).
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_ccc"));

        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry,
            &disabled,
            &BTreeSet::new(),
        )
        .unwrap();

        let landed: Vec<&str> = split
            .per_bidder
            .iter()
            .map(|group| group.bidder.as_str())
            .collect();
        assert_eq!(
            landed,
            vec!["fake_bb"],
            "an overlay-enabled bidder must survive even when a sibling is disabled",
        );
        assert!(
            split
                .disabled_bidder_warnings
                .iter()
                .all(|(name, _)| name.as_str() != "fake_bb"),
            "fake_bb must not carry a disabled-bidder warning",
        );
    }

    #[test]
    fn bidder_absent_from_overlay_defaults_to_enabled() {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_bb", "https://demo.example.test/openrtb"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        // No overlay entry at all for "fake_bb" — the disabled set the
        // handler would hand in is empty.
        let req = overlay_test_request(&["fake_bb"]);
        let disabled: BTreeSet<BidderName> = BTreeSet::new();

        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry,
            &disabled,
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(split.per_bidder.len(), 1);
        assert_eq!(split.per_bidder[0].bidder.as_str(), "fake_bb");
        assert!(
            split.disabled_bidder_warnings.is_empty(),
            "a bidder absent from the overlay must default to enabled with no warning",
        );
    }

    #[test]
    fn request_alias_of_disabled_base_dropped_with_code_6_under_alias_name() {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_aaa", "https://demo.example.test/an"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");

        let mut alias_map = BTreeMap::new();
        alias_map.insert("fake_bz".to_owned(), "fake_aaa".to_owned());
        let req = BidRequest {
            id: "auc-alias-overlay-disabled".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        };

        // The overlay disables the ALIAS'S BASE, not the alias name
        // itself. The other OR-branch of the same check — an overlay
        // entry naming the alias directly — is covered by
        // `request_alias_whose_own_name_is_disabled_dropped_with_code_6`
        // below.
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_aaa"));

        let resolved = resolve_aliases(&req, &registry, &disabled, &no_host_aliases())
            .expect("a disabled-base alias is dropped with a warning, not a hard error");

        assert!(
            !resolved.map.bases.contains_key(&BidderName::new("fake_bz")),
            "an alias whose base is overlay-disabled must not resolve into the alias map",
        );
        assert_eq!(
            resolved.disabled_warnings.len(),
            1,
            "exactly one code-6 warning must be emitted for the dropped alias",
        );
        let (name, warning) = &resolved.disabled_warnings[0];
        assert_eq!(
            name.as_str(),
            "fake_bz",
            "the warning must key on the alias's own observable name, not its base",
        );
        assert_eq!(warning.code, 6, "BidderTemporarilyDisabled is code 6");
        assert!(
            warning.message.contains("fake_bz"),
            "expected the code-6 message to name the alias, got {}",
            warning.message,
        );
    }

    #[test]
    fn request_alias_whose_own_name_is_disabled_dropped_with_code_6() {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_aaa", "https://demo.example.test/an"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");

        let mut alias_map = BTreeMap::new();
        alias_map.insert("fake_bz".to_owned(), "fake_aaa".to_owned());
        let req = BidRequest {
            id: "auc-alias-name-overlay-disabled".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        };

        // This time the overlay names the ALIAS ITSELF, not its base —
        // the other half of the OR in the disabled-alias check.
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_bz"));

        let resolved = resolve_aliases(&req, &registry, &disabled, &no_host_aliases())
            .expect("an alias whose own name is disabled is dropped with a warning, not an error");
        assert!(
            !resolved.map.bases.contains_key(&BidderName::new("fake_bz")),
            "an alias whose own name is overlay-disabled must not resolve into the alias map",
        );
        assert_eq!(resolved.disabled_warnings.len(), 1);
        let (name, warning) = &resolved.disabled_warnings[0];
        assert_eq!(name.as_str(), "fake_bz");
        assert_eq!(warning.code, 6, "BidderTemporarilyDisabled is code 6");
    }

    /// Regression test for a request-killer bug: `aliasgvlids` naming
    /// an alias whose BASE the overlay disables used to hit the
    /// "unknown alias" guard (the alias never landed in `map.bases`)
    /// and hard-error the whole request with `AuctionError::BadInput` —
    /// turning a per-bidder kill-switch into a 400 for every OTHER
    /// bidder sharing the auction. The alias must still be dropped
    /// (and still warned about); only the request-wide failure was the
    /// bug.
    #[test]
    fn overlay_disabled_alias_base_with_aliasgvlids_does_not_fail_the_whole_request() {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_aaa", "https://demo.example.test/an"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");

        let mut alias_map = BTreeMap::new();
        alias_map.insert("fake_bz".to_owned(), "fake_aaa".to_owned());
        let mut gvlid_map = BTreeMap::new();
        gvlid_map.insert("fake_bz".to_owned(), 42_u32);
        let req = BidRequest {
            id: "auc-alias-gvlid-overlay-disabled".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    aliasgvlids: gvlid_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        };

        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_aaa"));

        let resolved = resolve_aliases(&req, &registry, &disabled, &no_host_aliases())
            .expect("aliasgvlids naming an overlay-disabled alias must not fail the whole auction");
        assert!(
            !resolved.map.bases.contains_key(&BidderName::new("fake_bz")),
            "the disabled alias must still be dropped from the map",
        );
        assert!(
            resolved.map.gvlids.is_empty(),
            "the gvlid override for a dropped alias must not apply",
        );
        assert_eq!(
            resolved.disabled_warnings.len(),
            1,
            "the alias drop must still be observable",
        );
    }

    /// Regression test for a competitor-params leak: once
    /// `resolve_aliases` drops a disabled-base alias from `map.bases`,
    /// `is_known_bidder` used to lose track of it entirely, so
    /// `build_scoped_imp`'s competitor strip stopped recognizing the
    /// alias's legacy top-level ext key as a bidder identity and left
    /// it in every OTHER bidder's outbound body — publisher placement
    /// config for one bidder leaking into a competitor's request.
    #[test]
    fn overlay_disabled_alias_base_does_not_leak_params_to_other_bidders() {
        use auctioneer_openrtb::types::inventory::Site;

        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_aaa", "https://demo.example.test/an"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        registry
            .register(
                leak("fake_ccc", "https://demo.example.test/fake_ccc"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");

        let mut alias_map = BTreeMap::new();
        alias_map.insert("fake_bz".to_owned(), "fake_aaa".to_owned());

        // fake_ccc via the modern hoisted slot; fake_bz's own placement
        // config via the legacy top-level slot, on the SAME imp.
        let mut bidder_obj = serde_json::Map::new();
        bidder_obj.insert("fake_ccc".into(), serde_json::json!({}));
        let mut legacy_entries = serde_json::Map::new();
        legacy_entries.insert("fake_bz".into(), serde_json::json!({"placementId": 42_i64}));

        let req = BidRequest {
            id: "auc-alias-leak-check".into(),
            site: Some(Site {
                id: Some("stub".into()),
                ..Site::default()
            }),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidder_obj)),
                    other: legacy_entries,
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            cur: vec!["USD".into()],
            ..BidRequest::default()
        };

        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_aaa"));

        let resolved = resolve_aliases(&req, &registry, &disabled, &no_host_aliases()).unwrap();
        let split = split_by_bidder(
            &req,
            &resolved.map,
            &registry,
            &disabled,
            &resolved.dropped_alias_names,
        )
        .unwrap();

        let fake_c_pbr = split
            .per_bidder
            .iter()
            .find(|pbr| pbr.bidder.as_str() == "fake_ccc")
            .expect("fake_ccc must still land with a valid scoped request");
        let ext = fake_c_pbr.request.imp[0].ext.as_ref().unwrap();
        assert!(
            !ext.other.contains_key("fake_bz"),
            "fake_bz's placement config must not leak into fake_ccc's outbound body, got {:?}",
            ext.other,
        );
    }

    // -----------------------------------------------------------------
    // Host-declared aliases (`[aliases]` in the resolved host config).
    // The handler hands `resolve_aliases` the resolved `host.aliases`
    // map as its fourth argument; these tests supply that map directly
    // and pin how the two declaring layers interact.
    // -----------------------------------------------------------------

    fn no_host_aliases() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn host_aliases(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(alias, base)| ((*alias).to_owned(), (*base).to_owned()))
            .collect()
    }

    /// Registry of `(name, baked_disabled)` pairs — the baked
    /// `BidderInfo.disabled` bit is a separate kill switch from the
    /// overlay set, and the host-alias path has to honour both.
    fn alias_registry(bidders: &[(&'static str, bool)]) -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        for &(name, disabled) in bidders {
            let info: &'static BidderInfo = Box::leak(Box::new(BidderInfo {
                name: name.into(),
                endpoint: "https://demo.example.test/openrtb".into(),
                uses_https: true,
                disabled,
                capabilities: Capabilities {
                    site: vec![MediaType::Banner],
                    app: vec![MediaType::Banner],
                    dooh: vec![],
                },
                ..BidderInfo::default()
            }));
            registry
                .register(info, Box::new(FakeBidder::default()))
                .expect("unique test bidder");
        }
        registry
    }

    /// A request carrying no `ext` at all — the shape a publisher sends
    /// when it declares nothing itself, which is exactly when the host's
    /// own `[aliases]` have to still take effect.
    fn request_without_prebid_ext() -> BidRequest {
        BidRequest {
            id: "auc-host-alias".into(),
            ..BidRequest::default()
        }
    }

    fn request_declaring(aliases: &[(&str, &str)], gvlids: &[(&str, u32)]) -> BidRequest {
        BidRequest {
            id: "auc-host-alias".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: aliases
                        .iter()
                        .map(|(alias, base)| ((*alias).to_owned(), (*base).to_owned()))
                        .collect(),
                    aliasgvlids: gvlids
                        .iter()
                        .map(|(alias, id)| ((*alias).to_owned(), *id))
                        .collect(),
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        }
    }

    fn request_with_param_location(names: &[&str], location: &str) -> BidRequest {
        let mut req = overlay_test_request(names);
        let JsonField::Value(params) = mem::take(&mut req.imp[0].ext.as_mut().unwrap().bidder)
        else {
            panic!("fixture has scoped bidder parameters");
        };
        req.imp[0].ext = Some(
            serde_json::from_value(match location {
                "prebid" => serde_json::json!({"prebid": {"bidder": params}}),
                "hoisted" => serde_json::json!({"bidder": params}),
                "legacy" => params,
                _ => panic!("unknown param location: {location}"),
            })
            .unwrap(),
        );
        req
    }

    fn alias_routing_inputs(
        names: &[&str],
        location: &str,
        alias: &str,
        request_declared: bool,
    ) -> (BidRequest, BTreeMap<String, String>) {
        let mut req = request_with_param_location(names, location);
        let host = if request_declared {
            req.ext = request_declaring(&[(alias, "fake_bb")], &[]).ext;
            // A dropped request alias must not revive the host mapping either.
            host_aliases(&[(alias, "fake_aaa")])
        } else {
            host_aliases(&[(alias, "fake_bb")])
        };
        (req, host)
    }

    fn assert_disabled_alias_does_not_route(
        registry: &BidderRegistry,
        disabled: &BTreeSet<BidderName>,
        alias: &str,
        request_declared: bool,
    ) {
        for location in ["prebid", "hoisted", "legacy"] {
            let (req, host) =
                alias_routing_inputs(&[alias, "fake_aaa"], location, alias, request_declared);
            let resolved = resolve_aliases(&req, registry, disabled, &host).unwrap();
            assert!(!resolved.map.bases.contains_key(&BidderName::new(alias)));
            assert_eq!(
                resolved.dropped_alias_names,
                BTreeSet::from([BidderName::new(alias)])
            );
            assert_eq!(resolved.disabled_warnings.len(), 1);
            assert_eq!(resolved.disabled_warnings[0].0.as_str(), alias);
            assert_eq!(resolved.disabled_warnings[0].1.code, 6);

            let split = split_by_bidder(
                &req,
                &resolved.map,
                registry,
                disabled,
                &resolved.dropped_alias_names,
            )
            .expect("the enabled sibling must still participate");
            let landed: Vec<_> = split
                .per_bidder
                .iter()
                .map(|pbr| (pbr.bidder.as_str(), pbr.base.as_str()))
                .collect();
            assert_eq!(
                landed,
                vec![("fake_aaa", "fake_aaa")],
                "disabled {alias} must not route: {location}, request_declared={request_declared}"
            );
            assert!(
                split.disabled_bidder_warnings.is_empty(),
                "the resolver's alias warning must not be duplicated by the split"
            );
            let scoped = &split.per_bidder[0];
            let ext = scoped.request.imp[0].ext.as_ref().unwrap();
            assert!(!ext.other.contains_key(alias));
            assert!(
                ext.prebid
                    .as_ref()
                    .is_none_or(|prebid| prebid.bidder.is_empty())
            );

            let (only_alias, _) = alias_routing_inputs(&[alias], location, alias, request_declared);
            assert!(
                matches!(
                    split_by_bidder(
                        &only_alias,
                        &resolved.map,
                        registry,
                        disabled,
                        &resolved.dropped_alias_names,
                    ),
                    Err(AuctionError::BadInput(message)) if message.contains("no valid bidders")
                ),
                "a disabled alias alone cannot satisfy imp participation: {location}"
            );
        }
    }

    #[test]
    fn disabled_alias_base_cannot_fall_back_to_compiled_identity() {
        let registry =
            alias_registry(&[("fake_bb", false), ("fake_ccc", false), ("fake_aaa", false)]);
        let disabled = BTreeSet::from([BidderName::new("fake_bb")]);
        for request_declared in [true, false] {
            for alias in ["fake_ccc", "fake_bz"] {
                assert_disabled_alias_does_not_route(&registry, &disabled, alias, request_declared);
            }
        }
    }

    #[test]
    fn disabled_alias_own_name_cannot_participate() {
        let registry =
            alias_registry(&[("fake_bb", false), ("fake_ccc", false), ("fake_aaa", false)]);
        for request_declared in [true, false] {
            for alias in ["fake_ccc", "fake_bz"] {
                let disabled = BTreeSet::from([BidderName::new(alias)]);
                assert_disabled_alias_does_not_route(&registry, &disabled, alias, request_declared);
            }
        }
    }

    #[test]
    fn baked_disabled_host_alias_base_cannot_fall_back_to_compiled_identity() {
        let registry =
            alias_registry(&[("fake_bb", true), ("fake_ccc", false), ("fake_aaa", false)]);
        for alias in ["fake_ccc", "fake_bz"] {
            assert_disabled_alias_does_not_route(&registry, &BTreeSet::new(), alias, false);
        }
    }

    #[test]
    fn enabled_aliases_route_to_declared_base_in_all_param_locations() {
        let registry =
            alias_registry(&[("fake_bb", false), ("fake_ccc", false), ("fake_aaa", false)]);
        for request_declared in [true, false] {
            for alias in ["fake_ccc", "fake_bz"] {
                for location in ["prebid", "hoisted", "legacy"] {
                    let (req, host) = alias_routing_inputs(
                        &[alias, "fake_aaa"],
                        location,
                        alias,
                        request_declared,
                    );
                    let resolved =
                        resolve_aliases(&req, &registry, &BTreeSet::new(), &host).unwrap();
                    let split = split_by_bidder(
                        &req,
                        &resolved.map,
                        &registry,
                        &BTreeSet::new(),
                        &resolved.dropped_alias_names,
                    )
                    .unwrap();
                    let landed: Vec<_> = split
                        .per_bidder
                        .iter()
                        .map(|pbr| (pbr.bidder.as_str(), pbr.base.as_str()))
                        .collect();
                    assert_eq!(landed, vec![("fake_aaa", "fake_aaa"), (alias, "fake_bb")]);
                    assert!(resolved.disabled_warnings.is_empty());
                    assert!(split.disabled_bidder_warnings.is_empty());
                }
            }
        }
    }

    #[test]
    fn compiled_bidders_without_aliases_participate_in_all_param_locations() {
        let registry = two_bidder_registry();
        for location in ["prebid", "hoisted", "legacy"] {
            let req = request_with_param_location(&["fake_bb", "fake_ccc"], location);
            let split = split_by_bidder(
                &req,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
            let landed: Vec<_> = split
                .per_bidder
                .iter()
                .map(|pbr| (pbr.bidder.as_str(), pbr.base.as_str()))
                .collect();
            assert_eq!(
                landed,
                vec![("fake_bb", "fake_bb"), ("fake_ccc", "fake_ccc")]
            );
            assert!(split.disabled_bidder_warnings.is_empty());
        }
    }

    #[test]
    fn host_alias_only_resolves_to_base() {
        let registry = alias_registry(&[("fake_aaa", false)]);
        let resolved = resolve_aliases(
            &request_without_prebid_ext(),
            &registry,
            &BTreeSet::new(),
            &host_aliases(&[("fake_host_alias", "fake_aaa")]),
        )
        .expect("a host-declared alias with a live base resolves");

        assert_eq!(
            resolved.map.bases.get(&BidderName::new("fake_host_alias")),
            Some(&BidderName::new("fake_aaa")),
            "a host `[aliases]` row must resolve even when the request declares no ext",
        );
        assert!(resolved.disabled_warnings.is_empty());
        assert!(resolved.dropped_alias_names.is_empty());
    }

    #[test]
    fn request_alias_only_resolves_to_base() {
        let registry = alias_registry(&[("fake_aaa", false)]);
        let resolved = resolve_aliases(
            &request_declaring(&[("fake_req_alias", "fake_aaa")], &[]),
            &registry,
            &BTreeSet::new(),
            &no_host_aliases(),
        )
        .expect("a request-declared alias with a live base resolves");

        assert_eq!(
            resolved.map.bases.get(&BidderName::new("fake_req_alias")),
            Some(&BidderName::new("fake_aaa")),
        );
        assert!(resolved.disabled_warnings.is_empty());
    }

    #[test]
    fn request_alias_wins_over_host_alias_with_same_key() {
        let registry = alias_registry(&[("fake_aaa", false), ("fake_ccc", false)]);
        let host = host_aliases(&[("fake_shared", "fake_aaa")]);

        let resolved = resolve_aliases(
            &request_declaring(&[("fake_shared", "fake_ccc")], &[]),
            &registry,
            &BTreeSet::new(),
            &host,
        )
        .expect("both layers name a live base");
        assert_eq!(
            resolved.map.bases.get(&BidderName::new("fake_shared")),
            Some(&BidderName::new("fake_ccc")),
            "the request's declaration must win over the host's for the same key",
        );

        // The harder half of "request wins": when the REQUEST's
        // declaration is dropped (its base is overlay-disabled), the
        // host's mapping for that key must not survive in its place —
        // the publisher spoke for `shared`, and its answer was "no".
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_ccc"));
        let shadowed = resolve_aliases(
            &request_declaring(&[("fake_shared", "fake_ccc")], &[]),
            &registry,
            &disabled,
            &host,
        )
        .expect("a disabled base drops the alias rather than failing the request");
        assert!(
            !shadowed
                .map
                .bases
                .contains_key(&BidderName::new("fake_shared")),
            "a dropped request alias must not fall back to the host's mapping, got {:?}",
            shadowed.map.bases,
        );
        assert_eq!(shadowed.disabled_warnings.len(), 1);
    }

    #[test]
    fn unknown_alias_reports_unknown_bidder() {
        // The request-declared counterpart to
        // `runtime_host_alias_with_unknown_base_is_dropped_silently_no_code`:
        // the request author controls this text, so it is a 400.
        let registry = alias_registry(&[("fake_aaa", false)]);
        let err = resolve_aliases(
            &request_declaring(&[("fake_req_alias", "fake_nope")], &[]),
            &registry,
            &BTreeSet::new(),
            &no_host_aliases(),
        )
        .expect_err("a request alias naming an unknown base is bad client input");
        assert!(
            matches!(&err, AuctionError::BadInput(msg) if msg.contains("unknown bidder fake_nope")),
            "expected an unknown-bidder BadInput, got {err:?}",
        );
    }

    #[test]
    fn host_alias_with_no_imp_targeting_does_not_participate() {
        let registry = alias_registry(&[("fake_aaa", false)]);
        let host = host_aliases(&[("fake_host_alias", "fake_aaa")]);

        // Resolution puts the alias in the map either way; only the imp
        // decides whether it actually runs.
        let resolved = resolve_aliases(
            &request_without_prebid_ext(),
            &registry,
            &BTreeSet::new(),
            &host,
        )
        .unwrap();
        assert!(
            resolved
                .map
                .bases
                .contains_key(&BidderName::new("fake_host_alias")),
            "the alias must be available to be targeted",
        );

        let untargeted = split_by_bidder(
            &overlay_test_request(&["fake_aaa"]),
            &resolved.map,
            &registry,
            &BTreeSet::new(),
            &resolved.dropped_alias_names,
        )
        .unwrap();
        let landed: Vec<&str> = untargeted
            .per_bidder
            .iter()
            .map(|group| group.bidder.as_str())
            .collect();
        assert_eq!(
            landed,
            vec!["fake_aaa"],
            "a host alias no imp names must not fan out on its own",
        );

        // Control: the same alias DOES participate once an imp names it,
        // so the assertion above is about targeting, not about the alias
        // being unusable.
        let targeted = split_by_bidder(
            &overlay_test_request(&["fake_host_alias"]),
            &resolved.map,
            &registry,
            &BTreeSet::new(),
            &resolved.dropped_alias_names,
        )
        .unwrap();
        let targeted_landed: Vec<&str> = targeted
            .per_bidder
            .iter()
            .map(|group| group.bidder.as_str())
            .collect();
        assert_eq!(targeted_landed, vec!["fake_host_alias"]);
    }

    #[test]
    fn host_alias_of_disabled_base_dropped_with_code_6_under_alias_name() {
        // A disabled base is "bidder turned off", not a config typo, so
        // it stays observable under the ALIAS's own name — the same
        // shape the request-alias path emits.
        let registry = alias_registry(&[("fake_aaa", false)]);
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_aaa"));

        let resolved = resolve_aliases(
            &request_without_prebid_ext(),
            &registry,
            &disabled,
            &host_aliases(&[("fake_host_alias", "fake_aaa")]),
        )
        .expect("a disabled base drops the alias rather than failing the request");

        assert!(
            !resolved
                .map
                .bases
                .contains_key(&BidderName::new("fake_host_alias")),
            "an alias whose base the overlay disabled must not resolve",
        );
        assert_eq!(resolved.disabled_warnings.len(), 1);
        let (name, warning) = &resolved.disabled_warnings[0];
        assert_eq!(
            name.as_str(),
            "fake_host_alias",
            "the warning must key on the alias's own observable name, not its base",
        );
        assert_eq!(warning.code, 6, "BidderTemporarilyDisabled is code 6");
        assert!(
            warning.message.contains("fake_host_alias"),
            "expected the code-6 message to name the alias, got {}",
            warning.message,
        );
        assert!(
            resolved
                .dropped_alias_names
                .contains(&BidderName::new("fake_host_alias")),
            "a dropped host alias is still a bidder identity downstream",
        );

        // The BAKED `BidderInfo.disabled` bit is the other half of the
        // same branch. On the request path it is a 400; on the host path
        // it must degrade to the identical code-6 drop.
        let baked_off = alias_registry(&[("fake_aaa", true)]);
        let baked_off_resolved = resolve_aliases(
            &request_without_prebid_ext(),
            &baked_off,
            &BTreeSet::new(),
            &host_aliases(&[("fake_host_alias", "fake_aaa")]),
        )
        .expect("a baked-disabled base must not 400 the auction on the host path");
        assert!(
            !baked_off_resolved
                .map
                .bases
                .contains_key(&BidderName::new("fake_host_alias"))
        );
        assert_eq!(baked_off_resolved.disabled_warnings.len(), 1);
        assert_eq!(baked_off_resolved.disabled_warnings[0].1.code, 6);
    }

    #[test]
    fn alias_whose_own_name_is_in_disabled_set_dropped_with_code_6_under_alias_name() {
        // The overlay names the ALIASES themselves, not their shared
        // base — and it has to reach an alias from either layer.
        let registry = alias_registry(&[("fake_aaa", false)]);
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_req_alias"));
        disabled.insert(BidderName::new("fake_host_alias"));

        let resolved = resolve_aliases(
            &request_declaring(&[("fake_req_alias", "fake_aaa")], &[]),
            &registry,
            &disabled,
            &host_aliases(&[("fake_host_alias", "fake_aaa")]),
        )
        .expect("an alias disabled by its own name is dropped, not an error");

        assert!(
            resolved.map.bases.is_empty(),
            "both aliases are disabled by name, so nothing resolves, got {:?}",
            resolved.map.bases,
        );
        let mut warned: Vec<&str> = resolved
            .disabled_warnings
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        warned.sort_unstable();
        assert_eq!(warned, vec!["fake_host_alias", "fake_req_alias"]);
        assert!(
            resolved
                .disabled_warnings
                .iter()
                .all(|(_, warning)| warning.code == 6),
            "both drops must carry BidderTemporarilyDisabled",
        );
        // The base itself was never disabled — only the alias names.
        assert!(!disabled.contains(&BidderName::new("fake_aaa")));
    }

    /// Named for what it proves: that the resolver ACCEPTS an
    /// `aliasgvlids` entry naming a host-declared alias and keys the
    /// override under the alias name. It does NOT drive a permission
    /// walk, and must not be read as covering one.
    ///
    /// The permission half genuinely needs no new coverage here.
    /// `PrivacyContext` builds its `alias_gvl_ids` cache straight from
    /// `ext.prebid.aliasgvlids` (`auctioneer-privacy/src/resolve.rs`, the
    /// `alias_gvl_ids(req)` helper) and never receives the `AliasMap`, so
    /// it cannot tell a host-declared alias from a request-declared one —
    /// this task changes nothing it sees. What it does with an override
    /// is pinned where the GVL / TC-string machinery lives, in
    /// `auctioneer-privacy/src/auction_context.rs`:
    /// `alias_gvl_id_override_with_vendor_consent_yields_allow_all` and
    /// `alias_without_gvl_id_entry_inherits_core_vendor_id`.
    ///
    /// What this task DID change is the guard one layer up: before host
    /// aliases reached the resolver, an `aliasgvlids` entry naming a
    /// host-only alias hit the unknown-alias branch and 400'd the whole
    /// auction. That is the breakable behaviour, and it is what this
    /// test holds.
    #[test]
    fn aliasgvlids_may_target_a_host_declared_alias() {
        // The request declares NO alias of its own; `host-alias-x` comes
        // from the host config, and the request only pins its vendor id.
        let registry = alias_registry(&[("fake_ccc", false)]);
        let resolved = resolve_aliases(
            &request_declaring(&[], &[("fake_host_alias_x", 42)]),
            &registry,
            &BTreeSet::new(),
            &host_aliases(&[("fake_host_alias_x", "fake_ccc")]),
        )
        .expect("aliasgvlids may name a host-declared alias");

        assert_eq!(
            resolved
                .map
                .bases
                .get(&BidderName::new("fake_host_alias_x")),
            Some(&BidderName::new("fake_ccc")),
        );
        assert_eq!(
            resolved
                .map
                .gvlids
                .get(&BidderName::new("fake_host_alias_x")),
            Some(&42_u32),
            "the override must survive resolution keyed under the ALIAS name, which is what \
             the privacy resolver later looks it up by",
        );
    }

    #[test]
    fn runtime_host_alias_with_unknown_base_is_dropped_silently_no_code() {
        testing_logger::setup();
        let registry = alias_registry(&[("fake_aaa", false)]);
        let resolved = resolve_aliases(
            &request_without_prebid_ext(),
            &registry,
            &BTreeSet::new(),
            &host_aliases(&[
                ("fake_typo_alias", "fake_appnexuz"),
                ("fake_good_alias", "fake_aaa"),
            ]),
        )
        .expect("an operator typo in host config must not 400 every auction");

        assert!(
            !resolved
                .map
                .bases
                .contains_key(&BidderName::new("fake_typo_alias")),
            "the unresolvable alias must be absent from the map",
        );
        assert_eq!(
            resolved.map.bases.get(&BidderName::new("fake_good_alias")),
            Some(&BidderName::new("fake_aaa")),
            "a sibling alias with a live base must still resolve",
        );
        assert!(
            resolved.disabled_warnings.is_empty(),
            "an unknown base is a config typo, not a disabled bidder: no code-6",
        );
        assert!(
            resolved.dropped_alias_names.is_empty(),
            "the alias never became a bidder identity, so nothing downstream tracks it",
        );

        // Silent to the publisher, not to the operator.
        testing_logger::validate(|captured| {
            assert!(
                captured.iter().any(|entry| entry.level == log::Level::Warn
                    && entry.body.contains("fake_typo_alias")
                    && entry.body.contains("fake_appnexuz")),
                "the drop must leave a WARN naming the alias and its bad base, got {:?}",
                captured.iter().map(|e| &e.body).collect::<Vec<_>>(),
            );
        });
    }
}
