//! Request limits resolve against the installed boot envelope, never
//! past it.
//!
//! Seven host knobs name a number the boot envelope also funds.
//! `max_bidders_per_auction` and `max_outbound_requests` decide how
//! many outbound calls, and how many retained bidder bodies, one
//! auction may produce. `max_outbound_raw_bytes` and
//! `max_outbound_request_bytes` bound one OUTBOUND body as the sink
//! writes it and after encoding. `max_transport_bytes` and
//! `max_decoded_bytes` bound one bidder response on the wire and after
//! decode, and `max_parsed_nodes` bounds what the inbound parse
//! retains. The boot
//! envelope FUNDS a number for each; the host config CONFIGURES one.
//! Those are different numbers from different places, and the shipped
//! baked defaults for the two populations are larger than what the wasm
//! envelopes fund — 50 bidders and 100 calls against a 32-call row. A
//! resolver that took the configured value directly would authorise
//! allocation and dispatch the envelope never budgeted for, and a
//! runtime override could widen that gap without a redeploy.
//!
//! The clamp runs in ONE direction. `validate_isolate_limits` proves at
//! boot that the envelope fits its ceiling, and that proof assumed the
//! funded figure for every term, so a runtime value ABOVE it would make
//! the proof false. Lowering is always safe, because every term is a
//! worst-case charge — which is why these are knobs at all.
//!
//! The stream gate does not cover this. It bounds how many bidder legs
//! are decoding SIMULTANEOUSLY; it puts no bound on how many calls an
//! auction issues in total, nor on how many response bodies it retains
//! once decoded.
//!
//! This is the ONLY test in its binary, for the same reason
//! `boot_order_gates.rs` is: the envelope sits behind a process-global
//! `OnceLock`, so "no envelope is installed" and "this exact envelope
//! is installed" are both facts about a whole process.

#[cfg(test)]
mod tests {
    use auctioneer_config::types::{HostConfig, ResourceLimits};
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::resolve::{limits_for_request, resolved_host};
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{boot_limits, install_boot_envelope};
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES,
        MIN_RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits, TargetPlatformConstants,
    };
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;

    const CAP_256_KIB: u64 = 1 << 18;
    const CAP_256_KIB_USIZE: usize = 1 << 18;
    const BYTES_1_MIB: u64 = 1 << 20;
    /// The response cap, DERIVED from the assembly scratch the budget
    /// requires it to hold (`max_outbound_requests x max_decoded_bytes x
    /// ASSEMBLY_FACTOR`). A literal here went stale the moment the
    /// factor was corrected.
    const FINAL_RESPONSE_BYTES: u64 = 64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    const BYTES_64_KIB: u32 = 1 << 16;
    const BYTES_16_KIB: u32 = 1 << 14;
    const OUTBOUND_META_BYTES: u32 = auctioneer_bidder::outbound_meta_bytes(100);

    /// What this envelope FUNDS. Deliberately the wasm row's call
    /// count, and a bidder count below it, so "clamped to the funded
    /// number" and "left at the configured number" are different
    /// assertions and neither can pass by coincidence.
    const FUNDED_CALLS: u32 = 32;
    const FUNDED_BIDDERS: u32 = 24;

    /// The same two numbers in the width `ResourceLimits` speaks.
    /// Written out rather than converted with `as`, which the workspace
    /// denies for hiding a lossy conversion, and pinned to their `u32`
    /// originals below so the pair cannot drift.
    const FUNDED_CALLS_USIZE: usize = 32;
    const FUNDED_BIDDERS_USIZE: usize = 24;

    const _: () = assert!(
        FUNDED_CALLS_USIZE == 32 && FUNDED_BIDDERS_USIZE == 24,
        "the usize twins drifted from the funded counts above"
    );

    /// What the shipped baked `auctioneer.toml` CONFIGURES — both above
    /// what the row above funds, which is the shipped situation this
    /// test exists for.
    const BAKED_BIDDERS: usize = 50;
    const BAKED_CALLS: usize = 100;

    fn funded_limits() -> ResolvedIsolateLimits {
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
            aux_route_permits: 5,
            // Funds config extraction and the checked sync/privacy workload.
            aux_route_peak_bytes: 768 * 1_024 * 1_024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: FUNDED_BIDDERS,
            max_outbound_requests: FUNDED_CALLS,
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

    /// A baked row whose every envelope-funded cap is ABOVE what the
    /// envelope above funds, which is what makes the default-config arm
    /// of the test prove something.
    ///
    /// The two population caps carry the shipped `auctioneer.toml` numbers,
    /// which already exceed the wasm row. The three byte/node caps are
    /// deliberately set OVER this fixture's envelope rather than at the
    /// shipped 256 KiB / 65536: the shipped values equal what every
    /// adapter funds, so a fixture that reused them would assert only
    /// that `min(x, x) == x` and would still pass with the clamp
    /// deleted.
    fn baked_host() -> HostConfig {
        HostConfig {
            status_response: "OK".into(),
            resource_limits: ResourceLimits {
                default_tmax_ms: 1_000,
                max_tmax_ms: 2_000,
                max_bidder_response_bytes: CAP_256_KIB_USIZE,
                max_bidders_per_auction: BAKED_BIDDERS,
                max_outbound_requests: BAKED_CALLS,
                max_outbound_raw_bytes: BAKED_OUTBOUND_RAW_BYTES,
                max_outbound_request_bytes: BAKED_OUTBOUND_REQUEST_BYTES,
                max_transport_bytes: BAKED_TRANSPORT_BYTES,
                max_decoded_bytes: BAKED_DECODED_BYTES,
                max_parsed_nodes: BAKED_PARSED_NODES,
            },
            ..HostConfig::default()
        }
    }

    /// The five envelope-funded byte/node caps a baked row declares
    /// here, each above the funded figure so the clamp has work to do.
    ///
    /// The two outbound caps are DIFFERENT multiples of the funded
    /// figure from each other and from the response caps: a fixture
    /// that gave them one number could not tell a clamp that resolves
    /// each against its own ceiling from one that happened to pick the
    /// right value twice.
    const BAKED_OUTBOUND_RAW_BYTES: u64 = CAP_256_KIB * 8;
    const BAKED_OUTBOUND_REQUEST_BYTES: u64 = CAP_256_KIB * 16;
    const BAKED_TRANSPORT_BYTES: u64 = CAP_256_KIB * 2;
    const BAKED_DECODED_BYTES: u64 = CAP_256_KIB * 4;
    const BAKED_PARSED_NODES: u32 = BYTES_64_KIB * 2;

    const _: () = assert!(
        BAKED_OUTBOUND_RAW_BYTES > CAP_256_KIB
            && BAKED_OUTBOUND_REQUEST_BYTES > CAP_256_KIB
            && BAKED_TRANSPORT_BYTES > CAP_256_KIB
            && BAKED_DECODED_BYTES > CAP_256_KIB
            && BAKED_PARSED_NODES > BYTES_64_KIB,
        "a baked cap at or below the funded one would let the default arm pass \
         with the clamp removed"
    );

    /// Default config, an override ABOVE the funded ceiling, and an
    /// override BELOW it — in one process, because the envelope they
    /// resolve against is process-global.
    #[test]
    fn configured_limits_never_exceed_what_the_envelope_funds() {
        let limits = funded_limits();
        install_boot_envelope(
            &limits,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the fixture envelope is valid and installs");
        assert_eq!(
            boot_limits().map(|row| row.max_outbound_requests),
            Some(FUNDED_CALLS),
            "the installed row is the one the clamp must resolve against",
        );

        // --- The DEFAULT configuration, with no runtime override at
        // all. This is the shipped situation: the baked row alone
        // already authorises more than the wasm envelopes fund, so a
        // resolver that only clamped overrides would still ship a gap.
        let default_row = resolved_host(&AuctioneerAppConfig::default(), &baked_host());
        assert_eq!(
            default_row.resource_limits.max_outbound_requests, FUNDED_CALLS_USIZE,
            "the shipped baked default authorises {BAKED_CALLS} calls against a \
             {FUNDED_CALLS}-call envelope, so the effective value must be the funded one",
        );
        assert_eq!(
            default_row.resource_limits.max_bidders_per_auction, FUNDED_BIDDERS_USIZE,
            "the shipped baked default authorises {BAKED_BIDDERS} bidders against a \
             {FUNDED_BIDDERS}-bidder envelope",
        );

        // --- A runtime override ABOVE the ceiling. Config pushed to the
        // store must not be able to widen the gap further.
        let over: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_bidders_per_auction = 500\nmax_outbound_requests = 1000\n",
        )
        .expect("the override parses");
        let over_row = resolved_host(&over, &baked_host());
        assert_eq!(
            over_row.resource_limits.max_outbound_requests, FUNDED_CALLS_USIZE,
            "a pushed override of 1000 calls must not authorise past the funded {FUNDED_CALLS}",
        );
        assert_eq!(
            over_row.resource_limits.max_bidders_per_auction, FUNDED_BIDDERS_USIZE,
            "a pushed override of 500 bidders must not authorise past the funded \
             {FUNDED_BIDDERS}",
        );

        // --- A runtime override BELOW the ceiling is an operator
        // decision and survives untouched. A clamp that pinned every
        // deployment to the funded number would silently discard this,
        // which is a different bug in the same line.
        let under: AuctioneerAppConfig =
            toml::from_str("[auction]\nmax_bidders_per_auction = 6\nmax_outbound_requests = 9\n")
                .expect("the override parses");
        let under_row = resolved_host(&under, &baked_host());
        assert_eq!(
            under_row.resource_limits.max_outbound_requests, 9,
            "an override below the funded ceiling is the operator's own smaller number",
        );
        assert_eq!(
            under_row.resource_limits.max_bidders_per_auction, 6,
            "an override below the funded ceiling is the operator's own smaller number",
        );

        // --- The two knobs are clamped INDEPENDENTLY. A single shared
        // minimum would make a row whose bidder and call ceilings differ
        // collapse to one of them.
        let split: AuctioneerAppConfig =
            toml::from_str("[auction]\nmax_bidders_per_auction = 500\nmax_outbound_requests = 9\n")
                .expect("the override parses");
        let split_row = resolved_host(&split, &baked_host());
        assert_eq!(
            (
                split_row.resource_limits.max_bidders_per_auction,
                split_row.resource_limits.max_outbound_requests,
            ),
            (FUNDED_BIDDERS_USIZE, 9),
            "each knob resolves against its OWN funded ceiling",
        );

        the_three_response_caps_resolve_against_the_installed_row(limits, &default_row);
        the_two_outbound_caps_resolve_against_the_installed_row(limits, &default_row);
    }

    /// The two OUTBOUND byte caps, asserted the same four ways.
    ///
    /// A helper for the same reason as the response-cap helper above:
    /// the envelope these resolve against is a process-global
    /// `OnceLock`, so the install and every assertion depending on it
    /// belong to one test.
    ///
    /// These two caps bound the body auctioneer SENDS, not the one it
    /// receives, and the per-call envelope term charges their SUM. The
    /// spec gives both a `[auction]` surface and says the overrides
    /// clamp DOWNWARD only, so an override above the funded figure must
    /// be refused by the clamp rather than accepted — a widened
    /// outbound cap would authorise a construction buffer and an
    /// encoded buffer the boot-time proof never charged for.
    fn the_two_outbound_caps_resolve_against_the_installed_row(
        limits: ResolvedIsolateLimits,
        default_row: &HostConfig,
    ) {
        // Default config: each baked value is above what this envelope
        // funds, so each resolves DOWN to the funded figure.
        assert_eq!(
            (
                default_row.resource_limits.max_outbound_raw_bytes,
                default_row.resource_limits.max_outbound_request_bytes,
            ),
            (CAP_256_KIB, CAP_256_KIB),
            "a baked outbound cap above the funded one must resolve to the funded one, \
             with no override pushed at all",
        );

        // An override ABOVE each funded figure is clamped, not accepted.
        let over: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_outbound_raw_bytes = 4194304\n\
             max_outbound_request_bytes = 8388608\n",
        )
        .expect("the override parses");
        let over_row = resolved_host(&over, &baked_host());
        assert_eq!(
            (
                over_row.resource_limits.max_outbound_raw_bytes,
                over_row.resource_limits.max_outbound_request_bytes,
            ),
            (CAP_256_KIB, CAP_256_KIB),
            "the outbound-cap overrides clamp DOWNWARD only: a pushed value above the \
             funded figure must not authorise a send buffer the envelope never charged \
             for",
        );

        // An override BELOW each funded figure is the operator's own
        // number and survives. This is the direction the knob exists
        // for.
        let under: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_outbound_raw_bytes = 65536\n\
             max_outbound_request_bytes = 32768\n",
        )
        .expect("the override parses");
        let under_row = resolved_host(&under, &baked_host());
        assert_eq!(
            (
                under_row.resource_limits.max_outbound_raw_bytes,
                under_row.resource_limits.max_outbound_request_bytes,
            ),
            (0x0001_0000, 0x0000_8000),
            "an override below the funded figure is the operator's own smaller number",
        );

        // Each resolves against its OWN funded figure, and the two are
        // independent of each other: one over, one under.
        let split: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_outbound_raw_bytes = 4194304\n\
             max_outbound_request_bytes = 32768\n",
        )
        .expect("the override parses");
        let split_row = resolved_host(&split, &baked_host());
        assert_eq!(
            (
                split_row.resource_limits.max_outbound_raw_bytes,
                split_row.resource_limits.max_outbound_request_bytes,
            ),
            (CAP_256_KIB, 0x0000_8000),
            "the raw cap and the encoded cap resolve independently; a shared minimum \
             would collapse the pair onto whichever is smaller",
        );

        // The narrowed row the request path receives carries the
        // LOWERED outbound figures. This is the assertion that proves
        // enforcement reads the lowered value: `drive_fanout` takes its
        // encoded ceiling from `limits.max_outbound_request_bytes` on
        // this row, and `outbound::build` hands the same field to the
        // host's `max_request_body_bytes`. A resolver that lowered the
        // config but left this row alone would pass every assertion
        // above while every outbound body still enforced the envelope's
        // own 256 KiB.
        let narrowed = limits_for_request(&limits, &under_row);
        assert_eq!(
            (
                narrowed.max_outbound_raw_bytes,
                narrowed.max_outbound_request_bytes,
            ),
            (0x0001_0000, 0x0000_8000),
            "the row the fanout reads its outbound ceilings from must carry the \
             resolved caps, not the envelope's own",
        );
        assert!(
            narrowed.max_outbound_request_bytes < limits.max_outbound_request_bytes
                && narrowed.max_outbound_raw_bytes < limits.max_outbound_raw_bytes,
            "the narrowed row must be STRICTLY below the installed envelope's figures, \
             or this fixture cannot tell a narrowing from a passthrough",
        );
    }

    /// The three response/parse caps, asserted the same three ways the
    /// two population caps are above.
    ///
    /// A helper rather than a second `#[test]`: the envelope these
    /// resolve against is a process-global `OnceLock`, so the install
    /// and every assertion that depends on it belong to one test.
    fn the_three_response_caps_resolve_against_the_installed_row(
        limits: ResolvedIsolateLimits,
        default_row: &HostConfig,
    ) {
        // --- The three response/parse caps, the same three ways. These
        // are the caps the request path reads for the per-stream byte
        // charge and the inbound node charge, and until they were
        // settable a deployment could only lower them by rebuilding.

        // Default config: each baked value is above what this envelope
        // funds, so each resolves DOWN to the funded figure.
        assert_eq!(
            (
                default_row.resource_limits.max_transport_bytes,
                default_row.resource_limits.max_decoded_bytes,
                default_row.resource_limits.max_parsed_nodes,
            ),
            (CAP_256_KIB, CAP_256_KIB, BYTES_64_KIB),
            "a baked cap above the funded one must resolve to the funded one, with no \
             override pushed at all",
        );

        // An override ABOVE each funded figure is clamped, not accepted.
        let caps_over: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_transport_bytes = 1048576\nmax_decoded_bytes = 2097152\n\
             max_parsed_nodes = 262144\n",
        )
        .expect("the override parses");
        let caps_over_row = resolved_host(&caps_over, &baked_host());
        assert_eq!(
            (
                caps_over_row.resource_limits.max_transport_bytes,
                caps_over_row.resource_limits.max_decoded_bytes,
                caps_over_row.resource_limits.max_parsed_nodes,
            ),
            (CAP_256_KIB, CAP_256_KIB, BYTES_64_KIB),
            "a pushed override above the funded figure must not authorise memory the \
             boot-time envelope proof never charged for",
        );

        // An override BELOW each funded figure is the operator's own
        // number and survives. This is the direction the knob exists
        // for: lowering is always safe because every envelope term is a
        // worst-case charge.
        let caps_under: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_transport_bytes = 65536\nmax_decoded_bytes = 32768\n\
             max_parsed_nodes = 4096\n",
        )
        .expect("the override parses");
        let caps_under_row = resolved_host(&caps_under, &baked_host());
        assert_eq!(
            (
                caps_under_row.resource_limits.max_transport_bytes,
                caps_under_row.resource_limits.max_decoded_bytes,
                caps_under_row.resource_limits.max_parsed_nodes,
            ),
            (0x0001_0000, 0x0000_8000, 0x0000_1000),
            "an override below the funded figure is the operator's own smaller number; \
             a clamp that pinned every deployment to the funded one would discard it",
        );

        // Each of the three resolves against its OWN funded figure. One
        // shared minimum across the group would collapse all three onto
        // the node budget, which is numerically the smallest.
        let caps_split: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_transport_bytes = 1048576\nmax_decoded_bytes = 32768\n\
             max_parsed_nodes = 262144\n",
        )
        .expect("the override parses");
        let caps_split_row = resolved_host(&caps_split, &baked_host());
        assert_eq!(
            (
                caps_split_row.resource_limits.max_transport_bytes,
                caps_split_row.resource_limits.max_decoded_bytes,
                caps_split_row.resource_limits.max_parsed_nodes,
            ),
            (CAP_256_KIB, 0x0000_8000, BYTES_64_KIB),
            "a knob over its ceiling, one under it and one over it again must each \
             resolve against its own funded figure",
        );

        // The narrowed row the request path actually receives carries
        // exactly those three figures and leaves every other envelope
        // field alone. A clamp that resolved the config but never
        // reached the row would pass every assertion above and still
        // leave the baked value in force where the caps are read.
        let narrowed = limits_for_request(&limits, &caps_under_row);
        assert_eq!(
            (
                narrowed.max_transport_bytes,
                narrowed.max_decoded_bytes,
                narrowed.max_parsed_nodes,
            ),
            (0x0001_0000, 0x0000_8000, 0x0000_1000),
            "the row handed to the fanout must carry the resolved caps, not the \
             envelope's own",
        );
        assert_eq!(
            ResolvedIsolateLimits {
                max_outbound_raw_bytes: limits.max_outbound_raw_bytes,
                max_outbound_request_bytes: limits.max_outbound_request_bytes,
                max_transport_bytes: limits.max_transport_bytes,
                max_decoded_bytes: limits.max_decoded_bytes,
                response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
                response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
                max_parsed_nodes: limits.max_parsed_nodes,
                ..narrowed
            },
            limits,
            "narrowing the five config-settable caps must leave every other envelope \
             field untouched",
        );
    }
}
