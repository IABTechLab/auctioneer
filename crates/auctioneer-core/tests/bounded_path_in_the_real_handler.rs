//! The bounded outbound path, asserted through the REAL construction
//! path rather than by building a fanout by hand.
//!
//! Every other fanout test in this crate constructs a
//! `EdgeZeroFanout` directly and then asserts what it does. That
//! proves the fanout works; it cannot prove the HANDLER builds that
//! fanout. These assertions drive `App::dispatch_ingress` — router,
//! admission policy, the auction handler, the per-request fanout the
//! handler selects — and read what reached the outbound boundary.
//!
//! What is pinned here:
//!
//! 1. A served auction's outbound bidder request carries an ABSOLUTE
//!    deadline and the envelope's response caps. The legacy per-call
//!    bridge sets neither: it sends a plain `OutboundRequest` with a
//!    method, a URI, headers, a body and a cache policy, so the
//!    bidder deadline is not propagated to the wire at all and the
//!    response body is collected with no cap.
//! 2. The wave scheduler is what dispatched it — one batch, not one
//!    `send` per call — so the installed stream gate, the collection
//!    policies and the decode cooperator are on the live path.
//! 3. A test-injected fanout still wins over all of it, and an
//!    adapter with no boot envelope installed still answers rather
//!    than panicking.
//!
//! The gates are process-global and the harness runs tests in
//! parallel, so the tests that read gate counters take one mutex.

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod tests {
    use crate::serving_catalog::passthrough_info;
    use async_trait::async_trait;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::handlers::TestFanoutHandle;
    use auctioneer_core::handlers::health_gvl::{counter, gcl_stale_counter};
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, SystemClock,
        bootstrap_adapter_with_envelope, stream_gate,
    };
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_core::test_support::config_store::runtime_registry;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_openrtb::WarningCode;
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        Clock, FakeClock, ImmediateResolveTimer, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator,
        ResolvedIsolateLimits, Rng, SeededRng, TargetPlatformConstants, Timer as _,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::Request;
    use edgezero_core::http::{HeaderMap, Method, StatusCode, request_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
        OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture, OutboundHead,
        OutboundHeaderFidelity, OutboundHeaderInput, OutboundHttpClient, OutboundPending,
        OutboundPreparedBatch, OutboundProgressRuntime, OutboundRequest, OutboundSendMode,
        OutboundTime, ResponseMode,
    };
    use edgezero_core::response_egress::ResponseEgressEnvelope;
    use edgezero_core::store_registry::ConfigRegistry;
    use edgezero_core::time::Deadline;
    use edgezero_core::time::{MonotonicClock, MonotonicInstant};
    use futures::StreamExt as _;
    use futures::executor::block_on;
    use futures::stream;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    const CAP_256_KIB: u64 = 1 << 18;
    const BYTES_1_MIB: u64 = 1 << 20;
    /// The response cap, DERIVED from the assembly scratch the budget
    /// requires it to hold (`max_outbound_requests x max_decoded_bytes x
    /// ASSEMBLY_FACTOR`). A literal here went stale the moment the
    /// factor was corrected.
    const FINAL_RESPONSE_BYTES: u64 = 64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    const BYTES_64_KIB: u32 = 1 << 16;
    const BYTES_16_KIB: u32 = 1 << 14;
    const OUTBOUND_META_BYTES: u32 = auctioneer_bidder::outbound_meta_bytes(100);

    /// The decoded-response cap the envelope declares. Distinctive, so
    /// an assertion reading it off the recorded outbound request cannot
    /// pass against a neighbouring cap or an upstream default.
    const DECODED_CAP: u64 = 0x0002_0000;

    /// The transport cap, deliberately DIFFERENT from `DECODED_CAP`:
    /// the two are independent rows and a test that gave them the same
    /// value could not tell which one was propagated.
    const TRANSPORT_CAP: u64 = 98_304;

    /// A minimal well-formed auction request bound to one bidder.
    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const VALID_REQUEST: &str = r#"{
      "id": "auc-bounded-path",
      "tmax": 1000,
      "cur": ["USD"],
      "site": {
        "page": "https://publisher.example/story",
        "publisher": { "id": "acct-bounded" }
      },
      "imp": [
        {
          "id": "imp-1",
          "banner": { "w": 300, "h": 250 },
          "ext": { "prebid": { "bidder": { "fake_bidder": {} } } }
        }
      ]
    }"#;

    fn gate_counters_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn boot() {
        static BOOTED: OnceLock<()> = OnceLock::new();
        BOOTED.get_or_init(|| {
            bootstrap_adapter_with_envelope(
                BootEnvelope {
                    limits: &limits(),
                    target: &target(),
                    outbound_meta_bytes: OUTBOUND_META_BYTES,
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
        });
    }

    fn limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 1_600_000_000,
            baseline_bytes: 96_000_000,
            inbound_request_bytes: CAP_256_KIB,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: CAP_256_KIB,
            max_outbound_raw_bytes: CAP_256_KIB,
            max_transport_bytes: TRANSPORT_CAP,
            max_decoded_bytes: DECODED_CAP,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: FINAL_RESPONSE_BYTES,
            egress_permits: 3,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 5,
            // Funds config extraction and the checked sync/privacy workload.
            aux_route_peak_bytes: 768 * 1_024 * 1_024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 50,
            max_outbound_requests: 64,
            max_concurrent_auctions: 4,
            max_active_body_streams: 7,
            max_runtime_config_bytes: BYTES_1_MIB,
            max_runtime_config_blob_bytes: BYTES_1_MIB,
            max_parsed_nodes: BYTES_64_KIB,
            max_copied_header_bytes: 4_096_u32,
            max_admitted_header_bytes: BYTES_16_KIB,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 6,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096_u32,
        }
    }

    fn target() -> TargetPlatformConstants {
        TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576_u32,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: BYTES_16_KIB,
            egress_handoff_factor: 2,
        }
    }

    fn config_registry() -> ConfigRegistry {
        config_registry_for(&AuctioneerAppConfig::default())
    }

    fn config_registry_for(cfg: &AuctioneerAppConfig) -> ConfigRegistry {
        let mut data = serde_json::to_value(cfg).expect("config serialises to json");
        data["host"]["aliases"]["fake_bidder"] = serde_json::json!(passthrough_info().name);
        runtime_registry(&data)
    }

    fn configured_app() -> App {
        let mut app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("the app builds");
        configure_app(&mut app).expect("configure is idempotent");
        app
    }

    /// One outbound bidder request exactly as the proxy received it,
    /// plus which dispatch entry point delivered it.
    struct Recorded {
        deadline: Option<Deadline>,
        max_decoded_response_bytes: Option<u64>,
        max_encoded_response_bytes: Option<u64>,
        max_response_bytes: Option<u64>,
    }

    /// Records every dispatch and answers 200 with an empty bid
    /// response.
    ///
    /// `batches` and `singles` are counted SEPARATELY because that is
    /// the structural difference between the two drives: the wave
    /// scheduler opens a batch, the per-call bridge calls `send`.
    #[derive(Clone)]
    struct RecordingClient {
        recorded: Arc<Mutex<Vec<Recorded>>>,
        batches: Arc<AtomicU32>,
        singles: Arc<AtomicU32>,
        clock: Arc<FakeClock>,
    }

    impl RecordingClient {
        fn new(clock: &Arc<FakeClock>) -> Self {
            Self {
                recorded: Arc::new(Mutex::new(Vec::new())),
                batches: Arc::new(AtomicU32::new(0)),
                singles: Arc::new(AtomicU32::new(0)),
                clock: Arc::clone(clock),
            }
        }

        fn locked(&self) -> MutexGuard<'_, Vec<Recorded>> {
            self.recorded.lock().unwrap_or_else(PoisonError::into_inner)
        }

        fn record(&self, request: OutboundDispatchRequest) -> Result<OutboundHead, EdgeError> {
            let (parts, context) = request.into_parts();
            // Both arms named rather than a wildcard: `ResponseMode` has
            // exactly two, and a wildcard here would silently absorb a
            // third if upstream ever adds one — on a field this test
            // reads precisely to prove a cap was set.
            let max_response_bytes = match parts.response_mode {
                ResponseMode::Buffered { max_bytes } => Some(max_bytes),
                ResponseMode::Streamed => None,
            };
            self.locked().push(Recorded {
                deadline: parts.deadline,
                max_decoded_response_bytes: parts.max_decoded_response_bytes,
                max_encoded_response_bytes: parts.max_encoded_response_bytes,
                max_response_bytes,
            });
            OutboundHead::new(
                StatusCode::OK,
                OutboundHeaderInput::new(
                    "fixture",
                    HeaderMap::new(),
                    OutboundHeaderFidelity::ProviderFieldBoundariesUnknown,
                ),
                // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
                // `serde_json::json!` yields a `Value`, so converting would mean
                // `.to_string().into_bytes()` at the call site for no gain.
                Box::pin(stream::iter([Ok(bytes::Bytes::from_static(
                    br#"{"id":"auc-bounded-path","seatbid":[]}"#,
                ))])),
                context,
                OutboundExchangeOwner::new(|| {}),
            )
        }
    }

    impl OutboundHttpClient for RecordingClient {
        fn begin(&self, request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            let pending_head = self.dispatch_head(request);
            Ok(OutboundPending::new(
                Box::pin(async move { pending_head.await?.finish().await }),
                OutboundExchangeOwner::new(|| {}),
            ))
        }

        fn dispatch_head(&self, request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            self.singles.fetch_add(1, Ordering::SeqCst);
            let head = self.record(request);
            Box::pin(async move { head })
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        /// Completion order: input order. Every request is recorded eagerly;
        /// the stream awaits cooperatively yielding body finalization.
        /// Cutoff: never emitted. Dropping the stream drops pending finalizers.
        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            self.batches.fetch_add(1, Ordering::SeqCst);
            let (context, slots) = batch.into_slots();
            let completions = slots
                .into_iter()
                .map(|(index, request)| {
                    let pending_head = self.record(request);
                    let clock = context.clock().clone();
                    async move {
                        let outcome = match pending_head {
                            Ok(resolved_head) => resolved_head.finish().await,
                            Err(error) => Err(error),
                        };
                        OutboundBatchDriverEvent::Item {
                            index,
                            completed_at: clock.now(),
                            outcome,
                        }
                    }
                })
                .collect::<Vec<_>>();
            OutboundBatch::from_driver(
                context,
                stream::iter(completions).then(|completion| completion),
                OutboundExchangeOwner::new(|| {}),
            )
        }

        fn time(&self) -> OutboundTime {
            let read_clock = Arc::clone(&self.clock);
            let waits = Arc::clone(&self.clock);
            OutboundTime::new(
                MonotonicClock::new(move || read_clock.now_monotonic()),
                move |deadline, clock| {
                    let waiter = Arc::clone(&waits);
                    Box::pin(async move {
                        if let Some(remaining) = deadline.remaining_at(clock.now()) {
                            waiter.sleep(remaining).await;
                        }
                        Ok(())
                    })
                },
            )
        }

        fn validate_supported_options(
            &self,
            _: &OutboundRequest,
            _: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Ok(())
        }
    }

    /// A fanout that records that it was consulted and answers 204.
    struct CountingFanout {
        calls: Arc<AtomicU32>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for CountingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(calls
                .iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], Vec::new())),
                    elapsed: Duration::from_millis(1),
                })
                .collect())
        }
    }

    /// Builds an auction request carrying the production seam: a real
    /// `HttpClient` in the extensions and NO `TestFanoutHandle`, so the
    /// handler has to build its own fanout — which is the thing under
    /// test.
    fn auction_request_with_client(body: &str, client: &RecordingClient) -> Request {
        let mut request = bare_auction_request(body);
        request
            .extensions_mut()
            .insert(TestClockHandle(Arc::<FakeClock>::clone(&client.clock)));
        let clock = Arc::clone(&client.clock);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                clock.now_monotonic()
            })));
        request
            .extensions_mut()
            .insert(HttpClient::with_client(client.clone()));
        request
    }

    fn bare_auction_request(body: &str) -> Request {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let mut request = request_builder()
            .method(Method::POST)
            .uri("/openrtb2/auction")
            .header("content-type", "application/json")
            .body(Body::from_bytes(body.as_bytes().to_vec()))
            .expect("the fixture request builds");
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
        request.extensions_mut().insert(config_registry());
        request
    }

    /// Drives one auction request through admission, dispatch and into
    /// an owned egress envelope.
    fn dispatch(app: &App, request: Request) -> Option<ResponseEgressEnvelope> {
        let outcome = block_on(app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .expect("admission begins");
        match outcome {
            IngressDispatchOutcome::Response(envelope) => Some(*envelope),
            IngressDispatchOutcome::Aborted => None,
            _ => panic!("unrecognized ingress dispatch outcome"),
        }
    }

    /// The outbound bidder request a SERVED auction produces carries
    /// the bidder deadline and the envelope's collection caps.
    ///
    /// This is the assertion the per-call bridge cannot pass. That
    /// bridge builds an `OutboundRequest` with a method, a URI,
    /// headers, a body and `cache_policy(Bypass)` and nothing else, so
    /// `deadline` arrives as `None` — the bidder's own timeout is never
    /// propagated to the wire — and every response cap arrives as
    /// `None`, which is the unbounded collection the envelope was
    /// supposed to bound.
    #[test]
    fn a_served_auction_propagates_its_deadline_and_caps_to_the_wire() {
        // Serialized with the other rows in this binary: the row
        // funds `max_concurrent_auctions: 4` and this binary has four
        // tests that each dispatch, so four parallel leases against
        // exactly four permits let any overlap refuse one with 503.
        let _serialized = gate_counters_lock();
        boot();
        let app = configured_app();
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let client = RecordingClient::new(&clock);

        let mut envelope = dispatch(&app, auction_request_with_client(VALID_REQUEST, &client))
            .expect("a valid auction is answered");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::OK,
            "the auction answered normally, so a bidder call really was dispatched",
        );
        drop(envelope);

        let recorded = client.locked();
        let first = recorded
            .first()
            .expect("the auction dispatched at least one bidder call");

        assert!(
            first.deadline.is_some(),
            "the bidder's deadline must reach the outbound request: without it the host \
             enforces no per-call budget at all and the auction's tmax bounds nothing on the \
             wire",
        );
        assert_eq!(
            first.max_decoded_response_bytes,
            Some(DECODED_CAP),
            "the envelope's decoded-response cap must reach the wire, so the response body is \
             bounded while it is collected rather than rejected afterwards",
        );
        assert_eq!(
            first.max_encoded_response_bytes,
            Some(TRANSPORT_CAP),
            "and the transport cap is its own independent row, not a copy of the decoded one",
        );
        assert_eq!(
            first.max_response_bytes,
            Some(DECODED_CAP.max(TRANSPORT_CAP)),
            "buffered collection is bounded by the wider of the two caps, never by usize::MAX",
        );
    }

    #[test]
    fn publisherless_response_caps_do_not_select_the_unknown_account_row() {
        let _serialized = gate_counters_lock();
        boot();
        let app = configured_app();
        let cfg: AuctioneerAppConfig = toml::from_str(
            "schema_version = 1\n[host.resource_limits]\nmax_transport_bytes = 8192\nmax_decoded_bytes = 16384\n[accounts.unknown]\nmax_transport_bytes = 1024\nmax_decoded_bytes = 2048\n",
        )
        .unwrap();
        for (publisher, transport, decoded) in
            [(None, 0x2000, 0x4000), (Some("unknown"), 1_024, 2_048)]
        {
            let mut body: serde_json::Value = serde_json::from_str(VALID_REQUEST).unwrap();
            body["site"]["publisher"] =
                publisher.map_or_else(|| serde_json::json!({}), |id| serde_json::json!({"id": id}));
            let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
            let client = RecordingClient::new(&clock);
            let mut request =
                auction_request_with_client(&serde_json::to_string(&body).unwrap(), &client);
            request.extensions_mut().insert(config_registry_for(&cfg));
            let mut envelope = dispatch(&app, request).expect("the auction is answered");
            assert_eq!(envelope.response_mut().status(), StatusCode::OK);
            drop(envelope);
            let recorded = client.locked();
            assert_eq!(recorded.len(), 1);
            assert_eq!(
                recorded.first().unwrap().max_encoded_response_bytes,
                Some(transport)
            );
            assert_eq!(
                recorded.first().unwrap().max_decoded_response_bytes,
                Some(decoded)
            );
        }
    }

    #[test]
    fn scoped_alias_response_caps_reach_the_wire_and_warn_only_at_funded_ceilings() {
        let _serialized = gate_counters_lock();
        boot();
        let app = configured_app();
        let mut request_json: serde_json::Value = serde_json::from_str(VALID_REQUEST).unwrap();
        request_json["imp"][0]["ext"]["prebid"]["bidder"] = serde_json::json!({"fake_alias": {}});
        let body = serde_json::to_string(&request_json).unwrap();
        for (own, transport, decoded, warning_count) in [
            ("", 0x8000, 0x0001_0000, 0),
            ("max_decoded_bytes = 8192", 0x8000, 0x2000, 0),
            ("max_transport_bytes = 8192", 0x2000, 0x0001_0000, 0),
            (
                "max_transport_bytes = 100000",
                TRANSPORT_CAP,
                0x0001_0000,
                1,
            ),
            (
                "max_transport_bytes = 8192\nmax_decoded_bytes = 200000",
                0x2000,
                DECODED_CAP,
                1,
            ),
        ] {
            let own_caps: toml::Value = toml::from_str(own).unwrap();
            let cfg: AuctioneerAppConfig = serde_json::from_value(serde_json::json!({
                "schema_version": 1_u64,
                "host": {
                    "resource_limits": {"max_transport_bytes": 1_024_u64, "max_decoded_bytes": 2_048_u64},
                    "aliases": {"fake_alias": passthrough_info().name},
                },
                "accounts": {"acct-bounded": {"max_transport_bytes": 0x8000_u64, "max_decoded_bytes": 0xc000_u64}},
                "bidders": {
                    (passthrough_info().name.as_str()): {"max_decoded_bytes": 0x0001_0000_u64},
                    "fake_alias": own_caps
                },
            }))
            .unwrap();
            let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
            let client = RecordingClient::new(&clock);
            let mut request = auction_request_with_client(&body, &client);
            request.extensions_mut().insert(config_registry_for(&cfg));
            let before_unavailable = counter().count_last_hour(clock.now());
            let before_stale = gcl_stale_counter().count_last_hour(clock.now());
            let mut envelope = dispatch(&app, request).expect("the auction is answered");
            assert_eq!(envelope.response_mut().status(), StatusCode::OK);
            let response: serde_json::Value = serde_json::from_slice(
                envelope
                    .response_mut()
                    .body()
                    .as_bytes()
                    .expect("buffered response"),
            )
            .unwrap();
            let warnings = response
                .pointer("/ext/warnings/fake_alias")
                .and_then(serde_json::Value::as_array);
            assert_eq!(
                warnings.map_or(0, |rows| rows
                    .iter()
                    .filter(|row| row["code"] == WarningCode::OutboundBudgetClamp.as_u32())
                    .count()),
                warning_count
            );
            assert!(response.pointer("/ext/warnings/fake_bidder").is_none());
            assert!(
                response["ext"]["warnings"]
                    .get(&passthrough_info().name)
                    .is_none()
            );
            assert_eq!(
                counter().count_last_hour(clock.now()),
                before_unavailable,
                "budget clamps must not count as unavailable GVL lookups",
            );
            assert_eq!(
                gcl_stale_counter().count_last_hour(clock.now()),
                before_stale,
                "budget clamps must not count as stale GCL lookups",
            );
            drop(envelope);
            let recorded = client.locked();
            assert_eq!(recorded.len(), 1);
            let first = recorded.first().unwrap();
            assert_eq!(first.max_encoded_response_bytes, Some(transport));
            assert_eq!(first.max_decoded_response_bytes, Some(decoded));
            assert_eq!(first.max_response_bytes, Some(transport.max(decoded)));
        }
    }

    /// The wave scheduler is what dispatched the call: ONE batch, and
    /// no per-call `send`.
    ///
    /// Distinct from the caps assertion above because the caps could in
    /// principle be set by a patched per-call bridge, while the batch
    /// entry point is reachable only through `drive_fanout` — and
    /// `drive_fanout` is what consults the installed stream gate,
    /// applies the collection policies and yields through the decode
    /// cooperator.
    #[test]
    fn a_served_auction_dispatches_through_the_wave_scheduler() {
        // Serialized with the other rows in this binary: the row
        // funds `max_concurrent_auctions: 4` and this binary has four
        // tests that each dispatch, so four parallel leases against
        // exactly four permits let any overlap refuse one with 503.
        let _serialized = gate_counters_lock();
        boot();
        let app = configured_app();
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let client = RecordingClient::new(&clock);

        let envelope = dispatch(&app, auction_request_with_client(VALID_REQUEST, &client))
            .expect("a valid auction is answered");
        drop(envelope);

        assert_eq!(
            client.batches.load(Ordering::SeqCst),
            1,
            "the auction opened exactly one outbound wave: the wave scheduler is the only \
             drive that reaches the stream gate, the collection policies and the decode \
             cooperator",
        );
        assert_eq!(
            client.singles.load(Ordering::SeqCst),
            0,
            "and nothing went out through the per-call bridge, whose deadline and timer \
             arguments the dev strategy discards",
        );
    }

    /// The wave fanout the handler builds holds the PROCESS-INSTALLED
    /// stream gate, not a gate of its own.
    ///
    /// Read while the fanout is mid-flight would need a parked client;
    /// instead this reads the gate's capacity through the dispatch and
    /// asserts the auction completed against the installed instance —
    /// a fanout carrying a private gate would leave the installed one
    /// untouched but would also be unobservable here, so the assertion
    /// that carries the weight is the batch count above. What this
    /// pins is that the installed gate EXISTS on the path the handler
    /// takes, so a later change that stops installing it fails here
    /// rather than silently unbounding the wave.
    #[test]
    fn the_handler_path_runs_against_the_installed_stream_gate() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = stream_gate().expect("boot installed the stream gate");
        let before = gate.granted();
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let client = RecordingClient::new(&clock);

        let envelope = dispatch(&app, auction_request_with_client(VALID_REQUEST, &client))
            .expect("a valid auction is answered");
        drop(envelope);

        assert_eq!(client.batches.load(Ordering::SeqCst), 1, "the wave ran");
        assert_eq!(
            gate.granted(),
            before,
            "and every wave permit it took is back: a wave that leaked a permit would \
             strand capacity the memory envelope charged for",
        );
    }

    /// A test-injected fanout still wins over the production seam.
    ///
    /// The handler consults extensions FIRST, so a test that installs
    /// its own fanout must not be forced to supply an outbound client,
    /// a stream gate or an envelope — and must not silently have its
    /// fanout replaced by the wave drive.
    #[test]
    fn an_injected_fanout_still_overrides_the_production_drive() {
        // Serialized with the other rows in this binary: the row
        // funds `max_concurrent_auctions: 4` and this binary has four
        // tests that each dispatch, so four parallel leases against
        // exactly four permits let any overlap refuse one with 503.
        let _serialized = gate_counters_lock();
        boot();
        let app = configured_app();
        let calls = Arc::new(AtomicU32::new(0));
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let client = RecordingClient::new(&clock);

        // BOTH seams present: an injected fanout AND a live outbound
        // client. The injected one has to win, or every handler-path
        // test in the suite starts talking to the client instead.
        let mut request = auction_request_with_client(VALID_REQUEST, &client);
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(CountingFanout {
            calls: Arc::clone(&calls),
        });
        request.extensions_mut().insert(TestFanoutHandle(fanout));

        let mut envelope = dispatch(&app, request).expect("a valid auction is answered");
        assert_eq!(envelope.response_mut().status(), StatusCode::OK);
        drop(envelope);

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the injected fanout is the one that ran",
        );
        assert_eq!(
            client.batches.load(Ordering::SeqCst),
            0,
            "and the production drive was not built behind its back",
        );
        assert_eq!(client.singles.load(Ordering::SeqCst), 0);
    }
}
