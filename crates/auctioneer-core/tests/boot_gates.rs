//! The two `App`-lifecycle properties: `configure` is idempotent across
//! repeated SEQUENTIAL builds, and concurrent bootstrap installs the
//! gates exactly ONCE.
//!
//! These are deliberately two tests rather than one. They are different
//! properties and neither substitutes for the other: a repeated-build
//! test runs sequentially and so can never observe a race, while a
//! concurrency test cannot observe a setter that accumulates instead of
//! overwriting. Collapsing them would silently drop one property.
//!
//! Boot ORDERING is not tested here. It needs a process where no gate
//! has been installed yet, and both tests below install gates, so it
//! lives in `boot_order_gates.rs` in its own binary.

#[cfg(test)]
mod tests {
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, SystemClock, boot_gates,
        bootstrap_adapter_with_envelope, egress_gate,
    };
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        ImmediateResolveTimer, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator,
        ResolvedIsolateLimits, TargetPlatformConstants,
    };
    use edgezero_core::app::App as EdgeApp;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use std::iter::repeat_with;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;

    // Both tests observe process-global gates; only bootstrap's own threads race.
    static BOOT_TEST_LOCK: Mutex<()> = Mutex::new(());

    const CAP_256_KIB: u64 = 1 << 18;
    const BYTES_1_MIB: u64 = 1 << 20;
    /// The response cap, DERIVED from the assembly scratch the budget
    /// requires it to hold (`max_outbound_requests x max_decoded_bytes x
    /// ASSEMBLY_FACTOR`). A literal here went stale the moment the
    /// factor was corrected.
    const FINAL_RESPONSE_BYTES: u64 = 64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    const BYTES_64_KIB: u32 = 1 << 16;
    const BYTES_16_KIB: u32 = 1 << 14;

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
            // Covers sync work as well as privacy enforcement's config-extraction floor.
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
            max_admitted_header_count: 128,
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
            max_request_target_bytes: 768,
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

    /// `configure` runs on every `App` BUILD — on a serverless adapter
    /// that can be once per request. It must therefore be idempotent
    /// across REPEATED SEQUENTIAL builds.
    ///
    /// This is a different property from the concurrency test below and
    /// neither substitutes for the other: a repeated-build test cannot
    /// observe a race, and a concurrency test cannot observe a
    /// non-idempotent setter.
    #[test]
    fn configure_is_idempotent_across_repeated_builds() {
        let _guard = BOOT_TEST_LOCK.lock().expect("boot test lock");
        let first =
            EdgeApp::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("first build configures");
        let second =
            EdgeApp::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("second build configures");
        let third =
            EdgeApp::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("third build configures");

        // The observable per-`App` state converges: build N looks like
        // build 1. A setter that accumulated — pushing onto a list
        // rather than overwriting a field — would diverge here.
        assert_eq!(first.name(), second.name());
        assert_eq!(second.name(), third.name());
        assert_eq!(
            first.config_extraction_limits(),
            second.config_extraction_limits(),
            "extraction limits must be a set field, not an accumulated one",
        );
        assert_eq!(
            second.config_extraction_limits(),
            third.config_extraction_limits(),
        );
        assert_eq!(
            first.ingress_head_limits(),
            second.ingress_head_limits(),
            "head limits must converge across builds",
        );

        // Building an `App` must not touch the process-global gates.
        // If `configure` installed them, each build would hand its own
        // `App` a private capacity and bound nothing.
        let before = boot_gates().is_some();
        let _fourth =
            EdgeApp::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("fourth build configures");
        assert_eq!(
            boot_gates().is_some(),
            before,
            "an App build must not install or remove process-global gates",
        );
    }

    /// CONCURRENT bootstrap installs exactly one set of gates.
    ///
    /// This is the `OnceLock` path: many threads boot at once — which is
    /// what a per-request entrypoint does under load — and every one of
    /// them must end up looking at the same counters. The
    /// repeated-build test above cannot observe this, because sequential
    /// calls never race.
    #[test]
    fn concurrent_app_builds_install_gates_once() {
        const THREADS: usize = 16;
        let _guard = BOOT_TEST_LOCK.lock().expect("boot test lock");

        let limits = valid_limits();
        let target = target();
        let failures = Arc::new(AtomicU32::new(0));

        let observed: Vec<_> = thread::scope(|scope| {
            let handles: Vec<_> = repeat_with(|| {
                let thread_failures = Arc::clone(&failures);
                scope.spawn(move || {
                    // A validation failure is recorded rather than
                    // panicked, so the assertion below reports HOW
                    // MANY threads failed instead of whichever one
                    // happened to unwind first.
                    if bootstrap_adapter_with_envelope(
                        BootEnvelope {
                            limits: &limits,
                            target: &target,
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
                    .is_err()
                    {
                        thread_failures.fetch_add(1, Ordering::AcqRel);
                    }
                    // Build an `App` too, so the race covers the
                    // real per-request shape: boot, then build.
                    let built = EdgeApp::build::<auctioneer_core::App>(
                        edgezero_core::PlatformMetadata::default(),
                    );
                    assert!(built.is_ok(), "a concurrent build must still configure");
                    egress_gate().expect("this thread's own boot call returned before this read")
                })
            })
            .take(THREADS)
            .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("no thread panicked"))
                .collect()
        });

        assert_eq!(
            failures.load(Ordering::Acquire),
            0,
            "a valid envelope must validate on every racing thread",
        );

        // Every thread saw the SAME gate. `Arc::ptr_eq` against one
        // representative is the whole claim: if two installs had won,
        // some pair would differ and the capacity bound would be
        // doubled rather than shared.
        let canonical = observed.first().expect("threads returned gates");
        for (index, gate) in observed.iter().enumerate() {
            assert!(
                Arc::ptr_eq(canonical, gate),
                "thread {index} observed a different egress gate; a second install \
                 would double the effective capacity",
            );
        }
        assert_eq!(
            canonical.capacity(),
            EGRESS_PERMITS,
            "the one installed gate carries the envelope capacity",
        );
    }
}
