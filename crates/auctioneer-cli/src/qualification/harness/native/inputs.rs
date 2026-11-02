use super::super::{HarnessError, Result, fixture};
use super::geometry::{self, AuctionOptions};
use crate::qualification::profile::{
    self, Backend, ByteBounds, DebugLimit, Digest, Effects, Family, Feature, Hardware, Identity,
    Load, OutcomeCriteria, Platform, Profile, ResourceExport, Revision, SdkSettings, Subcase,
    Target,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use std::collections::BTreeMap;
use std::env::var_os;
use std::ffi::OsString;

pub const FIXTURE_PROTOCOL_VERSION: u16 = 1;
const INPUT_CAP: usize = 1 << 20_u32;
const EXPORT_CAP: usize = 32 << 10_u32;
const MAX_SAFE_INTEGER: u64 = 0x001f_ffff_ffff_ffff;

#[derive(Clone, Copy, Serialize)]
struct DebugGeometry {
    max_banner_formats: u8,
}

const DEBUG_GEOMETRY: DebugGeometry = DebugGeometry {
    max_banner_formats: 8,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    NativeAxumCandidate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionOracle {
    ParkedTwoAuctionsV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedLimits {
    pub isolate_total_bytes: u64,
    pub max_outbound_requests: u32,
    pub max_concurrent_auctions: u32,
    pub inbound_request_bytes: u64,
    pub max_parsed_nodes: u32,
    pub max_impressions_per_auction: u16,
    pub max_transport_bytes: u64,
    pub max_decoded_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebugBounds {
    pub limiting_factor: DebugLimit,
    pub capture_bytes: ByteBounds,
    pub output_bytes: ByteBounds,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub schema_version: u16,
    pub kind: Kind,
    pub platform: Platform,
    pub headroom_bytes: u64,
    pub expected_limits: ExpectedLimits,
    pub sdk: SdkSettings,
    pub duration_ms: u64,
    pub warmup_ms: u64,
    pub repetitions: u8,
    pub cold_window_ms: u64,
    pub warm_window_ms: u64,
    pub max_requests: u32,
    pub request_timeout_ms: u64,
    pub latency_threshold_us: u64,
    pub max_peak_bytes: u64,
    pub max_retained_growth_bytes: u64,
    pub debug_reference: DebugBounds,
    pub admission_oracle: AdmissionOracle,
    pub base_load: Load,
    pub maximum_load: Load,
    pub seed: u32,
    pub max_run_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Base,
    Maximum,
    Decode,
    Debug,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: u16,
    pub family: Family,
    pub subcase: Subcase,
    pub variant_id: u16,
    pub shape: Shape,
    pub mode: fixture::Mode,
    pub effects: Effects,
    pub outcomes: OutcomeCriteria,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u16,
    pub cases: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Derived {
    pub total_bytes: u64,
    pub catalog_resident_term: u64,
    pub catalog_initialization_term: u64,
    pub schema_validation_term: u64,
    pub catalog_context_term: u64,
}

impl From<auctioneer_runtime::Derived> for Derived {
    fn from(value: auctioneer_runtime::Derived) -> Self {
        Self {
            total_bytes: value.total_bytes,
            catalog_resident_term: value.catalog_resident_term,
            catalog_initialization_term: value.catalog_initialization_term,
            schema_validation_term: value.schema_validation_term,
            catalog_context_term: value.catalog_context_term,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub schema_version: u16,
    pub target: Target,
    pub backend: Backend,
    pub edgezero_pin: Revision,
    pub build_features: Vec<Feature>,
    pub baked_host_sha256: Digest,
    pub baked_account_sha256: Digest,
    pub baked_config_sha256: Digest,
    pub reference_data_sha256: Digest,
    pub effective_profile_sha256: Digest,
    pub resources: ResourceExport,
    pub derived: Derived,
}

pub fn parse_candidate(bytes: &[u8]) -> Result<Candidate> {
    parse_json(bytes, INPUT_CAP)
}

pub fn parse_catalog(bytes: &[u8]) -> Result<Catalog> {
    parse_json(bytes, INPUT_CAP)
}

pub(super) fn parse_json<T: DeserializeOwned>(bytes: &[u8], cap: usize) -> Result<T> {
    if bytes.len() > cap {
        return Err(HarnessError::new("input_limit"));
    }
    auctioneer_runtime::count_json_nodes(bytes, 1 << 16_u32)
        .map_err(|_error| HarnessError::new("invalid_input"))?;
    // Ignored serde members still consume the same bounded depth and nodes.
    let _depth_checked: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_error| HarnessError::new("invalid_input"))?;
    serde_json::from_slice(bytes).map_err(|_error| HarnessError::new("invalid_input"))
}

pub fn validate_candidate(candidate: &Candidate, catalog: &Catalog) -> Result<()> {
    validate_candidate_values(candidate)?;
    if catalog.schema_version != 1 || catalog.cases.len() != profile::REQUIRED_SUBCASES.len() {
        return Err(HarnessError::new("invalid_input"));
    }
    for (index, (row, required)) in catalog
        .cases
        .iter()
        .zip(profile::REQUIRED_SUBCASES)
        .enumerate()
    {
        let id = u16::try_from(index)
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(HarnessError::new("invalid_input"))?;
        if row.id != id
            || row.subcase != *required
            || row.family != required.family()
            || row.variant_id != variant_id(*required)
            || !matches!(
                row.mode,
                fixture::Mode::Gzip | fixture::Mode::Mixed | fixture::Mode::Stall
            )
            || effect_values(row.effects)
                .into_iter()
                .any(|value| value > MAX_SAFE_INTEGER)
            || outcome_values(&row.outcomes)
                .into_iter()
                .any(|value| value > 10_000)
        {
            return Err(HarnessError::new("invalid_input"));
        }
    }
    Ok(())
}

fn validate_candidate_values(candidate: &Candidate) -> Result<()> {
    let bounds = &candidate.debug_reference;
    if candidate.schema_version != 1
        || candidate.repetitions != 2
        || !(1000..=60_000).contains(&candidate.duration_ms)
        || [
            candidate.warmup_ms,
            candidate.cold_window_ms,
            candidate.warm_window_ms,
        ]
        .into_iter()
        .any(|value| !(1..=60_000).contains(&value))
        || !(1..=10_000).contains(&candidate.max_requests)
        || !(1..=10_000).contains(&candidate.request_timeout_ms)
        || !(1000..=900_000).contains(&candidate.max_run_ms)
        || [
            candidate.headroom_bytes,
            candidate.latency_threshold_us,
            candidate.max_peak_bytes,
            candidate.max_retained_growth_bytes,
            candidate.platform.memory_bytes,
            u64::from(candidate.platform.concurrent_requests),
            candidate.platform.wall_time_ms,
            candidate.platform.cpu_time_ms,
        ]
        .into_iter()
        .any(|value| !(1..=MAX_SAFE_INTEGER).contains(&value))
        || candidate
            .max_peak_bytes
            .checked_add(candidate.headroom_bytes)
            .is_none_or(|bytes| bytes > candidate.platform.memory_bytes)
        || bounds.limiting_factor != DebugLimit::FixtureShape
        || [bounds.capture_bytes, bounds.output_bytes]
            .into_iter()
            .any(|interval| {
                interval.min == 0 || interval.min > interval.max || interval.max > 0x0001_0000
            })
        || candidate
            .sdk
            .ingress
            .as_ref()
            .is_none_or(|sdk| sdk.max_connections != 256)
    {
        return Err(HarnessError::new("invalid_input"));
    }
    validate_expected_limits(&candidate.expected_limits)?;
    for load in [candidate.base_load, candidate.maximum_load] {
        if !(1..=16).contains(&load.concurrency) {
            return Err(HarnessError::new("invalid_input"));
        }
        geometry::make_auction(load, AuctionOptions::default())?;
        geometry::make_response(load.encoded_bytes, load.decoded_bytes, true)?;
    }
    geometry::make_auction(
        Load {
            impressions: candidate.base_load.impressions,
            ..candidate.maximum_load
        },
        AuctionOptions {
            fpd: true,
            ..AuctionOptions::default()
        },
    )?;
    service_environment(9999, &candidate.sdk)?;
    Ok(())
}

fn validate_expected_limits(limits: &ExpectedLimits) -> Result<()> {
    let values = [
        limits.isolate_total_bytes,
        u64::from(limits.max_outbound_requests),
        u64::from(limits.max_concurrent_auctions),
        limits.inbound_request_bytes,
        u64::from(limits.max_parsed_nodes),
        u64::from(limits.max_impressions_per_auction),
        limits.max_transport_bytes,
        limits.max_decoded_bytes,
    ];
    if values
        .into_iter()
        .any(|value| !(1..=MAX_SAFE_INTEGER).contains(&value))
    {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(())
}

fn effect_values(effects: Effects) -> [u64; 13] {
    [
        effects.gvl_parses,
        effects.fpd_expansions,
        effects.decoded_responses,
        effects.slow_reads,
        effects.debug_captures,
        effects.debug_denials,
        effects.debug_output_bytes,
        effects.auxiliary_requests,
        effects.config_extractions,
        effects.queued_calls,
        effects.cancellations,
        effects.stalled_reads,
        effects.recovery_successes,
    ]
}

fn outcome_values(outcomes: &OutcomeCriteria) -> [u64; 9] {
    [
        outcomes.min_offered,
        outcomes.min_admitted,
        outcomes.min_success,
        outcomes.min_rejected,
        outcomes.min_errors,
        outcomes.min_timeouts,
        outcomes.max_rejected,
        outcomes.max_errors,
        outcomes.max_timeouts,
    ]
}

const fn variant_id(subcase: Subcase) -> u16 {
    match subcase {
        Subcase::ConfigVariant => 2,
        Subcase::ConfigRestoration => 3,
        Subcase::ColdStartup
        | Subcase::WarmBaseline
        | Subcase::GvlColdParse
        | Subcase::MaximumShape
        | Subcase::FpdExpansion
        | Subcase::EncodedDecoded
        | Subcase::SlowReader
        | Subcase::MixedOutcomes
        | Subcase::Unrequested
        | Subcase::Denied
        | Subcase::Permitted
        | Subcase::ConcurrentCapture
        | Subcase::CaptureRelease
        | Subcase::AuxiliaryOverlap
        | Subcase::ConfigSnapshot
        | Subcase::GvlMissOverlap
        | Subcase::ExcessAdmission
        | Subcase::OutboundQueue
        | Subcase::StalledIo
        | Subcase::Cancellation
        | Subcase::Timeout
        | Subcase::HealthyRecovery => 1,
    }
}

pub fn parse_export(bytes: &[u8]) -> Result<Export> {
    let export = parse_json(bytes, EXPORT_CAP)
        .map_err(|_error| HarnessError::new("invalid_child_output"))?;
    validate_export(&export)?;
    Ok(export)
}

fn validate_export(export: &Export) -> Result<()> {
    if export.schema_version != profile::RESOURCE_SCHEMA_VERSION
        || export.target != Target::Axum
        || export.backend != Backend::StartupSnapshot
        || export.derived.total_bytes == 0
        || export.derived.total_bytes > MAX_SAFE_INTEGER
        || export.build_features.len() > 3
        || export
            .build_features
            .iter()
            .enumerate()
            .any(|(index, feature)| {
                export
                    .build_features
                    .iter()
                    .skip(index.saturating_add(1))
                    .any(|next| next == feature)
            })
    {
        return Err(HarnessError::new("invalid_child_output"));
    }
    let derived = export
        .resources
        .derive()
        .map_err(|_error| HarnessError::new("invalid_child_output"))?;
    let digest = export
        .resources
        .digest()
        .map_err(|_error| HarnessError::new("invalid_child_output"))?;
    if export.derived != Derived::from(derived) || export.effective_profile_sha256 != digest {
        return Err(HarnessError::new("invalid_child_output"));
    }
    Ok(())
}

pub fn service_environment(port: u16, sdk: &SdkSettings) -> Result<BTreeMap<OsString, OsString>> {
    if port < 1024 || sdk.http2_enabled {
        return Err(HarnessError::new("invalid_input"));
    }
    if port == 8787 {
        return Err(HarnessError::new("port_collision"));
    }
    let ingress = sdk
        .ingress
        .as_ref()
        .ok_or(HarnessError::new("invalid_input"))?;
    let store = sdk
        .config_store
        .as_ref()
        .ok_or(HarnessError::new("invalid_input"))?;
    let settings = [
        (
            "EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS",
            u64::from(ingress.max_connections),
        ),
        (
            "EDGEZERO__ADAPTER__INGRESS__MAX_RAW_HEAD_BYTES",
            u64::from(ingress.max_raw_head_bytes),
        ),
        (
            "EDGEZERO__ADAPTER__INGRESS__MAX_HEADER_COUNT",
            u64::from(ingress.max_header_count),
        ),
        (
            "EDGEZERO__ADAPTER__INGRESS__HEAD_READ_TIMEOUT_MS",
            ingress.head_read_timeout_ms,
        ),
        (
            "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_ENTRIES",
            u64::from(store.max_entries),
        ),
        (
            "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_FILE_BYTES",
            store.max_file_bytes,
        ),
        (
            "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_KEY_BYTES",
            u64::from(store.max_key_bytes),
        ),
        (
            "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_VALUE_BYTES",
            store.max_value_bytes,
        ),
        (
            "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_RESIDENT_BYTES",
            store.max_resident_bytes,
        ),
        (
            "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_STARTUP_BYTES",
            store.max_startup_bytes,
        ),
    ];
    let mut environment = BTreeMap::from([
        (
            OsString::from("EDGEZERO__ADAPTER__HOST"),
            OsString::from("127.0.0.1"),
        ),
        (
            OsString::from("EDGEZERO__ADAPTER__PORT"),
            OsString::from(port.to_string()),
        ),
        (
            OsString::from("EDGEZERO__LOGGING__LEVEL"),
            OsString::from("off"),
        ),
        (OsString::from("RUST_LOG"), OsString::from("off")),
    ]);
    for (name, value) in settings {
        if !(1..=MAX_SAFE_INTEGER).contains(&value) {
            return Err(HarnessError::new("invalid_input"));
        }
        environment.insert(OsString::from(name), OsString::from(value.to_string()));
    }
    Ok(environment)
}

pub fn tool_environment() -> BTreeMap<OsString, OsString> {
    let mut environment = [
        "PATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "CARGO_TARGET_DIR",
        "RUSTUP_TOOLCHAIN",
        "TMPDIR",
    ]
    .into_iter()
    .filter_map(|name| {
        var_os(name)
            .filter(|value| !value.is_empty())
            .map(|value| (OsString::from(name), value))
    })
    .collect::<BTreeMap<_, _>>();
    environment.insert(OsString::from("LC_ALL"), OsString::from("C"));
    environment
}

pub fn bind_profile(
    candidate: &Candidate,
    catalog: &Catalog,
    export: &Export,
    mut identity: Identity,
    hardware: Hardware,
    variants: &[profile::Variant],
) -> Result<Profile> {
    validate_candidate(candidate, catalog)?;
    validate_export(export)?;
    if !matches_limits(&candidate.expected_limits, &export.resources)
        || export.resources.sdk != candidate.sdk
        || export.resources.actual_gvl_cache_entries != 4
        || !export.build_features.contains(&Feature::InsecureBidders)
        || variants.len() != 3
    {
        return Err(HarnessError::new("candidate_mismatch"));
    }
    let bound_variants = bind_variants(candidate, variants)?;
    let scenarios = catalog
        .cases
        .iter()
        .map(|row| bind_scenario(candidate, row, &export.resources))
        .collect::<Result<Vec<_>>>()?;
    if scenarios
        .iter()
        .map(|scenario| u32::from(scenario.repetitions))
        .sum::<u32>()
        != 45
    {
        return Err(HarnessError::new("candidate_mismatch"));
    }
    identity.edgezero_pin = export.edgezero_pin.clone();
    identity.build_features.clone_from(&export.build_features);
    identity.target = export.target;
    identity.reference_data_sha256 = export.reference_data_sha256.clone();
    identity.baked_config_sha256 = export.baked_config_sha256.clone();
    let profile = Profile {
        schema_version: profile::RESOURCE_SCHEMA_VERSION,
        scope: profile::Scope::Local,
        identity,
        resources: export.resources.clone(),
        platform: candidate.platform.clone(),
        hardware,
        headroom_bytes: candidate.headroom_bytes,
        backend: Backend::StartupSnapshot,
        variants: bound_variants,
        required_capabilities: profile::REQUIRED_CAPABILITIES.to_vec(),
        scenarios,
    };
    profile
        .validate()
        .map_err(|_error| HarnessError::new("candidate_mismatch"))?;
    Ok(profile)
}

fn matches_limits(expected: &ExpectedLimits, resources: &ResourceExport) -> bool {
    let actual = resources.limits;
    expected.isolate_total_bytes == actual.isolate_total_bytes
        && expected.max_outbound_requests == actual.max_outbound_requests
        && expected.max_concurrent_auctions == actual.max_concurrent_auctions
        && expected.inbound_request_bytes == actual.inbound_request_bytes
        && expected.max_parsed_nodes == actual.max_parsed_nodes
        && expected.max_impressions_per_auction == actual.max_impressions_per_auction
        && expected.max_transport_bytes == actual.max_transport_bytes
        && expected.max_decoded_bytes == actual.max_decoded_bytes
}

fn bind_variants(
    candidate: &Candidate,
    variants: &[profile::Variant],
) -> Result<Vec<profile::Variant>> {
    (1..=3_u16)
        .map(|id| {
            let mut matching = variants.iter().filter(|variant| variant.id == id);
            let variant = matching
                .next()
                .ok_or(HarnessError::new("candidate_mismatch"))?;
            if matching.next().is_some() {
                return Err(HarnessError::new("candidate_mismatch"));
            }
            Ok(profile::Variant {
                id,
                config_sha256: variant.config_sha256.clone(),
                settings_sha256: variant.settings_sha256.clone(),
                cold_window_ms: candidate.cold_window_ms,
                warm_window_ms: candidate.warm_window_ms,
                max_cold_peak_bytes: candidate.max_peak_bytes,
                max_warm_peak_bytes: candidate.max_peak_bytes,
            })
        })
        .collect()
}

const fn debug_case(subcase: Subcase) -> bool {
    matches!(
        subcase,
        Subcase::Permitted | Subcase::ConcurrentCapture | Subcase::CaptureRelease
    )
}

fn scenario_load(candidate: &Candidate, row: &Row) -> Load {
    let mut load = if matches!(row.shape, Shape::Maximum | Shape::Debug) || debug_case(row.subcase)
    {
        candidate.maximum_load
    } else {
        candidate.base_load
    };
    if row.subcase == Subcase::FpdExpansion {
        load.impressions = candidate.base_load.impressions;
    }
    if matches!(row.shape, Shape::Decode | Shape::Debug) {
        load.encoded_bytes = candidate.maximum_load.encoded_bytes;
        load.decoded_bytes = candidate.maximum_load.decoded_bytes;
    }
    load
}

#[derive(Serialize)]
struct FixtureDeclaration<'a> {
    row: &'a Row,
    load: Load,
    seed: u32,
    debug_reference: Option<&'a DebugBounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug_geometry: Option<DebugGeometry>,
    fixture_protocol_version: u16,
}

fn bind_scenario(
    candidate: &Candidate,
    row: &Row,
    resources: &ResourceExport,
) -> Result<profile::Scenario> {
    let debug = debug_case(row.subcase);
    let load = scenario_load(candidate, row);
    let declaration = FixtureDeclaration {
        row,
        load,
        seed: candidate.seed,
        debug_reference: debug.then_some(&candidate.debug_reference),
        debug_geometry: debug.then_some(DEBUG_GEOMETRY),
        fixture_protocol_version: FIXTURE_PROTOCOL_VERSION,
    };
    let fixture = profile::Fixture {
        id: row.id,
        sha256: Digest::canonical(&declaration)
            .map_err(|_error| HarnessError::new("invalid_input"))?,
    };
    let mut effects = row.effects;
    let debug_workload = if debug {
        effects.debug_captures = u64::from(load.concurrency)
            .checked_mul(u64::from(load.calls))
            .ok_or(HarnessError::new("invalid_input"))?;
        effects.debug_output_bytes = u64::from(load.concurrency)
            .checked_mul(candidate.debug_reference.output_bytes.min)
            .ok_or(HarnessError::new("invalid_input"))?;
        Some(debug_workload(candidate, &fixture, resources)?)
    } else {
        None
    };
    Ok(profile::Scenario {
        family: row.family,
        subcase: row.subcase,
        variant_id: row.variant_id,
        fixture,
        seed: u64::from(candidate.seed),
        repetitions: if matches!(
            row.subcase,
            Subcase::ColdStartup | Subcase::ConfigVariant | Subcase::ConfigRestoration
        ) {
            1
        } else {
            candidate.repetitions
        },
        warmup_ms: candidate.warmup_ms,
        duration_ms: candidate.duration_ms,
        meter_window_ms: candidate.duration_ms,
        load,
        effects,
        outcomes: row.outcomes.clone(),
        latency: profile::LatencyCriteria {
            start: profile::LatencyStart::ClientDispatch,
            end: profile::LatencyEnd::TerminalOutcome,
            population: profile::Population::AllOffered,
            percentile: 99,
            threshold_us: candidate.latency_threshold_us,
            max_histogram_buckets: 4,
        },
        max_peak_bytes: candidate.max_peak_bytes,
        max_retained_growth_bytes: candidate.max_retained_growth_bytes,
        maximum_mode: profile::MaximumMode::Simultaneous,
        justification_sha256: None,
        debug_workload,
    })
}

fn debug_workload(
    candidate: &Candidate,
    fixture: &profile::Fixture,
    resources: &ResourceExport,
) -> Result<profile::DebugWorkload> {
    let reference = profile::DebugReference {
        fixture: fixture.clone(),
        resources_sha256: Digest::canonical(resources)
            .map_err(|_error| HarnessError::new("invalid_input"))?,
        review_sha256: Digest::canonical(
            &json!({"bounds": candidate.debug_reference, "geometry": DEBUG_GEOMETRY}),
        )
        .map_err(|_error| HarnessError::new("invalid_input"))?,
        limiting_factor: candidate.debug_reference.limiting_factor,
        limits: profile::DebugLimits::effective(resources),
        min_call_rows: candidate.maximum_load.calls,
        max_call_rows: candidate.maximum_load.calls,
        capture_bytes: candidate.debug_reference.capture_bytes,
        output_bytes: candidate.debug_reference.output_bytes,
    };
    Ok(profile::DebugWorkload {
        reference_sha256: Digest::canonical(&reference)
            .map_err(|_error| HarnessError::new("invalid_input"))?,
        reference,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::ffi::OsStr;
    use std::str::from_utf8;

    const CANDIDATE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/testing/fixtures/resource-qualification/native-axum.json"
    ));
    const CATALOG: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/testing/fixtures/resource-qualification/catalog.json"
    ));
    const CAPTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/testing/fixtures/resource-qualification/native-axum-local-82075f1.json"
    ));
    const REPORT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/testing/evidence/native-axum-82075f1-local-pass/report.json"
    ));

    fn candidate() -> Candidate {
        serde_json::from_slice(CANDIDATE).unwrap()
    }

    fn catalog() -> Catalog {
        serde_json::from_slice(CATALOG).unwrap()
    }

    fn synthetic_profile_fixture() -> Profile {
        profile::tests::synthetic_profile(CAPTURE)
    }

    fn export_value() -> Value {
        let recorded = synthetic_profile_fixture();
        let report: Value = serde_json::from_slice(REPORT).unwrap();
        // Historical evidence retains the resources, not the full export wrapper.
        json!({
            "schema_version": 2_u16,
            "target": recorded.identity.target,
            "backend": recorded.backend,
            "edgezero_pin": recorded.identity.edgezero_pin,
            "build_features": recorded.identity.build_features,
            "baked_host_sha256": Digest::of(b"export-wrapper-host"),
            "baked_account_sha256": Digest::of(b"export-wrapper-account"),
            "baked_config_sha256": recorded.identity.baked_config_sha256,
            "reference_data_sha256": recorded.identity.reference_data_sha256,
            "effective_profile_sha256": recorded.resources.digest().unwrap(),
            "resources": recorded.resources,
            "derived": {"total_bytes": report["accounted"]["total_bytes"],
                "catalog_resident_term":0_u64,"catalog_initialization_term":0_u64,
                "schema_validation_term":0_u64,"catalog_context_term":0_u64},
        })
    }

    fn exported() -> Export {
        serde_json::from_value(export_value()).unwrap()
    }

    fn rebind_export_resources(raw: &mut Value) {
        let resources: ResourceExport = serde_json::from_value(raw["resources"].clone()).unwrap();
        raw["effective_profile_sha256"] = json!(resources.digest().unwrap());
        raw["derived"] = json!(Derived::from(resources.derive().unwrap()));
    }

    #[test]
    fn export_rederives_each_catalog_cost_and_rejects_forged_terms_or_digest() {
        let baseline = exported();
        parse_export(&serde_json::to_vec(&baseline).unwrap()).unwrap();
        for field in [
            "resident_bytes",
            "initialization_bytes",
            "schema_validation_per_auction_bytes",
            "context_per_auction_bytes",
        ] {
            let mut raw = export_value();
            raw["resources"]["catalog_costs"][field] = json!(101_u64);
            let resources: ResourceExport =
                serde_json::from_value(raw["resources"].clone()).unwrap();
            raw["effective_profile_sha256"] = json!(resources.digest().unwrap());
            raw["derived"] = json!(Derived::from(resources.derive().unwrap()));
            let valid = parse_export(&serde_json::to_vec(&raw).unwrap()).unwrap();
            assert_ne!(
                valid.resources.digest().unwrap(),
                baseline.resources.digest().unwrap()
            );
            let increment = if matches!(
                field,
                "schema_validation_per_auction_bytes" | "context_per_auction_bytes"
            ) {
                202_u64
            } else {
                101_u64
            };
            assert_eq!(baseline.resources.limits.max_concurrent_auctions, 2);
            assert_eq!(
                valid.derived.total_bytes,
                baseline.derived.total_bytes + increment
            );
            let profile = bind(&candidate(), &catalog(), &valid).unwrap();
            assert_eq!(profile.schema_version, 2);
            assert_eq!(profile.resources, valid.resources);
            assert_eq!(Derived::from(profile.derive().unwrap()), valid.derived);
            for term in [
                "total_bytes",
                "catalog_resident_term",
                "catalog_initialization_term",
                "schema_validation_term",
                "catalog_context_term",
            ] {
                let mut forged = raw.clone();
                let original = forged["derived"][term].as_u64().unwrap();
                forged["derived"][term] = json!(original + 1);
                assert_eq!(
                    parse_export(&serde_json::to_vec(&forged).unwrap()).unwrap_err(),
                    HarnessError::new("invalid_child_output"),
                    "{field}: {term}"
                );
            }
            let mut stale = raw.clone();
            stale["effective_profile_sha256"] = json!(baseline.effective_profile_sha256);
            assert_eq!(
                parse_export(&serde_json::to_vec(&stale).unwrap()).unwrap_err(),
                HarnessError::new("invalid_child_output"),
                "{field}: stale digest"
            );
            let mut unfunded = raw;
            unfunded["derived"] = json!(baseline.derived);
            assert_eq!(
                parse_export(&serde_json::to_vec(&unfunded).unwrap()).unwrap_err(),
                HarnessError::new("invalid_child_output"),
                "{field}: stale derivation"
            );
        }
    }

    #[test]
    fn export_requires_every_catalog_cost_and_derived_term_without_defaults() {
        let raw = export_value();
        parse_export(&serde_json::to_vec(&raw).unwrap()).unwrap();
        let mut positional = raw.clone();
        positional["resources"]["catalog_costs"] = json!([0_u64, 0_u64, 0_u64, 0_u64]);
        assert_eq!(
            parse_export(&serde_json::to_vec(&positional).unwrap()).unwrap_err(),
            HarnessError::new("invalid_child_output")
        );
        for (owner, fields) in [
            (
                "/resources/catalog_costs",
                [
                    "resident_bytes",
                    "initialization_bytes",
                    "schema_validation_per_auction_bytes",
                    "context_per_auction_bytes",
                ],
            ),
            (
                "/derived",
                [
                    "catalog_resident_term",
                    "catalog_initialization_term",
                    "schema_validation_term",
                    "catalog_context_term",
                ],
            ),
        ] {
            for field in fields {
                let mut missing = raw.clone();
                assert!(
                    missing
                        .pointer_mut(owner)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(field)
                        .is_some()
                );
                assert_eq!(
                    parse_export(&serde_json::to_vec(&missing).unwrap()).unwrap_err(),
                    HarnessError::new("invalid_child_output"),
                    "missing {owner}/{field}"
                );
                let encoded = serde_json::to_string(&raw).unwrap();
                let key = format!("\"{field}\":");
                let duplicate = encoded.replacen(&key, &format!("\"{field}\":0,{key}"), 1);
                assert_ne!(duplicate, encoded);
                assert_eq!(
                    parse_export(duplicate.as_bytes()).unwrap_err(),
                    HarnessError::new("invalid_child_output"),
                    "duplicate {owner}/{field}"
                );
            }
        }
    }

    fn bind(candidate: &Candidate, catalog: &Catalog, export: &Export) -> Result<Profile> {
        let recorded = synthetic_profile_fixture();
        bind_profile(
            candidate,
            catalog,
            export,
            recorded.identity,
            recorded.hardware,
            &recorded.variants,
        )
    }

    #[test]
    fn checked_in_candidate_and_catalog_parse_and_validate_without_live_config() {
        let parsed = parse_candidate(CANDIDATE).unwrap();
        let rows = parse_catalog(CATALOG).unwrap();
        validate_candidate(&parsed, &rows).unwrap();
        assert_eq!(parsed.platform.memory_bytes, 0x0001_0000_0000_u64);
        assert_eq!(parsed.seed, 7);
        assert_eq!(rows.cases.len(), 24);
        assert_eq!(serde_json::to_value(parsed).unwrap(), json_candidate());
    }

    fn json_candidate() -> Value {
        serde_json::from_slice(CANDIDATE).unwrap()
    }

    #[test]
    fn raw_candidate_rejects_unknown_duplicate_and_mistyped_fields_without_source() {
        for pointer in [
            "",
            "/expected_limits",
            "/platform",
            "/base_load",
            "/sdk/ingress",
            "/debug_reference/capture_bytes",
        ] {
            let mut value = json_candidate();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("SECRET_CANARY".to_owned(), json!(true));
            assert_eq!(
                parse_candidate(&serde_json::to_vec(&value).unwrap()).unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        let source = from_utf8(CANDIDATE).unwrap();
        for duplicate in [
            source.replacen(
                "\"schema_version\": 1",
                "\"schema_version\": 1, \"schema_version\": 1",
                1,
            ),
            source.replacen("\"calls\": 16", "\"calls\": 16, \"calls\": 16", 1),
        ] {
            assert_eq!(
                parse_candidate(duplicate.as_bytes()).unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        for (pointer, replacement) in [
            ("/kind", json!("SECRET_CANARY")),
            ("/admission_oracle", json!("SECRET_CANARY")),
            ("/seed", json!(0x0001_0000_0000_u64)),
            ("/duration_ms", json!(2_000.0_f64)),
            ("/expected_limits/max_parsed_nodes", json!(-1_i32)),
        ] {
            let mut value = json_candidate();
            *value.pointer_mut(pointer).unwrap() = replacement;
            let error = parse_candidate(&serde_json::to_vec(&value).unwrap()).unwrap_err();
            assert_eq!(error, HarnessError::new("invalid_input"));
            assert!(!format!("{error:?}").contains("SECRET_CANARY"));
        }
    }

    #[test]
    fn candidate_and_catalog_enforce_exact_byte_caps() {
        let mut candidate_bytes = CANDIDATE.to_vec();
        candidate_bytes.resize(INPUT_CAP, b' ');
        parse_candidate(&candidate_bytes).unwrap();
        candidate_bytes.push(b' ');
        assert_eq!(
            parse_candidate(&candidate_bytes).unwrap_err(),
            HarnessError::new("input_limit")
        );
        let mut catalog_bytes = CATALOG.to_vec();
        catalog_bytes.resize(INPUT_CAP, b' ');
        parse_catalog(&catalog_bytes).unwrap();
        catalog_bytes.push(b' ');
        assert_eq!(
            parse_catalog(&catalog_bytes).unwrap_err(),
            HarnessError::new("input_limit")
        );
    }

    #[test]
    fn candidate_numeric_bounds_and_reachable_geometry_match_the_frozen_contract() {
        let valid = candidate();
        let rows = catalog();
        validate_candidate(&valid, &rows).unwrap();
        for (pointer, replacement) in [
            ("/schema_version", json!(2_u16)),
            ("/duration_ms", json!(999_u64)),
            ("/duration_ms", json!(60_001_u64)),
            ("/warmup_ms", json!(0_u64)),
            ("/repetitions", json!(1_u8)),
            ("/repetitions", json!(3_u8)),
            ("/cold_window_ms", json!(0_u64)),
            ("/warm_window_ms", json!(60_001_u64)),
            ("/max_requests", json!(10_001_u32)),
            ("/request_timeout_ms", json!(10_001_u64)),
            ("/max_run_ms", json!(999_u64)),
            ("/max_run_ms", json!(900_001_u64)),
            ("/headroom_bytes", json!(0x0001_0000_0000_u64)),
            ("/sdk/ingress/max_connections", json!(255_u32)),
            ("/sdk/http2_enabled", json!(true)),
            ("/debug_reference/limiting_factor", json!("capture_storage")),
            ("/debug_reference/capture_bytes/min", json!(0_u64)),
            ("/debug_reference/output_bytes/max", json!(0x0001_0001_u64)),
            ("/base_load/concurrency", json!(17_u32)),
            ("/base_load/nodes", json!(1_u32)),
            ("/maximum_load/calls", json!(65_u32)),
            ("/maximum_load/encoded_bytes", json!(64_u64)),
        ] {
            let mut changed_json = serde_json::to_value(&valid).unwrap();
            *changed_json.pointer_mut(pointer).unwrap() = replacement;
            let changed: Candidate = serde_json::from_value(changed_json).unwrap();
            let expected = if matches!(pointer, "/base_load/nodes" | "/maximum_load/encoded_bytes")
            {
                HarnessError::new("unreachable_shape")
            } else {
                HarnessError::new("invalid_input")
            };
            assert_eq!(
                validate_candidate(&changed, &rows).unwrap_err(),
                expected,
                "{pointer}"
            );
        }
    }

    #[test]
    fn catalog_is_closed_ordered_and_has_exact_families_variants_modes_and_outcome_caps() {
        let candidate = candidate();
        let valid = catalog();
        validate_candidate(&candidate, &valid).unwrap();
        for (pointer, replacement) in [
            ("/schema_version", json!(2_u16)),
            ("/cases/0/id", json!(2_u16)),
            ("/cases/0/family", json!("debug")),
            ("/cases/0/subcase", json!("warm_baseline")),
            ("/cases/0/variant_id", json!(2_u16)),
            ("/cases/16/variant_id", json!(1_u16)),
            ("/cases/17/variant_id", json!(2_u16)),
            ("/cases/0/mode", json!("plain")),
            ("/cases/0/outcomes/max_errors", json!(10_001_u64)),
        ] {
            let mut changed_json = serde_json::to_value(&valid).unwrap();
            *changed_json.pointer_mut(pointer).unwrap() = replacement;
            let changed: Catalog = serde_json::from_value(changed_json).unwrap();
            assert_eq!(
                validate_candidate(&candidate, &changed).unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        let mut missing = valid.clone();
        missing.cases.pop();
        assert_eq!(
            validate_candidate(&candidate, &missing).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let mut extra = valid.clone();
        extra.cases.push(valid.cases[0].clone());
        assert_eq!(
            validate_candidate(&candidate, &extra).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let mut reordered = valid.clone();
        reordered.cases.swap(0, 1);
        assert_eq!(
            validate_candidate(&candidate, &reordered).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let mut raw: Value = serde_json::from_slice(CATALOG).unwrap();
        raw["cases"][0]["SECRET_CANARY"] = json!(true);
        assert_eq!(
            parse_catalog(&serde_json::to_vec(&raw).unwrap()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let compact =
            serde_json::to_string(&serde_json::from_slice::<Value>(CATALOG).unwrap()).unwrap();
        let duplicate = compact.replacen("\"id\":1", "\"id\":1,\"id\":1", 1);
        assert_ne!(
            duplicate, compact,
            "the duplicate-key mutation must take effect"
        );
        assert_eq!(
            parse_catalog(duplicate.as_bytes()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn export_retains_catalog_costs_and_accounting_but_discards_unclassified_derived_data() {
        let mut raw = export_value();
        raw["derived"]["discarded"] = json!({"SECRET_CANARY": [true, false]});
        let parsed = parse_export(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(parsed.resources, synthetic_profile_fixture().resources);
        assert_eq!(parsed.derived.total_bytes, 2_533_631_672);
        let safe = serde_json::to_value(parsed).unwrap();
        assert_eq!(
            safe["derived"],
            json!({"total_bytes": 2_533_631_672_u64,
            "catalog_resident_term":0_u64,"catalog_initialization_term":0_u64,
            "schema_validation_term":0_u64,"catalog_context_term":0_u64})
        );
        assert!(!safe.to_string().contains("SECRET_CANARY"));
    }

    #[test]
    fn export_rejects_unknown_duplicate_wrong_target_backend_features_and_digest_fields() {
        for (pointer, replacement) in [
            ("/schema_version", json!(1_u16)),
            ("/schema_version", json!(3_u16)),
            ("/target", json!("fastly")),
            ("/backend", json!("live_refresh")),
            ("/edgezero_pin", json!("A".repeat(40))),
            (
                "/build_features",
                json!(["insecure_bidders", "insecure_bidders"]),
            ),
            ("/build_features", json!(["SECRET_CANARY"])),
            ("/baked_host_sha256", json!("a".repeat(63))),
            ("/baked_account_sha256", json!("A".repeat(64))),
            ("/baked_config_sha256", json!("SECRET_CANARY")),
            ("/reference_data_sha256", json!(0_u8)),
            ("/effective_profile_sha256", Value::Null),
            ("/derived/total_bytes", json!(0_u8)),
        ] {
            let mut raw = export_value();
            *raw.pointer_mut(pointer).unwrap() = replacement;
            assert_eq!(
                parse_export(&serde_json::to_vec(&raw).unwrap()).unwrap_err(),
                HarnessError::new("invalid_child_output")
            );
        }
        for pointer in [
            "",
            "/resources",
            "/resources/limits",
            "/resources/sdk/config_store",
        ] {
            let mut raw = export_value();
            raw.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("SECRET_CANARY".to_owned(), json!(true));
            assert_eq!(
                parse_export(&serde_json::to_vec(&raw).unwrap()).unwrap_err(),
                HarnessError::new("invalid_child_output")
            );
        }
        let encoded = serde_json::to_string(&export_value()).unwrap();
        let duplicate = encoded.replacen(
            "\"schema_version\":2",
            "\"schema_version\":2,\"schema_version\":2",
            1,
        );
        assert_ne!(duplicate, encoded);
        assert_eq!(
            parse_export(duplicate.as_bytes()).unwrap_err(),
            HarnessError::new("invalid_child_output")
        );
        for features in [
            vec![],
            vec![Feature::TestHarnessOverride],
            vec![
                Feature::InsecureBidders,
                Feature::ConformanceSuite,
                Feature::TestHarnessOverride,
            ],
        ] {
            let mut raw = export_value();
            raw["build_features"] = serde_json::to_value(&features).unwrap();
            assert_eq!(
                parse_export(&serde_json::to_vec(&raw).unwrap())
                    .unwrap()
                    .build_features,
                features
            );
        }
    }

    #[test]
    fn export_caps_and_structural_limits_cover_discarded_derived_members() {
        let mut bytes = serde_json::to_vec(&export_value()).unwrap();
        bytes.resize(EXPORT_CAP, b' ');
        parse_export(&bytes).unwrap();
        bytes.push(b' ');
        assert_eq!(
            parse_export(&bytes).unwrap_err(),
            HarnessError::new("invalid_child_output")
        );
        let encoded = serde_json::to_string(&export_value()).unwrap();
        let deep = format!("{}0{}", "[".repeat(129), "]".repeat(129));
        let raw = encoded.replacen(
            "\"derived\":{",
            &format!("\"derived\":{{\"discarded\":{deep},"),
            1,
        );
        assert_eq!(
            parse_export(raw.as_bytes()).unwrap_err(),
            HarnessError::new("invalid_child_output")
        );
    }

    #[test]
    fn service_environment_has_exactly_ten_sdk_values_and_four_fixed_values() {
        let sdk = candidate().sdk;
        let environment = service_environment(9999, &sdk).unwrap();
        let expected = BTreeMap::from([
            ("EDGEZERO__ADAPTER__HOST", "127.0.0.1"),
            ("EDGEZERO__ADAPTER__PORT", "9999"),
            ("EDGEZERO__LOGGING__LEVEL", "off"),
            ("RUST_LOG", "off"),
            ("EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS", "256"),
            ("EDGEZERO__ADAPTER__INGRESS__MAX_RAW_HEAD_BYTES", "16384"),
            ("EDGEZERO__ADAPTER__INGRESS__MAX_HEADER_COUNT", "100"),
            ("EDGEZERO__ADAPTER__INGRESS__HEAD_READ_TIMEOUT_MS", "5000"),
            ("EDGEZERO__ADAPTER__CONFIG_STORE__MAX_ENTRIES", "16"),
            ("EDGEZERO__ADAPTER__CONFIG_STORE__MAX_FILE_BYTES", "1048576"),
            ("EDGEZERO__ADAPTER__CONFIG_STORE__MAX_KEY_BYTES", "256"),
            ("EDGEZERO__ADAPTER__CONFIG_STORE__MAX_VALUE_BYTES", "262144"),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_RESIDENT_BYTES",
                "2097152",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_STARTUP_BYTES",
                "8388608",
            ),
        ])
        .into_iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect();
        assert_eq!(environment, expected);
        for port in [1024_u16, u16::MAX] {
            service_environment(port, &sdk).unwrap();
        }
        for port in [0_u16, 1023] {
            assert_eq!(
                service_environment(port, &sdk).unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        assert_eq!(
            service_environment(8787, &sdk).unwrap_err(),
            HarnessError::new("port_collision")
        );
        for changed in [
            SdkSettings {
                ingress: None,
                ..sdk.clone()
            },
            SdkSettings {
                config_store: None,
                ..sdk.clone()
            },
            SdkSettings {
                http2_enabled: true,
                ..sdk.clone()
            },
        ] {
            assert_eq!(
                service_environment(9999, &changed).unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        let mut changed = sdk;
        changed.ingress.as_mut().unwrap().max_connections = 0;
        assert_eq!(
            service_environment(9999, &changed).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn tool_environment_only_selects_explicit_tools_and_fixes_the_locale() {
        let environment = tool_environment();
        let allowed = [
            "PATH",
            "HOME",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "CARGO_TARGET_DIR",
            "RUSTUP_TOOLCHAIN",
            "TMPDIR",
            "LC_ALL",
        ];
        assert_eq!(
            environment.get(OsStr::new("LC_ALL")),
            Some(&OsString::from("C"))
        );
        for key in environment.keys() {
            assert!(allowed.iter().any(|name| key == OsStr::new(name)));
        }
        for name in allowed.into_iter().filter(|name| *name != "LC_ALL") {
            let expected = var_os(name).filter(|value| !value.is_empty());
            assert_eq!(environment.get(OsStr::new(name)).cloned(), expected);
        }
    }

    fn assert_scenario_criteria(
        candidate: &Candidate,
        scenario: &profile::Scenario,
        row: &Row,
        subcase: Subcase,
    ) {
        assert_eq!(scenario.subcase, subcase);
        assert_eq!(scenario.family, row.family);
        assert_eq!(scenario.variant_id, row.variant_id);
        assert_eq!(scenario.outcomes, row.outcomes);
        assert_eq!(scenario.duration_ms, candidate.duration_ms);
        assert_eq!(scenario.warmup_ms, candidate.warmup_ms);
        assert_eq!(scenario.meter_window_ms, candidate.duration_ms);
        assert_eq!(scenario.max_peak_bytes, candidate.max_peak_bytes);
        assert_eq!(
            scenario.max_retained_growth_bytes,
            candidate.max_retained_growth_bytes
        );
        assert_eq!(
            scenario.latency.start,
            profile::LatencyStart::ClientDispatch
        );
        assert_eq!(scenario.latency.population, profile::Population::AllOffered);
        assert_eq!(scenario.latency.percentile, 99);
        assert_eq!(
            scenario.latency.threshold_us,
            candidate.latency_threshold_us
        );
        assert_eq!(scenario.latency.max_histogram_buckets, 4);
    }

    #[test]
    fn binding_preserves_all_criteria_and_exactly_forty_five_rows_and_three_variants() {
        let candidate = candidate();
        let catalog = catalog();
        let profile = bind(&candidate, &catalog, &exported()).unwrap();
        profile.validate().unwrap();
        assert_eq!(profile.scenarios.len(), 24);
        assert_eq!(
            profile
                .scenarios
                .iter()
                .map(|row| usize::from(row.repetitions))
                .sum::<usize>(),
            45
        );
        assert_eq!(
            profile
                .variants
                .iter()
                .map(|variant| variant.id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            profile.required_capabilities,
            profile::REQUIRED_CAPABILITIES
        );
        for ((scenario, row), subcase) in profile
            .scenarios
            .iter()
            .zip(&catalog.cases)
            .zip(profile::REQUIRED_SUBCASES)
        {
            assert_scenario_criteria(&candidate, scenario, row, *subcase);
        }
        for variant in &profile.variants {
            assert_eq!(variant.cold_window_ms, candidate.cold_window_ms);
            assert_eq!(variant.warm_window_ms, candidate.warm_window_ms);
            assert_eq!(variant.max_cold_peak_bytes, candidate.max_peak_bytes);
            assert_eq!(variant.max_warm_peak_bytes, candidate.max_peak_bytes);
        }
        let release = profile
            .scenarios
            .iter()
            .find(|row| row.subcase == Subcase::CaptureRelease)
            .unwrap();
        assert_eq!(catalog.cases[12].shape, Shape::Base);
        assert_eq!(release.load, candidate.maximum_load);
        assert_eq!(release.effects.debug_captures, 32);
        assert_eq!(release.effects.debug_output_bytes, 0x0000_4000_u64);
        let fpd = profile
            .scenarios
            .iter()
            .find(|row| row.subcase == Subcase::FpdExpansion)
            .unwrap();
        assert_eq!(fpd.load.impressions, candidate.base_load.impressions);
        assert_eq!(fpd.load.nodes, candidate.maximum_load.nodes);
    }

    #[test]
    fn binding_checks_every_expected_limit_sdk_cache_feature_and_variant_matrix() {
        let candidate = candidate();
        let catalog = catalog();
        bind(&candidate, &catalog, &exported()).unwrap();
        for key in serde_json::to_value(&candidate.expected_limits)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
        {
            let mut changed_json = export_value();
            let limit = &mut changed_json["resources"]["limits"][key];
            // Lower the declaration so the assembly floor remains funded. A
            // mismatched candidate must fail after a consistent export, not at
            // the newly checked resource derivation.
            let original = limit.as_u64().unwrap();
            assert!(original > 1, "{key}");
            *limit = json!(original - 1);
            rebind_export_resources(&mut changed_json);
            let changed: Export = serde_json::from_value(changed_json).unwrap();
            assert_eq!(
                bind(&candidate, &catalog, &changed).unwrap_err(),
                HarnessError::new("candidate_mismatch")
            );
        }
        for (pointer, replacement) in [
            ("/resources/sdk/ingress/max_connections", json!(128_u32)),
            ("/resources/actual_gvl_cache_entries", json!(3_u32)),
            ("/build_features", json!([])),
        ] {
            let mut changed_json = export_value();
            *changed_json.pointer_mut(pointer).unwrap() = replacement;
            rebind_export_resources(&mut changed_json);
            let changed: Export = serde_json::from_value(changed_json).unwrap();
            assert_eq!(
                bind(&candidate, &catalog, &changed).unwrap_err(),
                HarnessError::new("candidate_mismatch")
            );
        }
        let recorded = synthetic_profile_fixture();
        for ids in [
            vec![1_u16, 2],
            vec![1, 2, 2],
            vec![1, 2, 4],
            vec![1, 2, 3, 4],
        ] {
            let variants = ids
                .into_iter()
                .map(|id| profile::Variant {
                    id,
                    ..recorded.variants[0].clone()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                bind_profile(
                    &candidate,
                    &catalog,
                    &exported(),
                    recorded.identity.clone(),
                    recorded.hardware.clone(),
                    &variants
                )
                .unwrap_err(),
                HarnessError::new("candidate_mismatch")
            );
        }
        let mut variants = recorded.variants.clone();
        variants[2].config_sha256 = variants[1].config_sha256.clone();
        assert_eq!(
            bind_profile(
                &candidate,
                &catalog,
                &exported(),
                recorded.identity,
                recorded.hardware,
                &variants
            )
            .unwrap_err(),
            HarnessError::new("candidate_mismatch")
        );
    }

    #[test]
    fn binding_takes_export_owned_identity_fields_from_the_actual_export() {
        let recorded = synthetic_profile_fixture();
        let mut identity = recorded.identity.clone();
        identity.target = Target::Fastly;
        identity.build_features.clear();
        identity.edgezero_pin = Revision::try_from("b".repeat(40)).unwrap();
        identity.reference_data_sha256 = Digest::of(b"stale-reference");
        identity.baked_config_sha256 = Digest::of(b"stale-baked");
        let mut export = exported();
        export.build_features = vec![
            Feature::InsecureBidders,
            Feature::ConformanceSuite,
            Feature::TestHarnessOverride,
        ];
        let profile = bind_profile(
            &candidate(),
            &catalog(),
            &export,
            identity,
            recorded.hardware,
            &recorded.variants,
        )
        .unwrap();
        assert_eq!(profile.identity.target, Target::Axum);
        assert_eq!(profile.identity.build_features, export.build_features);
        assert_eq!(profile.identity.edgezero_pin, export.edgezero_pin);
        assert_eq!(
            profile.identity.reference_data_sha256,
            export.reference_data_sha256
        );
        assert_eq!(
            profile.identity.baked_config_sha256,
            export.baked_config_sha256
        );
        assert_eq!(
            profile.identity.artifact_sha256,
            recorded.identity.artifact_sha256
        );
        assert_eq!(
            profile.identity.harness_sha256,
            recorded.identity.harness_sha256
        );
        assert_eq!(
            profile.identity.fixtures_sha256,
            recorded.identity.fixtures_sha256
        );
    }

    #[test]
    fn fixture_digests_bind_rows_load_seed_reviewed_debug_geometry_and_protocol_one() {
        let candidate = candidate();
        let catalog = catalog();
        let profile = bind(&candidate, &catalog, &exported()).unwrap();
        assert_eq!(FIXTURE_PROTOCOL_VERSION, 1);
        for (scenario, row) in profile.scenarios.iter().zip(&catalog.cases) {
            let debug = matches!(
                row.subcase,
                Subcase::Permitted | Subcase::ConcurrentCapture | Subcase::CaptureRelease
            );
            let mut declaration = json!({"row": row, "load": scenario.load, "seed": candidate.seed,
                "debug_reference": debug.then_some(&candidate.debug_reference),
                "fixture_protocol_version": 1_u16});
            if debug {
                declaration["debug_geometry"] = json!({"max_banner_formats": 8_u8});
            }
            assert_eq!(
                scenario.fixture.sha256,
                Digest::canonical(&declaration).unwrap()
            );
            declaration
                .as_object_mut()
                .unwrap()
                .remove("fixture_protocol_version");
            assert_ne!(
                scenario.fixture.sha256,
                Digest::canonical(&declaration).unwrap()
            );
            if let Some(workload) = &scenario.debug_workload {
                assert_eq!(
                    workload.reference_sha256,
                    Digest::canonical(&workload.reference).unwrap()
                );
                assert_eq!(
                    workload.reference.resources_sha256,
                    Digest::canonical(&profile.resources).unwrap()
                );
                assert_eq!(workload.reference.review_sha256, Digest::canonical(&json!({"bounds": candidate.debug_reference, "geometry": {"max_banner_formats": 8_u8}})).unwrap());
            }
        }
        let mut changed_seed = candidate.clone();
        changed_seed.seed += 1;
        let seeded = bind(&changed_seed, &catalog, &exported()).unwrap();
        assert!(
            profile
                .scenarios
                .iter()
                .zip(&seeded.scenarios)
                .all(|(a, b)| a.fixture.sha256 != b.fixture.sha256)
        );
        let mut changed_rows = catalog.clone();
        changed_rows.cases[0].mode = fixture::Mode::Mixed;
        assert_ne!(
            profile.scenarios[0].fixture.sha256,
            bind(&candidate, &changed_rows, &exported())
                .unwrap()
                .scenarios[0]
                .fixture
                .sha256
        );
        let mut changed_bounds = candidate;
        changed_bounds.debug_reference.capture_bytes.min += 1;
        let reviewed = bind(&changed_bounds, &catalog, &exported()).unwrap();
        assert_eq!(
            profile.scenarios[0].fixture.sha256,
            reviewed.scenarios[0].fixture.sha256
        );
        assert_ne!(
            profile.scenarios[12].fixture.sha256,
            reviewed.scenarios[12].fixture.sha256
        );
    }

    #[test]
    fn debug_reference_clamps_each_resource_axis_independently() {
        let mut candidate = candidate();
        let fixture = profile::Fixture {
            id: 11,
            sha256: Digest::of(b"reviewed-fixture"),
        };
        let mut resources = synthetic_profile_fixture().resources;
        resources.limits.max_outbound_requests = 128;
        resources.limits.max_decoded_bytes = 128_u64 << 10_u32;
        resources.limits.max_parsed_nodes = 8192;
        resources.limits.final_response_bytes = 1_u64 << 20_u32;
        let upper = debug_workload(&candidate, &fixture, &resources).unwrap();
        assert_eq!(
            upper.reference.limits,
            profile::DebugLimits {
                call_rows: 64,
                input_bytes: 0x0001_0000,
                body_nodes: 4096,
                body_bytes: 0x0000_4000,
                capture_bytes: 0x0001_0000,
                output_bytes: 0x0001_0000
            }
        );
        resources.limits.max_outbound_requests = 4;
        resources.limits.max_decoded_bytes = 8192;
        resources.limits.max_parsed_nodes = 512;
        resources.limits.final_response_bytes = 0x0000_8000;
        candidate.maximum_load.calls = 4;
        candidate.debug_reference.output_bytes.max = 0x0000_8000;
        let lower = debug_workload(&candidate, &fixture, &resources).unwrap();
        assert_eq!(
            lower.reference.limits,
            profile::DebugLimits {
                call_rows: 4,
                input_bytes: 8192,
                body_nodes: 512,
                body_bytes: 0x0000_4000,
                capture_bytes: 0x0001_0000,
                output_bytes: 0x0000_8000
            }
        );
        assert_eq!(lower.reference.min_call_rows, 4);
        assert_eq!(lower.reference.max_call_rows, 4);
        assert_eq!(lower.reference.fixture, fixture);
        assert_ne!(
            upper.reference.resources_sha256,
            lower.reference.resources_sha256
        );
        assert_ne!(upper.reference_sha256, lower.reference_sha256);
    }
}
