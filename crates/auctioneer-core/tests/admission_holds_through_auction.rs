//! Resource ownership on the REAL request path, driven through
//! `dispatch_ingress` rather than by constructing a lease by hand.
//!
//! Every other admission test in this crate builds a
//! `AuctioneerIngressLease` directly, or calls `decide()` and inspects what
//! comes back. Both shapes prove the primitives behave; neither proves
//! the HANDLER holds what admission gave it. These assertions run a
//! request through router resolution, auctioneer's admission policy, the
//! auction handler and into an owned egress envelope, and read the
//! process-global gates while the auction is still in flight.
//!
//! Two properties are pinned here:
//!
//! 1. The auction-concurrency slot is held for the whole auction, not
//!    released the moment the handler extracts its egress slot. With
//!    `max_concurrent_auctions` at one, a second auction must be
//!    refused while the first is mid-fanout.
//! 2. Every admitted auction holds a terminal-response slot until its
//!    response reaches terminal egress — including the auctions that
//!    answer with an error long before any egress permit exists.
//!
//! The gates are process-global by design, and the harness runs tests
//! in parallel, so every test takes one mutex. A snapshot delta is not
//! enough: a concurrent holder can acquire and release between the
//! snapshot and the assertion, which would make an over-admission look
//! like correct accounting.

#[cfg(all(test, not(target_arch = "wasm32")))]
mod ipv6_growth_allocations {
    #![expect(
        unsafe_code,
        reason = "test-only allocator delegates to System and injects one recoverable realloc failure"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::ptr::null_mut;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Mode {
        Control,
        FailSecond,
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Snapshot {
        pub matching_reallocs: u64,
        pub successful_reallocs: u64,
        pub between_scope_arms: u64,
        pub injected_failures: u64,
    }

    #[derive(Clone, Copy)]
    struct Probe {
        mode: Mode,
        counters: Snapshot,
        armed: bool,
    }

    std::thread_local! {
        static PROBE: Cell<Option<Probe>> = const { Cell::new(None) };
    }

    #[must_use]
    pub struct Observation;

    pub fn start(mode: Mode) -> Observation {
        PROBE.with(|slot| {
            assert!(
                slot.get().is_none(),
                "allocation observation already active"
            );
            slot.set(Some(Probe {
                mode,
                counters: Snapshot::default(),
                armed: false,
            }));
        });
        Observation
    }

    impl Observation {
        #[expect(
            clippy::expect_used,
            reason = "test fixture invariant: a live observation must own its TLS probe"
        )]
        pub fn finish(self) -> Snapshot {
            let snapshot =
                PROBE.with(|slot| slot.take().expect("allocation observation active").counters);
            drop(self);
            snapshot
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Observation {
        fn drop(&mut self) {
            let _cleared = PROBE.try_with(|slot| slot.set(None));
        }
    }

    pub fn between_scopes() {
        let _recorded = PROBE.try_with(|slot| {
            let Some(mut probe) = slot.get() else {
                return;
            };
            // Mutation reads no clock: this callback follows the first scrub.
            if probe.counters.matching_reallocs == 1
                && probe.counters.successful_reallocs == 1
                && probe.counters.between_scope_arms == 0
            {
                probe.armed = true;
                probe.counters.between_scope_arms = 1;
                slot.set(Some(probe));
            }
        });
    }

    fn matching_realloc_should_fail() -> bool {
        PROBE
            .try_with(|slot| {
                let Some(mut probe) = slot.get() else {
                    return false;
                };
                probe.counters.matching_reallocs =
                    probe.counters.matching_reallocs.saturating_add(1);
                let fail = probe.mode == Mode::FailSecond
                    && probe.armed
                    && probe.counters.matching_reallocs == 2
                    && probe.counters.successful_reallocs == 1
                    && probe.counters.injected_failures == 0;
                if fail {
                    probe.armed = false;
                    probe.counters.injected_failures = 1;
                }
                slot.set(Some(probe));
                fail
            })
            .unwrap_or(false)
    }

    fn record_success() {
        let _recorded = PROBE.try_with(|slot| {
            if let Some(mut probe) = slot.get() {
                probe.counters.successful_reallocs =
                    probe.counters.successful_reallocs.saturating_add(1);
                slot.set(Some(probe));
            }
        });
    }

    pub struct Allocator;

    // SAFETY: successful operations delegate unchanged to System. A refused
    // realloc leaves its original allocation intact; callbacks never allocate.
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller supplies a valid allocation layout.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller supplies a valid allocation layout.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: pointer and layout describe a matching System allocation.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let matches = layout.align() == 1 && layout.size() == 20 && new_size == 21;
            if matches && matching_realloc_should_fail() {
                return null_mut();
            }
            // SAFETY: pointer/layout match System; new_size is the caller's request.
            let next = unsafe { System.realloc(ptr, layout, new_size) };
            if matches && !next.is_null() {
                record_success();
            }
            next
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[global_allocator]
static ALLOCATOR: ipv6_growth_allocations::Allocator = ipv6_growth_allocations::Allocator;

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
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::handlers::TestFanoutHandle;
    use auctioneer_core::handlers::auction::{TestAuctionBudget, auction};
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::ingress_lease::{
        AuctioneerIngressLease, RouteAdmissionMarker,
    };
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, SystemClock, admission_gate, boot_gates,
        bootstrap_adapter_with_envelope, egress_gate, static_egress_gate,
    };
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_core::test_support::config_store;
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
    use edgezero_core::response_egress::ResponseEgressEnvelope;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::Deadline;
    use edgezero_core::time::{MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use futures::stream::poll_fn;
    use futures::task::noop_waker;
    use serde_json::{Value, json};
    use std::cell::RefCell;
    use std::future::Future;
    use std::mem;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
    use std::task::{Context, Poll};
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

    /// Capacity ONE on the auction class. That is what makes the
    /// hold-until-done assertion provable: a second auction either can
    /// or cannot be admitted, with no slack to hide an early release in.
    const CONCURRENT_AUCTIONS: u32 = 1;

    /// Distinctive terminal capacity, so an assertion that reads this
    /// class cannot pass by coincidence against a neighbouring one.
    const TERMINAL_PERMITS: u32 = 6;

    /// A minimal well-formed auction request bound to one bidder.
    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const VALID_REQUEST: &str = r#"{
      "id": "auc-admission",
      "tmax": 1000,
      "cur": ["USD"],
      "site": {
        "page": "https://publisher.example/story",
        "publisher": { "id": "acct-admission" }
      },
      "imp": [
        {
          "id": "imp-1",
          "banner": { "w": 300, "h": 250 },
          "ext": { "prebid": { "bidder": { "fake_bb": {} } } }
        }
      ]
    }"#;

    /// Structurally invalid: no `imp` array at all, so `prepare()`
    /// rejects it and the handler answers 400 without ever reaching
    /// `run_auction` — and therefore without ever acquiring an
    /// egress-class permit.
    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const INVALID_REQUEST: &str = r#"{"id":"auc-no-imps","tmax":1000}"#;

    fn gate_counters_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(PoisonError::into_inner)
    }

    struct DisclosureObservation {
        record: String,
        charged_bytes: u64,
        admission_permits: u32,
    }

    struct DisclosureCapture {
        ledger: Arc<auctioneer_runtime::AuctionBudget>,
        observations: Vec<DisclosureObservation>,
    }

    std::thread_local! {
        static DISCLOSURE_CAPTURE: RefCell<Option<DisclosureCapture>> = const { RefCell::new(None) };
    }

    struct DisclosureLogger;

    impl log::Log for DisclosureLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.target() == "auctioneer::request_disclosure"
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            assert_eq!(record.level(), log::Level::Debug);
            DISCLOSURE_CAPTURE.with(|slot| {
                if let Some(capture) = slot.borrow_mut().as_mut() {
                    capture.observations.push(DisclosureObservation {
                        record: record.args().to_string(),
                        charged_bytes: capture.ledger.in_use(),
                        admission_permits: admission_gate().expect("admission installed").in_use(),
                    });
                }
            });
        }

        fn flush(&self) {}
    }

    fn capture_disclosure(ledger: &Arc<auctioneer_runtime::AuctionBudget>) {
        DISCLOSURE_CAPTURE.with(|capture| {
            assert!(capture.borrow().is_none(), "previous capture consumed");
            *capture.borrow_mut() = Some(DisclosureCapture {
                ledger: Arc::clone(ledger),
                observations: Vec::new(),
            });
        });
    }

    fn assert_disclosure_before_release(
        status: &str,
        attempted: u64,
        failed: u64,
        counts: [u64; 8],
    ) {
        DISCLOSURE_CAPTURE.with(|slot| {
            let capture = slot.borrow_mut().take().expect("capture started");
            assert_eq!(
                capture.observations.len(),
                1,
                "exactly one real observation"
            );
            let observed = &capture.observations[0];
            let [
                id,
                geo,
                coarsen,
                fpd,
                tid,
                unknown,
                malformed,
                coppa_removed,
            ] = counts;
            assert_eq!(
                observed.record,
                format!(
                    "request_disclosure status={status} attempted_scopes={attempted} \
                 failed_scopes={failed} id={id} geo={geo} coarsen={coarsen} fpd={fpd} tid={tid} \
                 unknown={unknown} malformed={malformed} coppa_removed={coppa_removed} saturated=0"
                )
            );
            assert_eq!(observed.charged_bytes, capture.ledger.limit());
            assert_eq!(
                observed.admission_permits, 1,
                "observe before lease release"
            );
            assert_eq!(
                capture.ledger.in_use(),
                0,
                "reservation released afterwards"
            );
            assert_eq!(admission_gate().unwrap().in_use(), 0);
        });
    }

    fn assert_no_disclosure() {
        DISCLOSURE_CAPTURE.with(|slot| {
            let capture = slot.borrow_mut().take().expect("capture started");
            assert!(
                capture.observations.is_empty(),
                "no scope before prepayment"
            );
        });
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
                    log_sink: &DisclosureLogger,
                    catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
                },
                || Arc::new(SystemClock),
                || Arc::new(DevRng::new()),
                || Arc::new(ImmediateResolveTimer),
                || Arc::new(NoopDecodeCooperator),
                || Arc::new(DelegateToDevStrategy),
            )
            .expect("the fixture envelope validates");
            log::set_max_level(log::LevelFilter::Debug);
        });
    }

    fn limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 64_000_000_000,
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
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 5,
            // Covers sync work as well as privacy enforcement's config-extraction floor.
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
            terminal_response_permits: TERMINAL_PERMITS,
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

    /// A fixed in-memory `ConfigStore` returning one blob for any key.
    ///
    /// The handler's own loader reads this, so these tests drive the
    /// REAL config path rather than bypassing it: without a bound store
    /// the fail-closed bootstrap answers 500 before the auction is ever
    /// reached, and none of the resource assertions below would be
    /// exercising the code they name.
    struct FixedStore(ConfigValue, Option<Arc<AtomicUsize>>, Option<ParkedFanout>);

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for FixedStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            if let Some(reads) = &self.1 {
                reads.fetch_add(1, Ordering::SeqCst);
            }
            if let Some(pause) = &self.2 {
                ParkUntilReleased {
                    released: Arc::clone(&pause.released),
                    entered: Arc::clone(&pause.entered),
                }
                .await;
            }
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

    /// The `"auctioneer_runtime"` binding the handler's loader looks up,
    /// carrying defaults plus the fixture's real serving binding.
    fn config_registry() -> ConfigRegistry {
        config_registry_with(&AuctioneerAppConfig::default())
    }

    fn config_registry_with(cfg: &AuctioneerAppConfig) -> ConfigRegistry {
        config_registry_observed(cfg, None)
    }

    fn config_registry_observed(
        cfg: &AuctioneerAppConfig,
        reads: Option<Arc<AtomicUsize>>,
    ) -> ConfigRegistry {
        config_registry_paused(cfg, reads, None)
    }

    fn config_registry_paused(
        cfg: &AuctioneerAppConfig,
        reads: Option<Arc<AtomicUsize>>,
        pause: Option<ParkedFanout>,
    ) -> ConfigRegistry {
        let mut aliases = cfg.host.aliases.clone().unwrap_or_default();
        aliases.insert(
            "fake_bb".to_owned(),
            serving_catalog::passthrough_info().name.clone(),
        );
        let mut data = serde_json::to_value(cfg).expect("config serialises to json");
        data["host"]["aliases"] = serde_json::to_value(aliases).expect("aliases serialise to json");
        let envelope = BlobEnvelope::new(data, "1970-01-01T00:00:00Z".to_owned());
        let blob = serde_json::to_string(&envelope).expect("envelope serialises");
        StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(FixedStore(blob.into(), reads, pause))),
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

    /// A fanout that parks on its first poll and answers only once the
    /// test releases it.
    ///
    /// This is what makes "while the auction is in flight" an observable
    /// instant rather than a race: the dispatch future is polled until
    /// it parks INSIDE `send_all`, the gates are read there, and the
    /// auction is resumed afterwards.
    struct ParkedFanout {
        released: Arc<AtomicBool>,
        entered: Arc<AtomicBool>,
    }

    /// Parks until `released` is set, reporting entry through `entered`.
    struct ParkUntilReleased {
        released: Arc<AtomicBool>,
        entered: Arc<AtomicBool>,
    }

    impl Future for ParkUntilReleased {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.entered.store(true, Ordering::SeqCst);
            if self.released.load(Ordering::SeqCst) {
                return Poll::Ready(());
            }
            // Re-arm so the outer stepper makes progress the moment the
            // test flips the flag.
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    #[async_trait(?Send)]
    impl BidderFanout for ParkedFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            ParkUntilReleased {
                released: Arc::clone(&self.released),
                entered: Arc::clone(&self.entered),
            }
            .await;
            Ok(calls
                .iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], Vec::new())),
                    elapsed: Duration::from_millis(1),
                })
                .collect())
        }
    }

    struct UnavailableFanout;

    #[async_trait(?Send)]
    impl BidderFanout for UnavailableFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), 1, "survivor was constructed before failure");
            Err(FanoutInternalError::NoOutboundCapability)
        }
    }

    /// Builds an auction request carrying the test-side handles the
    /// handler reads: a controllable fanout, and one `FakeClock` driving
    /// both wall and monotonic time.
    fn auction_request(body: &str, fanout: Arc<dyn BidderFanout + Send + Sync>) -> Request {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
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
        request
    }

    /// Drives one auction request all the way through admission,
    /// dispatch and into an owned egress envelope.
    ///
    /// The envelope is RETURNED rather than consumed: what these tests
    /// assert is what the gates read while the response is still
    /// unwritten, and dropping the envelope is what closes its
    /// completion.
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
            // The host's outcome set is open, so an unrecognized
            // disposition fails loudly rather than being folded into
            // `Aborted` and silently passing an abort assertion.
            _ => panic!("unrecognized ingress dispatch outcome"),
        }
    }

    fn syntactic_nodes(value: &Value) -> usize {
        let children = match value {
            Value::Array(values) => values.iter().map(syntactic_nodes).sum::<usize>(),
            Value::Object(values) => values
                .len()
                .checked_add(values.values().map(syntactic_nodes).sum::<usize>())
                .expect("fixture object count fits"),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
        };
        children.checked_add(1).expect("fixture node count fits")
    }

    fn request_with_nodes(nodes: usize, extension: &Value) -> String {
        let mut request = json!({
            "id": "node-budget",
            "tmax": 1000_i32,
            "site": {
                "page": "https://publisher.example/story",
                "publisher": {"id": "acct-admission"}
            },
            "imp": [{
                "id": "imp-1",
                "banner": {"w": 300_i32, "h": 250_i32, "ext": extension},
                "ext": {"prebid": {"bidder": {"fake_bb": {}}}}
            }],
            "unknown": []
        });
        let padding = nodes
            .checked_sub(syntactic_nodes(&request))
            .expect("room for fixture");
        request["unknown"] = json!(vec![0_u8; padding]);
        assert_eq!(syntactic_nodes(&request), nodes);
        serde_json::to_string(&request).expect("fixture serializes")
    }

    #[test]
    fn node_budget_wire_body_exact_prefix_and_newline() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let admission = admission_gate().expect("admission installed");
        let statics = static_egress_gate().expect("terminal gate installed");
        let budget = usize::try_from(limits().max_parsed_nodes).expect("node cap fits");

        for disabled in [false, true] {
            for extension in [json!({}), json!({"nested": [{}, null, true]})] {
                for nodes in [budget - 1, budget, budget + 1] {
                    let body = request_with_nodes(nodes, &extension);
                    assert!(body.len() < usize::try_from(CAP_256_KIB).expect("byte cap fits"));
                    let entered = Arc::new(AtomicBool::new(false));
                    let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(ParkedFanout {
                        released: Arc::new(AtomicBool::new(true)),
                        entered: Arc::clone(&entered),
                    });
                    let mut request = auction_request(&body, fanout);
                    let cfg: AuctioneerAppConfig = serde_json::from_value(json!({
                        "schema_version": 1_u64,
                        "accounts": {"acct-admission": {"disabled": disabled}},
                    }))
                    .expect("config deserializes");
                    request.extensions_mut().insert(config_registry_with(&cfg));
                    let before = statics.in_use();
                    let mut envelope = dispatch(&app, request).expect("auction answered");
                    let expected_status = if nodes > budget {
                        StatusCode::BAD_REQUEST
                    } else if disabled {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        StatusCode::OK
                    };
                    assert_eq!(
                        envelope.response_mut().status(),
                        expected_status,
                        "nodes={nodes}, disabled={disabled}"
                    );
                    assert_eq!(entered.load(Ordering::SeqCst), nodes <= budget && !disabled);
                    if nodes > budget {
                        let response_body = mem::take(envelope.response_mut().body_mut());
                        let bytes = block_on(response_body.into_bytes_bounded(256))
                            .expect("bounded error response");
                        assert_eq!(bytes.as_ref(), b"Invalid request: too many JSON nodes\n");
                    }
                    assert_eq!(admission.in_use(), 0);
                    assert_eq!(statics.in_use(), before + 1);
                    drop(envelope);
                    assert_eq!(statics.in_use(), before);
                }
            }
        }
    }

    #[test]
    fn exhausted_phase_prepayment_is_terminal_before_body_or_fanout() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let charges = boot_gates().unwrap().derived.auction_charges;
        let mut boundary = 0_u64;
        for next in [
            charges.config,
            charges.ingress,
            charges.privacy,
            charges.projection,
            charges.outbound,
            charges.response,
        ] {
            for disabled in [false, true] {
                let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(boundary));
                capture_disclosure(&ledger);
                let entered = Arc::new(AtomicBool::new(false));
                let fanout = Arc::new(ParkedFanout {
                    released: Arc::new(AtomicBool::new(true)),
                    entered: Arc::clone(&entered),
                });
                let mut request = auction_request(VALID_REQUEST, fanout);
                request
                    .extensions_mut()
                    .insert(TestAuctionBudget(Arc::clone(&ledger)));
                let cfg: AuctioneerAppConfig = serde_json::from_value(json!({
                    "schema_version": 1_u64,
                    "host": {
                        "gdpr": {"enabled":!disabled},
                    },
                }))
                .unwrap();
                let reads = Arc::new(AtomicUsize::new(0));
                request
                    .extensions_mut()
                    .insert(config_registry_observed(&cfg, Some(Arc::clone(&reads))));
                // A poll would panic: reservation failure must win even with a
                // granting/disabled privacy policy and an unread body.
                *request.body_mut() = Body::from_stream(poll_fn(|_| {
                    panic!("body read after failed memory reservation")
                }));
                let mut response = dispatch(&app, request).unwrap();
                assert_eq!(
                    response.response_mut().status(),
                    StatusCode::SERVICE_UNAVAILABLE
                );
                let body = mem::take(response.response_mut().body_mut());
                assert_eq!(
                    block_on(body.into_bytes_bounded(256)).unwrap().as_ref(),
                    b"overloaded\n"
                );
                assert!(!entered.load(Ordering::SeqCst));
                assert_eq!(reads.load(Ordering::SeqCst), 0);
                assert_eq!(ledger.in_use(), 0);
                assert_eq!(admission_gate().unwrap().in_use(), 0);
                drop(response);
                assert_no_disclosure();
            }
            boundary += next;
        }
    }

    #[test]
    fn live_memory_charges_survive_suspension_and_release_on_cancel_or_success() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        for cancel in [false, true] {
            let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
            capture_disclosure(&ledger);
            let entered = Arc::new(AtomicBool::new(false));
            let released = Arc::new(AtomicBool::new(false));
            let fanout = Arc::new(ParkedFanout {
                released: Arc::clone(&released),
                entered: Arc::clone(&entered),
            });
            let mut request = auction_request(VALID_REQUEST, fanout);
            request
                .extensions_mut()
                .insert(TestAuctionBudget(Arc::clone(&ledger)));
            let mut dispatching = Box::pin(app.dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ));
            poll_until_parked(dispatching.as_mut(), &entered);
            assert_eq!(ledger.in_use(), total);
            if cancel {
                drop(dispatching);
            } else {
                released.store(true, Ordering::SeqCst);
                let outcome = block_on(dispatching).unwrap();
                assert_eq!(
                    ledger.in_use(),
                    0,
                    "work charges end after serialization, not at egress"
                );
                drop(outcome);
            }
            assert_eq!(ledger.in_use(), 0);
            assert_eq!(admission_gate().unwrap().in_use(), 0);
            assert_disclosure_before_release(
                if cancel { "cancelled" } else { "success" },
                1,
                0,
                [0; 8],
            );
        }
    }

    #[test]
    fn config_read_is_prepaid_and_cancellation_releases_all_phases() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
        capture_disclosure(&ledger);
        let config_entered = Arc::new(AtomicBool::new(false));
        let fanout_entered = Arc::new(AtomicBool::new(false));
        let mut request = auction_request(
            VALID_REQUEST,
            Arc::new(ParkedFanout {
                released: Arc::new(AtomicBool::new(true)),
                entered: Arc::clone(&fanout_entered),
            }),
        );
        request
            .extensions_mut()
            .insert(TestAuctionBudget(Arc::clone(&ledger)));
        request.extensions_mut().insert(config_registry_paused(
            &AuctioneerAppConfig::default(),
            None,
            Some(ParkedFanout {
                released: Arc::new(AtomicBool::new(false)),
                entered: Arc::clone(&config_entered),
            }),
        ));
        let mut pending = Box::pin(app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ));
        poll_until_parked(pending.as_mut(), &config_entered);
        assert_eq!(ledger.in_use(), total);
        assert_eq!(admission_gate().unwrap().in_use(), 1);
        assert!(!fanout_entered.load(Ordering::SeqCst));
        drop(pending);
        assert_eq!(ledger.in_use(), 0);
        assert_eq!(admission_gate().unwrap().in_use(), 0);
        assert_eq!(static_egress_gate().unwrap().in_use(), 0);
        assert_disclosure_before_release("cancelled", 0, 0, [0; 8]);
    }

    #[test]
    fn charged_config_and_parse_refusals_observe_error_before_release() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        for malformed_config in [false, true] {
            let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
            capture_disclosure(&ledger);
            let entered = Arc::new(AtomicBool::new(false));
            let mut request = auction_request(
                INVALID_REQUEST,
                Arc::new(ParkedFanout {
                    released: Arc::new(AtomicBool::new(true)),
                    entered: Arc::clone(&entered),
                }),
            );
            request.extensions_mut().insert(TestAuctionBudget(ledger));
            if malformed_config {
                request.extensions_mut().insert(StoreRegistry::single_id(
                    "auctioneer_runtime".to_owned(),
                    ConfigStoreBinding {
                        default_key: "auctioneer_runtime".to_owned(),
                        handle: ConfigStoreHandle::new(Arc::new(FixedStore(
                            "malformed-config".into(),
                            None,
                            None,
                        ))),
                    },
                ));
            }
            let mut envelope = dispatch(&app, request).expect("refusal answered");
            assert_eq!(
                envelope.response_mut().status(),
                if malformed_config {
                    StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    StatusCode::BAD_REQUEST
                }
            );
            if malformed_config {
                assert_eq!(
                    envelope.response_mut().body().as_bytes().unwrap(),
                    b"Malformed host configuration"
                );
            }
            assert!(!entered.load(Ordering::SeqCst));
            drop(envelope);
            assert_disclosure_before_release("error", 0, 0, [0; 8]);
        }
    }

    #[test]
    fn live_disclosure_counts_survive_until_success_or_fanout_cancellation() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let mut body: Value = serde_json::from_str(VALID_REQUEST).unwrap();
        body["user"] = json!({"id": "disclosure-secret"});
        body["device"] = json!({"lmt": 1_i32});
        let body_json = serde_json::to_string(&body).unwrap();
        for cancel in [false, true] {
            let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
            capture_disclosure(&ledger);
            let entered = Arc::new(AtomicBool::new(false));
            let released = Arc::new(AtomicBool::new(false));
            let mut request = auction_request(
                &body_json,
                Arc::new(ParkedFanout {
                    released: Arc::clone(&released),
                    entered: Arc::clone(&entered),
                }),
            );
            request.extensions_mut().insert(TestAuctionBudget(ledger));
            let cfg: AuctioneerAppConfig = serde_json::from_value(json!({
                "schema_version": 1_u64,
                "host": {
                    "lmt": {"enforce": true},
                },
            }))
            .unwrap();
            request.extensions_mut().insert(config_registry_with(&cfg));
            let mut pending = Box::pin(app.dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ));
            poll_until_parked(pending.as_mut(), &entered);
            if cancel {
                drop(pending);
            } else {
                released.store(true, Ordering::SeqCst);
                let outcome = block_on(pending).unwrap();
                drop(outcome);
            }
            assert_disclosure_before_release(
                if cancel { "cancelled" } else { "success" },
                1,
                0,
                [1, 0, 0, 0, 0, 0, 0, 0],
            );
        }
    }

    #[test]
    fn fanout_error_observes_prior_disclosure_changes_before_release() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
        capture_disclosure(&ledger);
        let mut body: Value = serde_json::from_str(VALID_REQUEST).unwrap();
        body["user"] = json!({"id": "disclosure-secret"});
        body["device"] = json!({"lmt": 1_i32});
        let body_json = serde_json::to_string(&body).unwrap();
        let mut request = auction_request(&body_json, Arc::new(UnavailableFanout));
        request.extensions_mut().insert(TestAuctionBudget(ledger));
        let cfg: AuctioneerAppConfig = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "host": {
                "lmt": {"enforce": true},
            },
        }))
        .unwrap();
        request.extensions_mut().insert(config_registry_with(&cfg));
        let mut envelope = dispatch(&app, request).expect("fanout refusal answered");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        drop(envelope);
        assert_disclosure_before_release("error", 1, 0, [1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    const IPV6_GROWTH_SOURCE: &str = "2001:4860::3:4:5:6:7";
    #[cfg(not(target_arch = "wasm32"))]
    const IPV6_GROWTH_CANONICAL: &str = "2001:4860:0:3:4:5:6:0";

    #[cfg(not(target_arch = "wasm32"))]
    #[derive(Default)]
    struct CanonicalIpv6Fanout {
        batches: AtomicUsize,
        bodies: Mutex<Vec<Value>>,
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[async_trait(?Send)]
    impl BidderFanout for CanonicalIpv6Fanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.batches.fetch_add(1, Ordering::SeqCst);
            for call in &calls {
                let body = serde_json::from_slice(call.request.body()).expect("outbound JSON");
                self.bodies
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(body);
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

    #[cfg(not(target_arch = "wasm32"))]
    fn drive_charged_ipv6_growth(
        app: &App,
        mode: super::ipv6_growth_allocations::Mode,
    ) -> (
        ResponseEgressEnvelope,
        Arc<CanonicalIpv6Fanout>,
        super::ipv6_growth_allocations::Snapshot,
    ) {
        assert_eq!(IPV6_GROWTH_SOURCE.len(), 20);
        assert_eq!(IPV6_GROWTH_CANONICAL.len(), 21);
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
        capture_disclosure(&ledger);
        let mut body: Value = serde_json::from_str(VALID_REQUEST).unwrap();
        let base = &serving_catalog::passthrough_info().name;
        body["imp"][0]["ext"]["prebid"]["bidder"] = json!({"fake_a": {}, "fake_b": {}});
        body["ext"] = json!({"prebid": {"aliases": {"fake_a": base, "fake_b": base}}});
        body["device"] = json!({"lmt": 1_i32, "ipv6": IPV6_GROWTH_SOURCE});
        body["regs"] = json!({"gdpr": 0_i32});
        // Offset alias growth in an existing string, preserving wire bytes and nodes.
        body["site"]["page"] = json!("https://t/");
        let padding = 358_usize
            .checked_sub(serde_json::to_vec(&body).unwrap().len())
            .expect("fixture aliases fit the original wire budget");
        body["site"]["page"] = json!(format!("https://t/{}", "x".repeat(padding)));
        let body_json = serde_json::to_string(&body).unwrap();
        assert_eq!(body_json.len(), 358);
        let fanout = Arc::new(CanonicalIpv6Fanout::default());
        let mut request = auction_request(&body_json, Arc::<CanonicalIpv6Fanout>::clone(&fanout));
        request.extensions_mut().insert(TestAuctionBudget(ledger));
        let cfg: AuctioneerAppConfig = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "host": {
                "lmt": {"enforce": true},
            },
            "accounts": {
                "acct-admission": {
                    "privacy": {
                        "ipv6": {"anon_keep_bits": 112_u8},
                    },
                },
            },
        }))
        .unwrap();
        request.extensions_mut().insert(config_registry_with(&cfg));
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let wall_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        request.extensions_mut().insert(TestClockHandle(wall_clock));
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                super::ipv6_growth_allocations::between_scopes();
                clock.now_monotonic()
            })));
        let observation = super::ipv6_growth_allocations::start(mode);
        let envelope = dispatch(app, request).expect("charged auction answered");
        let allocations = observation.finish();
        (envelope, fanout, allocations)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn assert_charged_ipv6_growth_control(app: &App) {
        let (mut envelope, fanout, allocations) =
            drive_charged_ipv6_growth(app, super::ipv6_growth_allocations::Mode::Control);
        assert_eq!(envelope.response_mut().status(), StatusCode::OK);
        assert_eq!(
            allocations.matching_reallocs, 2,
            "only the two scrub resizes match"
        );
        assert_eq!(allocations.successful_reallocs, 2);
        assert_eq!(allocations.between_scope_arms, 1);
        assert_eq!(allocations.injected_failures, 0);
        assert_eq!(fanout.batches.load(Ordering::SeqCst), 1);
        {
            let bodies = fanout.bodies.lock().unwrap_or_else(PoisonError::into_inner);
            assert_eq!(bodies.len(), 2);
            for body in bodies.iter() {
                assert_eq!(body["device"]["ipv6"].as_str(), Some(IPV6_GROWTH_CANONICAL));
            }
        }
        drop(envelope);
        assert_disclosure_before_release("success", 2, 0, [0, 0, 2, 0, 0, 0, 0, 0]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn charged_ipv6_growth_control_scrubs_two_scopes_before_dispatch() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        assert_charged_ipv6_growth_control(&app);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn later_scope_ipv6_reserve_failure_observes_prior_changes_before_release() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        // Qualify the signature even when this failure test runs by itself.
        assert_charged_ipv6_growth_control(&app);
        let (mut envelope, fanout, allocations) =
            drive_charged_ipv6_growth(&app, super::ipv6_growth_allocations::Mode::FailSecond);
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let response_body = mem::take(envelope.response_mut().body_mut());
        let bytes =
            block_on(response_body.into_bytes_bounded(256)).expect("bounded error response");
        assert_eq!(
            bytes.as_ref(),
            b"internal: request disclosure construction limit exceeded\n"
        );
        assert_eq!(allocations.matching_reallocs, 2);
        assert_eq!(allocations.successful_reallocs, 1);
        assert_eq!(allocations.between_scope_arms, 1);
        assert_eq!(
            allocations.injected_failures, 1,
            "recoverable fault exercised"
        );
        assert_eq!(fanout.batches.load(Ordering::SeqCst), 0);
        assert!(
            fanout
                .bodies
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_empty()
        );
        drop(envelope);
        assert_disclosure_before_release("error", 2, 1, [0, 0, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn disabled_disclosure_logging_still_releases_charged_handler_resources() {
        struct RestoreFilter(log::LevelFilter);
        #[expect(
            clippy::missing_trait_methods,
            reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
        )]
        impl Drop for RestoreFilter {
            fn drop(&mut self) {
                log::set_max_level(self.0);
            }
        }
        boot();
        let _serialized = gate_counters_lock();
        // Every test in this binary uses the same gate lock, including logger users.
        let _restore = RestoreFilter(log::max_level());
        log::set_max_level(log::LevelFilter::Off);
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        for cancel in [false, true] {
            let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
            capture_disclosure(&ledger);
            let entered = Arc::new(AtomicBool::new(false));
            let released = Arc::new(AtomicBool::new(false));
            let mut request = auction_request(
                VALID_REQUEST,
                Arc::new(ParkedFanout {
                    released: Arc::clone(&released),
                    entered: Arc::clone(&entered),
                }),
            );
            request
                .extensions_mut()
                .insert(TestAuctionBudget(Arc::clone(&ledger)));
            let mut pending = Box::pin(app.dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ));
            poll_until_parked(pending.as_mut(), &entered);
            assert_eq!(ledger.in_use(), total);
            if cancel {
                drop(pending);
            } else {
                released.store(true, Ordering::SeqCst);
                drop(block_on(pending).unwrap());
            }
            assert_eq!(ledger.in_use(), 0);
            assert_eq!(admission_gate().unwrap().in_use(), 0);
            DISCLOSURE_CAPTURE.with(|slot| {
                let capture = slot.borrow_mut().take().expect("capture started");
                assert!(
                    capture.observations.is_empty(),
                    "debug observation filtered"
                );
            });
        }
    }

    #[test]
    fn suspended_body_cancellation_observes_zero_before_resource_release() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
        capture_disclosure(&ledger);
        let body_entered = Arc::new(AtomicBool::new(false));
        let fanout_entered = Arc::new(AtomicBool::new(false));
        let mut request = auction_request(
            VALID_REQUEST,
            Arc::new(ParkedFanout {
                released: Arc::new(AtomicBool::new(true)),
                entered: Arc::clone(&fanout_entered),
            }),
        );
        request.extensions_mut().insert(TestAuctionBudget(ledger));
        let entered = Arc::clone(&body_entered);
        *request.body_mut() = Body::from_stream(poll_fn(move |_| {
            entered.store(true, Ordering::SeqCst);
            Poll::Pending
        }));
        let mut pending = Box::pin(app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ));
        poll_until_parked(pending.as_mut(), &body_entered);
        assert!(!fanout_entered.load(Ordering::SeqCst));
        drop(pending);
        assert_disclosure_before_release("cancelled", 0, 0, [0; 8]);
    }

    #[test]
    fn zero_survivor_success_observes_zero_before_resource_release() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let ledger = Arc::new(auctioneer_runtime::AuctionBudget::new(total));
        capture_disclosure(&ledger);
        let entered = Arc::new(AtomicBool::new(false));
        let mut request = auction_request(
            VALID_REQUEST,
            Arc::new(ParkedFanout {
                released: Arc::new(AtomicBool::new(true)),
                entered: Arc::clone(&entered),
            }),
        );
        request.extensions_mut().insert(TestAuctionBudget(ledger));
        let cfg: AuctioneerAppConfig = serde_json::from_value(json!({
            "schema_version": 1_u64,
            "accounts": {
                "acct-admission": {
                    "privacy": {
                        "allowactivities": {
                            "fetchBids": {"default": false},
                        },
                    },
                },
            },
        }))
        .unwrap();
        request.extensions_mut().insert(config_registry_with(&cfg));
        let mut envelope = dispatch(&app, request).expect("zero-call auction answered");
        assert_eq!(envelope.response_mut().status(), StatusCode::OK);
        assert!(!entered.load(Ordering::SeqCst));
        drop(envelope);
        assert_disclosure_before_release("success", 0, 0, [0; 8]);
    }

    /// Polls `future` until it parks, without completing it.
    ///
    /// `futures::executor::block_on` would run the parked fanout
    /// forever; this steps the same future a bounded number of times so
    /// the test can observe the mid-flight instant and then resume.
    fn poll_until_parked<F: Future>(mut future: Pin<&mut F>, entered: &Arc<AtomicBool>) {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        for _step in 0..10_000_u32 {
            if entered.load(Ordering::SeqCst) {
                return;
            }
            match future.as_mut().poll(&mut cx) {
                Poll::Pending => {}
                Poll::Ready(_done) => {
                    panic!("the auction completed without ever entering the fanout");
                }
            }
        }
        panic!("the auction never reached the fanout");
    }

    #[test]
    fn auction_capture_preserves_original_lease_clock_and_extraction_limits() {
        boot();
        let _serialized = gate_counters_lock();
        let admission = admission_gate().expect("admission installed");
        let statics = static_egress_gate().expect("terminal gate installed");
        let terminal_before = statics.in_use();
        assert_eq!(admission.in_use(), 0);
        let now = MonotonicInstant::now();
        let request = request_builder()
            .method(Method::POST)
            .uri("/openrtb2/auction")
            .body(Body::empty())
            .expect("capture request builds");
        let (ctx, envelope) = config_store::auction_context_for_route(
            request,
            "/openrtb2/{operation}",
            MonotonicClock::new(move || now),
        );

        assert_eq!(ctx.request_start(), now);
        assert_eq!(ctx.monotonic_clock().now(), now);
        assert_eq!(ctx.path_params().get("operation").unwrap(), "auction");
        assert_eq!(
            ctx.config_extraction_limits(),
            configured_app().config_extraction_limits()
        );
        assert_eq!(admission.in_use(), 1);
        assert_eq!(statics.in_use(), terminal_before + 1);
        let lease = ctx
            .take_ingress_grant()
            .expect("capture retains the original grant")
            .downcast::<AuctioneerIngressLease>()
            .expect("production admission supplies the auction lease");
        assert_eq!(lease.admission, RouteAdmissionMarker::Auction);
        assert!(lease.auction_admission.is_some());
        assert!(ctx.take_ingress_grant().is_none());
        drop(ctx);
        assert_eq!(admission.in_use(), 1, "the original lease owns capacity");
        assert!(admission.try_acquire().is_none());

        drop(lease);
        assert_eq!(admission.in_use(), 0);
        assert_eq!(
            statics.in_use(),
            terminal_before + 1,
            "releasing work does not release the paired terminal completion"
        );
        drop(envelope);
        assert_eq!(statics.in_use(), terminal_before);
    }

    #[test]
    fn auction_capture_holds_capacity_through_handler_until_explicit_envelope_release() {
        boot();
        let _serialized = gate_counters_lock();
        let admission = admission_gate().expect("admission installed");
        let statics = static_egress_gate().expect("terminal gate installed");
        let egress = egress_gate().expect("egress gate installed");
        let terminal_before = statics.in_use();
        let egress_before = egress.in_use();
        assert_eq!(admission.in_use(), 0);

        for cancel in [false, true] {
            let entered = Arc::new(AtomicBool::new(false));
            let released = Arc::new(AtomicBool::new(false));
            let fanout = Arc::new(ParkedFanout {
                released: Arc::clone(&released),
                entered: Arc::clone(&entered),
            });
            let request = auction_request(VALID_REQUEST, fanout);
            let clock = request
                .extensions()
                .get::<TestMonotonicClockHandle>()
                .expect("request has its deterministic clock")
                .0
                .clone();
            let (ctx, mut envelope) =
                config_store::auction_context_for_route(request, "/openrtb2/auction", clock);
            assert!(
                !entered.load(Ordering::SeqCst),
                "capture does not run fanout"
            );
            assert_eq!(admission.in_use(), 1);
            assert_eq!(statics.in_use(), terminal_before + 1);
            assert_eq!(egress.in_use(), egress_before);
            assert!(admission.try_acquire().is_none());

            let mut operation = Box::pin(auction(ctx));
            assert_eq!(
                admission.in_use(),
                1,
                "an unpolled handler retains its grant"
            );
            poll_until_parked(operation.as_mut(), &entered);
            assert_eq!(
                admission.in_use(),
                1,
                "the handler reuses its original admission"
            );
            assert!(admission.try_acquire().is_none());
            assert_eq!(statics.in_use(), terminal_before + 1);
            assert_eq!(egress.in_use(), egress_before);

            if cancel {
                drop(operation);
            } else {
                released.store(true, Ordering::SeqCst);
                *envelope.response_mut() =
                    block_on(operation).expect("the admitted handler answers");
                assert_eq!(envelope.response_mut().status(), StatusCode::OK);
            }
            assert_eq!(admission.in_use(), 0);
            assert_eq!(
                statics.in_use(),
                terminal_before + 1,
                "handler completion or cancellation cannot close the original envelope"
            );
            assert_eq!(
                egress.in_use(),
                egress_before + u32::from(!cancel),
                "a successful handler installs its permit into the still-open original completion"
            );

            drop(envelope);
            assert_eq!(admission.in_use(), 0);
            assert_eq!(statics.in_use(), terminal_before);
            assert_eq!(egress.in_use(), egress_before);
        }
    }

    #[test]
    fn auction_capture_does_not_change_extraction_only_helpers() {
        boot();
        let _serialized = gate_counters_lock();
        let admission = admission_gate().expect("admission installed");
        let statics = static_egress_gate().expect("terminal gate installed");
        let terminal_before = statics.in_use();
        let expected_limits = configured_app().config_extraction_limits();
        assert_eq!(admission.in_use(), 0);

        for explicit_route in [false, true] {
            let request = request_builder()
                .method(Method::POST)
                .uri("/openrtb2/auction")
                .body(Body::empty())
                .expect("extraction-only request builds");
            let ctx = if explicit_route {
                config_store::context_for_route(
                    request,
                    "/openrtb2/{operation}",
                    MonotonicClock::default(),
                )
            } else {
                config_store::context(request)
            };
            assert_eq!(ctx.config_extraction_limits(), expected_limits);
            assert!(
                ctx.take_ingress_grant()
                    .and_then(|grant| grant.downcast::<AuctioneerIngressLease>().ok())
                    .is_none(),
                "extraction-only contexts must not gain auction admission"
            );
            assert_eq!(admission.in_use(), 0);
            assert_eq!(statics.in_use(), terminal_before);
            drop(ctx);
            assert_eq!(admission.in_use(), 0);
            assert_eq!(statics.in_use(), terminal_before);
        }
    }

    /// The concurrency slot is held for the WHOLE auction, not returned
    /// the instant the handler takes its egress slot out of the lease.
    ///
    /// `max_concurrent_auctions` is one here, so the assertion is
    /// direct: while the first auction is parked inside its fanout, the
    /// class must have no capacity left. An early release shows up as a
    /// free slot, which is the over-admission the concurrency term
    /// exists to prevent.
    #[test]
    fn a_mid_flight_auction_holds_its_concurrency_slot() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let admission = admission_gate().expect("boot installed the admission gate");
        assert_eq!(
            admission.in_use(),
            0,
            "the fixture starts from an idle class"
        );

        let released = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(AtomicBool::new(false));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(ParkedFanout {
            released: Arc::clone(&released),
            entered: Arc::clone(&entered),
        });
        let request = auction_request(VALID_REQUEST, fanout);

        let dispatching = app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        );
        futures::pin_mut!(dispatching);
        poll_until_parked(dispatching.as_mut(), &entered);

        // The auction is now inside its fanout: body parsed, bidders
        // split, calls dispatched, nothing answered yet.
        assert_eq!(
            admission.in_use(),
            1,
            "the in-flight auction still holds the concurrency slot it was admitted with",
        );
        assert!(
            admission.try_acquire().is_none(),
            "at capacity one a second auction cannot be admitted while the first is \
             mid-fanout: releasing the slot before the auction runs defeats the concurrency \
             term in the memory envelope",
        );

        // Let the parked auction finish and drain the response.
        released.store(true, Ordering::SeqCst);
        let outcome = block_on(dispatching).expect("admission begins");
        let IngressDispatchOutcome::Response(envelope) = outcome else {
            panic!("the completed auction is answered, not aborted");
        };

        assert_eq!(
            admission.in_use(),
            0,
            "the concurrency slot returns when the auction's work ends",
        );
        assert!(
            admission.try_acquire().is_some(),
            "and the class is reusable by the next auction",
        );
        drop(envelope);
    }

    /// Every admitted auction holds a terminal-response slot until its
    /// response reaches terminal egress — including one that answers
    /// with a validation error long before an egress permit exists.
    ///
    /// The error path is the one that cannot be covered by the egress
    /// class: `egress_leg` never runs, so nothing is ever installed into
    /// the late-bound slot, and the response travels out accounted for
    /// by nothing at all unless admission reserved a terminal slot for
    /// it.
    #[test]
    fn an_early_error_response_holds_a_terminal_slot_until_egress() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let statics = static_egress_gate().expect("boot installed the static class");
        let admission = admission_gate().expect("boot installed the admission gate");
        let before = statics.in_use();

        // A fanout that would park if it were reached. It is not: the
        // request fails structural validation first, which is the whole
        // point of this fixture.
        let released = Arc::new(AtomicBool::new(true));
        let entered = Arc::new(AtomicBool::new(false));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(ParkedFanout {
            released,
            entered: Arc::clone(&entered),
        });
        let mut envelope = dispatch(&app, auction_request(INVALID_REQUEST, fanout))
            .expect("an invalid auction is answered, not aborted");

        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::BAD_REQUEST,
            "a request with no impressions is a validation error",
        );
        assert!(
            !entered.load(Ordering::SeqCst),
            "the fixture never reached the fanout, so no egress permit was ever acquired",
        );
        assert_eq!(
            statics.in_use(),
            before.saturating_add(1),
            "the error response holds a terminal-response slot while it is still unwritten: \
             an admitted request's answer is a response like any other",
        );

        drop(envelope);
        assert_eq!(
            statics.in_use(),
            before,
            "terminal egress releases the reserved slot exactly once",
        );
        assert_eq!(
            admission.in_use(),
            0,
            "and the auction's concurrency slot is back too",
        );
    }

    /// A successful auction's terminal slot is reserved at admission and
    /// released once, not leaked and not double-released, alongside the
    /// egress-class permit its `egress_leg` installs.
    #[test]
    fn a_successful_auction_reserves_and_releases_one_terminal_slot() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let statics = static_egress_gate().expect("boot installed the static class");
        let before = statics.in_use();

        let released = Arc::new(AtomicBool::new(true));
        let entered = Arc::new(AtomicBool::new(false));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(ParkedFanout {
            released,
            entered: Arc::clone(&entered),
        });
        let mut envelope = dispatch(&app, auction_request(VALID_REQUEST, fanout))
            .expect("a valid auction is answered");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::OK,
            "the auction answered normally",
        );
        assert!(
            entered.load(Ordering::SeqCst),
            "the auction really ran its fanout"
        );
        assert_eq!(
            statics.in_use(),
            before.saturating_add(1),
            "the auction response holds its reserved terminal slot until it is written",
        );

        drop(envelope);
        assert_eq!(
            statics.in_use(),
            before,
            "terminal egress releases it exactly once",
        );
    }

    /// An abandoned response — the client disconnected, nothing was
    /// transmitted — still returns both reservations. Cancellation is
    /// the path where a leak would be permanent, because no terminal
    /// report ever arrives.
    #[test]
    fn an_abandoned_auction_response_returns_both_reservations() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let statics = static_egress_gate().expect("boot installed the static class");
        let admission = admission_gate().expect("boot installed the admission gate");
        let statics_before = statics.in_use();

        let released = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(AtomicBool::new(false));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(ParkedFanout {
            released: Arc::clone(&released),
            entered: Arc::clone(&entered),
        });
        // Boxed rather than stack-pinned: `pin_mut!` pins in place, so
        // dropping the resulting `Pin` would drop only the wrapper and
        // leave the future — and everything it owns — alive until the
        // end of scope. Cancellation has to drop the FUTURE, which is
        // what a box lets the test do at a chosen instant.
        let mut dispatching = Box::pin(app.dispatch_ingress(
            auction_request(VALID_REQUEST, fanout),
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ));
        poll_until_parked(dispatching.as_mut(), &entered);

        assert_eq!(
            statics.in_use(),
            statics_before.saturating_add(1),
            "the in-flight auction already holds its terminal reservation",
        );

        // Drop the dispatch future itself: the request is cancelled
        // mid-auction and no response is ever produced.
        drop(dispatching);
        assert_eq!(
            statics.in_use(),
            statics_before,
            "cancelling mid-auction returns the terminal reservation rather than stranding it",
        );
        assert_eq!(
            admission.in_use(),
            0,
            "and the concurrency slot comes back with the cancelled work",
        );
    }
}
