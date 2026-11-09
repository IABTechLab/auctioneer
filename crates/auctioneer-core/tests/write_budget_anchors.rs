//! Where each response's write budget is anchored, asserted on the
//! REAL request path.
//!
//! Two anchors, and they are different anchors on purpose:
//!
//! 1. **A normal auction** anchors its write budget at EGRESS ENTRY —
//!    the instant `run_auction` returns and the handler starts
//!    producing bytes. Anchoring it at request admission instead makes
//!    the upload, the config load and the auction itself spend the
//!    RESPONSE's allowance: a 30-second budget on a 29-second auction
//!    leaves one second to write, and a slow-enough auction hands its
//!    own answer a deadline already in the past. The budget bounds the
//!    WRITE leg; the auction's own time is bounded by tmax, which is a
//!    separate and already-enforced thing.
//! 2. **A refusal** anchors at the refusal itself, with its own fresh
//!    and much shorter budget. A refusal born from an auction whose
//!    budget is gone cannot inherit that budget — the 503 would abort
//!    before its first byte — and a refusal born at admission has no
//!    auction budget to inherit at all, so without an explicit value
//!    it falls to the host's own 30-second default and a slow refusal
//!    reader holds static-class capacity fifteen times longer than the
//!    class is sized for.
//!
//! These assertions read the `ResponseEgressDeadline` the handler and
//! the admission policy attach, driven through `dispatch_ingress`. The
//! attached value is what the host resolves and enforces, so it is the
//! thing the defect is about.

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod tests {
    use crate::bounded_store::bounded_over_get;
    use crate::serving_catalog;
    use async_trait::async_trait;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_config::policy::HostPolicy;
    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::handlers::TestFanoutHandle;
    use auctioneer_core::handlers::egress_deadline::{EgressDeadline, REFUSAL_WRITE_BUDGET};
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, SystemClock, admission_gate,
        bootstrap_adapter_with_envelope,
    };
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        Clock, FakeClock, ImmediateResolveTimer, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator,
        ResolvedIsolateLimits, Rng, SeededRng, TargetPlatformConstants,
    };
    use edgezero_core::app::App;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
    };
    use edgezero_core::http::Request;
    use edgezero_core::http::{Method, StatusCode, request_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::response_egress::{
        DEFAULT_RESPONSE_WRITE_BUDGET, ResponseEgressDeadline, ResponseEgressEnvelope,
    };
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::Deadline;
    use edgezero_core::time::{MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use std::collections::BTreeMap;
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

    /// The envelope's write budget for these tests. Deliberately SHORT
    /// and distinctive: the whole question is which instant the budget
    /// hangs off, so the budget itself has to be small enough that an
    /// auction can outrun it on a driven clock.
    const WRITE_BUDGET_MS: u64 = 500;

    /// How long the fixture's auction takes. LONGER than the write
    /// budget, which is what makes the two anchors distinguishable: a
    /// request-start anchor yields a deadline already in the past, and
    /// an egress-entry anchor yields a live one.
    const SLOW_AUCTION: Duration = Duration::from_millis(900);

    /// One concurrent auction, so the refusal test can saturate the
    /// class with a single in-flight request.
    const CONCURRENT_AUCTIONS: u32 = 1;

    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const VALID_REQUEST: &str = r#"{
      "id": "auc-write-budget",
      "tmax": 1000,
      "cur": ["USD"],
      "site": {
        "page": "https://publisher.example/story",
        "publisher": { "id": "acct-write-budget" }
      },
      "imp": [
        {
          "id": "imp-1",
          "banner": { "w": 300, "h": 250 },
          "ext": { "prebid": { "bidder": { "fake_bb": {} } } }
        }
      ]
    }"#;

    fn gate_counters_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn boot() {
        static BOOTED: OnceLock<()> = OnceLock::new();
        let _info = serving_catalog::passthrough_info();
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
            max_transport_bytes: CAP_256_KIB,
            max_decoded_bytes: CAP_256_KIB,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: FINAL_RESPONSE_BYTES,
            egress_permits: 3,
            response_write_deadline_ms: WRITE_BUDGET_MS,
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
            max_concurrent_auctions: CONCURRENT_AUCTIONS,
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

    struct FixedStore(ConfigValue);

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for FixedStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            Ok(Some(self.0.clone()))
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

    fn config_registry() -> ConfigRegistry {
        let cfg = AuctioneerAppConfig {
            host: HostPolicy {
                aliases: Some(BTreeMap::from([(
                    "fake_bb".to_owned(),
                    serving_catalog::passthrough_info().name.clone(),
                )])),
                ..HostPolicy::default()
            },
            ..AuctioneerAppConfig::default()
        };
        let data = serde_json::to_value(&cfg).expect("config serialises to json");
        let envelope = BlobEnvelope::new(data, "1970-01-01T00:00:00Z".to_owned());
        let blob = serde_json::to_string(&envelope).expect("envelope serialises");
        StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(FixedStore(blob.into()))),
            },
        )
    }

    fn configured_app() -> App {
        let mut app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("the app builds");
        configure_app(&mut app).expect("configure is idempotent");
        app
    }

    /// A fanout that ADVANCES the driven clock before answering, so the
    /// auction really consumes time on the clock the handler reads.
    ///
    /// The advance is the whole fixture. Without it the two anchors
    /// coincide and the test is green against either one — which is
    /// exactly the shape that let this defect ship.
    struct SlowFanout {
        clock: Arc<FakeClock>,
        by: Duration,
    }

    #[async_trait(?Send)]
    impl BidderFanout for SlowFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.clock.advance(self.by);
            Ok(calls
                .iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], Vec::new())),
                    elapsed: self.by,
                })
                .collect())
        }
    }

    /// Builds an auction request on one driven clock and hands the
    /// clock back, so the test can compute the anchors itself.
    fn auction_request(body: &str, advance_by: Duration) -> (Request, Arc<FakeClock>) {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(SlowFanout {
            clock: Arc::<FakeClock>::clone(&clock),
            by: advance_by,
        });
        let mut request = request_builder()
            .method(Method::POST)
            .uri("/openrtb2/auction")
            .header("content-type", "application/json")
            .body(Body::from_bytes(body.as_bytes().to_vec()))
            .expect("the fixture request builds");
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
        request.extensions_mut().insert(config_registry());
        (request, clock)
    }

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

    /// Reads the attached write deadline off a response.
    ///
    /// Compared through `Debug` because the type is opaque by design —
    /// it carries no accessor, since the host is the only thing meant
    /// to resolve it. The two shapes print differently (`At(..)` vs
    /// `After(..)`) and an absolute instant prints its own value, so
    /// the comparison is exact rather than approximate.
    fn attached(envelope: &mut ResponseEgressEnvelope) -> String {
        let deadline = envelope
            .response_mut()
            .extensions()
            .get::<ResponseEgressDeadline>()
            .copied()
            .expect("every response this handler returns carries a write deadline");
        format!("{deadline:?}")
    }

    /// A SLOW auction's write budget is anchored at egress entry, so
    /// the auction's own elapsed time does not come out of it.
    ///
    /// The fixture's auction runs 900ms against a 500ms write budget.
    /// Anchored at request start the attached deadline is 400ms IN THE
    /// PAST and the response is shed by its own checkpoints; anchored
    /// at egress entry it is 500ms ahead of the instant the auction
    /// finished, which is what the budget is for.
    #[test]
    fn a_slow_auction_anchors_its_write_budget_at_egress_entry() {
        boot();
        // The gates are process-global and this binary's other test
        // saturates the auction class by hand. Without the mutex this
        // request races that saturation and is REFUSED at admission —
        // a 503 that looks exactly like the write-budget shed this test
        // is written to catch, which would make a real regression
        // indistinguishable from a scheduling accident.
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let (request, clock) = auction_request(VALID_REQUEST, SLOW_AUCTION);
        let request_start = clock.now_monotonic();

        let mut envelope = dispatch(&app, request).expect("the slow auction is answered");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::OK,
            "a 900ms auction under a 1000ms tmax completes: the write budget must not shed it",
        );

        // Egress entry is request_start + the auction's own elapsed
        // time, which on this driven clock is exactly the advance the
        // fanout made.
        let egress_entry = request_start
            .checked_add(SLOW_AUCTION)
            .expect("test instants do not overflow");
        let expected = EgressDeadline::new(egress_entry, Duration::from_millis(WRITE_BUDGET_MS));
        let wrong = EgressDeadline::new(request_start, Duration::from_millis(WRITE_BUDGET_MS));

        let got = attached(&mut envelope);
        assert_ne!(
            got,
            format!("{:?}", wrong.to_response_extension()),
            "the budget must not be anchored at request admission: the upload, the config \
             load and the auction would all spend the RESPONSE's allowance, and a 900ms \
             auction under a 500ms budget would be handed a deadline already in the past",
        );
        assert_eq!(
            got,
            format!("{:?}", expected.to_response_extension()),
            "the write budget starts when the handler enters egress, so the whole budget is \
             available to the write leg it bounds",
        );
        assert!(
            !expected.is_expired_at(clock.now_monotonic()),
            "and the completed auction's answer has live budget to be written with",
        );
        drop(envelope);
    }

    /// An ADMISSION refusal carries its own fresh refusal budget, not
    /// the host's 30-second default.
    ///
    /// `AdmissionDecision::Refuse` goes out through DETACHED egress,
    /// which hardcodes the host's default policy, so auctioneer's installed
    /// policy is never consulted and the refusal's only chance to be
    /// bounded is the extension it carries itself. Without it a slow
    /// refusal reader holds a static-class permit for thirty seconds
    /// against a class sized for two.
    #[test]
    fn an_admission_refusal_carries_a_fresh_refusal_budget() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let admission = admission_gate().expect("boot installed the admission gate");

        // Saturate the auction class by hand, so the NEXT request is
        // refused at admission rather than reaching the handler.
        let held = admission
            .try_acquire()
            .expect("the fixture starts from an idle class");

        let (request, _clock) = auction_request(VALID_REQUEST, Duration::ZERO);
        let mut envelope = dispatch(&app, request).expect("a refused request is still answered");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "the fixture's value really reached the refusal branch: an admitted request \
             would answer 200 and this test would be asserting nothing",
        );

        assert_eq!(
            attached(&mut envelope),
            format!("{:?}", ResponseEgressDeadline::after(REFUSAL_WRITE_BUDGET)),
            "a refusal is anchored at its own egress start with the short refusal budget: \
             the static class is sized against that drain time, and the host's own default \
             is fifteen times longer",
        );

        drop(envelope);
        drop(held);
    }

    /// An error rendered on auctioneer's behalf carries the refusal budget
    /// too, not the host's 30-second default.
    ///
    /// This is the leg the aux wrapper cannot reach. `with_aux_egress`
    /// attaches its deadline to the response the handler produced, so
    /// an `Err` returns through the `?` above that attachment and
    /// arrives at the renderer with nothing attached. The renderer
    /// attaches the budget itself, which is what this row reads.
    ///
    /// auctioneer's installed fallback egress policy is a SECOND layer
    /// under this one, for responses that reach egress with no
    /// extension at all — the buffered ingress drain terminals, which
    /// carry no extensions by construction. It preserves an explicit
    /// deadline wherever one exists, so it cannot change the value
    /// asserted here. Every aux route has reachable `Err` sites — the
    /// status probe, both bidder-info routes and the GVL health check.
    ///
    /// Driven as a method mismatch on a real aux path, which is a
    /// host-generated error on the live dispatch path: the point is
    /// which budget the RENDERED response carries, and that is one
    /// value for every error reaching the renderer.
    #[test]
    fn a_rendered_error_carries_a_fresh_refusal_budget() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();

        let request = request_builder()
            .method(Method::POST)
            .uri("/info/bidders")
            .body(Body::empty())
            .expect("the fixture request builds");
        let mut envelope = dispatch(&app, request).expect("an error is still answered");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "the fixture really reached the error renderer: a served GET would answer 200 \
             and this test would be asserting nothing",
        );

        assert_eq!(
            attached(&mut envelope),
            format!("{:?}", ResponseEgressDeadline::after(REFUSAL_WRITE_BUDGET)),
            "a rendered error is bounded by auctioneer's short refusal budget: it is a \
             static-class answer out of a small fixed pool whose capacity argument is \
             that a held permit drains within that budget",
        );
        assert_ne!(
            attached(&mut envelope),
            format!(
                "{:?}",
                ResponseEgressDeadline::after(DEFAULT_RESPONSE_WRITE_BUDGET)
            ),
            "and specifically not the host's own default, which is what an unattached \
             response inherits",
        );

        drop(envelope);
    }
}
