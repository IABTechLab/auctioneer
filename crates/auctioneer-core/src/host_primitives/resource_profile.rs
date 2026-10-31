//! Adapter-owned resource declarations shared by boot and offline inspection.

use super::BootEnvelope;
use auctioneer_runtime::{
    CatalogResourceCosts, HeadroomClass, RequestConstructionCosts, ResolvedIsolateLimits,
    TargetPlatformConstants,
};
use edgezero_core::{MemoryCeilingScope, PlatformMetadata};

/// One adapter's source declarations, not normalized limits or measured evidence.
///
/// The adapter owns platform selection and raw inputs. Core still normalizes
/// config limits at boot, and runtime remains the only accounting owner.
#[derive(Clone, Copy)]
pub struct ResourceProfile {
    /// Descriptive deployment and dispatch labels, not evidence of equivalence.
    pub name: &'static str,
    pub dispatch_name: &'static str,
    /// Intended executable ABI; exporting it is not an ABI allocation proof.
    pub target_abi: &'static str,
    /// Runtime-owned headroom policy for this target.
    pub headroom_class: HeadroomClass,
    /// Scope against which the declared populations are charged.
    pub memory_scope: MemoryCeilingScope,
    /// Selected deployment facts, preserving the SDK's unknown states.
    pub platform: PlatformMetadata,
    /// Facts of the actual dispatch path; configurable entrypoints pass these on.
    pub dispatch_platform: PlatformMetadata,
    /// Raw caps; core performs normalization once at its existing boundary.
    pub limits: ResolvedIsolateLimits,
    /// Provider constants, including parser and egress ownership declarations.
    pub target: TargetPlatformConstants,
    /// Construction descriptors from the owning bidder, exchange and core code.
    pub outbound_meta_bytes: u32,
    pub construction_costs: RequestConstructionCosts,
    /// Incremental catalog costs; never folded into baked configuration bytes.
    pub catalog_costs: CatalogResourceCosts,
    /// The boot logger must be available before the envelope warning.
    pub log_sink: &'static (dyn log::Log + 'static),
}

impl ResourceProfile {
    /// Borrow the exact raw declarations; this does not install or normalize.
    #[must_use]
    pub fn boot_envelope(&self) -> BootEnvelope<'_> {
        BootEnvelope {
            limits: &self.limits,
            target: &self.target,
            outbound_meta_bytes: self.outbound_meta_bytes,
            construction_costs: self.construction_costs,
            catalog_costs: self.catalog_costs,
            log_sink: self.log_sink,
        }
    }

    /// A matching label alone cannot make different platform facts equivalent.
    #[must_use]
    pub fn dispatch_matches_platform(&self) -> bool {
        self.platform == self.dispatch_platform
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_primitives::boot_gates;
    use std::ptr;

    #[test]
    fn boot_borrows_the_complete_owned_profile_without_normalizing_or_installing() {
        let mut profile = test_profile();
        profile.limits.max_runtime_config_bytes = 0;
        profile.limits.max_runtime_config_blob_bytes = 7919;
        profile.limits.max_config_json_nodes = 127;
        let boot = profile.boot_envelope();
        assert!(ptr::eq(boot.limits, &raw const profile.limits));
        assert!(ptr::eq(boot.target, &raw const profile.target));
        assert_eq!(boot.limits.max_runtime_config_bytes, 0);
        assert_eq!(boot.limits.max_runtime_config_blob_bytes, 7919);
        assert_eq!(boot.limits.max_config_json_nodes, 127);
        assert_eq!(boot.outbound_meta_bytes, 3571);
        assert_eq!(boot.construction_costs, profile.construction_costs);
        assert_eq!(boot.catalog_costs, profile.catalog_costs);
        assert!(ptr::eq(boot.log_sink, profile.log_sink));
        assert!(boot_gates().is_none());
    }

    #[test]
    fn dispatch_matching_compares_metadata_not_provider_names_or_target_identity() {
        let mut profile = test_profile();
        assert!(profile.dispatch_matches_platform());
        profile.dispatch_platform = edgezero_core::PlatformMetadata::new(
            edgezero_core::PlatformFact::unknown(
                edgezero_core::PlatformUnknownReason::ProviderUnpublished,
            ),
            edgezero_core::PlatformFact::unknown(edgezero_core::PlatformUnknownReason::Unspecified),
            edgezero_core::PlatformFact::unknown(edgezero_core::PlatformUnknownReason::Unspecified),
        );
        assert!(!profile.dispatch_matches_platform());
    }

    fn test_profile() -> ResourceProfile {
        let platform = edgezero_core::PlatformMetadata::new(
            edgezero_core::PlatformFact::unknown(edgezero_core::PlatformUnknownReason::Unspecified),
            edgezero_core::PlatformFact::unknown(edgezero_core::PlatformUnknownReason::Unspecified),
            edgezero_core::PlatformFact::unknown(edgezero_core::PlatformUnknownReason::Unspecified),
        );
        ResourceProfile {
            name: "test-platform",
            dispatch_name: "different-label-same-facts",
            target_abi: "native",
            headroom_class: HeadroomClass::Native,
            memory_scope: MemoryCeilingScope::PerInstance,
            platform,
            dispatch_platform: platform,
            outbound_meta_bytes: 3571,
            construction_costs: RequestConstructionCosts {
                fixed_per_auction_bytes: 17,
                reusable_scratch_bytes: 19,
                retained_per_scope_bytes: 23,
            },
            catalog_costs: CatalogResourceCosts {
                resident_bytes: 29,
                initialization_bytes: 31,
                schema_validation_per_auction_bytes: 37,
                context_per_auction_bytes: 41,
            },
            log_sink: &super::super::StderrBootLog,
            limits: ResolvedIsolateLimits {
                isolate_total_bytes: 4 << 30,
                baseline_bytes: 96_000_000,
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
                aux_route_peak_bytes: 768 * 1_024 * 1_024,
                max_brotli_wbits: 22,
                max_brotli_layers: 1,
                gvl_cache_entries: 4,
                gvl_parsed_bytes: auctioneer_runtime::MAX_GVL_PARSED_BYTES,
                max_bidders_per_auction: 2,
                max_outbound_requests: 2,
                max_concurrent_auctions: 2,
                max_active_body_streams: 2,
                max_runtime_config_bytes: 0x0010_0000,
                max_runtime_config_blob_bytes: 0x0010_0000,
                max_parsed_nodes: 0x0001_0000,
                max_copied_header_bytes: 4_096,
                max_admitted_header_bytes: 0x4000,
                max_admitted_header_count: 128,
                max_impressions_per_auction: 100,
                max_config_json_nodes: 8_192,
                terminal_response_permits: 68,
                refusal_body_bytes: 256,
                transmit_state_bytes: 4096,
            },
            target: TargetPlatformConstants {
                platform_concurrent_ceiling: 7,
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
        }
    }
}
