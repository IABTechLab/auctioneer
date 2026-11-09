//! Response-terminal egress binding, in a process where the boot
//! envelope is actually installed.
//!
//! The gates live behind process-global `OnceLock`s, so a test that
//! needs them installed cannot share a binary with one that needs them
//! absent. This binary installs them ONCE, up front, and every test
//! here assumes they are present. The mirror-image assertions — the
//! fail-closed answers a gate accessor gives BEFORE boot — live in
//! `boot_order_gates.rs` for exactly that reason.

#[cfg(test)]
mod tests {
    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::host_primitives::{
        AuctioneerIngressLease, BootEnvelope, DelegateToDevStrategy, DevRng, RouteAdmissionMarker,
        RouteEgressPermit, StderrBootLog, SystemClock, admit, bootstrap_adapter_with_envelope,
        egress_gate, install_route_permit, static_egress_gate,
    };
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        ImmediateResolveTimer, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator,
        ResolvedIsolateLimits, TargetPlatformConstants,
    };
    use edgezero_core::app::App;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{Method, StatusCode};
    use edgezero_core::ingress::IngressDispatchOutcome;
    use edgezero_core::response_egress::{ResponseEgressCompletion, ResponseEgressDeadline};
    use edgezero_core::time::MonotonicInstant;
    use std::iter::repeat_with;
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

    const CAP_256_KIB: u64 = 1 << 18;
    const BYTES_1_MIB: u64 = 1 << 20;
    /// The response cap, DERIVED from the assembly scratch the budget
    /// requires it to hold (`max_outbound_requests x max_decoded_bytes x
    /// ASSEMBLY_FACTOR`). A literal here went stale the moment the
    /// factor was corrected.
    const FINAL_RESPONSE_BYTES: u64 = 64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    const BYTES_64_KIB: u32 = 1 << 16;
    const BYTES_16_KIB: u32 = 1 << 14;

    /// The per-call metadata charge at the 100-impression limit, as
    /// `auctioneer_bidder::outbound_meta_bytes(100)` returns it.
    const OUTBOUND_META_BYTES: u32 = auctioneer_bidder::outbound_meta_bytes(100);

    /// Distinctive capacities, so an assertion that a gate carries the
    /// envelope's number cannot pass by coincidence against a default
    /// or against a neighbouring gate's number.
    const EGRESS_PERMITS: u32 = 3;
    const TERMINAL_PERMITS: u32 = 68;

    /// Serializes every test that reads a gate's in-use counter.
    ///
    /// The gates are process-global by design — that is what makes them
    /// bound the isolate rather than one `App` — and the test harness
    /// runs these in parallel by default, so two tests acquiring from
    /// the same class would each see the other's permits. A snapshot
    /// delta is not enough: a concurrent holder can acquire and release
    /// BETWEEN the snapshot and the assertion. So the counter-reading
    /// tests take one lock and the sharing is removed rather than
    /// tolerated.
    ///
    /// Every test here takes it, because every one of them either reads
    /// a counter or goes through the detached path, which ACQUIRES from
    /// the static class and so perturbs the counter another test is
    /// reading.
    fn gate_counters_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Installs the boot envelope exactly once for this binary.
    ///
    /// Every test here calls it first. `install_boot_envelope` is
    /// `OnceLock`-backed and idempotent, so the repeated calls are
    /// harmless; the `OnceLock` here just keeps the repeated validation
    /// off the per-test path.
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
            egress_permits: EGRESS_PERMITS,
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
            max_concurrent_auctions: 2,
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

    /// Renders `error` through a CONFIGURED app's detached pre-admission
    /// path and returns the status and body bytes that reach the wire.
    ///
    /// This is the only public seam onto the installed renderer — the
    /// host keeps its own render step private and exposes no getter —
    /// and it is also the real path a 414 takes, so the assertion covers
    /// the wiring rather than a test-only shortcut.
    fn render_detached(app: &App, error: EdgeError) -> (StatusCode, Vec<u8>, bool) {
        let outcome =
            app.detached_ingress_error_egress(error, Method::GET, MonotonicInstant::now());
        let IngressDispatchOutcome::Response(mut envelope) = outcome else {
            panic!("a booted, configured app sends pre-admission rejections rather than aborting");
        };
        let response = envelope.response_mut();
        let status = response.status();
        let has_deadline = response
            .extensions()
            .get::<ResponseEgressDeadline>()
            .is_some();
        let body = response
            .body()
            .as_bytes()
            .expect("a bounded error body is buffered, never a stream")
            .to_vec();
        (status, body, has_deadline)
    }

    fn configured_app() -> App {
        let mut app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("the app builds");
        // `build_app` already ran `configure_app`; running it again is
        // the idempotence path and leaves the same state.
        configure_app(&mut app).expect("configure is idempotent");
        app
    }

    /// The pinned wire bodies, asserted as EXACT BYTES through a real
    /// app. A length-only assertion passes while the wire format is
    /// wrong, which is how a blanket `error.kind()` renderer got past
    /// review once — so these compare byte strings.
    #[test]
    fn the_app_renders_the_pinned_414_and_431_bodies() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();

        let (uri_status, uri_body, _) = render_detached(
            &app,
            EdgeError::uri_too_long("/a/very/long/caller/supplied/request/target"),
        );
        assert_eq!(uri_status, StatusCode::URI_TOO_LONG);
        assert_eq!(
            uri_body, b"uri too long\n",
            "the 414 wire body is pinned; `uri_too_long` is the error KIND, not the contract",
        );

        let (header_status, header_body, _) =
            render_detached(&app, EdgeError::request_header_fields_too_large("x-huge"));
        assert_eq!(header_status, StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);
        assert_eq!(
            header_body, b"too many header bytes\n",
            "the 431 wire body is pinned; `request_header_fields_too_large` is the KIND",
        );
    }

    /// An unpinned error falls back to its kind, and NEVER to the
    /// message — which for an unmatched route is the whole
    /// caller-supplied path.
    #[test]
    fn unpinned_errors_render_their_kind_without_echoing_the_path() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let path = "/caller/supplied/".repeat(32);

        let (status, body, _) = render_detached(&app, EdgeError::not_found(path));
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, b"not_found", "an unpinned error renders its kind");
        assert!(
            !String::from_utf8_lossy(&body).contains("/caller/supplied/"),
            "the renderer must never echo caller-supplied text",
        );
    }

    /// A pre-admission rejection carries a write deadline, so the host
    /// can bound its transmission leg rather than falling back to its
    /// own default budget.
    #[test]
    fn pre_admission_rejections_carry_a_write_deadline() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let (_status, _body, has_deadline) =
            render_detached(&app, EdgeError::uri_too_long("/long"));
        assert!(
            has_deadline,
            "the detached factory attaches an absolute write deadline to every rejection",
        );
    }

    /// Every pre-admission rejection draws a STATIC permit, and that
    /// permit is released only when its completion closes. This is the
    /// exemption that used to exist being closed: these responses are
    /// emitted before admission runs, and before the factory they held
    /// nothing.
    #[test]
    fn pre_admission_rejections_hold_a_static_permit_until_terminal() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = static_egress_gate().expect("boot installed the static class");
        let before = gate.in_use();

        let outcome = app.detached_ingress_error_egress(
            EdgeError::uri_too_long("/long"),
            Method::GET,
            MonotonicInstant::now(),
        );
        let IngressDispatchOutcome::Response(envelope) = outcome else {
            panic!("a booted app sends the rejection");
        };
        assert_eq!(
            gate.in_use(),
            before + 1,
            "the rejection holds a static permit while its response is unwritten",
        );

        drop(envelope);
        assert_eq!(
            gate.in_use(),
            before,
            "abandoning the response closes its completion and releases the permit exactly once",
        );
    }

    /// An exhausted static class ABORTS rather than emitting an
    /// unbounded response. That is what makes "every response auctioneer
    /// puts on the wire holds a permit" true without qualification.
    #[test]
    fn an_exhausted_static_class_aborts_rather_than_emitting_ungated() {
        boot();
        let _serialized = gate_counters_lock();
        let app = configured_app();
        let gate = static_egress_gate().expect("boot installed the static class");

        // Drain the class. Held in a vec so every permit stays live for
        // the assertion below.
        let held: Vec<_> = repeat_with(|| gate.try_acquire())
            .take_while(Option::is_some)
            .flatten()
            .collect();
        assert!(!held.is_empty(), "the class had capacity to drain");
        assert!(gate.try_acquire().is_none(), "the class is now exhausted");

        let outcome = app.detached_ingress_error_egress(
            EdgeError::uri_too_long("/long"),
            Method::GET,
            MonotonicInstant::now(),
        );
        assert!(
            matches!(outcome, IngressDispatchOutcome::Aborted),
            "with no static permit available the rejection aborts at the transport rather \
             than emitting a response nothing bounds",
        );

        drop(held);
        assert!(
            gate.try_acquire().is_some(),
            "the class recovers once the drained permits release",
        );
    }

    /// The admission binding, against the REAL gate: the permit lives in
    /// the completion, so dropping the grant the way the host's fallback
    /// path does releases nothing.
    #[test]
    fn the_admission_permit_survives_dropping_the_grant() {
        boot();
        let _serialized = gate_counters_lock();
        let gate = egress_gate().expect("boot installed the egress class");
        let before = gate.in_use();

        let permit = gate.try_acquire().expect("the class has capacity");
        let (lease, completion) = admit(
            RouteAdmissionMarker::Auction,
            RouteEgressPermit::Auction(permit),
        );
        assert_eq!(gate.in_use(), before + 1, "admission holds the slot");

        // Exactly what the host's router does before it builds a
        // fallback response.
        drop(lease);
        assert_eq!(
            gate.in_use(),
            before + 1,
            "dropping the grant must not release the permit: the response is still unwritten",
        );

        drop(completion);
        assert_eq!(
            gate.in_use(),
            before,
            "only the completion's close releases it, and it releases exactly once",
        );
    }

    /// A lease carries the class marker and the slot, and nothing
    /// releasable — so a handler can still prove it was admitted.
    #[test]
    fn the_lease_carries_the_class_marker_and_the_slot() {
        boot();
        let _serialized = gate_counters_lock();
        let gate = egress_gate().expect("boot installed the egress class");
        let permit = gate.try_acquire().expect("the class has capacity");
        let (lease, completion) = admit(
            RouteAdmissionMarker::Auction,
            RouteEgressPermit::Auction(permit),
        );

        let AuctioneerIngressLease {
            admission,
            route_egress_resource,
            auction_admission,
        } = lease;
        assert_eq!(admission, RouteAdmissionMarker::Auction);
        assert!(
            auction_admission.is_none(),
            "`admit` is the aux shape: it takes no auction-concurrency slot",
        );
        assert_eq!(admission.class(), "auction");

        // The slot is already occupied by the admission-time permit, so
        // a second install is refused and its permit drops.
        let second = gate.try_acquire().expect("the class has more capacity");
        assert!(
            !install_route_permit(&route_egress_resource, RouteEgressPermit::Auction(second)),
            "the slot holds one permit; a second is refused rather than replacing it",
        );

        drop(completion);
    }

    /// A late-bound slot whose completion already closed refuses the
    /// install and drops the permit, so the class recovers. This is the
    /// client-disconnected-mid-auction shape against the real gate.
    #[test]
    fn a_closed_slot_releases_the_permit_back_to_the_real_gate() {
        boot();
        let _serialized = gate_counters_lock();
        let gate = egress_gate().expect("boot installed the egress class");
        let before = gate.in_use();

        let (resource, completion) = ResponseEgressCompletion::late_bound::<RouteEgressPermit>();
        drop(completion);

        let permit = gate.try_acquire().expect("the class has capacity");
        assert_eq!(gate.in_use(), before + 1);
        assert!(
            !install_route_permit(&resource, RouteEgressPermit::Auction(permit)),
            "a closed slot takes no ownership",
        );
        assert_eq!(
            gate.in_use(),
            before,
            "the refused permit dropped inside the install and returned its slot",
        );
    }
}
