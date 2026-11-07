//! The over-ceiling warning REACHES the sink the envelope carries.
//!
//! # Why this exists next to `boot_warning_reaches_a_sink.rs`
//!
//! That binary asserts the sink is installed, by probing
//! `log::max_level()` after `boot()` returns. Every one of its
//! assertions is taken after boot, so it observes post-boot residue
//! rather than ordering: validating the envelope before installing the
//! sink leaves `max_level() >= Warn` true all the same, and the test
//! stays green.
//!
//! That ordering is the whole guarantee. `OVER_CEILING_WARNED` is a
//! one-shot `OnceLock` CONSUMED before the emit — `.set(())` succeeds,
//! then `warn_over_ceiling` logs — so a validation that runs before any
//! sink exists spends the latch on a warning that goes nowhere, and no
//! later request re-emits it. The isolate then serves for its whole life
//! with its only safety signal silently gone.
//!
//! So this binary asserts the EFFECT instead of the residue: it hands
//! `BootEnvelope` a capturing `log::Log` and requires the over-ceiling
//! record to be in it afterwards. That can only hold if the sink was
//! claimed before validation ran, which makes the assertion fail under
//! reordering rather than pass either way.
//!
//! # Why this is a binary of its own
//!
//! `log`'s global logger is process-global and claimable once, and the
//! claim under test is that the boot sequence is what claims it. A
//! neighbour test installing a capturing logger first would make this
//! pass while proving the opposite — the same reason its sibling is
//! alone in its binary. Both need to be the only test in a process, so
//! they cannot be the same one.

#[cfg(test)]
mod tests {
    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, FanoutStrategy, SystemClock, boot_gates,
        bootstrap_adapter_with_envelope,
    };
    use auctioneer_runtime::budget::{ASSEMBLY_FACTOR, CONFIG_PARSE_FACTOR};
    use auctioneer_runtime::{
        BudgetError, Clock, DecodeCooperator, ImmediateResolveTimer, MAX_GVL_PARSED_BYTES,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        NoopDecodeCooperator, ResolvedIsolateLimits, Rng, TargetPlatformConstants, Timer,
        serialized_json_bytes, validate_isolate_limits,
    };
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use std::sync::{Arc, Mutex, PoisonError};

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

    /// Every record this process logs, in arrival order.
    ///
    /// A `Mutex<Vec<..>>` rather than a channel because the assertion is
    /// about CONTENT, not timing: by the time it runs, boot has
    /// returned, so anything the warning emitted is already here.
    static CAPTURED: CapturingBootLog = CapturingBootLog {
        records: Mutex::new(Vec::new()),
    };

    /// A sink that keeps what it is given.
    ///
    /// `enabled` answers true at every level, so a record missing from
    /// `records` was never offered rather than filtered — which keeps
    /// the assertion below about ordering and not about this sink's own
    /// level policy.
    struct CapturingBootLog {
        records: Mutex<Vec<(String, String)>>,
    }

    impl log::Log for CapturingBootLog {
        fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            let Ok(mut records) = self.records.lock() else {
                return;
            };
            records.push((record.target().to_owned(), record.args().to_string()));
        }

        fn flush(&self) {}
    }

    impl CapturingBootLog {
        /// Records whose target and message both match, as whole
        /// messages so a failure prints what did arrive.
        fn matching(&self, target: &str, needle: &str) -> Vec<String> {
            let records = self.records.lock().unwrap_or_else(PoisonError::into_inner);
            records
                .iter()
                .filter(|(got, message)| got == target && message.contains(needle))
                .map(|(_, message)| message.clone())
                .collect()
        }

        fn len(&self) -> usize {
            self.records.lock().map_or(0, |records| records.len())
        }

        /// Every record, for a failure message that shows what did
        /// arrive rather than only that the expected one did not.
        fn all(&self) -> Vec<String> {
            let records = self.records.lock().unwrap_or_else(PoisonError::into_inner);
            records
                .iter()
                .map(|(target, message)| format!("[{target}] {message}"))
                .collect()
        }
    }

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

    /// Run the real boot entrypoint with the capturing sink in the
    /// envelope.
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
                log_sink: &CAPTURED,
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
    fn the_over_ceiling_warning_arrives_in_the_sink_the_envelope_carried() {
        let limits = over_ceiling_limits();
        let platform = target();
        let costs = core_construction_costs();
        let catalog = auctioneer_runtime::CatalogResourceCosts::UNCHANGED;

        // --- The fixture must really take the warn path. A row that
        // quietly fits would make every assertion below vacuous while
        // still passing, because nothing would be emitted either way.
        let bare_validation_required = match validate_isolate_limits(
            &limits,
            &platform,
            OUTBOUND_META_BYTES,
            costs,
            catalog,
        ) {
            Err(BudgetError::BudgetOverCeiling { required, ceiling }) => {
                assert_eq!(ceiling, limits.isolate_total_bytes);
                assert!(required > ceiling, "the fixture must be over its ceiling");
                required
            }
            other => panic!("the fixture must take the warn path, got {other:?}"),
        };

        // --- Nothing has been captured yet, and nothing has claimed the
        // global sink. This binary installs no logger of its own, which
        // is the state a production entrypoint boots in.
        assert_eq!(
            CAPTURED.len(),
            0,
            "the sink must start empty, or a neighbour already logged and this \
             test cannot attribute what it finds to boot",
        );
        assert_eq!(
            log::max_level(),
            log::LevelFilter::Off,
            "this binary must boot with no sink, or it cannot observe who installs one",
        );

        boot(&limits).expect("an over-ceiling envelope warns and boots");

        // --- The load-bearing assertion. The warning is emitted from
        // inside `install_boot_envelope`, so it can only be in this
        // sink if the sink was claimed BEFORE validation ran. Swap the
        // two statements in `bootstrap_adapter_with_envelope` and the
        // record is emitted into a process with no logger, this vector
        // stays empty, and this fails — which is what the sibling
        // binary's post-boot `max_level()` probe cannot see.
        // The diagnostic is a headline plus one record per dominating
        // term, so this matches the headline alone — the term list is
        // `the_over_ceiling_row_names_the_terms_that_dominate`'s job on
        // each adapter row.
        let warnings = CAPTURED.matching(
            edgezero_core::BOOT_LOG_TARGET,
            "boot envelope over ceiling: requires",
        );
        assert_eq!(
            warnings.len(),
            1,
            "the over-ceiling warning must reach the envelope's own sink exactly \
             once. Finding none means the envelope was validated before the sink \
             was installed: the `OVER_CEILING_WARNED` latch is consumed before the \
             emit, so that warning is gone for the isolate's whole life. Captured \
             records: {:?}",
            CAPTURED.all(),
        );

        // --- It is the real diagnostic, not an empty line that happens
        // to match the target. The overshoot an operator acts on has to
        // be in the message. Derive it from validation: the budget's unit
        // tests own formula arithmetic; this test owns sink delivery.
        let mut roomy = limits;
        roomy.isolate_total_bytes = u64::MAX;
        let bare_required =
            validate_isolate_limits(&roomy, &platform, OUTBOUND_META_BYTES, costs, catalog)
                .expect("roomy envelope is valid")
                .total_bytes;
        assert_eq!(bare_required, bare_validation_required);
        assert_eq!(
            validate_isolate_limits(&limits, &platform, OUTBOUND_META_BYTES, costs, catalog),
            Err(BudgetError::BudgetOverCeiling {
                required: bare_required,
                ceiling: limits.isolate_total_bytes,
            }),
        );
        let baked = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .expect("baked config is serializable");
        let required = boot_gates()
            .expect("boot installed gates")
            .derived
            .total_bytes;
        assert_eq!(
            required,
            bare_required + u64::from(limits.max_concurrent_auctions) * baked * CONFIG_PARSE_FACTOR,
            "boot funds a baked-config clone per concurrent auction",
        );
        let overshoot = required - limits.isolate_total_bytes;
        let warning = &warnings[0];
        assert!(
            warning.contains(&required.to_string())
                && warning.contains(&limits.isolate_total_bytes.to_string())
                && warning.contains(&overshoot.to_string()),
            "the captured warning must carry the required total, the ceiling and the \
             overshoot, got {warning:?}",
        );

        // --- The latch is spent, so a second boot does not re-emit.
        // This is why the ordering above is the only chance the warning
        // gets, and asserting it here keeps that premise honest rather
        // than stated in a comment.
        boot(&limits).expect("a repeated boot stays Ok");
        assert_eq!(
            CAPTURED
                .matching(
                    edgezero_core::BOOT_LOG_TARGET,
                    "boot envelope over ceiling: requires"
                )
                .len(),
            1,
            "the once-per-process latch must not re-emit on a second boot, which is \
             what makes the first emit's ordering load-bearing",
        );
    }
}
