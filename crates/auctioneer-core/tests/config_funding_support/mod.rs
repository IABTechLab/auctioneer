//! Explicit funding for test contexts constructed with the SDK's default caps.

/// Unmodified seed row, also reused by fixtures with custom boot assertions.
pub(crate) use crate::fanout_limits as envelope;

use auctioneer_core::handlers::request_disclosure_audit::construction_costs;
use auctioneer_core::host_primitives::{boot_gates, install_boot_envelope};
use auctioneer_runtime::budget::{ASSEMBLY_FACTOR, derive_aux_work_for_profile};
use auctioneer_runtime::{ResolvedIsolateLimits, TargetPlatformConstants, serialized_json_bytes};
use edgezero_core::config_store::ConfigExtractionLimits;
use std::sync::OnceLock;

/// Fund the SDK's captured default caps and their derived auxiliary allowance.
///
/// # Panics
/// Panics if the fixture's auxiliary charges cannot be derived.
pub fn fund_sdk_defaults(limits: &mut ResolvedIsolateLimits, target: &TargetPlatformConstants) {
    let caps = ConfigExtractionLimits::default();
    limits.max_runtime_config_bytes = caps.max_backend_bytes;
    limits.max_runtime_config_blob_bytes = caps.max_blob_bytes;
    limits.max_config_json_nodes = caps.max_json_nodes;
    let baked = serialized_json_bytes(&(
        auctioneer_config_data::host_default(),
        auctioneer_config_data::account_default(),
    ))
    .unwrap();
    limits.aux_route_peak_bytes = limits.aux_route_peak_bytes.max(
        derive_aux_work_for_profile(limits, target, baked)
            .unwrap()
            .peak_bytes(),
    );
}

/// Install a real default-cap envelope for standalone resolver/handler fixtures.
///
/// # Panics
/// Panics if the fixture cannot boot or installed gates do not fund the SDK caps.
pub fn install() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let mut limits = envelope::limits();
        // Standalone handlers previously resolved the baked workload ceilings.
        // Fund those too, so installing config funding does not narrow fanout.
        let workload = &auctioneer_config_data::host_default().resource_limits;
        limits.egress_permits = 64;
        limits.max_bidders_per_auction = u32::try_from(workload.max_bidders_per_auction).unwrap();
        limits.max_outbound_requests = u32::try_from(workload.max_outbound_requests).unwrap();
        limits.final_response_bytes = limits.final_response_bytes.max(
            u64::from(limits.max_outbound_requests)
                .checked_mul(limits.max_decoded_bytes)
                .unwrap()
                .checked_mul(ASSEMBLY_FACTOR)
                .unwrap(),
        );
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        fund_sdk_defaults(&mut limits, &target);
        install_boot_envelope(
            &limits,
            &target,
            auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction),
            construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
    });
    let caps = ConfigExtractionLimits::default();
    let funded = &boot_gates().unwrap().limits;
    assert!(caps.max_backend_bytes <= funded.max_runtime_config_bytes);
    assert!(caps.max_blob_bytes <= funded.max_runtime_config_blob_bytes);
    assert!(caps.max_json_nodes <= funded.max_config_json_nodes);
}
