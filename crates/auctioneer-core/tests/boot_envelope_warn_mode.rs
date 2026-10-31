//! An over-ceiling boot envelope WARNS and boots; an inconsistent one
//! still fails.
//!
//! This is the regression that keeps warn-mode from becoming
//! warn-everything. `install_boot_envelope` classifies every
//! `BudgetError` variant, and only `BudgetOverCeiling` is tolerated —
//! it says the declared caps PERMIT a state that would not fit, which
//! is an ambition about load that tuning against production traffic
//! can settle. A cap below a MEASURED floor is a different thing: it
//! under-reserves memory the code needs on every request, so booting
//! past one means overrunning at the first request that exercises it.
//! If the fatal half of that distinction ever stops being tested, the
//! warn path silently swallows it.
//!
//! This is the ONLY test in its binary, for the same reason
//! `boot_order_gates.rs` is: the gates sit behind a process-global
//! `OnceLock`, so "nothing is installed yet" holds only in a process
//! nothing else has booted, and a fatal envelope leaving NO gates
//! behind is exactly what the assertions below rest on.

#[cfg(test)]
mod tests {
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{boot_gates, boot_limits, install_boot_envelope};
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        BudgetError, LimitField, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits,
        TargetPlatformConstants, validate_isolate_limits,
    };
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;

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

    /// Distinctive gate capacities, so an assertion that a gate carries
    /// the envelope's capacity cannot pass by coincidence against a
    /// default or against a neighbouring gate's number.
    const EGRESS_PERMITS: u32 = 3;
    const AUX_PERMITS: u32 = 5;
    const CONCURRENT_AUCTIONS: u32 = 2;
    const BODY_STREAMS: u32 = 7;

    /// A row whose worst-case total is far over the ceiling it declares
    /// — and over it for the ordinary reason, a population the caps
    /// permit rather than any inconsistency. Every other check passes.
    fn over_ceiling_limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            // Deliberately tiny: the row below needs thousands of
            // megabytes, so this is the overshoot and nothing else is.
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

    /// Every envelope that must still END a boot, each asserted while
    /// NO gates exist.
    ///
    /// This is the regression that matters most: it is how warn-mode is
    /// kept from becoming warn-everything. These are caps below a
    /// MEASURED floor, so they under-reserve memory the code needs on
    /// every request rather than in some worst case, and no amount of
    /// traffic tuning makes booting past one safe.
    ///
    /// Each also asserts that nothing was installed. A fatal envelope
    /// that left gates behind would hand the next caller capacities
    /// nobody validated.
    fn assert_every_floor_breach_is_fatal() {
        let mut under_parse = over_ceiling_limits();
        under_parse.inbound_parse_factor = MIN_INBOUND_PARSE_FACTOR - 1;

        let mut under_gvl = over_ceiling_limits();
        under_gvl.gvl_parsed_bytes = MAX_GVL_PARSED_BYTES - 1;

        let mut short_response = over_ceiling_limits();
        short_response.final_response_bytes = CAP_256_KIB;

        let mut under_response_bytes = over_ceiling_limits();
        under_response_bytes.response_parse_factor = MIN_RESPONSE_PARSE_FACTOR - 1;

        let mut under_response_nodes = over_ceiling_limits();
        under_response_nodes.response_node_peak_bytes = MIN_RESPONSE_NODE_PEAK_BYTES - 1;

        let cases = [
            (
                "a parse factor under the measured floor",
                under_parse,
                BudgetError::LimitBelowMinimum {
                    field: LimitField::InboundParseFactor,
                    value: MIN_INBOUND_PARSE_FACTOR - 1,
                    minimum: MIN_INBOUND_PARSE_FACTOR,
                },
            ),
            (
                "a resident-GVL reservation under the measured retention",
                under_gvl,
                BudgetError::LimitBelowMinimum {
                    field: LimitField::GvlParsedBytes,
                    value: MAX_GVL_PARSED_BYTES - 1,
                    minimum: MAX_GVL_PARSED_BYTES,
                },
            ),
            (
                "a response cap below the scratch every auction produces",
                short_response,
                BudgetError::FinalResponseBelowAssemblyFloor {
                    configured: CAP_256_KIB,
                    required: 64 * CAP_256_KIB * ASSEMBLY_FACTOR,
                },
            ),
            (
                "a response parse factor under the measured floor",
                under_response_bytes,
                BudgetError::LimitBelowMinimum {
                    field: LimitField::ResponseParseFactor,
                    value: MIN_RESPONSE_PARSE_FACTOR - 1,
                    minimum: MIN_RESPONSE_PARSE_FACTOR,
                },
            ),
            (
                "a per-node response charge under the measured peak",
                under_response_nodes,
                BudgetError::LimitBelowMinimum {
                    field: LimitField::ResponseNodePeakBytes,
                    value: MIN_RESPONSE_NODE_PEAK_BYTES - 1,
                    minimum: MIN_RESPONSE_NODE_PEAK_BYTES,
                },
            ),
        ];

        for (what, limits, expected) in cases {
            assert_eq!(
                install_boot_envelope(
                    &limits,
                    &target(),
                    OUTBOUND_META_BYTES,
                    core_construction_costs(),
                    auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
                ),
                Err(expected),
                "{what} must END the boot, not warn: it under-reserves memory every \
                 request needs, so tuning against traffic cannot make it safe",
            );
            assert!(
                boot_gates().is_none(),
                "{what} must install NO gates, or a failed boot leaves the isolate \
                 serving with capacities nobody validated",
            );
        }
    }

    /// The whole classification, in one function on one process.
    ///
    /// Sequential rather than several `#[test]`s because each step's
    /// precondition is the previous step's postcondition on a
    /// process-global `OnceLock`: the fatal envelopes must be refused
    /// while NO gates exist, which stops being checkable the moment the
    /// warn path installs any. `cargo test` runs test functions
    /// concurrently, so separate functions could not establish that
    /// order.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the process-global boot fixture checks fatal refusals before installing charged gates and once-only warnings afterwards"
    )]
    fn over_ceiling_warns_and_boots_while_a_floor_breach_still_fails() {
        testing_logger::setup();

        assert_every_floor_breach_is_fatal();

        // --- An over-ceiling envelope is the one the validator refuses
        // and the boot sequence tolerates. Confirm the validator really
        // does refuse this row, so the test below is exercising the
        // warn path and not a row that quietly fits.
        let limits = over_ceiling_limits();
        let bare_required = match validate_isolate_limits(
            &limits,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        ) {
            Err(BudgetError::BudgetOverCeiling { required, ceiling }) => {
                assert_eq!(ceiling, limits.isolate_total_bytes);
                required
            }
            other => panic!("the fixture must be over its ceiling, got {other:?}"),
        };
        assert!(bare_required > limits.isolate_total_bytes);

        install_boot_envelope(
            &limits,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .expect("an over-ceiling envelope warns and boots rather than failing");

        // --- It BOOTED: the gates exist, at the envelope's own
        // capacities. A warn that installed nothing would refuse every
        // request and look exactly like the refusal it replaced.
        let gates = boot_gates().expect("the warn path installs the gates");
        // Boot also funds baked policy clones independently of the fetched
        // blob. Its warning must report that complete, retained derivation.
        let required = gates.derived.total_bytes;
        let overshoot = required - limits.isolate_total_bytes;
        assert_eq!(gates.streams.capacity(), BODY_STREAMS);
        // `AdmissionGate` exposes no capacity accessor, so its capacity
        // is read the way a request reads it: take every permit and
        // confirm the next one is refused. That also proves the gate
        // ADMITS, which a capacity number alone would not — a warn path
        // that installed a zero-width gate would refuse every request
        // and look exactly like the boot failure it replaced.
        let permits: Vec<_> = (0..CONCURRENT_AUCTIONS)
            .map(|slot| {
                gates
                    .admission
                    .try_acquire()
                    .unwrap_or_else(|| panic!("admission slot {slot} must be grantable"))
            })
            .collect();
        assert_eq!(gates.admission.in_use(), CONCURRENT_AUCTIONS);
        assert!(
            gates.admission.try_acquire().is_none(),
            "the admission gate must bound at the envelope's capacity",
        );
        drop(permits);
        assert_eq!(gates.egress.capacity(), EGRESS_PERMITS);
        assert_eq!(gates.aux.capacity(), AUX_PERMITS);
        assert_ne!(
            gates.aux.capacity(),
            gates.egress.capacity(),
            "the fixture must distinguish the capacities for this to prove anything",
        );
        assert_eq!(
            boot_limits().map(|installed| installed.isolate_total_bytes),
            Some(limits.isolate_total_bytes),
            "the over-ceiling envelope is retained for per-App configuration to read",
        );

        // --- A repeat call is what a serverless adapter does on every
        // request, and it must still succeed while warning only once:
        // a warning repeated on every fetch is one an operator learns
        // to filter out.
        install_boot_envelope(
            &limits,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a repeated over-ceiling install stays Ok");

        // --- The warning is the entire safety margin on this path, so
        // assert what an operator actually reads: the three numbers
        // they need, and a term to act on.
        testing_logger::validate(|captured| {
            let warns: Vec<&String> = captured
                .iter()
                .filter(|line| line.level == log::Level::Warn)
                .map(|line| &line.body)
                .collect();
            assert!(
                !warns.is_empty(),
                "booting over the ceiling must warn; silence here is the whole risk",
            );
            let headlines: Vec<&&String> = warns
                .iter()
                .filter(|body| body.contains("requires"))
                .collect();
            assert_eq!(
                headlines.len(),
                1,
                "the over-ceiling warning fires once per process, not once per \
                 install call: {warns:?}",
            );
            let headline = headlines[0];
            for needle in [
                &required.to_string(),
                &limits.isolate_total_bytes.to_string(),
                &overshoot.to_string(),
            ] {
                assert!(
                    headline.contains(needle.as_str()),
                    "the warning must carry {needle}: {headline}",
                );
            }
            assert!(
                headline.contains("Booting anyway"),
                "the warning must say the process is starting regardless: {headline}",
            );

            // "You are N bytes over" without saying where is not
            // actionable, so the dominating terms are named too. At
            // this fixture's capacities the auction and sync-funded aux
            // populations dominate the charge.
            for cap in ["aux_route_permits", "max_concurrent_auctions"] {
                assert!(
                    warns.iter().any(|body| body.contains(cap)),
                    "the warning must name {cap} to act on: {warns:?}",
                );
            }
        });
    }
}
