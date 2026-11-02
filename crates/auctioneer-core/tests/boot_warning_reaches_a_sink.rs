//! The boot sequence installs a `log` sink BEFORE it validates the
//! envelope.
//!
//! The over-ceiling diagnostic replaced a boot refusal, so it is the
//! only safety signal between a deployment and an isolate whose
//! declared caps exceed the platform's memory. It also fires at most
//! once per process, behind a `OnceLock`, so a cold start that emits it
//! into a process with no sink suppresses it for the isolate's whole
//! life — no later request re-emits it.
//!
//! # Why the existing warn-mode test cannot catch this
//!
//! `boot_envelope_warn_mode.rs` calls `testing_logger::setup()` first
//! and then `install_boot_envelope` directly. Both of those are things
//! the production entrypoints do NOT do: no adapter installs a logger
//! before booting, and every adapter goes through
//! `bootstrap_adapter_with_envelope` rather than calling the installer
//! itself. So that test proves the warning is EMITTED and says nothing
//! about whether anything is listening when it is — which is the whole
//! defect. This binary therefore installs no logger of its own and
//! drives the real entrypoint.
//!
//! # What this binary does NOT prove
//!
//! Every assertion here is taken after `boot()` returns, so it observes
//! that a sink EXISTS and not that it existed before validation ran.
//! Swapping the two statements in `bootstrap_adapter_with_envelope`
//! leaves `max_level() >= Warn` true and this test green.
//! `boot_warning_lands_in_the_envelopes_sink.rs` covers the ordering, by
//! passing a capturing sink and requiring the warning to be in it.
//!
//! This is the ONLY test in its binary. `log`'s global logger is
//! process-global and claimable once, and the assertion below is that
//! the BOOT SEQUENCE is what claims it. A neighbour test that installed
//! a capturing logger first would make this pass while proving the
//! opposite.

#[cfg(test)]
mod tests {
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, FanoutStrategy, StderrBootLog, SystemClock,
        bootstrap_adapter_with_envelope, install_stderr_boot_logger,
    };
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        BudgetError, Clock, DecodeCooperator, ImmediateResolveTimer, MAX_GVL_PARSED_BYTES,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        NoopDecodeCooperator, ResolvedIsolateLimits, Rng, TargetPlatformConstants, Timer,
        validate_isolate_limits,
    };
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
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
    const OUTBOUND_META_BYTES: u32 = auctioneer_bidder::outbound_meta_bytes(100);

    /// A row over its ceiling for the ordinary reason — a population the
    /// caps permit — so it takes the warn-and-boot path rather than any
    /// fatal one.
    fn over_ceiling_limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 64_000_000,
            baseline_bytes: 27_328_512,
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
            // Funds config extraction and the checked sync/privacy workload.
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

    /// Run the real boot entrypoint with this fixture's envelope.
    ///
    /// The host-primitive factories are written as functions with
    /// explicit return types rather than closures with `as Arc<dyn ..>`
    /// casts: the workspace denies `as` outright, and an annotated
    /// return type performs the same unsizing coercion without one.
    fn boot(limits: &ResolvedIsolateLimits) -> Result<(), BudgetError> {
        fn clock() -> Arc<dyn Clock + Send + Sync> {
            Arc::new(SystemClock::new())
        }
        fn rng() -> Arc<dyn Rng + Send + Sync> {
            Arc::new(DevRng::new())
        }
        fn timer() -> Arc<dyn Timer + Send + Sync> {
            Arc::new(ImmediateResolveTimer)
        }
        fn cooperator() -> Arc<dyn DecodeCooperator + Send + Sync> {
            Arc::new(NoopDecodeCooperator)
        }
        fn strategy() -> Arc<dyn FanoutStrategy + Send + Sync> {
            Arc::new(DelegateToDevStrategy)
        }
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                limits,
                target: &target(),
                outbound_meta_bytes: OUTBOUND_META_BYTES,
                construction_costs: core_construction_costs(),
                log_sink: &StderrBootLog,
                catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            },
            clock,
            rng,
            timer,
            cooperator,
            strategy,
        )
    }

    #[test]
    fn the_boot_sequence_claims_the_log_sink_before_it_validates() {
        let limits = over_ceiling_limits();

        // --- The fixture must really take the warn path. A row that
        // quietly fits would make every assertion below vacuous while
        // still passing.
        match validate_isolate_limits(
            &limits,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        ) {
            Err(BudgetError::BudgetOverCeiling { required, ceiling }) => {
                assert_eq!(ceiling, limits.isolate_total_bytes);
                assert!(required > ceiling, "the fixture must be over its ceiling");
            }
            other => panic!("the fixture must take the warn path, got {other:?}"),
        }

        // --- Before boot: nothing has claimed the global sink, so
        // `log` is filtering everything out. This test installs no
        // logger of its own, which is exactly the state a production
        // entrypoint boots in — and the state in which the over-ceiling
        // warning used to be emitted and dropped.
        //
        // `max_level` is the probe rather than `set_logger`, because
        // `set_logger` would CLAIM the slot on the way to answering and
        // the boot sequence would then lose the race this test is about.
        // An uninstalled `log` filters at `Off`.
        assert_eq!(
            log::max_level(),
            log::LevelFilter::Off,
            "this binary must boot with no sink, or it cannot observe who installs one",
        );
        assert!(
            !log::log_enabled!(target: "auctioneer::budget", log::Level::Warn),
            "with no sink installed the over-ceiling warning reaches nothing, which is the \
             state every adapter entrypoint booted in",
        );

        boot(&limits).expect("an over-ceiling envelope warns and boots");

        // --- After boot: a sink exists, and it passes `Warn`. This is
        // the assertion the entrypoint ordering used to fail: the sink
        // has to be claimed by the time validation runs, because the
        // `OnceLock` guarding the warning is consumed there and no
        // later request re-emits it.
        assert!(
            log::max_level() >= log::LevelFilter::Warn,
            "the boot sequence must claim a log sink, or the over-ceiling warning (the only \
             safety signal on the warn-and-boot path) is emitted into nothing and the \
             once-per-process guard is already spent",
        );
        assert!(
            log::log_enabled!(target: "auctioneer::budget", log::Level::Warn),
            "the installed sink must pass the level the over-ceiling diagnostic uses",
        );

        // --- The installer is idempotent and reports the race honestly.
        // A serverless entrypoint re-enters this sequence on every
        // request, so a second attempt must not fail a boot — and must
        // not claim it installed a sink it did not.
        assert!(
            !install_stderr_boot_logger(),
            "a second install does not claim the sink the first one already holds",
        );
        boot(&limits).expect("a repeated boot stays Ok");
    }
}
