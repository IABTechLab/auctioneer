//! The installed ingress admission policy, driven through a real
//! configured `App`. The missing-boot row runs in an isolated child
//! process; the other rows run with a boot envelope installed.
//!
//! These are the assertions that could not be written before the policy
//! was installed. Everything in `egress_binding.rs` proves the lease and
//! the completion behave correctly once someone builds them; this binary
//! proves that the HOST builds them, on every route class, for responses
//! no auctioneer handler produces.
//!
//! The gates are process-global `OnceLock`s and the harness runs tests
//! in parallel, so every gate-accounting test takes one mutex. A snapshot
//! delta is not sufficient: a concurrent holder can acquire and release
//! between the snapshot and the assertion, which would make an
//! over-admission look like correct accounting.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::host_primitives::gates::BootGates;
    use auctioneer_core::host_primitives::{
        AuctioneerIngressLease, BootEnvelope, DelegateToDevStrategy, DevRng, RouteAdmissionMarker,
        StderrBootLog, SystemClock, admission_gate, aux_gate, boot_gates,
        bootstrap_adapter_with_envelope, probe_egress_gate, static_egress_gate,
    };
    use auctioneer_core::ingress::{AUCTION_CLASS, AUX_CLASS, PROBE_CLASS, decide};
    use auctioneer_runtime::body_caps::DEFAULT_CAP_BYTES;
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        ImmediateResolveTimer, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator,
        ResolvedIsolateLimits, TargetPlatformConstants,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
        DEFAULT_CONFIG_JSON_NODES,
    };
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{Method, Request, StatusCode, request_builder};
    use edgezero_core::ingress::{
        AdmissionDecision, IngressDispatchOutcome, IngressFraming, IngressHead,
        IngressHeadAccounting,
    };
    use edgezero_core::response_egress::{
        ResponseEgressBodyKind, ResponseEgressEnvelope, ResponseEgressFallbackDisposition,
        ResponseEgressObserver, ResponseEgressOutcome, ResponseEgressReport,
    };
    use edgezero_core::response_egress_framing::ResponseEgressFramingMode;
    use edgezero_core::router::RouteResolution;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
    use futures::{
        channel::oneshot, executor::block_on, future::pending, stream, task::noop_waker_ref,
    };
    use std::collections::HashSet;
    use std::future::Future as _;
    use std::iter::repeat_with;
    use std::mem::take;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
    use std::task::{Context, Poll};

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

    /// Capacity ONE on both route classes, which is what makes the
    /// hold-until-terminal assertions provable: a second request either
    /// can or cannot acquire, and there is no slack to hide an early
    /// release in.
    const AUX_PERMITS: u32 = 1;
    const CONCURRENT_AUCTIONS: u32 = 1;
    const TERMINAL_PERMITS: u32 = 8;
    // Every terminal value in the pinned SDK, supplied from the writing state.
    const TERMINAL_OUTCOMES: [ResponseEgressOutcome; 9] = [
        ResponseEgressOutcome::Completed,
        ResponseEgressOutcome::ClientDisconnected,
        ResponseEgressOutcome::ConversionError,
        ResponseEgressOutcome::DeadlineExceeded,
        ResponseEgressOutcome::HostHandoff,
        ResponseEgressOutcome::RequestCancelled,
        ResponseEgressOutcome::SourceError,
        ResponseEgressOutcome::TransportError,
        ResponseEgressOutcome::Unspecified,
    ];

    /// Serializes every test that reads a gate's in-use counter.
    ///
    /// The gates bound the ISOLATE, so they are process-global and the
    /// parallel harness would have two tests draw from the same class.
    /// Snapshot deltas cannot fix that — a concurrent holder can acquire
    /// and release between the snapshot and the assertion — so the
    /// sharing is removed rather than tolerated.
    fn gate_counters_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn boot() {
        static BOOTED: OnceLock<()> = OnceLock::new();
        BOOTED.get_or_init(|| install_envelope(&limits()));
    }

    fn install_envelope(limits: &ResolvedIsolateLimits) {
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                limits,
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
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: AUX_PERMITS,
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

    fn configured_app() -> App {
        let mut app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("the app builds");
        configure_app(&mut app).expect("configure is idempotent");
        app
    }

    /// Drives one request all the way through admission, dispatch and
    /// into an owned egress envelope — the real path, policy included.
    ///
    /// The envelope is RETURNED rather than consumed, because the whole
    /// point of these tests is what the gate reads while the response is
    /// still unwritten. Dropping the envelope is what closes its
    /// completion.
    fn dispatch(
        app: &App,
        method: Method,
        path: &str,
        body: Body,
    ) -> Option<ResponseEgressEnvelope> {
        dispatch_with_origin(app, method, path, body, None)
    }

    fn dispatch_with_origin(
        app: &App,
        method: Method,
        path: &str,
        body: Body,
        origin: Option<&str>,
    ) -> Option<ResponseEgressEnvelope> {
        let mut builder = request_builder().method(method).uri(path);
        if let Some(origin_value) = origin {
            builder = builder.header("origin", origin_value);
        }
        let request = builder.body(body).expect("the fixture request builds");
        block_on(dispatch_request(app, request))
    }

    fn assert_auxiliary_receipt_held(gates: &BootGates, peak: u64) {
        assert_eq!(gates.aux.in_use(), 1);
        assert_eq!(gates.aux_work_budget.in_use(), peak);
    }

    #[cfg(not(target_family = "wasm"))]
    fn ran_isolated_child(control: &str, test: &str) -> bool {
        use std::env::{current_exe, var_os};
        use std::process::Command;
        if var_os(control).is_some() {
            return false;
        }
        let child = Command::new(current_exe().expect("ingress test executable"))
            .args(["--exact", test, "--nocapture"])
            .env(control, "1")
            .output()
            .expect("isolated ingress test executes");
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        assert!(child.status.success(), "{output}");
        assert!(
            output.contains("1 passed"),
            "the child must execute: {output}"
        );
        true
    }

    #[cfg(not(target_family = "wasm"))]
    fn immediate_probe(app: &App) -> ResponseEgressEnvelope {
        use futures::FutureExt as _;
        let request = request_builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let outcome = app
            .dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            )
            .now_or_never()
            .expect("liveness must not wait for disabled aux capacity")
            .unwrap();
        let IngressDispatchOutcome::Response(response) = outcome else {
            panic!("the independent probe must be admitted");
        };
        *response
    }

    async fn dispatch_request(app: &App, request: Request) -> Option<ResponseEgressEnvelope> {
        let outcome = app
            .dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            )
            .await
            .expect("admission begins");
        match outcome {
            IngressDispatchOutcome::Response(envelope) => Some(*envelope),
            IngressDispatchOutcome::Aborted => None,
            // The host's outcome set is open. A third disposition is not
            // something this fixture can interpret, so it fails loudly
            // rather than being folded into `Aborted` and silently
            // passing the abort assertions.
            _ => panic!("unrecognized ingress dispatch outcome"),
        }
    }

    enum StoreBehavior {
        Pending,
        Unavailable,
    }

    struct AdmissionStore {
        behavior: StoreBehavior,
        reads: AtomicUsize,
        active: AtomicUsize,
    }

    struct StoreRead<'a>(&'a AtomicUsize);

    impl Drop for StoreRead<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[async_trait(?Send)]
    impl ConfigStore for AdmissionStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("admitted configuration must use bounded reads");
        }

        async fn get_bounded(
            &self,
            _key: &str,
            _clock: &MonotonicClock,
            _deadline: Deadline,
            _max_backend_bytes: u64,
            _max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.active.fetch_add(1, Ordering::SeqCst);
            let _read = StoreRead(&self.active);
            match self.behavior {
                StoreBehavior::Pending => pending().await,
                StoreBehavior::Unavailable => Err(ConfigStoreError::Unavailable {
                    message: "fixed test outage".to_owned(),
                }),
            }
        }
    }

    fn admission_store(behavior: StoreBehavior) -> Arc<AdmissionStore> {
        Arc::new(AdmissionStore {
            behavior,
            reads: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
        })
    }

    fn store_request(method: Method, path: &str, store: Arc<AdmissionStore>) -> Request {
        let mut request = request_builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let registry: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(store),
            },
        );
        request.extensions_mut().insert(registry);
        request
    }

    /// Builds the admission view the policy sees, without a router: the
    /// resolution is supplied directly so a single decision arm can be
    /// tested in isolation.
    fn head(method: Method, path: &str, resolution: RouteResolution) -> IngressHead {
        let request = request_builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .expect("the fixture request builds");
        IngressHead::from_request(
            &request,
            MonotonicInstant::now(),
            resolution,
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        )
    }

    // -----------------------------------------------------------------
    // The policy is actually installed.
    // -----------------------------------------------------------------

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn an_auction_without_boot_gates_aborts_before_body_reads() {
        use auctioneer_core::host_primitives::{boot_gates, bootstrap_adapter};
        use futures::stream::poll_fn;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::task::Poll;

        const CHILD_ENV: &str = "AUCTIONEER__TEST__INGRESS_WITHOUT_BOOT_GATES_CHILD";
        // OnceLock gates cannot be cleared; a fresh process excludes sibling boots.
        if ran_isolated_child(
            CHILD_ENV,
            "tests::an_auction_without_boot_gates_aborts_before_body_reads",
        ) {
            return;
        }

        bootstrap_adapter(
            || Arc::new(SystemClock),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        );
        assert!(
            boot_gates().is_none(),
            "primitives alone install no boot gates"
        );
        let app = configured_app();
        assert!(
            boot_gates().is_none(),
            "app configuration must not install gates"
        );
        assert!(
            static_egress_gate().is_none(),
            "no bounded 503 can be funded"
        );
        let resolved = app.router().resolve(&Method::POST, "/openrtb2/auction");
        assert!(
            matches!(
                resolved.resolution(),
                RouteResolution::Matched(route) if route.class() == Some(AUCTION_CLASS)
            ),
            "the fixture must reach auction admission, not a fallback",
        );

        let reads = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&reads);
        let body = Body::stream(poll_fn(move |_cx| {
            observed.fetch_add(1, Ordering::SeqCst);
            Poll::Ready(None)
        }));
        // An admitted dispatch always returns a response, so Abort also proves
        // that neither the handler nor its config reads or bidder calls ran.
        assert!(
            dispatch(&app, Method::POST, "/openrtb2/auction", body).is_none(),
            "without boot gates the served auction must abort before dispatch",
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0, "admission is body-blind");
    }

    /// The host's DEFAULT policy admits every request with an empty
    /// grant, so a handler finds no lease and its permit releases at
    /// handler return rather than at terminal egress. This is the
    /// assertion that the default is gone: a real dispatch through a
    /// configured app leaves a auctioneer lease where the handler looks for
    /// one.
    #[test]
    fn a_configured_app_installs_auctioneers_admission_policy() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = aux_gate().expect("boot installed the aux class");
        let before = gate.in_use();

        // An aux route rather than the probe: this asserts auctioneer's own
        // policy replaced the host default, which shows up as a charged
        // class. The probe has an independent output pool.
        let envelope = dispatch(&app, Method::GET, "/info/bidders", Body::empty())
            .expect("a booted app answers /status");
        assert_eq!(
            gate.in_use(),
            before + 1,
            "the aux route's permit is still held while its response is unwritten; with the \
             host's default policy installed nothing would have been acquired at all",
        );
        drop(envelope);
        assert_eq!(
            gate.in_use(),
            before,
            "terminal egress releases it exactly once",
        );
    }

    // -----------------------------------------------------------------
    // The fallback arms, which no handler serves.
    // -----------------------------------------------------------------

    /// A canonical 404 holds aux capacity until its response reaches
    /// terminal egress.
    ///
    /// This is the assertion the permit-ownership rule exists for, and
    /// the one a handler-served aux test cannot make: the host's router
    /// DROPS the ingress grant before it builds this response — verified
    /// in its fallback drain, where `drop(grant)` precedes the match that
    /// produces the 404. A permit stored in the grant would be released
    /// there, while the 404 was still unwritten, and the next request
    /// would acquire the slot that response is still going to occupy.
    #[test]
    fn fallback_404_holds_aux_capacity_until_terminal() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = aux_gate().expect("boot installed the aux class");
        let work = boot_gates().expect("boot installed the aux work budget");
        assert_eq!(gate.in_use(), 0, "the fixture starts from an idle class");
        assert_eq!(work.aux_work_budget.in_use(), 0);

        let mut envelope = dispatch_with_origin(
            &app,
            Method::GET,
            "/no/such/route",
            Body::empty(),
            Some("https://buyer.example"),
        )
        .expect("an unmatched route is answered, not aborted");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::NOT_FOUND,
            "the host's canonical 404 is what came back",
        );
        assert_eq!(
            envelope
                .response_mut()
                .headers()
                .get("cache-control")
                .unwrap(),
            "no-store"
        );
        assert!(
            envelope
                .response_mut()
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
        assert_eq!(
            gate.in_use(),
            1,
            "the 404 still holds its aux permit: the grant it rode in was dropped by the \
             router's fallback drain, but the permit lives in the completion",
        );
        assert_eq!(
            work.aux_work_budget.in_use(),
            work.aux_work_derived.peak_bytes()
        );
        assert!(
            gate.try_acquire().is_none(),
            "at capacity one a second aux request cannot acquire while the 404 is unwritten",
        );

        drop(envelope);
        assert_eq!(gate.in_use(), 0, "terminal egress releases it");
        assert_eq!(work.aux_work_budget.in_use(), 0);
        assert!(
            gate.try_acquire().is_some(),
            "and the slot is reusable by the next request",
        );
    }

    /// The same for a wrong-method 405, which takes the other fallback
    /// arm through the same drain.
    #[test]
    fn fallback_405_holds_aux_capacity_until_terminal() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = aux_gate().expect("boot installed the aux class");
        assert_eq!(gate.in_use(), 0, "the fixture starts from an idle class");

        // `/status` is wired GET-only, so a POST resolves to
        // MethodNotAllowed rather than NotFound.
        let mut envelope = dispatch(&app, Method::POST, "/status", Body::empty())
            .expect("a wrong-method request is answered, not aborted");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "the host's canonical 405 is what came back",
        );
        assert_eq!(
            gate.in_use(),
            1,
            "the 405 holds its aux permit through its own transmission",
        );

        drop(envelope);
        assert_eq!(gate.in_use(), 0);
    }

    /// A fallback drain whose body crosses the cap answers auctioneer's
    /// bounded oversize response — and still holds its permit until that
    /// response is terminal. The drain is the only reason the body is
    /// read at all, so it gets the small non-body-route cap.
    #[test]
    fn a_fallback_body_over_the_cap_is_bounded_and_still_holds_its_permit() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = aux_gate().expect("boot installed the aux class");
        assert_eq!(gate.in_use(), 0);

        let oversize = vec![b'x'; DEFAULT_CAP_BYTES + 1];
        let mut envelope = dispatch(&app, Method::POST, "/no/such/route", Body::from(oversize))
            .expect("an oversize fallback body is answered, not aborted");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::BAD_REQUEST,
            "crossing the drain cap is a 400, not the 404 the route would otherwise get",
        );
        assert_eq!(
            envelope
                .response_mut()
                .headers()
                .get("cache-control")
                .unwrap(),
            "no-store"
        );
        assert_eq!(
            gate.in_use(),
            1,
            "the oversize rejection holds its permit through its own transmission too",
        );
        drop(envelope);
        assert_eq!(gate.in_use(), 0);
    }

    // -----------------------------------------------------------------
    // Saturation: refuse through the static class, then abort.
    // -----------------------------------------------------------------

    /// A saturated aux class refuses with a bounded 503 that travels out
    /// holding a STATIC-class permit.
    ///
    /// The two halves of this — that a full gate refuses, and that the
    /// refusal travels through the static class — could not be asserted
    /// together before the policy existed: the gate half had no response
    /// to look at, and the static-class half had no admission decision to
    /// produce one. Both are here.
    /// A saturated aux class must not shed the liveness probe.
    ///
    /// This is the property the probe class exists for. Whatever polls
    /// `/status` reads 503 as "dead" and restarts the instance, so
    /// charging the probe aux capacity would let a burst of info-route
    /// traffic take a healthy isolate down through its own health check.
    /// The handler is permissive by design — it answers 200 with
    /// `config: degraded (<code>)` even on a broken config — and a
    /// refusal at admission would override that from outside the
    /// handler, where it cannot report anything.
    #[test]
    fn a_saturated_aux_class_still_admits_the_probe() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let aux = aux_gate().expect("boot installed the aux class");

        let held: Vec<_> = repeat_with(|| aux.try_acquire())
            .take_while(Option::is_some)
            .flatten()
            .collect();
        assert!(!held.is_empty(), "the class had capacity to drain");
        assert!(aux.try_acquire().is_none(), "the aux class is saturated");
        let aux_in_use = aux.in_use();

        let mut envelope = dispatch(&app, Method::GET, "/status", Body::empty())
            .expect("the probe is admitted while the aux class is full");
        assert_ne!(
            envelope.response_mut().status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "the probe is exempt from the aux class, so saturation cannot shed it",
        );
        assert_eq!(
            aux.in_use(),
            aux_in_use,
            "admitting the probe charges no aux capacity at all",
        );

        drop(envelope);
        drop(held);
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn zero_auxiliary_capacity_preserves_independent_probe_and_static_owners() {
        use auctioneer_core::handlers::probe_config::{
            config_work_capacity, config_work_in_flight,
        };
        use auctioneer_runtime::PROBE_CONFIG_MIN_PERMITS;

        const CHILD_ENV: &str = "AUCTIONEER__TEST__ZERO_AUX_TERMINAL_CHILD";
        if ran_isolated_child(
            CHILD_ENV,
            "tests::zero_auxiliary_capacity_preserves_independent_probe_and_static_owners",
        ) {
            return;
        }
        let mut zero_aux = limits();
        zero_aux.aux_route_permits = 0;
        install_envelope(&zero_aux);
        let gates = boot_gates().unwrap();
        assert_eq!(gates.aux.capacity(), 0);
        assert_eq!(gates.aux_work_budget.limit(), 0);
        assert!(gates.derived.aux_term > 0, "probe config remains funded");
        let app = configured_app();
        let mut probe = immediate_probe(&app);
        assert_eq!(probe.response_mut().status(), StatusCode::OK);
        assert_eq!(config_work_capacity(), Some(PROBE_CONFIG_MIN_PERMITS));
        assert_eq!(config_work_in_flight(), 0);
        assert_eq!(probe_egress_gate().in_use(), 1);
        assert_eq!(gates.aux.in_use(), 0);
        assert_eq!(gates.aux_work_budget.in_use(), 0);
        assert_eq!(gates.admission.in_use(), 0);
        assert_eq!(gates.static_egress.in_use(), 0);
        let mut refusal = dispatch(&app, Method::GET, "/info/bidders", Body::empty()).unwrap();
        assert_eq!(
            refusal.response_mut().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(gates.static_egress.in_use(), 1);
        assert_eq!(probe_egress_gate().in_use(), 1);
        assert_eq!(gates.aux_work_budget.in_use(), 0);
        drop(refusal);
        assert_eq!(gates.static_egress.in_use(), 0);
        assert_eq!(probe_egress_gate().in_use(), 1);
        drop(probe);
        assert_eq!(probe_egress_gate().in_use(), 0);
        drop(immediate_probe(&app));
        assert_eq!(probe_egress_gate().in_use(), 0);
        assert_eq!(config_work_in_flight(), 0);
        assert_eq!(gates.aux.in_use(), 0);
        assert_eq!(gates.aux_work_budget.in_use(), 0);
    }

    #[test]
    fn retained_probe_outputs_saturate_and_recover_independently() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let mut held: Vec<_> = repeat_with(|| {
            dispatch(&app, Method::GET, "/status", Body::empty())
                .expect("reserved probe output capacity is available")
        })
        .take(usize::try_from(auctioneer_runtime::PROBE_RESPONSE_PERMITS).unwrap())
        .collect();
        assert!(
            dispatch(&app, Method::GET, "/status", Body::empty()).is_none(),
            "a blocked probe writer cannot create an unfunded fifth output"
        );
        assert!(
            dispatch(&app, Method::GET, "/info/bidders", Body::empty()).is_some(),
            "probe saturation does not consume the aux class"
        );
        drop(held.pop());
        held.push(
            dispatch(&app, Method::GET, "/status", Body::empty())
                .expect("abandoning one output returns exactly one probe slot"),
        );
        assert!(dispatch(&app, Method::GET, "/status", Body::empty()).is_none());
        drop(held);
        assert!(dispatch(&app, Method::GET, "/status", Body::empty()).is_some());
    }

    #[test]
    fn every_probe_terminal_outcome_returns_its_output_permit_once() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = probe_egress_gate();
        let routes = registered_terminal_requests(&app, PROBE_CLASS);
        let expected_routes = routes.len();
        let mut checked = 0_usize;
        for (method, path) in routes {
            for outcome in TERMINAL_OUTCOMES {
                assert_eq!(gate.in_use(), 0);
                let envelope = dispatch(&app, method.clone(), &path, Body::empty()).unwrap();
                assert_eq!(
                    gate.in_use(),
                    1,
                    "handler return must retain probe capacity"
                );
                let (prepared, _policy, mut attempt, clock) = envelope
                    .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                    .unwrap_or_else(|_| panic!("probe response egress begins"));
                assert!(attempt.begin_writing());
                drop(prepared);
                assert!(attempt.terminate(outcome, clock.now()));
                assert_eq!(gate.in_use(), 0, "terminal outcome {outcome:?}");
                assert!(!attempt.terminate(outcome, clock.now()));
                drop(attempt);
                assert_eq!(gate.in_use(), 0, "terminal/drop must not release twice");
            }
            drop(dispatch(&app, method, &path, Body::empty()).unwrap());
            assert_eq!(gate.in_use(), 0);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, expected_routes);
        assert!(checked > 0);
    }

    #[test]
    fn aux_output_and_work_stay_funded_until_each_real_sdk_terminal_outcome() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gates = boot_gates().unwrap();
        let peak = gates.aux_work_derived.peak_bytes();
        assert!(peak > 0);
        let cases = auxiliary_terminal_requests(&app);
        let expected_cases = cases.len();
        let mut checked = 0_usize;
        for (method, path) in cases {
            for outcome in TERMINAL_OUTCOMES {
                assert_eq!(gates.aux.in_use(), 0);
                assert_eq!(gates.aux_work_budget.in_use(), 0);
                let envelope = dispatch(&app, method.clone(), &path, Body::empty()).unwrap();
                assert_auxiliary_receipt_held(gates, peak);
                let (prepared, _policy, mut attempt, clock) = envelope
                    .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                    .unwrap_or_else(|_| panic!("aux response egress begins"));
                assert!(attempt.begin_writing());
                drop(prepared);
                assert_auxiliary_receipt_held(gates, peak);
                assert!(gates.aux.try_acquire().is_none());
                assert!(attempt.terminate(outcome, clock.now()));
                assert_eq!(gates.aux.in_use(), 0, "{path}, {outcome:?}");
                assert_eq!(gates.aux_work_budget.in_use(), 0, "{path}, {outcome:?}");
                assert!(!attempt.terminate(outcome, clock.now()));
                drop(attempt);
                assert_eq!(gates.aux.in_use(), 0);
                assert_eq!(gates.aux_work_budget.in_use(), 0);
            }
            let envelope = dispatch(&app, method.clone(), &path, Body::empty()).unwrap();
            let (prepared, _policy, attempt, _clock) = envelope
                .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                .unwrap_or_else(|_| panic!("aux response egress begins"));
            drop(prepared);
            assert_eq!(gates.aux.in_use(), 1);
            assert_eq!(gates.aux_work_budget.in_use(), peak);
            drop(attempt);
            assert_eq!(gates.aux.in_use(), 0);
            assert_eq!(gates.aux_work_budget.in_use(), 0);
            drop(dispatch(&app, method, &path, Body::empty()).unwrap());
            assert_eq!(gates.aux.in_use(), 0);
            assert_eq!(gates.aux_work_budget.in_use(), 0);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, expected_cases);
        assert!(checked > 0);
    }

    fn registered_terminal_requests(app: &App, class: &str) -> Vec<(Method, String)> {
        app.router()
            .routes()
            .into_iter()
            .filter(|route| route.class() == Some(class))
            .map(|route| {
                (
                    route.method().clone(),
                    route.pattern().replace("{name}", "fake_fixture_bidder"),
                )
            })
            .collect()
    }

    fn auxiliary_terminal_requests(app: &App) -> Vec<(Method, String)> {
        registered_terminal_requests(app, AUX_CLASS)
            .into_iter()
            .chain([
                (Method::GET, "/no/such/route".to_owned()),
                (Method::POST, "/status".to_owned()),
            ])
            .collect()
    }

    #[derive(Clone, Default)]
    struct EgressReports(Arc<Mutex<Vec<ResponseEgressReport>>>);

    impl ResponseEgressObserver for EgressReports {
        fn complete(&self, report: &ResponseEgressReport) {
            self.0.lock().unwrap().push(report.clone());
        }
    }

    #[test]
    fn aux_work_is_released_once_when_application_egress_is_abandoned_before_or_during_writing() {
        boot();
        let _serialized = gate_counters_lock();
        let reports = EgressReports::default();
        let mut app = configured_app();
        app.set_response_egress_observer(reports.clone());
        let gates = boot_gates().unwrap();
        let peak = gates.aux_work_derived.peak_bytes();
        let cases = auxiliary_terminal_requests(&app);
        assert!(!cases.is_empty());
        for (method, path) in cases {
            for writing in [false, true] {
                let envelope = dispatch(&app, method.clone(), &path, Body::empty()).unwrap();
                let (prepared, _policy, mut attempt, _clock) = envelope
                    .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                    .unwrap_or_else(|_| panic!("aux response egress begins"));
                if writing {
                    assert!(attempt.begin_writing());
                }
                drop(prepared);
                assert_auxiliary_receipt_held(gates, peak);
                assert!(reports.0.lock().unwrap().is_empty());
                assert!(gates.aux.try_acquire().is_none());
                drop(attempt);
                assert_eq!(gates.aux.in_use(), 0);
                assert_eq!(gates.aux_work_budget.in_use(), 0);
                let completed = take(&mut *reports.0.lock().unwrap());
                assert_eq!(completed.len(), 1, "{path}, writing={writing}");
                assert_eq!(completed[0].body_kind, ResponseEgressBodyKind::Application);
                assert_eq!(completed[0].fallback_disposition, None);
                assert_eq!(
                    completed[0].outcome,
                    if writing {
                        ResponseEgressOutcome::TransportError
                    } else {
                        ResponseEgressOutcome::ConversionError
                    }
                );
            }
        }
    }

    #[test]
    fn aux_work_survives_real_sdk_fallback_preparation_until_completion_or_abandonment() {
        boot();
        let _serialized = gate_counters_lock();
        let reports = EgressReports::default();
        let mut app = configured_app();
        app.set_response_egress_observer(reports.clone());
        let gates = boot_gates().unwrap();
        let peak = gates.aux_work_derived.peak_bytes();
        let cases = auxiliary_terminal_requests(&app);
        assert!(!cases.is_empty());
        let endings = [
            (Some(ResponseEgressFallbackDisposition::Completed), true),
            (Some(ResponseEgressFallbackDisposition::Aborted), false),
            (Some(ResponseEgressFallbackDisposition::Aborted), true),
            (None, false),
            (None, true),
        ];
        for (method, path) in cases {
            for cause in [
                ResponseEgressOutcome::ConversionError,
                ResponseEgressOutcome::DeadlineExceeded,
            ] {
                for (disposition, writing) in endings {
                    let mut envelope =
                        dispatch(&app, method.clone(), &path, Body::empty()).unwrap();
                    assert_auxiliary_receipt_held(gates, peak);
                    let (mut attempt, clock) = if cause == ResponseEgressOutcome::ConversionError {
                        // The real normalizer returns the original live attempt,
                        // rather than a fabricated completion or a second grant.
                        envelope
                            .response_mut()
                            .headers_mut()
                            .insert("content-length", "invalid".parse().unwrap());
                        let Err(failure) =
                            envelope.begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                        else {
                            panic!("invalid framing must return the live attempt");
                        };
                        let (attempt, clock, actual) = failure.into_parts();
                        assert_eq!(actual, cause);
                        (attempt, clock)
                    } else {
                        let (prepared, _policy, attempt, clock) = envelope
                            .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                            .unwrap_or_else(|_| panic!("aux response egress begins"));
                        drop(prepared);
                        (attempt, clock)
                    };
                    assert_auxiliary_receipt_held(gates, peak);
                    assert!(attempt.begin_fallback(cause));
                    assert_auxiliary_receipt_held(gates, peak);
                    assert!(!attempt.begin_fallback(cause));
                    let fallback = http::Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from_bytes(b"internal error\n".to_vec()))
                        .unwrap();
                    let prepared = attempt
                        .prepare_response(
                            ResponseEgressFramingMode::ExplicitBufferedLength,
                            &method,
                            fallback,
                        )
                        .unwrap();
                    assert_auxiliary_receipt_held(gates, peak);
                    if writing {
                        assert!(attempt.begin_writing());
                    }
                    drop(prepared);
                    assert_auxiliary_receipt_held(gates, peak);
                    assert!(reports.0.lock().unwrap().is_empty());
                    assert!(gates.aux.try_acquire().is_none());
                    if let Some(ending) = disposition {
                        assert!(attempt.finish_fallback(ending, clock.now()));
                        assert_eq!(gates.aux.in_use(), 0);
                        assert_eq!(gates.aux_work_budget.in_use(), 0);
                        assert!(!attempt.finish_fallback(ending, clock.now()));
                    }
                    drop(attempt);
                    assert_eq!(gates.aux.in_use(), 0);
                    assert_eq!(gates.aux_work_budget.in_use(), 0);
                    let completed = take(&mut *reports.0.lock().unwrap());
                    assert_eq!(
                        completed.len(),
                        1,
                        "{path}, {cause:?}, {disposition:?}, writing={writing}"
                    );
                    assert_eq!(completed[0].body_kind, ResponseEgressBodyKind::Fallback);
                    assert_eq!(completed[0].outcome, cause);
                    assert_eq!(
                        completed[0].fallback_disposition,
                        Some(disposition.unwrap_or(ResponseEgressFallbackDisposition::Aborted))
                    );
                }
            }
        }
    }

    #[test]
    fn terminal_cases_join_every_registered_auxiliary_and_both_router_fallbacks() {
        let app = configured_app();
        let expected: HashSet<_> = app
            .router()
            .routes()
            .into_iter()
            .filter(|route| route.class() == Some(AUX_CLASS))
            .map(|route| route.id().clone())
            .collect();
        assert!(!expected.is_empty());
        let mut matched = HashSet::new();
        let mut not_found = 0_usize;
        let mut wrong_method = 0_usize;
        for (method, path) in auxiliary_terminal_requests(&app) {
            assert!(
                !path.contains(['{', '}']),
                "unexpanded route fixture: {path}"
            );
            match app.router().resolve(&method, &path).resolution() {
                RouteResolution::Matched(route) => {
                    assert_eq!(route.class(), Some(AUX_CLASS));
                    assert!(
                        matched.insert(route.id().clone()),
                        "duplicate route: {method} {path}"
                    );
                }
                RouteResolution::NotFound => not_found = not_found.checked_add(1).unwrap(),
                RouteResolution::MethodNotAllowed { .. } => {
                    wrong_method = wrong_method.checked_add(1).unwrap();
                }
                _ => panic!("unrecognized router disposition"),
            }
        }
        assert_eq!(matched, expected);
        assert_eq!(not_found, 1);
        assert_eq!(wrong_method, 1);

        let expected_probes: HashSet<_> = app
            .router()
            .routes()
            .into_iter()
            .filter(|route| route.class() == Some(PROBE_CLASS))
            .map(|route| route.id().clone())
            .collect();
        assert!(!expected_probes.is_empty());
        let mut matched_probes = HashSet::new();
        for (method, path) in registered_terminal_requests(&app, PROBE_CLASS) {
            assert!(
                !path.contains(['{', '}']),
                "unexpanded probe fixture: {path}"
            );
            let resolved = app.router().resolve(&method, &path);
            let RouteResolution::Matched(route) = resolved.resolution() else {
                panic!("probe fixture must reach its registered handler");
            };
            assert_eq!(route.class(), Some(PROBE_CLASS));
            assert!(matched_probes.insert(route.id().clone()));
        }
        assert_eq!(matched_probes, expected_probes);
    }

    #[test]
    fn pending_configuration_cancellation_releases_each_admitted_owner() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gates = boot_gates().unwrap();
        let peak = gates.aux_work_derived.peak_bytes();
        let mut checked = 0_usize;
        for (method, path) in registered_terminal_requests(&app, AUX_CLASS) {
            let store = admission_store(StoreBehavior::Pending);
            let request = store_request(method.clone(), &path, Arc::clone(&store));
            let mut construction = Box::pin(dispatch_request(&app, request));
            let first = construction
                .as_mut()
                .poll(&mut Context::from_waker(noop_waker_ref()));
            assert_auxiliary_receipt_held(gates, peak);
            assert!(
                first.is_pending(),
                "{method} {path} must suspend at its bounded config read"
            );
            assert_eq!(store.reads.load(Ordering::SeqCst), 1);
            assert_eq!(store.active.load(Ordering::SeqCst), 1);
            let refusal = dispatch(&app, Method::GET, "/info/bidders", Body::empty()).unwrap();
            assert_eq!(gates.static_egress.in_use(), 1);
            assert_eq!(gates.aux.in_use(), 1);
            assert_eq!(gates.aux_work_budget.in_use(), peak);
            drop(refusal);
            assert_eq!(gates.static_egress.in_use(), 0);
            let probe = dispatch(&app, Method::GET, "/status", Body::empty()).unwrap();
            assert_eq!(probe_egress_gate().in_use(), 1);
            assert_eq!(gates.aux.in_use(), 1);
            assert_eq!(gates.aux_work_budget.in_use(), peak);
            drop(probe);
            assert_eq!(probe_egress_gate().in_use(), 0);
            assert_eq!(store.active.load(Ordering::SeqCst), 1);
            drop(construction);
            assert_eq!(store.active.load(Ordering::SeqCst), 0);
            assert_eq!(gates.aux.in_use(), 0);
            assert_eq!(gates.aux_work_budget.in_use(), 0);
            drop(dispatch(&app, method, &path, Body::empty()).unwrap());
            assert_eq!(gates.aux.in_use(), 0);
            assert_eq!(gates.aux_work_budget.in_use(), 0);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(
            checked,
            app.router()
                .routes()
                .iter()
                .filter(|route| route.class() == Some(AUX_CLASS))
                .count()
        );
        assert!(checked > 0);
    }

    #[test]
    fn failed_configuration_keeps_original_work_receipt_through_error_handoff() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gates = boot_gates().unwrap();
        let peak = gates.aux_work_derived.peak_bytes();
        let mut checked = 0_usize;
        for (method, path) in registered_terminal_requests(&app, AUX_CLASS) {
            let store = admission_store(StoreBehavior::Unavailable);
            let request = store_request(method, &path, Arc::clone(&store));
            let mut response = block_on(dispatch_request(&app, request)).unwrap();
            assert_eq!(
                response.response_mut().status(),
                if path == "/_auctioneer/health/gvl" {
                    StatusCode::UNAUTHORIZED
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            );
            assert_eq!(store.reads.load(Ordering::SeqCst), 1);
            assert_eq!(store.active.load(Ordering::SeqCst), 0);
            assert_eq!(gates.aux.in_use(), 1);
            assert_eq!(gates.aux_work_budget.in_use(), peak);
            drop(response);
            assert_eq!(gates.aux.in_use(), 0);
            assert_eq!(gates.aux_work_budget.in_use(), 0);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(
            checked,
            app.router()
                .routes()
                .iter()
                .filter(|route| route.class() == Some(AUX_CLASS))
                .count()
        );
        assert!(checked > 0);
    }

    #[test]
    fn body_read_timeout_and_cancellation_preserve_the_original_auxiliary_receipt() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gates = boot_gates().unwrap();
        let peak = gates.aux_work_derived.peak_bytes();
        // Exercise the matched non-body collector and both SDK fallback drains.
        // Sync handlers resolve configuration before their own body reads;
        // their fault/byte controls remain in the usersync route owner.
        let cases = auxiliary_terminal_requests(&app)
            .into_iter()
            .filter(|(method, path)| {
                path == "/info/bidders"
                    || !matches!(
                        app.router().resolve(method, path).resolution(),
                        RouteResolution::Matched(_)
                    )
            })
            .collect::<Vec<_>>();
        assert_eq!(cases.len(), 3);
        let expected_cases = cases.len().checked_mul(2).unwrap();
        let mut checked = 0_usize;
        for (method, path) in cases {
            for cancel in [false, true] {
                let store = admission_store(StoreBehavior::Pending);
                let mut request = store_request(method.clone(), &path, Arc::clone(&store));
                let (wake, timeout) = oneshot::channel::<()>();
                *request.body_mut() = Body::from_stream(stream::once(async move {
                    timeout
                        .await
                        .expect("the fixture delivers the adapter timeout");
                    Err::<bytes::Bytes, _>(EdgeError::request_timeout("fixed adapter timeout"))
                }));
                let mut construction = Box::pin(dispatch_request(&app, request));
                assert!(
                    construction
                        .as_mut()
                        .poll(&mut Context::from_waker(noop_waker_ref()))
                        .is_pending()
                );
                assert!(!wake.is_canceled(), "the admitted body must still be held");
                assert_eq!(store.reads.load(Ordering::SeqCst), 0);
                assert_eq!(gates.aux.in_use(), 1);
                assert_eq!(gates.aux_work_budget.in_use(), peak);
                assert_eq!(gates.admission.in_use(), 0);
                assert_eq!(gates.static_egress.in_use(), 0);
                if cancel {
                    drop(construction);
                    assert!(
                        wake.is_canceled(),
                        "cancellation must drop the stalled body"
                    );
                } else {
                    wake.send(()).unwrap();
                    let Poll::Ready(Some(mut response)) = construction
                        .as_mut()
                        .poll(&mut Context::from_waker(noop_waker_ref()))
                    else {
                        panic!("{method} {path}: the adapter timeout must finish dispatch");
                    };
                    assert_eq!(
                        response.response_mut().status(),
                        StatusCode::REQUEST_TIMEOUT
                    );
                    assert_eq!(
                        response.response_mut().body().as_bytes(),
                        Some(b"request timeout\n".as_slice())
                    );
                    assert_eq!(gates.aux.in_use(), 1);
                    assert_eq!(gates.aux_work_budget.in_use(), peak);
                    drop(response);
                    drop(construction);
                }
                assert_eq!(store.reads.load(Ordering::SeqCst), 0);
                assert_eq!(gates.aux.in_use(), 0);
                assert_eq!(gates.aux_work_budget.in_use(), 0);
                drop(dispatch(&app, method.clone(), &path, Body::empty()).unwrap());
                assert_eq!(gates.aux.in_use(), 0);
                assert_eq!(gates.aux_work_budget.in_use(), 0);
                checked = checked.checked_add(1).unwrap();
            }
        }
        assert_eq!(checked, expected_cases);
        assert!(checked > 0);
    }

    #[test]
    fn aux_gate_full_returns_503_through_static_class() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let aux = aux_gate().expect("boot installed the aux class");
        let statics = static_egress_gate().expect("boot installed the static class");

        // Saturate the aux class by hand, so the refusal path is the
        // only one left.
        let held: Vec<_> = repeat_with(|| aux.try_acquire())
            .take_while(Option::is_some)
            .flatten()
            .collect();
        assert!(!held.is_empty(), "the class had capacity to drain");
        assert!(aux.try_acquire().is_none(), "the aux class is saturated");
        let statics_before = statics.in_use();

        // An AUX route, not /status: the probe is exempt from this class
        // precisely so saturation cannot shed it.
        let mut envelope = dispatch_with_origin(
            &app,
            Method::GET,
            "/info/bidders",
            Body::empty(),
            Some("https://buyer.example"),
        )
        .expect("a saturated isolate refuses rather than aborting while statics remain");
        assert_eq!(
            envelope.response_mut().status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "a saturated route class sheds rather than queueing",
        );
        assert_eq!(
            envelope
                .response_mut()
                .headers()
                .get("cache-control")
                .unwrap(),
            "no-store"
        );
        assert!(
            envelope
                .response_mut()
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
        assert_eq!(
            statics.in_use(),
            statics_before.saturating_add(1),
            "the refusal holds a static-class permit: it is a response like any other",
        );

        drop(envelope);
        assert_eq!(
            statics.in_use(),
            statics_before,
            "the refusal's terminal egress releases its static permit exactly once",
        );
        drop(held);
        assert!(aux.try_acquire().is_some(), "the aux class recovers");
    }

    /// With BOTH the route class and the static class exhausted there is
    /// no bounded buffer to render a refusal into, so the request is
    /// ABORTED at the transport rather than answered with a response
    /// nothing accounts for.
    #[test]
    fn an_unbounded_refusal_aborts_instead_of_emitting() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let aux = aux_gate().expect("boot installed the aux class");
        let statics = static_egress_gate().expect("boot installed the static class");

        let held_aux: Vec<_> = repeat_with(|| aux.try_acquire())
            .take_while(Option::is_some)
            .flatten()
            .collect();
        let held_static: Vec<_> = repeat_with(|| statics.try_acquire())
            .take_while(Option::is_some)
            .flatten()
            .collect();
        assert!(!held_aux.is_empty() && !held_static.is_empty());

        assert!(
            dispatch(&app, Method::GET, "/info/bidders", Body::empty()).is_none(),
            "with nothing left to bound the refusal, the connection is torn down instead",
        );

        drop(held_aux);
        drop(held_static);
    }

    /// An auction request is bounded by the AUCTION admission class, not
    /// by the aux class — so an aux flood cannot shed auctions and a
    /// saturated auction gate does not shed `/status`.
    #[test]
    fn the_auction_route_draws_from_the_auction_class() {
        boot();
        let _serialized = gate_counters_lock();
        let admission = admission_gate().expect("boot installed the admission gate");
        let aux = aux_gate().expect("boot installed the aux class");
        assert_eq!(admission.in_use(), 0);
        assert_eq!(aux.in_use(), 0);

        let decision = decide(&head(
            Method::POST,
            "/openrtb2/auction",
            resolution_for(&Method::POST, "/openrtb2/auction"),
        ));
        assert!(
            matches!(decision, AdmissionDecision::Admit { .. }),
            "an idle auction class admits",
        );
        assert_eq!(
            admission.in_use(),
            1,
            "the auction route took an auction-concurrency slot",
        );
        assert_eq!(
            aux.in_use(),
            0,
            "and it took NO aux capacity: the classes are separate so an aux flood cannot \
             shed auctions",
        );

        drop(decision);
        assert_eq!(
            admission.in_use(),
            0,
            "dropping the decision releases both the concurrency slot and the empty slot",
        );
    }

    /// The auction lease hands back an EMPTY egress slot and the
    /// concurrency permit, which is the shape the handler expects:
    /// acquiring an egress permit at admission would hold transmission
    /// capacity for the whole fan-out.
    #[test]
    fn an_auction_admission_carries_the_concurrency_slot_and_an_empty_egress_slot() {
        boot();
        let _serialized = gate_counters_lock();
        let admission = admission_gate().expect("boot installed the admission gate");
        assert_eq!(admission.in_use(), 0);

        let decision = decide(&head(
            Method::POST,
            "/openrtb2/auction",
            resolution_for(&Method::POST, "/openrtb2/auction"),
        ));
        let AdmissionDecision::Admit {
            grant, completion, ..
        } = decision
        else {
            panic!("an idle auction class admits");
        };
        let lease = grant
            .downcast::<AuctioneerIngressLease>()
            .unwrap_or_else(|_grant| panic!("the grant carries auctioneer's own lease"));
        assert_eq!(
            lease.admission,
            RouteAdmissionMarker::Auction,
            "the handler reads the auction class back off the marker",
        );
        assert!(
            lease.auction_admission.is_some(),
            "the concurrency slot rides the lease, because it bounds work rather than \
             transmission",
        );
        assert_eq!(admission.in_use(), 1);

        // Dropping the grant is what the router's fallback path does.
        // It releases the CONCURRENCY slot — correctly, the work is
        // over — and leaves the egress slot to the completion.
        drop(lease);
        assert_eq!(
            admission.in_use(),
            0,
            "the concurrency slot returns when the request's work ends",
        );
        drop(completion);
    }

    /// An aux admission takes NO auction-concurrency slot. The two
    /// classes are separate accounting, and a shared one would let an
    /// aux flood shed auctions.
    #[test]
    fn an_aux_admission_takes_no_auction_concurrency_slot() {
        boot();
        let _serialized = gate_counters_lock();
        let admission = admission_gate().expect("boot installed the admission gate");
        let aux = aux_gate().expect("boot installed the aux class");
        assert_eq!(admission.in_use(), 0);

        let decision = decide(&head(
            Method::GET,
            "/info/bidders",
            resolution_for(&Method::GET, "/info/bidders"),
        ));
        assert!(matches!(decision, AdmissionDecision::Admit { .. }));
        assert_eq!(aux.in_use(), 1, "the aux class bounds it");
        assert_eq!(
            admission.in_use(),
            0,
            "and the auction class is untouched by an aux route",
        );
        drop(decision);
        assert_eq!(aux.in_use(), 0);
    }

    /// The unmatched and wrong-method arms select the host's bounded
    /// DRAIN disposition, not plain dispatch — the host has to consume
    /// the request body under a cap before it can write its canonical
    /// answer, and that read runs under the aux class rather than the
    /// auction class.
    #[test]
    fn the_fallback_arms_select_a_bounded_drain_under_the_aux_class() {
        boot();
        let _serialized = gate_counters_lock();
        let aux = aux_gate().expect("boot installed the aux class");
        let admission = admission_gate().expect("boot installed the admission gate");
        assert_eq!(aux.in_use(), 0);

        for resolution in [
            RouteResolution::NotFound,
            resolution_for(&Method::POST, "/status"),
        ] {
            let decision = decide(&head(Method::POST, "/status", resolution));
            // Destructured by REFERENCE, so the decision stays alive and
            // the permit it holds is still countable below. Consuming it
            // here would release the permit before the assertion.
            let AdmissionDecision::ReadBodyBeforeFallback { max_body_bytes, .. } = &decision else {
                panic!("a fallback arm drains before the canonical response")
            };
            assert_eq!(
                *max_body_bytes, DEFAULT_CAP_BYTES,
                "the drain enforces the small non-body-route cap",
            );
            assert_eq!(aux.in_use(), 1, "the drain runs under the aux class");
            assert_eq!(
                admission.in_use(),
                0,
                "auction capacity is never spent on a 404 or a 405",
            );
            drop(decision);
            assert_eq!(aux.in_use(), 0);
        }
    }

    /// Resolves `method` + `path` through the REAL router, so a test
    /// asserting on one arm cannot pass against a resolution the router
    /// would never produce.
    fn resolution_for(method: &Method, path: &str) -> RouteResolution {
        configured_app()
            .router()
            .resolve(method, path)
            .resolution()
            .clone()
    }

    // -----------------------------------------------------------------
    // Manifest coverage.
    // -----------------------------------------------------------------

    /// EVERY registered route carries an admission class this policy
    /// recognizes.
    ///
    /// The policy's unclassified arm is a fail-safe, not a destination:
    /// it bounds an unknown route as aux and logs loudly, which keeps a
    /// manifest gap from becoming an unbounded route. This test is what
    /// stops that arm from being reached in the first place. It reads the
    /// ROUTER — so a class present in `edgezero.toml` but not propagated
    /// into route metadata fails here — and it enumerates the whole route
    /// set, so a route added without a class fails rather than shipping.
    #[test]
    fn manifest_class_semantics_match_expected_map() {
        let app = configured_app();
        let routes = app.router().routes();
        assert!(!routes.is_empty(), "the app wires routes");

        let expected = |pattern: &str| match pattern {
            "/openrtb2/auction" => AUCTION_CLASS,
            // Reserved output capacity is independent of the aux class.
            "/status" => PROBE_CLASS,
            _ => AUX_CLASS,
        };
        for route in &routes {
            let class = route.class().unwrap_or_else(|| {
                panic!(
                    "route {} {} carries no admission class; add `class = \"auction\"` or \
                     `class = \"aux\"` to its [[triggers.http]] block",
                    route.method(),
                    route.pattern(),
                )
            });
            assert_eq!(
                class,
                expected(route.pattern()),
                "route {} {} is classified as `{class}`, which is not the class its body \
                 profile calls for",
                route.method(),
                route.pattern(),
            );
        }

        // Exactly one auction-class route. More than one would mean two
        // routes contending for auction concurrency, which the envelope
        // sizes for one entry point.
        let auction_routes = routes
            .iter()
            .filter(|route| route.class() == Some(AUCTION_CLASS))
            .count();
        assert_eq!(
            auction_routes, 1,
            "the auction class has exactly one member: `max_concurrent_auctions` sizes one \
             entry point",
        );
    }
}
