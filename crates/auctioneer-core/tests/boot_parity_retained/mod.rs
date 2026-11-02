//! Existing boot probe's exhaustive retained-state exports, independent of CLI DTOs.

use auctioneer_runtime::{
    AuctionCharges, CatalogResourceCosts, Derived, IsolateTerms, RequestConstructionCosts,
    ResolvedIsolateLimits, TargetPlatformConstants,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

macro_rules! export {
    ($function:ident, $model:ident { $($field:ident),+ $(,)? }) => {
        pub fn $function(value: &$model) -> Value {
            let $model { $($field),+ } = value;
            let mut fields = serde_json::Map::new();
            $(fields.insert(stringify!($field).to_owned(), json!($field));)+
            Value::Object(fields)
        }
    };
}

export!(
    limits,
    ResolvedIsolateLimits {
        isolate_total_bytes,
        baseline_bytes,
        inbound_request_bytes,
        inbound_parse_factor,
        max_outbound_request_bytes,
        max_outbound_raw_bytes,
        max_transport_bytes,
        max_decoded_bytes,
        response_parse_factor,
        response_node_peak_bytes,
        final_response_bytes,
        egress_permits,
        response_write_deadline_ms,
        guest_decoder_bound_bytes,
        aux_route_permits,
        aux_route_peak_bytes,
        max_brotli_wbits,
        max_brotli_layers,
        gvl_cache_entries,
        gvl_parsed_bytes,
        max_bidders_per_auction,
        max_outbound_requests,
        max_concurrent_auctions,
        max_active_body_streams,
        max_runtime_config_bytes,
        max_runtime_config_blob_bytes,
        max_parsed_nodes,
        max_copied_header_bytes,
        max_admitted_header_bytes,
        max_admitted_header_count,
        max_impressions_per_auction,
        max_config_json_nodes,
        terminal_response_permits,
        refusal_body_bytes,
        transmit_state_bytes,
    }
);
export!(
    target,
    TargetPlatformConstants {
        platform_concurrent_ceiling,
        header_entry_overhead_bytes,
        parser_header_ceiling_bytes,
        parser_header_knob_bytes,
        max_request_target_bytes,
        h1_max_buf_size_bytes,
        h1_max_headers,
        parser_max_header_entries,
        pseudoheader_allowance_bytes,
        parser_transient_bytes,
        egress_handoff_factor,
    }
);
export!(
    construction,
    RequestConstructionCosts {
        fixed_per_auction_bytes,
        reusable_scratch_bytes,
        retained_per_scope_bytes,
    }
);
export!(
    catalog,
    CatalogResourceCosts {
        resident_bytes,
        initialization_bytes,
        schema_validation_per_auction_bytes,
        context_per_auction_bytes,
    }
);
export!(
    receipts,
    AuctionCharges {
        config,
        ingress,
        privacy,
        projection,
        outbound,
        response
    }
);

pub fn terms(value: &IsolateTerms) -> Value {
    json!(
        value
            .entries()
            .into_iter()
            .map(|(term, bytes)| (term.name(), bytes))
            .collect::<BTreeMap<_, _>>()
    )
}

pub fn derived(value: &Derived) -> Value {
    let Derived {
        isolate_terms,
        auction_charges,
        construction_costs,
        catalog_resident_term,
        catalog_initialization_term,
        schema_validation_term,
        catalog_context_term,
        runtime_config_term,
        request_side_term,
        call_side_term,
        bookkeeping_term,
        debug_term,
        assembly_scratch_bytes,
        per_auction_peak,
        per_stream_peak,
        decoder_chain_bytes,
        gvl_terms,
        egress_term,
        aux_term,
        terminal_term,
        probe_output_term,
        h2_slot_peak,
        h1_slot_peak,
        pre_admission_parser_term,
        total_bytes,
        margin_bytes,
        max_outbound_raw_bytes_usize,
    } = value;
    json!({
        "isolate_terms": terms(isolate_terms), "auction_charges": receipts(auction_charges),
        "construction_costs": construction(construction_costs),
        "catalog_resident_term": catalog_resident_term, "catalog_initialization_term": catalog_initialization_term,
        "schema_validation_term": schema_validation_term, "catalog_context_term": catalog_context_term,
        "runtime_config_term": runtime_config_term, "request_side_term": request_side_term,
        "call_side_term": call_side_term, "bookkeeping_term": bookkeeping_term, "debug_term": debug_term,
        "assembly_scratch_bytes": assembly_scratch_bytes, "per_auction_peak": per_auction_peak,
        "per_stream_peak": per_stream_peak, "decoder_chain_bytes": decoder_chain_bytes, "gvl_terms": gvl_terms,
        "egress_term": egress_term, "aux_term": aux_term, "terminal_term": terminal_term,
        "probe_output_term": probe_output_term, "h2_slot_peak": h2_slot_peak, "h1_slot_peak": h1_slot_peak,
        "pre_admission_parser_term": pre_admission_parser_term, "total_bytes": total_bytes,
        "margin_bytes": margin_bytes, "max_outbound_raw_bytes_usize": max_outbound_raw_bytes_usize,
    })
}
