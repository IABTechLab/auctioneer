//! Boot ORDERING for the concurrency gates: the envelope is validated
//! before any gate exists, a rejected envelope installs nothing, and the
//! gates that do get installed are process-global.
//!
//! This is the ONLY test in its binary, and that is load-bearing rather
//! than tidiness. The gates sit behind a process-global `OnceLock`, so
//! the "no gate is installed yet" precondition the whole test rests on
//! is true only in a process nothing else has booted. A neighbour that
//! installed an envelope first — even a passing one — would turn the
//! pre-install assertions vacuous without failing, which is the worst
//! way for a test to stop testing something. The per-`App` lifecycle
//! tests therefore live in `boot_gates.rs`, in their own process.
//!
//! The steps run in sequence inside ONE `#[test]` function for the same
//! reason: `cargo test` runs test functions concurrently, and two
//! functions racing on one `OnceLock` cannot each observe the
//! pre-install state.

#[cfg(test)]
mod tests {
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, SystemClock, admission_gate,
        aux_gate, boot_gates, boot_limits, bootstrap_adapter_with_envelope, egress_gate,
        install_boot_envelope, stream_gate,
    };
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        BudgetError, ImmediateResolveTimer, LimitField, MAX_GVL_PARSED_BYTES,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        NoopDecodeCooperator, ResolvedIsolateLimits, TargetPlatformConstants,
    };
    use edgezero_core::app::App as EdgeApp;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::ingress::{
        DEFAULT_MAX_REQUEST_HEADER_BYTES, DEFAULT_MAX_REQUEST_TARGET_BYTES,
    };
    use std::sync::Arc;

    const CAP_256_KIB: u64 = 1 << 18;
    const BYTES_1_MIB: u64 = 1 << 20;
    /// The response cap, DERIVED from the assembly scratch the budget
    /// requires it to hold (`max_outbound_requests x max_decoded_bytes x
    /// ASSEMBLY_FACTOR`). A literal here went stale the moment the
    /// factor was corrected.
    const FINAL_RESPONSE_BYTES: u64 = 64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    const BYTES_64_KIB: u32 = 1 << 16;
    const BYTES_16_KIB: u32 = 1 << 14;
    /// Inbound header entries admitted at the ingress seam. Named so
    /// the fixture and the assertion below cannot drift apart.
    const ADMITTED_HEADER_COUNT: u16 = 128;
    /// The request-target cap every shipped adapter declares. Named so
    /// the fixture and the assertion cannot drift apart.
    const MAX_REQUEST_TARGET_BYTES: u32 = 768;

    /// Distinctive gate capacities, so an assertion that a gate has the
    /// envelope's capacity cannot pass by coincidence against a default
    /// or against another gate's number.
    const EGRESS_PERMITS: u32 = 3;
    const AUX_PERMITS: u32 = 5;
    const CONCURRENT_AUCTIONS: u32 = 2;
    const BODY_STREAMS: u32 = 7;

    /// A validating envelope, with the four gate capacities set to
    /// distinguishable values.
    ///
    /// The capacities are deliberately all different, so an assertion
    /// that a gate carries the envelope's number cannot pass by
    /// coincidence against a default or against a neighbouring gate's
    /// number. Raising them above the shipped single-auction row raises
    /// the memory demand with them, so the ceiling here is larger than
    /// the shipped one — these are gate-plumbing fixtures, not a
    /// proposed production row, and `budget.rs` owns the proof that the
    /// shipped row fits its own ceiling.
    fn valid_limits() -> ResolvedIsolateLimits {
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
            aux_route_permits: AUX_PERMITS,
            // Covers sync work as well as M4's config-extraction floor.
            aux_route_peak_bytes: 768 * 1_024 * 1_024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 50,
            max_outbound_requests: 64,
            max_concurrent_auctions: CONCURRENT_AUCTIONS,
            max_active_body_streams: BODY_STREAMS,
            max_runtime_config_bytes: BYTES_1_MIB,
            max_runtime_config_blob_bytes: BYTES_1_MIB,
            max_parsed_nodes: BYTES_64_KIB,
            max_copied_header_bytes: 4_096_u32,
            max_admitted_header_bytes: BYTES_16_KIB,
            max_admitted_header_count: ADMITTED_HEADER_COUNT,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 68,
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
            max_request_target_bytes: MAX_REQUEST_TARGET_BYTES,
            h1_max_buf_size_bytes: 24_576_u32,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: BYTES_16_KIB,
            egress_handoff_factor: 2,
        }
    }

    /// The owning bidder layout charge at this fixture's 100-impression limit.
    const OUTBOUND_META_BYTES: u32 = auctioneer_bidder::outbound_meta_bytes(100);

    /// The whole boot lifecycle, in the order an adapter observes it.
    ///
    /// One function rather than several because each step's precondition
    /// is the previous step's postcondition on a process-global
    /// `OnceLock`, which concurrent test functions could not establish.
    #[test]
    fn boot_order_rejects_a_request_before_gates_install() {
        // --- Before boot: no gate exists, and every accessor says so.
        assert!(
            boot_gates().is_none(),
            "nothing in this binary may install gates before the boot sequence runs",
        );
        for (name, present) in [
            ("admission", admission_gate().is_some()),
            ("stream", stream_gate().is_some()),
            ("egress", egress_gate().is_some()),
            ("aux", aux_gate().is_some()),
        ] {
            assert!(
                !present,
                "the {name} gate must be absent before boot, so a request arriving \
                 now is refused rather than admitted ungated",
            );
        }
        assert!(boot_limits().is_none(), "no envelope is installed yet");

        // --- A REJECTED envelope installs nothing. This is the
        // ordering claim that matters: validation happens BEFORE the
        // install, so a bad envelope cannot leave half-sized gates
        // behind for the next caller to inherit.
        let mut bad = valid_limits();
        bad.max_concurrent_auctions = 0;
        let rejected = install_boot_envelope(
            &bad,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        );
        assert!(
            matches!(
                rejected,
                Err(BudgetError::LimitBelowMinimum {
                    field: LimitField::MaxConcurrentAuctions,
                    value: 0,
                    minimum: 1,
                })
            ),
            "a zero admission capacity must be rejected by name, got {rejected:?}",
        );
        assert!(
            boot_gates().is_none(),
            "a rejected envelope must install NO gates; otherwise a failed boot \
             leaves the isolate serving with capacities nobody validated",
        );

        // --- A VALID envelope installs every gate at its own capacity.
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                limits: &valid_limits(),
                target: &target(),
                outbound_meta_bytes: OUTBOUND_META_BYTES,
                construction_costs: core_construction_costs(),
                log_sink: &StderrBootLog,
                catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            },
            || Arc::new(SystemClock::new()),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("the envelope validates");

        let gates = boot_gates().expect("boot installed the gates");
        assert_eq!(
            gates.egress.capacity(),
            EGRESS_PERMITS,
            "the egress gate takes its capacity from the envelope",
        );
        assert_eq!(
            gates.aux.capacity(),
            AUX_PERMITS,
            "the aux gate takes its OWN capacity, not the auction one",
        );
        assert_ne!(
            gates.aux.capacity(),
            gates.egress.capacity(),
            "the fixture must distinguish the two capacities for this to prove anything",
        );
        assert_eq!(
            boot_limits().map(|limits| limits.max_concurrent_auctions),
            Some(CONCURRENT_AUCTIONS),
            "the validated envelope is retained for per-App configuration to read",
        );

        assert_configure_installs_the_envelope_head();

        // Every accessor now answers, and the admission and stream
        // gates carry their own envelope capacities.
        let admission = admission_gate().expect("the admission gate is installed");
        let streams = stream_gate().expect("the stream gate is installed");
        assert_eq!(admission.in_use(), 0, "a fresh gate leases nothing");
        assert_eq!(streams.capacity(), BODY_STREAMS);
        assert_eq!(streams.in_flight(), 0, "a fresh stream gate leases nothing");
        let admitted: Vec<_> = (0..CONCURRENT_AUCTIONS)
            .map(|slot| {
                admission
                    .try_acquire()
                    .unwrap_or_else(|| panic!("envelope capacity admits slot {slot}"))
            })
            .collect();
        assert!(
            admission.try_acquire().is_none(),
            "the admission gate is bounded by the envelope's max_concurrent_auctions",
        );
        drop(admitted);

        assert_gates_are_process_global();
        assert_repeat_boot_keeps_the_live_gates(&bad);
    }

    /// Per-App configuration reads the installed envelope's head
    /// figures, rather than leaving the host's defaults in place.
    ///
    /// The sibling idempotence row in `boot_gates.rs` compares two
    /// builds to each other, which two uninstalled `App`s also
    /// satisfy; the defect was five policies installed and the head
    /// limits never, so every head validated at the host's `65_536`
    /// while the budget reserved against the envelope's `16_384`. The
    /// negative against the host default is what makes this row
    /// unable to pass against an `App` that installed nothing.
    fn assert_configure_installs_the_envelope_head() {
        let app =
            EdgeApp::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("the App builds once the envelope is installed");
        let head = app.ingress_head_limits();
        assert_eq!(
            head.max_request_header_bytes(),
            u64::from(BYTES_16_KIB),
            "the admitted header bound must be the envelope's figure, not the host's",
        );
        assert_eq!(
            head.max_request_header_count(),
            u64::from(ADMITTED_HEADER_COUNT),
            "the admitted entry count must be the envelope's figure",
        );
        assert_ne!(
            head.max_request_header_bytes(),
            DEFAULT_MAX_REQUEST_HEADER_BYTES,
            "a bound equal to the host default means configure_app installed nothing",
        );
        // The request-TARGET cap too. It lives on the platform
        // constants rather than the envelope row, and boot used to
        // drop those after validating — so this one went uninstalled
        // while the header pair did not, and live requests of 769,
        // 1_000 and 8_192 bytes were all admitted against a declared
        // 768.
        assert_eq!(
            head.max_request_target_bytes(),
            u64::from(MAX_REQUEST_TARGET_BYTES),
            "the request-target cap must be the target's own figure",
        );
        assert_ne!(
            head.max_request_target_bytes(),
            DEFAULT_MAX_REQUEST_TARGET_BYTES,
            "a target cap equal to the host default means it was never installed",
        );
    }

    /// Every accessor hands back the SAME gate, so all requests contend
    /// for one set of counters rather than each getting a private
    /// capacity.
    fn assert_gates_are_process_global() {
        let first = egress_gate().expect("installed");
        let second = egress_gate().expect("installed");
        assert!(
            Arc::ptr_eq(&first, &second),
            "two reads must return one gate; separate gates would bound nothing",
        );
        let held = first
            .try_acquire()
            .expect("the envelope capacity admits the first");
        assert_eq!(
            second.in_use(),
            1,
            "a permit taken through one handle is visible through the other",
        );
        drop(held);
    }

    /// Re-booting with a DIFFERENT envelope does not replace the
    /// installed gates. Idempotence has to mean "the first install
    /// wins", not "the last caller wins": a per-request entrypoint calls
    /// boot on every request, and swapping the gates under live requests
    /// would release permits into a gate that no longer counts them.
    fn assert_repeat_boot_keeps_the_live_gates(bad: &ResolvedIsolateLimits) {
        let installed = egress_gate().expect("boot already completed");
        let mut wider = valid_limits();
        wider.egress_permits = EGRESS_PERMITS.saturating_add(10);
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                limits: &wider,
                target: &target(),
                outbound_meta_bytes: OUTBOUND_META_BYTES,
                construction_costs: core_construction_costs(),
                log_sink: &StderrBootLog,
                catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            },
            || Arc::new(SystemClock::new()),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("the wider envelope also validates");
        assert_eq!(
            egress_gate().map(|gate| gate.capacity()),
            Some(EGRESS_PERMITS),
            "a repeat boot must not resize the live gate",
        );
        assert!(
            Arc::ptr_eq(&egress_gate().expect("installed"), &installed),
            "a repeat boot must hand back the SAME gate, not a fresh one",
        );

        // --- And a repeat boot with a BAD envelope is still reported as
        // an error rather than silently inheriting the good gates.
        let repeat_bad = install_boot_envelope(
            bad,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        );
        assert!(
            repeat_bad.is_err(),
            "validation runs on every call, so a second entrypoint passing a bad \
             envelope is told so rather than quietly adopting the first one's gates",
        );
    }
}
