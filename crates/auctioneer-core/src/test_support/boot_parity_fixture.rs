//! Shared raw rows for the existing boot probe and the isolated CLI adapter.

use crate::auction::audit::construction_costs;
use crate::host_primitives::{ResourceProfile, StderrBootLog};
use auctioneer_runtime::{
    CatalogResourceCosts, HeadroomClass, ResolvedIsolateLimits, TargetPlatformConstants,
};
use edgezero_core::{
    HostIngressMemoryAccounting, InboundRequestPopulationBound, MemoryCeiling, MemoryCeilingScope,
    PlatformFact, PlatformMetadata, PlatformResourceSource,
};
use std::env::{VarError, var};
use std::num::NonZeroU32;

pub const CASE_ENV: &str = "AUCTIONEER__TEST__BOOT_STRICT_PARITY_CASE";
pub const ADAPTER_ENV: &str = "AUCTIONEER__TEST__BOOT_STRICT_PARITY_ADAPTER";
pub const ADAPTERS: [(&str, u64); 4] = [
    ("axum", 96_000_000),
    ("cloudflare", 96_010_007),
    ("fastly", 96_020_011),
    ("spin", 96_030_013),
];
pub const CASES: [(&str, u8, bool); 10] = [
    ("unequal_explicit_caps", 0, false),
    ("default_backend_cap", 1, false),
    ("default_blob_cap", 2, false),
    ("default_backend_and_blob_caps", 3, false),
    ("default_node_cap", 4, false),
    ("default_backend_and_node_caps", 5, false),
    ("default_blob_and_node_caps", 6, false),
    ("all_default_caps", 7, false),
    ("unequal_explicit_caps_warn_and_install", 0, true),
    ("all_default_caps_warn_and_install", 7, true),
];

pub const CATALOG: CatalogResourceCosts = CatalogResourceCosts {
    resident_bytes: 1009,
    initialization_bytes: 2017,
    schema_validation_per_auction_bytes: 3011,
    context_per_auction_bytes: 4001,
};

/// Resolve a fixed boot row using the adapter selected by the test environment.
///
/// # Panics
/// Panics for an invalid adapter environment value or an unknown case/adapter.
#[expect(
    clippy::panic,
    reason = "invalid fixture environment must fail the probe"
)]
#[must_use]
pub fn profile_for(case: &str) -> ResourceProfile {
    let adapter = match var(ADAPTER_ENV) {
        Ok(adapter) => adapter,
        Err(VarError::NotPresent) => "axum".to_owned(),
        Err(VarError::NotUnicode(_)) => panic!("invalid test adapter"),
    };
    profile_for_adapter(case, &adapter)
}

/// Resolve one of the fixed boot rows for a named adapter.
///
/// # Panics
/// Panics if the case or adapter is not in the captured input matrix.
#[must_use]
pub fn profile_for_adapter(case: &str, adapter: &str) -> ResourceProfile {
    let (_, mask, over) = CASES
        .into_iter()
        .find(|(name, _, _)| *name == case)
        .expect("known boot parity case");
    let (_, baseline_bytes) = ADAPTERS
        .into_iter()
        .find(|(name, _)| *name == adapter)
        .expect("known boot parity adapter");
    let source = PlatformResourceSource::RuntimeConfiguration {
        runtime: "test-only-boot-parity",
    };
    let platform = PlatformMetadata::new(
        PlatformFact::known(
            MemoryCeiling::new(512 << 30_u32, MemoryCeilingScope::PerInstance, None),
            source,
        ),
        PlatformFact::known(
            InboundRequestPopulationBound::new(NonZeroU32::new(384).unwrap_or(NonZeroU32::MIN)),
            source,
        ),
        PlatformFact::known(HostIngressMemoryAccounting::CountsTowardCeiling, source),
    );
    ResourceProfile {
        name: "test-only-boot-parity",
        dispatch_name: "test-only-boot-parity",
        target_abi: "native",
        headroom_class: HeadroomClass::Native,
        memory_scope: MemoryCeilingScope::PerInstance,
        platform,
        dispatch_platform: platform,
        limits: ResolvedIsolateLimits {
            isolate_total_bytes: if over { 1 } else { 512 << 30_u32 },
            baseline_bytes,
            inbound_request_bytes: 0x0004_0000,
            inbound_parse_factor: auctioneer_runtime::MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: 0x0004_0000,
            max_outbound_raw_bytes: 0x0004_0000,
            max_transport_bytes: 0x0004_0000,
            max_decoded_bytes: 0x0004_0000,
            response_parse_factor: auctioneer_runtime::MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: auctioneer_runtime::MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 0x0100_0000,
            egress_permits: 3,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 5,
            aux_route_peak_bytes: 7 << 30_u32,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: auctioneer_runtime::MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 2,
            max_outbound_requests: 2,
            max_concurrent_auctions: if mask & 1 == 0 { 2 } else { 5 },
            max_active_body_streams: 7,
            max_runtime_config_bytes: if mask & 1 == 0 { 7919 } else { 0 },
            max_runtime_config_blob_bytes: if mask & 2 == 0 { 3571 } else { 0 },
            max_parsed_nodes: 0x0001_0000,
            max_copied_header_bytes: 4096,
            max_admitted_header_bytes: 0x4000,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            max_config_json_nodes: if mask & 4 == 0 { 127 } else { 0 },
            terminal_response_permits: 68,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4096,
        },
        target: TargetPlatformConstants {
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
        },
        outbound_meta_bytes: 5_248,
        construction_costs: construction_costs(),
        catalog_costs: CATALOG,
        log_sink: &StderrBootLog,
    }
}
