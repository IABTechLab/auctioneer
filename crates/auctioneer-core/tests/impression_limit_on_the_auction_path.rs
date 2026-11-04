//! A boot envelope below the historical 100-impression handler cap must
//! constrain the real ingress path. Node-budget denial still precedes
//! account resolution; a disabled account still precedes prepare validation.

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod tests {
    use crate::serving_catalog::{ServingBidder, default_active_publications};
    use async_trait::async_trait;
    use auctioneer_bidder::{BidderHttpResponse, outbound_meta_bytes};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::handlers::TestFanoutHandle;
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, SystemClock, TestClockHandle,
        TestMonotonicClockHandle, TestRngHandle, boot_limits, bootstrap_adapter_with_envelope,
    };
    use auctioneer_core::test_support::config_store::runtime_registry;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::budget::{ASSEMBLY_FACTOR, derive_aux_work_for_profile};
    use auctioneer_runtime::{
        BoundedJsonError, Clock, FakeClock, ImmediateResolveTimer, MAX_GVL_PARSED_BYTES,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        NoopDecodeCooperator, ResolvedIsolateLimits, Rng, SeededRng, TargetPlatformConstants,
        bounded_json, serialized_json_bytes,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::http::{Method, StatusCode, request_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::store_registry::ConfigRegistry;
    use edgezero_core::time::{MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use time::OffsetDateTime;

    const BODY_CAP: u64 = 1 << 18;
    const CONFIG_CAP: u64 = 1 << 20;
    const NODE_CAP: u32 = 2_048;
    const FUNDED_IMPRESSIONS: u16 = 8;
    const DISABLED_ACCOUNT: &str = "acct-disabled";

    fn limits() -> ResolvedIsolateLimits {
        let mut row = ResolvedIsolateLimits {
            isolate_total_bytes: 1_600_000_000,
            baseline_bytes: 96_000_000,
            inbound_request_bytes: BODY_CAP,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: BODY_CAP,
            max_outbound_raw_bytes: BODY_CAP,
            max_transport_bytes: 1_024,
            max_decoded_bytes: 1_024,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 1_024 * ASSEMBLY_FACTOR,
            egress_permits: 1,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 1,
            aux_route_peak_bytes: 0,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 1,
            max_outbound_requests: 1,
            max_concurrent_auctions: 1,
            max_active_body_streams: 1,
            max_runtime_config_bytes: CONFIG_CAP,
            max_runtime_config_blob_bytes: CONFIG_CAP,
            max_parsed_nodes: NODE_CAP,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 1 << 14,
            max_admitted_header_count: 128,
            max_impressions_per_auction: FUNDED_IMPRESSIONS,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 2,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        };
        let baked_config_bytes = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        row.aux_route_peak_bytes = derive_aux_work_for_profile(&row, &target(), baked_config_bytes)
            .unwrap()
            .peak_bytes();
        row
    }

    fn target() -> TargetPlatformConstants {
        TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: 1 << 14,
            egress_handoff_factor: 2,
        }
    }

    fn config_registry(config: &AuctioneerAppConfig) -> ConfigRegistry {
        runtime_registry(config)
    }

    struct CountingFanout {
        calls: Arc<AtomicUsize>,
        expected_impressions: usize,
    }

    #[async_trait(?Send)]
    impl BidderFanout for CountingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.calls.fetch_add(calls.len(), Ordering::Relaxed);
            for call in &calls {
                let request: BidRequest = serde_json::from_slice(call.request.body()).unwrap();
                assert_eq!(request.imp.len(), self.expected_impressions);
                assert_eq!(
                    call.request.covered_imp_ids().count(),
                    self.expected_impressions
                );
            }
            Ok(calls
                .iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], Vec::new())),
                    elapsed: Duration::from_millis(1),
                })
                .collect())
        }
    }

    fn auction_bidder() -> ServingBidder {
        default_active_publications()
            .into_iter()
            .find(|row| {
                let info = row.info;
                !info.disabled
                    && info.uses_https
                    && !info.endpoint.contains("{{")
                    && info.endpoint_compression.is_none()
                    && info.capabilities.site.contains(&MediaType::Banner)
                    && row.owner_request["imp"].as_array().is_some_and(|imps| {
                        !imps.is_empty()
                            && row
                                .owner_expected_request_body
                                .as_ref()
                                .is_some_and(|body| {
                                    body["imp"].as_array().map(Vec::len) == Some(imps.len())
                                })
                    })
            })
            .expect("impression-boundary coverage requires a fixed-endpoint banner bidder")
    }

    fn request_body(account: &str, impressions: u16, padding: usize) -> String {
        let bidder = auction_bidder();
        let imps = (0..impressions)
            .map(|index| {
                serde_json::json!({
                    "id": format!("imp-{index}"),
                    "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"prebid": {"bidder": {bidder.name(): &bidder.params}}}
                })
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&serde_json::json!({
            "id": "impression-limit",
            "tmax": 1_000_i32,
            "cur": ["USD"],
            "site": {"publisher": {"id": account}},
            "imp": imps,
            "ext": {"padding": vec![0_u8; padding]}
        }))
        .unwrap()
    }

    fn answer(
        app: &App,
        config: &AuctioneerAppConfig,
        body: &str,
        impressions: u16,
    ) -> (StatusCode, String, usize) {
        assert!(u64::try_from(body.len()).unwrap() < BODY_CAP);
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let calls = Arc::new(AtomicUsize::new(0));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(CountingFanout {
            calls: Arc::clone(&calls),
            expected_impressions: usize::from(impressions),
        });
        let mut request = request_builder()
            .method(Method::POST)
            .uri("/openrtb2/auction")
            .header("content-type", "application/json")
            .body(Body::from_bytes(body.as_bytes().to_vec()))
            .unwrap();
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        request
            .extensions_mut()
            .insert(TestClockHandle(handler_clock));
        let monotonic_source = Arc::<FakeClock>::clone(&clock);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                monotonic_source.now_monotonic()
            })));
        request.extensions_mut().insert(TestRngHandle(rng));
        request.extensions_mut().insert(config_registry(config));
        let outcome = block_on(app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .unwrap();
        let IngressDispatchOutcome::Response(mut envelope) = outcome else {
            panic!("the fixture must produce an ingress response");
        };
        let status = envelope.response_mut().status();
        let wire = envelope
            .response_mut()
            .body_mut()
            .as_bytes()
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        drop(envelope);
        (
            status,
            String::from_utf8(wire).unwrap(),
            calls.load(Ordering::Relaxed),
        )
    }

    // A single test owns boot's OnceLock and makes one request at a time.
    #[test]
    fn boot_funded_impression_limit_is_enforced_on_the_auction_path() {
        let funded = limits();
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                limits: &funded,
                target: &target(),
                outbound_meta_bytes: outbound_meta_bytes(funded.max_impressions_per_auction),
                construction_costs: core_construction_costs(),
                log_sink: &StderrBootLog,
                catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            },
            || Arc::new(SystemClock),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("the fixture envelope validates");
        assert_eq!(
            boot_limits().map(|row| row.max_impressions_per_auction),
            Some(FUNDED_IMPRESSIONS),
        );
        let mut app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap();
        configure_app(&mut app).unwrap();
        let default_config = AuctioneerAppConfig::default();
        for impressions in [FUNDED_IMPRESSIONS - 1, FUNDED_IMPRESSIONS] {
            let body = request_body("acct-enabled", impressions, 0);
            bounded_json::value_within(body.as_bytes(), NODE_CAP).unwrap();
            let (status, _wire, calls) = answer(&app, &default_config, &body, impressions);
            assert_eq!(status, StatusCode::OK, "below/at the funded cap is served");
            assert_eq!(
                calls, 1,
                "the allowed impressions reach a real adapter's outbound call"
            );
        }

        let disabled_config: AuctioneerAppConfig = serde_json::from_value(serde_json::json!({
            "schema_version": 1_u64,
            "accounts": {DISABLED_ACCOUNT: {"disabled": true}},
        }))
        .unwrap();
        let over = FUNDED_IMPRESSIONS + 1;
        let disabled_body = request_body(DISABLED_ACCOUNT, over, 0);
        bounded_json::value_within(disabled_body.as_bytes(), NODE_CAP).unwrap();
        let (disabled_status, _wire, disabled_calls) =
            answer(&app, &disabled_config, &disabled_body, over);
        assert_eq!(
            disabled_status,
            StatusCode::SERVICE_UNAVAILABLE,
            "account resolution still precedes the impression validation in prepare"
        );
        assert_eq!(disabled_calls, 0);

        let node_heavy_body =
            request_body(DISABLED_ACCOUNT, over, usize::try_from(NODE_CAP).unwrap());
        assert!(matches!(
            bounded_json::value_within(node_heavy_body.as_bytes(), NODE_CAP),
            Err(BoundedJsonError::NodeBudget(_)),
        ));
        let (node_status, node_wire, node_calls) =
            answer(&app, &disabled_config, &node_heavy_body, over);
        assert_eq!(node_status, StatusCode::BAD_REQUEST);
        assert_eq!(
            node_wire, "Invalid request: too many JSON nodes\n",
            "node denial wins over the disabled account and over-limit impressions"
        );
        assert_eq!(node_calls, 0);

        let body = request_body("acct-enabled", over, 0);
        bounded_json::value_within(body.as_bytes(), NODE_CAP).unwrap();
        let (status, wire, calls) = answer(&app, &default_config, &body, over);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "nine impressions fit the old 100 cap but exceed the boot-funded eight"
        );
        assert_eq!(
            wire,
            format!(
                "Invalid request: imp[] has {over} entries, exceeding the {FUNDED_IMPRESSIONS} limit\n"
            )
        );
        assert_eq!(calls, 0, "over-limit input is refused before fanout");
    }
}
