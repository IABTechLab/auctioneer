//! Offline, allowlisted description of this executable's funded resources.
//!
//! Schema version two always exports the complete catalog descriptor and its
//! four derived terms. Historical version-one output is not reproduced here.

use auctioneer_adapter_axum::build_features;
use auctioneer_bidder::CappedWriter;
use auctioneer_core::host_primitives::{BootEnvelope, BudgetInputs, resolve_budget_inputs};
use auctioneer_privacy::gvl::GVL_PARSE_CACHE_MAX_ENTRIES;
use auctioneer_runtime::{
    AuctionCharges, CatalogResourceCosts, Derived, RequestConstructionCosts, ResolvedIsolateLimits,
    TargetPlatformConstants, derive_isolate_budget_with_baked_config,
};
use edgezero_adapter_axum::{
    config_store_limits::ConfigStoreLimits, ingress_config::AxumIngressConfig,
};
use edgezero_core::env_config::EnvConfig;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use std::io::{self, Write as _};

const MAX_OUTPUT_BYTES: usize = 0x8000;
const EDGEZERO_PIN: &str = "775366fdac923135e8430b1e3dffebedc648ef7c";

// Exhaustive destructuring makes new runtime terms a compile error here. The
// original fields retain their owning types, including every integer width.
macro_rules! export {
    ($name:ident, $model:ident { $($field:ident),+ $(,)? }) => {
        struct $name($model);

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeStruct as _;
                let $model { $($field),+ } = &self.0;
                let count = [$(stringify!($field)),+].len();
                let mut state = serializer.serialize_struct(stringify!($model), count)?;
                $(state.serialize_field(stringify!($field), $field)?;)+
                state.end()
            }
        }
    };
}

export!(
    Limits,
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
    Target,
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
    Construction,
    RequestConstructionCosts {
        fixed_per_auction_bytes,
        reusable_scratch_bytes,
        retained_per_scope_bytes,
    }
);

export!(
    Catalog,
    CatalogResourceCosts {
        resident_bytes,
        initialization_bytes,
        schema_validation_per_auction_bytes,
        context_per_auction_bytes,
    }
);

export!(
    Charges,
    AuctionCharges {
        config,
        ingress,
        privacy,
        projection,
        outbound,
        response
    }
);

#[derive(Serialize)]
struct Terms {
    auction_charges: Charges,
    construction_costs: Construction,
    #[serde(flatten)]
    remaining: serde_json::Value,
}

fn terms(derived: &Derived) -> Terms {
    let Derived {
        isolate_terms: _isolate_terms,
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
    } = *derived;
    Terms {
        auction_charges: Charges(auction_charges),
        construction_costs: Construction(construction_costs),
        remaining: json!({
            "catalog_resident_term": catalog_resident_term,
            "catalog_initialization_term": catalog_initialization_term,
            "schema_validation_term": schema_validation_term,
            "catalog_context_term": catalog_context_term,
            "runtime_config_term": runtime_config_term, "request_side_term": request_side_term,
            "call_side_term": call_side_term, "bookkeeping_term": bookkeeping_term,
            "debug_term": debug_term, "assembly_scratch_bytes": assembly_scratch_bytes,
            "per_auction_peak": per_auction_peak, "per_stream_peak": per_stream_peak,
            "decoder_chain_bytes": decoder_chain_bytes, "gvl_terms": gvl_terms,
            "egress_term": egress_term, "aux_term": aux_term, "terminal_term": terminal_term,
            "probe_output_term": probe_output_term, "h2_slot_peak": h2_slot_peak,
            "h1_slot_peak": h1_slot_peak, "pre_admission_parser_term": pre_admission_parser_term,
            "total_bytes": total_bytes, "margin_bytes": margin_bytes,
            "max_outbound_raw_bytes_usize": max_outbound_raw_bytes_usize,
        }),
    }
}

#[derive(Serialize)]
struct Ingress {
    max_connections: u32,
    max_raw_head_bytes: u32,
    max_header_count: u32,
    head_read_timeout_ms: u64,
}

#[derive(Serialize)]
struct Store {
    #[serde(rename = "max_entries")]
    entries: u32,
    #[serde(rename = "max_file_bytes")]
    file: u64,
    #[serde(rename = "max_key_bytes")]
    key: u32,
    #[serde(rename = "max_value_bytes")]
    value: u64,
    #[serde(rename = "max_resident_bytes")]
    resident: u64,
    #[serde(rename = "max_startup_bytes")]
    startup: u64,
}

#[derive(Serialize)]
struct Sdk {
    ingress: Ingress,
    config_store: Store,
    http2_enabled: bool,
}

fn sdk(env: &EnvConfig) -> anyhow::Result<Sdk> {
    super::envelope::validate_sdk_settings(env)?;
    let ingress = AxumIngressConfig::from_env(env)?;
    let store = ConfigStoreLimits::from_env(env)?;
    Ok(Sdk {
        ingress: Ingress {
            max_connections: u32::try_from(ingress.max_connections())?,
            max_raw_head_bytes: u32::try_from(ingress.max_raw_head_bytes())?,
            max_header_count: u32::try_from(ingress.max_header_count())?,
            head_read_timeout_ms: u64::try_from(ingress.head_read_timeout().as_millis())?,
        },
        config_store: Store {
            entries: u32::try_from(store.max_entries())?,
            file: u64::try_from(store.max_file_bytes())?,
            key: u32::try_from(store.max_key_bytes())?,
            value: u64::try_from(store.max_value_bytes())?,
            resident: u64::try_from(store.max_resident_bytes())?,
            startup: u64::try_from(store.max_startup_bytes())?,
        },
        // This pinned SDK runner serves HTTP/1 only, independent of environment.
        http2_enabled: false,
    })
}

// Declaration order is the effective-profile digest contract, matching the
// CLI's ResourceExport DTO. Hash this struct directly, not a sorted JSON Value.
#[derive(Serialize)]
struct Resources {
    limits: Limits,
    target_constants: Target,
    construction_costs: Construction,
    catalog_costs: Catalog,
    outbound_meta_bytes: u32,
    baked_config_bytes: u64,
    actual_gvl_cache_entries: u32,
    sdk: Sdk,
}

#[derive(Serialize)]
struct Profile {
    schema_version: u16,
    target: &'static str,
    backend: &'static str,
    edgezero_pin: &'static str,
    build_features: Vec<&'static str>,
    baked_host_sha256: String,
    baked_account_sha256: String,
    baked_config_sha256: String,
    reference_data_sha256: String,
    effective_profile_sha256: String,
    resources: Resources,
    derived: Terms,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

fn reference_digest() -> anyhow::Result<String> {
    let archive: Vec<_> = auctioneer_config_data::gvl_archive()
        .iter()
        .map(|entry| {
            (
                entry.spec,
                entry.vendor_list_version,
                entry.tcf_policy_version,
                entry.sha256,
                entry.last_updated,
            )
        })
        .collect();
    digest(&(
        archive,
        auctioneer_config_data::gvl_archive_generated_at(),
        auctioneer_config_data::gvl_archive_max_age_days(),
        auctioneer_config_data::gcl_expires_at_unix_secs(),
        auctioneer_config_data::baked_gcl_json(),
        auctioneer_config_data::eea_set(),
        auctioneer_config_data::bidders(),
    ))
}

fn profile(boot: BootEnvelope<'_>, baked_config_bytes: u64, sdk: Sdk) -> anyhow::Result<Profile> {
    let baked = (
        auctioneer_config_data::host_default(),
        auctioneer_config_data::account_default(),
    );
    let derived = terms(&derive_isolate_budget_with_baked_config(
        boot.limits,
        boot.target,
        boot.outbound_meta_bytes,
        boot.construction_costs,
        baked_config_bytes,
        boot.catalog_costs,
    )?);
    let resources = Resources {
        limits: Limits(*boot.limits),
        target_constants: Target(*boot.target),
        construction_costs: Construction(boot.construction_costs),
        catalog_costs: Catalog(boot.catalog_costs),
        outbound_meta_bytes: boot.outbound_meta_bytes,
        baked_config_bytes,
        actual_gvl_cache_entries: u32::try_from(GVL_PARSE_CACHE_MAX_ENTRIES)?,
        sdk,
    };
    Ok(Profile {
        schema_version: 2,
        target: "axum",
        backend: "startup_snapshot",
        edgezero_pin: EDGEZERO_PIN,
        build_features: build_features(),
        baked_host_sha256: digest(baked.0)?,
        baked_account_sha256: digest(baked.1)?,
        baked_config_sha256: digest(&baked)?,
        reference_data_sha256: reference_digest()?,
        effective_profile_sha256: digest(&resources)?,
        resources,
        derived,
    })
}

pub fn write() -> anyhow::Result<()> {
    let mut resource_profile = super::envelope::resource_profile();
    let BudgetInputs {
        limits,
        baked_config_bytes,
    } = resolve_budget_inputs(&resource_profile.limits)?;
    resource_profile.limits = limits;
    let boot = resource_profile.boot_envelope();
    let profile = profile(boot, baked_config_bytes, sdk(&EnvConfig::from_env())?)?;
    let mut bytes = Vec::with_capacity(MAX_OUTPUT_BYTES);
    let mut writer = CappedWriter::new(&mut bytes, MAX_OUTPUT_BYTES)
        .map_err(|_error| anyhow::anyhow!("profile buffer unavailable"))?;
    writer
        .write_json(&profile)
        .and_then(|()| writer.write_all(b"\n"))
        .map_err(|_error| anyhow::anyhow!("profile serialization failed"))?;
    io::stdout().lock().write_all(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{digest, profile, sdk};
    use crate::envelope;
    use auctioneer_core::host_primitives::resolve_budget_inputs;
    use auctioneer_runtime::CatalogResourceCosts;
    use edgezero_core::env_config::EnvConfig;
    use serde_json::{Value, json};

    #[test]
    fn version_two_export_preserves_each_catalog_cost_and_derived_term() {
        let inputs = resolve_budget_inputs(&envelope::limits()).unwrap();
        let target = envelope::target();
        let mut boot = envelope::envelope(&inputs.limits, &target);
        let unchanged = CatalogResourceCosts::UNCHANGED;
        // Isolate each exported term from the shipped nonzero context charge.
        boot.catalog_costs = unchanged;
        let zero = profile(
            boot,
            inputs.baked_config_bytes,
            sdk(&EnvConfig::default()).unwrap(),
        )
        .unwrap();
        let zero_total = zero.derived.remaining["total_bytes"].as_u64().unwrap();
        let zero_digest = zero.effective_profile_sha256;
        for (costs, expected, increment) in [
            (
                CatalogResourceCosts {
                    resident_bytes: 101,
                    ..unchanged
                },
                [101_u64, 0, 0, 0],
                101,
            ),
            (
                CatalogResourceCosts {
                    initialization_bytes: 211,
                    ..unchanged
                },
                [0, 211, 0, 0],
                211,
            ),
            (
                CatalogResourceCosts {
                    schema_validation_per_auction_bytes: 307,
                    ..unchanged
                },
                [0, 0, 307, 0],
                307 * u64::from(inputs.limits.max_concurrent_auctions),
            ),
            (
                CatalogResourceCosts {
                    context_per_auction_bytes: 401,
                    ..unchanged
                },
                [0, 0, 0, 401],
                401 * u64::from(inputs.limits.max_concurrent_auctions),
            ),
        ] {
            let mut nonzero = boot;
            nonzero.catalog_costs = costs;
            let exported = profile(
                nonzero,
                inputs.baked_config_bytes,
                sdk(&EnvConfig::default()).unwrap(),
            )
            .unwrap();
            assert_ne!(exported.effective_profile_sha256, zero_digest);
            assert_eq!(
                exported.effective_profile_sha256,
                digest(&exported.resources).unwrap()
            );
            let value: Value =
                serde_json::from_slice(&serde_json::to_vec(&exported).unwrap()).unwrap();
            assert_eq!(value["schema_version"], 2_u16);
            assert_eq!(
                value["resources"]["catalog_costs"],
                json!({
                    "resident_bytes": expected[0],
                    "initialization_bytes": expected[1],
                    "schema_validation_per_auction_bytes": expected[2],
                    "context_per_auction_bytes": expected[3],
                })
            );
            for (field, bytes) in [
                "catalog_resident_term",
                "catalog_initialization_term",
                "schema_validation_term",
                "catalog_context_term",
            ]
            .into_iter()
            .zip(expected)
            {
                assert_eq!(value["derived"][field], bytes, "{field}");
            }
            assert_eq!(
                value["derived"]["total_bytes"],
                zero_total.checked_add(increment).unwrap(),
            );
        }
    }
}
