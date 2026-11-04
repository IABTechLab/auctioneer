//! Boot retains the same normalized caps and charges the handler will enforce.
//! The sole installer test owns this process; catalog controls boot in children.

#[cfg(test)]
mod fanout_limits;
use fanout_limits as envelope;
mod support;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod tests {
    use super::{envelope, serving_catalog, support};
    use auctioneer_core::handlers::auction::TestAuctionBudget;
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        BootEnvelope, BootGates, DelegateToDevStrategy, StderrBootLog, SystemClock,
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle, boot_gates,
        bootstrap_adapter_with_envelope, resolve_budget_inputs,
    };
    use auctioneer_runtime::budget::{CONFIG_PARSE_FACTOR, derive_isolate_budget};
    use auctioneer_runtime::{
        AuctionBudget, Clock, FakeClock, TargetPlatformConstants, serialized_json_bytes,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use serde_json::Value;
    use std::env::{current_exe, var};
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::thread::{sleep, spawn};
    use std::time::{Duration, Instant};

    struct NeverFanout;

    fn config() -> String {
        format!(
            "schema_version = 1\n[host.aliases]\nfake_bb={:?}\n[host.gdpr]\nenabled=false\n",
            serving_catalog::passthrough_info().name,
        )
    }

    #[async_trait::async_trait(?Send)]
    impl auctioneer_exchange::BidderFanout for NeverFanout {
        async fn send_all(
            &self,
            _: Vec<auctioneer_exchange::BidderCall>,
            _: auctioneer_exchange::AuctionDeadline,
        ) -> Result<
            Vec<auctioneer_exchange::BidderCallResult>,
            auctioneer_exchange::FanoutInternalError,
        > {
            panic!("request exceeded the boot envelope before fanout")
        }
    }

    struct CapturingFanout(Mutex<Vec<Value>>, Arc<AuctionBudget>);

    #[async_trait::async_trait(?Send)]
    impl auctioneer_exchange::BidderFanout for CapturingFanout {
        async fn send_all(
            &self,
            calls: Vec<auctioneer_exchange::BidderCall>,
            _: auctioneer_exchange::AuctionDeadline,
        ) -> Result<
            Vec<auctioneer_exchange::BidderCallResult>,
            auctioneer_exchange::FanoutInternalError,
        > {
            assert_eq!(
                self.1.in_use(),
                self.1.limit(),
                "all installed auction phases remain prepaid during fanout"
            );
            let mut captured = self.0.lock().unwrap();
            Ok(calls
                .iter()
                .map(|call| {
                    captured.push(serde_json::from_slice(call.request.body()).unwrap());
                    auctioneer_exchange::BidderCallResult {
                        result: Ok(auctioneer_bidder::BidderHttpResponse::new(
                            204,
                            vec![],
                            Vec::new(),
                        )),
                        elapsed: Duration::ZERO,
                    }
                })
                .collect())
        }
    }

    fn catalog_costs_for_process() -> auctioneer_runtime::CatalogResourceCosts {
        const CASE: &str = "AUCTIONEER__TEST__CATALOG_COSTS";
        let unchanged = auctioneer_runtime::CatalogResourceCosts::UNCHANGED;
        let cases = [
            (
                "resident",
                auctioneer_runtime::CatalogResourceCosts {
                    resident_bytes: 1009,
                    ..unchanged
                },
            ),
            (
                "initialization",
                auctioneer_runtime::CatalogResourceCosts {
                    initialization_bytes: 2017,
                    ..unchanged
                },
            ),
            (
                "schema",
                auctioneer_runtime::CatalogResourceCosts {
                    schema_validation_per_auction_bytes: 3011,
                    ..unchanged
                },
            ),
            (
                "context",
                auctioneer_runtime::CatalogResourceCosts {
                    context_per_auction_bytes: 4001,
                    ..unchanged
                },
            ),
            (
                "combined",
                auctioneer_runtime::CatalogResourceCosts {
                    resident_bytes: 1009,
                    initialization_bytes: 2017,
                    schema_validation_per_auction_bytes: 3011,
                    context_per_auction_bytes: 4001,
                },
            ),
        ];
        if let Ok(name) = var(CASE) {
            cases.iter().find(|(case, _)| *case == name).unwrap().1
        } else {
            for (name, _) in cases {
                run_catalog_child(CASE, name);
            }
            unchanged
        }
    }

    fn run_catalog_child(case_env: &str, name: &str) {
        const CAPTURE_LIMIT: u64 = 0x8000;
        let mut child = Command::new(current_exe().unwrap())
            .env_clear()
            .env(case_env, name)
            .args([
                "--exact",
                "tests::defaulted_config_caps_and_baked_clones_are_charged_before_requests",
                "--nocapture",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout_pipe = child.stdout.take().unwrap();
        let mut stderr_pipe = child.stderr.take().unwrap();
        let capture = |pipe: &mut dyn Read| {
            let mut bytes = Vec::new();
            pipe.take(CAPTURE_LIMIT + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        };
        let stdout_reader = spawn(move || capture(&mut stdout_pipe));
        let stderr_reader = spawn(move || capture(&mut stderr_pipe));
        let deadline = Instant::now().checked_add(Duration::from_secs(30)).unwrap();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                break None;
            }
            sleep(Duration::from_millis(10));
        };
        let stdout = stdout_reader.join().unwrap();
        let stderr = stderr_reader.join().unwrap();
        assert!(
            u64::try_from(stdout.len()).unwrap() <= CAPTURE_LIMIT,
            "{name}: stdout limit"
        );
        assert!(
            u64::try_from(stderr.len()).unwrap() <= CAPTURE_LIMIT,
            "{name}: stderr limit"
        );
        assert!(status.is_some(), "{name}: catalog boot exceeded 30 seconds");
        assert!(
            status.unwrap().success(),
            "{name}: {}\n{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
        assert!(String::from_utf8_lossy(&stdout).contains("1 passed"));
    }

    #[test]
    fn defaulted_config_caps_and_baked_clones_are_charged_before_requests() {
        let _info = serving_catalog::passthrough_info();
        let catalog_costs = catalog_costs_for_process();
        let mut limits = envelope::limits();
        limits.max_concurrent_auctions = 3;
        limits.max_active_body_streams = 7;
        limits.max_runtime_config_bytes = 0;
        limits.max_runtime_config_blob_bytes = 0;
        // The framework's zero/default config caps are larger than the
        // production 1 MiB profile; fund their conservative aux bound too.
        limits.aux_route_peak_bytes = 7 * 1_024 * 1_024 * 1_024;
        limits.inbound_request_bytes = 1024;
        limits.max_impressions_per_auction = 1;
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576_u32,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        let metadata = auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction);
        let construction_costs = core_construction_costs();
        assert!(boot_gates().is_none());
        let resolved = resolve_budget_inputs(&limits).unwrap();
        assert!(boot_gates().is_none());
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                limits: &limits,
                target: &target,
                outbound_meta_bytes: metadata,
                construction_costs,
                log_sink: &StderrBootLog,
                catalog_costs,
            },
            || Arc::new(SystemClock::new()),
            || Arc::new(auctioneer_runtime::SeededRng::new(42)),
            || Arc::new(auctioneer_runtime::ImmediateResolveTimer),
            || Arc::new(auctioneer_runtime::NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .unwrap();
        let gates = boot_gates().unwrap();
        assert_eq!(gates.catalog_costs, catalog_costs);
        assert_eq!(gates.rederive_budget().unwrap(), gates.derived);
        assert_catalog_funding(gates);
        assert_eq!(gates.limits, resolved.limits);
        assert_eq!(gates.baked_config_bytes, resolved.baked_config_bytes);
        assert_installed_config_funding(gates);
        assert_oversized_requests_are_refused();
        assert_small_body_header_enrichment_is_funded();
    }

    fn assert_installed_config_funding(gates: &BootGates) {
        let app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap();
        let caps = app.config_extraction_limits();
        assert!(caps.max_backend_bytes > 0);
        assert!(caps.max_blob_bytes > 0);
        assert_eq!(
            gates.limits.max_runtime_config_bytes,
            caps.max_backend_bytes
        );
        assert_eq!(
            gates.limits.max_runtime_config_blob_bytes,
            caps.max_blob_bytes
        );
        assert_eq!(gates.construction_costs, core_construction_costs());
        assert_eq!(gates.derived.construction_costs, gates.construction_costs);
        let bare = derive_isolate_budget(
            &gates.limits,
            &gates.target,
            gates.outbound_meta_bytes,
            gates.construction_costs,
            gates.catalog_costs,
        )
        .unwrap();
        let baked = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        let aux_inputs = gates.aux_work_derived.limits();
        assert_eq!(aux_inputs.config_envelope_bytes, caps.max_backend_bytes);
        assert_eq!(aux_inputs.config_blob_bytes, caps.max_blob_bytes);
        assert_eq!(aux_inputs.baked_config_bytes, baked);
        assert_eq!(
            gates.aux_work_budget.limit(),
            gates
                .aux_work_derived
                .peak_bytes()
                .checked_mul(u64::from(gates.limits.aux_route_permits))
                .unwrap()
        );
        assert_eq!(
            gates.derived.auction_charges.config,
            bare.auction_charges
                .config
                .checked_add(baked.checked_mul(CONFIG_PARSE_FACTOR).unwrap())
                .unwrap()
        );
        assert_eq!(
            gates.derived.auction_charges.total().unwrap(),
            gates.derived.per_auction_peak
        );
    }

    fn assert_oversized_requests_are_refused() {
        let imp = serde_json::json!({"id":"1","banner":{"w":300_i32,"h":250_i32},
            "ext":{"bidder":{"fake_bb":{}}}});
        let mut second = imp.clone();
        second["id"] = serde_json::json!("2");
        let oversized_count = serde_json::to_vec(&serde_json::json!({"id":"r", "site":{"id":"s"},
            "imp":[imp,second]}))
        .unwrap();
        for body in [vec![b' '; 1025], oversized_count] {
            let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
            let mut request = http::Request::builder()
                .method("POST")
                .uri("/openrtb2/auction")
                .body(Body::from_bytes(body))
                .unwrap();
            request
                .extensions_mut()
                .insert(TestFanoutHandle(Arc::new(NeverFanout)));
            request.extensions_mut().insert(TestClockHandle(clock));
            request.extensions_mut().insert(TestRngHandle(Arc::new(
                auctioneer_runtime::SeededRng::new(42),
            )));
            let ctx = support::ctx_with_config(&config(), request);
            let response = block_on(auction(ctx)).unwrap();
            assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
        }
    }

    fn assert_catalog_funding(gates: &BootGates) {
        let costs = gates.catalog_costs;
        let derive = |limits: &auctioneer_runtime::ResolvedIsolateLimits, baked, catalog| {
            auctioneer_runtime::derive_isolate_budget_with_baked_config(
                limits,
                &gates.target,
                gates.outbound_meta_bytes,
                gates.construction_costs,
                baked,
                catalog,
            )
            .unwrap()
        };
        let per_auction = costs
            .schema_validation_per_auction_bytes
            .checked_add(costs.context_per_auction_bytes)
            .unwrap();
        let delta = |funded: u64, base: u64| funded.checked_sub(base).unwrap();
        assert_eq!(
            gates.derived,
            derive(&gates.limits, gates.baked_config_bytes, costs)
        );
        let mut more_aux = gates.limits;
        more_aux.aux_route_permits = 11;
        let mut larger_default = auctioneer_config_data::host_default().clone();
        larger_default.resource_limits.max_bidder_response_bytes = usize::MAX;
        let larger_tuple_bytes =
            serialized_json_bytes(&(larger_default, auctioneer_config_data::account_default()))
                .unwrap();
        assert_ne!(larger_tuple_bytes, gates.baked_config_bytes);
        // Catalog residency and initialization are not config parse work or aux work.
        for (limits, baked) in [
            (&gates.limits, gates.baked_config_bytes),
            (&more_aux, gates.baked_config_bytes),
            (&gates.limits, larger_tuple_bytes),
        ] {
            let base = derive(
                limits,
                baked,
                auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            );
            let funded = derive(limits, baked, costs);
            assert_eq!(funded.catalog_resident_term, costs.resident_bytes);
            assert_eq!(
                funded.catalog_initialization_term,
                costs.initialization_bytes
            );
            assert_eq!(
                funded.schema_validation_term,
                costs.schema_validation_per_auction_bytes
            );
            assert_eq!(funded.catalog_context_term, costs.context_per_auction_bytes);
            assert_eq!(
                delta(funded.request_side_term, base.request_side_term),
                per_auction
            );
            assert_eq!(
                delta(
                    funded.auction_charges.projection,
                    base.auction_charges.projection
                ),
                per_auction
            );
            assert_eq!(
                delta(funded.per_auction_peak, base.per_auction_peak),
                per_auction
            );
            let mut expected_charges = base.auction_charges;
            expected_charges.projection = expected_charges
                .projection
                .checked_add(per_auction)
                .unwrap();
            assert_eq!(funded.auction_charges, expected_charges);
            assert_eq!(
                delta(funded.total_bytes, base.total_bytes),
                costs
                    .resident_bytes
                    .checked_add(costs.initialization_bytes)
                    .unwrap()
                    .checked_add(
                        u64::from(limits.max_concurrent_auctions)
                            .checked_mul(per_auction)
                            .unwrap()
                    )
                    .unwrap()
            );
            assert_eq!(
                funded.auction_charges.total().unwrap(),
                funded.per_auction_peak
            );
        }
        let budget = AuctionBudget::new(gates.derived.per_auction_peak);
        let receipts = gates.derived.auction_charges.prepay(&budget).unwrap();
        assert_eq!(receipts.len(), 6);
        assert_eq!(budget.in_use(), gates.derived.per_auction_peak);
        drop(receipts);
        assert_eq!(budget.in_use(), 0);
    }

    fn assert_small_body_header_enrichment_is_funded() {
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "normalized-source", "tmax": 1000_i32, "site": {"id": "s"},
            "imp": [{"id": "1", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"prebid": {"bidder": {"fake_bb": {}}}}}]
        }))
        .unwrap();
        assert!(body.len() < 1024);
        for user_agent in [String::new(), "a".repeat(2048)] {
            let budget = Arc::new(AuctionBudget::new(
                boot_gates().unwrap().derived.per_auction_peak,
            ));
            let fanout = Arc::new(CapturingFanout(Mutex::new(Vec::new()), Arc::clone(&budget)));
            let fake = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
            let mut request = http::Request::builder()
                .method("POST")
                .uri("/openrtb2/auction")
                .header("user-agent", &user_agent)
                .body(Body::from_bytes(body.clone()))
                .unwrap();
            request
                .extensions_mut()
                .insert(TestFanoutHandle(Arc::<CapturingFanout>::clone(&fanout)));
            request
                .extensions_mut()
                .insert(TestAuctionBudget(Arc::clone(&budget)));
            request
                .extensions_mut()
                .insert(TestClockHandle(Arc::<FakeClock>::clone(&fake)));
            request
                .extensions_mut()
                .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                    Clock::now_monotonic(fake.as_ref())
                })));
            request.extensions_mut().insert(TestRngHandle(Arc::new(
                auctioneer_runtime::SeededRng::new(42),
            )));
            let response = block_on(auction(support::ctx_with_config(&config(), request))).unwrap();
            assert_eq!(response.status(), http::StatusCode::OK);
            assert_eq!(budget.in_use(), 0);
            let captured = fanout.0.lock().unwrap();
            assert_eq!(captured.len(), 1, "the admitted survivor is dispatched");
            if !user_agent.is_empty() {
                assert_eq!(captured[0]["device"]["ua"], user_agent);
            }
        }
    }
}
