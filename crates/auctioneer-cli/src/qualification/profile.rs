use auctioneer_privacy::gvl::GVL_PARSE_CACHE_MAX_ENTRIES;
use auctioneer_runtime::{
    CatalogResourceCosts, HeadroomClass, RequestConstructionCosts, ResolvedIsolateLimits,
    TargetPlatformConstants, minimum_headroom_bytes,
};
use edgezero_adapter_axum::{
    config_store_limits::ConfigStoreLimits, ingress_config::AxumIngressConfig,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use sha2::{Digest as _, Sha256};
use std::{
    fmt::{self, Formatter},
    num::NonZeroU64,
    time::Duration,
};

use super::{Error, MAX_ROWS, decode};

pub const RESOURCE_SCHEMA_VERSION: u16 = 2;

/// Only validated lowercase hex crosses the identity boundary, never labels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Hex<const N: usize>(String);

impl<const N: usize> Hex<N> {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<const N: usize> TryFrom<String> for Hex<N> {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() != N
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid_digest");
        }
        Ok(Self(value))
    }
}

pub type Digest = Hex<64>;
pub type Revision = Hex<40>;

impl Digest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    /// Compact sorted-key JSON is shared with the offline fixture reviewer.
    pub fn canonical<T: Serialize>(value: &T) -> Result<Self, Error> {
        let sorted = serde_json::to_value(value).map_err(|_error| Error::InvalidProfile)?;
        Ok(Self::of(&super::serialize(&sorted)?))
    }
}

macro_rules! codes {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

codes!(Target {
    Axum,
    Cloudflare,
    Fastly,
    Spin
});
codes!(Scope { Local, Deployment });
codes!(Feature {
    InsecureBidders,
    ConformanceSuite,
    TestHarnessOverride
});
codes!(Architecture {
    Aarch64,
    X86_64,
    Wasm32
});
codes!(CpuKind {
    AppleMSeries,
    AmdEpyc,
    IntelXeon,
    VirtualCpu,
    ProviderManaged
});
codes!(Os {
    Macos,
    Linux,
    Provider
});
codes!(Meter {
    ProcessRss,
    Allocator,
    WasmGuest,
    Provider
});
codes!(Backend {
    StartupSnapshot,
    LiveRefresh
});
codes!(Capability {
    IngressAdmission,
    ConfigPreExtraction,
    TransportLifecycle,
    DeliveryLifetime,
    Deadline,
    Cancellation
});
codes!(Family {
    Baseline,
    MaximumAdmitted,
    DecodeOverlap,
    Debug,
    AuxiliaryConfig,
    SaturationRecovery
});
codes!(Subcase {
    ColdStartup,
    WarmBaseline,
    GvlColdParse,
    MaximumShape,
    FpdExpansion,
    EncodedDecoded,
    SlowReader,
    MixedOutcomes,
    Unrequested,
    Denied,
    Permitted,
    ConcurrentCapture,
    CaptureRelease,
    AuxiliaryOverlap,
    ConfigSnapshot,
    GvlMissOverlap,
    ConfigVariant,
    ConfigRestoration,
    ExcessAdmission,
    OutboundQueue,
    StalledIo,
    Cancellation,
    Timeout,
    HealthyRecovery
});
codes!(MaximumMode {
    Simultaneous,
    ReviewedNonSimultaneous
});
codes!(LatencyStart {
    ClientDispatch,
    Admission
});
codes!(LatencyEnd { TerminalOutcome });
codes!(Population {
    AllOffered,
    AllAdmitted,
    SuccessOnly
});
codes!(DebugLimit {
    FixtureShape,
    CaptureStorage,
    SerializedOutput,
    FinalResponse
});

pub const REQUIRED_SUBCASES: &[Subcase] = &[
    Subcase::ColdStartup,
    Subcase::WarmBaseline,
    Subcase::GvlColdParse,
    Subcase::MaximumShape,
    Subcase::FpdExpansion,
    Subcase::EncodedDecoded,
    Subcase::SlowReader,
    Subcase::MixedOutcomes,
    Subcase::Unrequested,
    Subcase::Denied,
    Subcase::Permitted,
    Subcase::ConcurrentCapture,
    Subcase::CaptureRelease,
    Subcase::AuxiliaryOverlap,
    Subcase::ConfigSnapshot,
    Subcase::GvlMissOverlap,
    Subcase::ConfigVariant,
    Subcase::ConfigRestoration,
    Subcase::ExcessAdmission,
    Subcase::OutboundQueue,
    Subcase::StalledIo,
    Subcase::Cancellation,
    Subcase::Timeout,
    Subcase::HealthyRecovery,
];
pub const REQUIRED_CAPABILITIES: &[Capability] = &[
    Capability::IngressAdmission,
    Capability::ConfigPreExtraction,
    Capability::TransportLifecycle,
    Capability::DeliveryLifetime,
    Capability::Deadline,
    Capability::Cancellation,
];

impl Subcase {
    pub const fn family(self) -> Family {
        match self {
            Self::ColdStartup | Self::WarmBaseline | Self::GvlColdParse => Family::Baseline,
            Self::MaximumShape | Self::FpdExpansion => Family::MaximumAdmitted,
            Self::EncodedDecoded | Self::SlowReader | Self::MixedOutcomes => Family::DecodeOverlap,
            Self::Unrequested
            | Self::Denied
            | Self::Permitted
            | Self::ConcurrentCapture
            | Self::CaptureRelease => Family::Debug,
            Self::AuxiliaryOverlap
            | Self::ConfigSnapshot
            | Self::GvlMissOverlap
            | Self::ConfigVariant
            | Self::ConfigRestoration => Family::AuxiliaryConfig,
            Self::ExcessAdmission
            | Self::OutboundQueue
            | Self::StalledIo
            | Self::Cancellation
            | Self::Timeout
            | Self::HealthyRecovery => Family::SaturationRecovery,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub auctioneer_commit: Revision,
    pub artifact_sha256: Digest,
    pub build_features: Vec<Feature>,
    pub edgezero_pin: Revision,
    pub target: Target,
    pub runtime_version: Version,
    pub harness_sha256: Digest,
    pub fixtures_sha256: Digest,
    pub reference_data_sha256: Digest,
    pub baked_config_sha256: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hardware {
    pub architecture: Architecture,
    pub cpu: CpuKind,
    pub logical_cores: u16,
    pub memory_bytes: u64,
    pub page_bytes: u32,
    pub os: Os,
    pub os_version: Version,
    pub meter: Meter,
    pub meter_version: Version,
    pub machine_sha256: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    pub memory_bytes: u64,
    pub concurrent_requests: u32,
    pub wall_time_ms: u64,
    pub cpu_time_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceExport {
    #[serde(with = "LimitsDto")]
    pub limits: ResolvedIsolateLimits,
    #[serde(with = "TargetDto")]
    pub target_constants: TargetPlatformConstants,
    #[serde(with = "ConstructionDto")]
    pub construction_costs: RequestConstructionCosts,
    #[serde(
        serialize_with = "CatalogCostsDto::serialize",
        deserialize_with = "catalog_costs"
    )]
    pub catalog_costs: CatalogResourceCosts,
    pub outbound_meta_bytes: u32,
    pub baked_config_bytes: u64,
    pub actual_gvl_cache_entries: u32,
    pub sdk: SdkSettings,
}

impl ResourceExport {
    pub fn derive(&self) -> Result<auctioneer_runtime::Derived, Error> {
        auctioneer_runtime::derive_isolate_budget_with_baked_config(
            &self.limits,
            &self.target_constants,
            self.outbound_meta_bytes,
            self.construction_costs,
            self.baked_config_bytes,
            self.catalog_costs,
        )
        .map_err(|_error| Error::InvalidProfile)
    }

    pub fn digest(&self) -> Result<Digest, Error> {
        Ok(Digest::of(&super::serialize(self)?))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "CatalogResourceCosts", deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "remote fields must exactly match the runtime's byte-cost inputs"
)]
struct CatalogCostsDto {
    resident_bytes: u64,
    initialization_bytes: u64,
    schema_validation_per_auction_bytes: u64,
    context_per_auction_bytes: u64,
}

fn catalog_costs<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<CatalogResourceCosts, D::Error> {
    struct Object;

    #[expect(
        clippy::missing_trait_methods,
        reason = "Serde defaults must reject every non-object representation"
    )]
    impl<'de> Visitor<'de> for Object {
        type Value = CatalogResourceCosts;

        fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            formatter.write_str("a catalog cost object")
        }

        fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
            CatalogCostsDto::deserialize(MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_map(Object)
}

// Remote derives preserve the runtime model's field widths and exhaustive shape
// without adding serialization policy to the runtime owner.
#[derive(Serialize, Deserialize)]
#[serde(remote = "ResolvedIsolateLimits", deny_unknown_fields)]
struct LimitsDto {
    isolate_total_bytes: u64,
    baseline_bytes: u64,
    inbound_request_bytes: u64,
    inbound_parse_factor: u64,
    max_outbound_request_bytes: u64,
    max_outbound_raw_bytes: u64,
    max_transport_bytes: u64,
    max_decoded_bytes: u64,
    response_parse_factor: u64,
    response_node_peak_bytes: u64,
    final_response_bytes: u64,
    egress_permits: u32,
    response_write_deadline_ms: u64,
    guest_decoder_bound_bytes: u64,
    aux_route_permits: u32,
    aux_route_peak_bytes: u64,
    max_brotli_wbits: u8,
    max_brotli_layers: u8,
    gvl_cache_entries: u32,
    gvl_parsed_bytes: u64,
    max_bidders_per_auction: u32,
    max_outbound_requests: u32,
    max_concurrent_auctions: u32,
    max_active_body_streams: u32,
    max_runtime_config_bytes: u64,
    max_runtime_config_blob_bytes: u64,
    max_parsed_nodes: u32,
    max_copied_header_bytes: u32,
    max_admitted_header_bytes: u32,
    max_admitted_header_count: u16,
    max_impressions_per_auction: u16,
    max_config_json_nodes: u32,
    terminal_response_permits: u32,
    refusal_body_bytes: u32,
    transmit_state_bytes: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "TargetPlatformConstants", deny_unknown_fields)]
struct TargetDto {
    platform_concurrent_ceiling: u32,
    header_entry_overhead_bytes: u32,
    parser_header_ceiling_bytes: u32,
    parser_header_knob_bytes: u32,
    max_request_target_bytes: u32,
    h1_max_buf_size_bytes: u32,
    h1_max_headers: u32,
    parser_max_header_entries: u32,
    pseudoheader_allowance_bytes: u32,
    parser_transient_bytes: u32,
    egress_handoff_factor: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "RequestConstructionCosts", deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "remote fields must exactly match the runtime's byte-cost inputs"
)]
struct ConstructionDto {
    fixed_per_auction_bytes: u64,
    reusable_scratch_bytes: u64,
    retained_per_scope_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SdkSettings {
    pub ingress: Option<IngressSettings>,
    pub config_store: Option<StoreSettings>,
    pub http2_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngressSettings {
    pub max_connections: u32,
    pub max_raw_head_bytes: u32,
    pub max_header_count: u32,
    pub head_read_timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "safe export fields match the SDK's max_* setting names"
)]
pub struct StoreSettings {
    pub max_entries: u32,
    pub max_file_bytes: u64,
    pub max_key_bytes: u32,
    pub max_value_bytes: u64,
    pub max_resident_bytes: u64,
    pub max_startup_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub id: u16,
    pub config_sha256: Digest,
    pub settings_sha256: Digest,
    pub cold_window_ms: u64,
    pub warm_window_ms: u64,
    pub max_cold_peak_bytes: u64,
    pub max_warm_peak_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub id: u16,
    pub sha256: Digest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteBounds {
    pub min: u64,
    pub max: u64,
}

impl ByteBounds {
    pub const fn within(self, declared: Self) -> bool {
        declared.min <= self.min && self.min <= self.max && self.max <= declared.max
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugLimits {
    pub call_rows: u32,
    pub input_bytes: u64,
    pub body_nodes: u32,
    pub body_bytes: u64,
    pub capture_bytes: u64,
    pub output_bytes: u64,
}

impl DebugLimits {
    pub fn effective(resources: &ResourceExport) -> Self {
        let l = resources.limits;
        Self {
            call_rows: l
                .max_outbound_requests
                .min(auctioneer_runtime::MAX_DEBUG_CALL_ROWS),
            input_bytes: l
                .max_decoded_bytes
                .min(auctioneer_runtime::MAX_DEBUG_INPUT_BYTES),
            body_nodes: l
                .max_parsed_nodes
                .min(auctioneer_runtime::MAX_DEBUG_BODY_NODES),
            body_bytes: auctioneer_runtime::MAX_DEBUG_SANITIZED_BODY_BYTES,
            capture_bytes: auctioneer_runtime::MAX_DEBUG_RETAINED_BYTES,
            output_bytes: l
                .final_response_bytes
                .min(auctioneer_runtime::MAX_DEBUG_SERIALIZED_BYTES),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugReference {
    pub fixture: Fixture,
    pub resources_sha256: Digest,
    pub review_sha256: Digest,
    pub limiting_factor: DebugLimit,
    pub limits: DebugLimits,
    pub min_call_rows: u32,
    pub max_call_rows: u32,
    pub capture_bytes: ByteBounds,
    pub output_bytes: ByteBounds,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugWorkload {
    pub reference_sha256: Digest,
    pub reference: DebugReference,
}

impl DebugWorkload {
    fn validate(&self, scenario: &Scenario, profile: &Profile) -> Result<(), Error> {
        let r = &self.reference;
        let content_cap = u64::from(r.max_call_rows)
            .saturating_mul(r.limits.body_bytes.saturating_mul(2))
            .min(r.limits.capture_bytes);
        if r.fixture != scenario.fixture
            || r.resources_sha256 != Digest::canonical(&profile.resources)?
            || self.reference_sha256 != Digest::canonical(r)?
            || r.limits != DebugLimits::effective(&profile.resources)
            || r.min_call_rows == 0
            || r.min_call_rows > r.max_call_rows
            || r.max_call_rows > r.limits.call_rows
            || !r.capture_bytes.within(ByteBounds {
                min: 0,
                max: content_cap,
            })
            || !r.output_bytes.within(ByteBounds {
                min: debug_wire_minimum(r.min_call_rows, r.capture_bytes.min)?,
                max: r.limits.output_bytes,
            })
            || scenario.effects.debug_captures
                < u64::from(scenario.load.concurrency).saturating_mul(u64::from(r.min_call_rows))
            || u64::from(scenario.load.concurrency)
                .checked_mul(r.output_bytes.min)
                .is_none_or(|minimum| scenario.effects.debug_output_bytes < minimum)
        {
            return Err(Error::InvalidProfile);
        }
        Ok(())
    }
}

fn debug_wire_minimum(rows: u32, content_bytes: u64) -> Result<u64, Error> {
    // A lower bound from the actual DTO's required fields, not a resource or
    // performance floor. Empty names/origins and absent responses undercount.
    let row = serde_json::json!({"call_index":0_u32,"request":{"method":"GET",
        "endpoint_origin":"","headers":{},"body_status":"empty"},"elapsed_ms":0_u32});
    let count = usize::try_from(rows).map_err(|_error| Error::InvalidProfile)?;
    if rows > auctioneer_runtime::MAX_DEBUG_CALL_ROWS {
        return Err(Error::InvalidProfile);
    }
    let shape = serde_json::json!({"httpcalls":{"":vec![row; count]},"truncated":true,"dropped_calls":0_u32});
    auctioneer_runtime::serialized_json_bytes(&shape)
        .map_err(|_error| Error::InvalidProfile)?
        .checked_add(content_bytes)
        .ok_or(Error::InvalidProfile)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Load {
    pub concurrency: u32,
    pub calls: u32,
    pub request_bytes: u64,
    pub nodes: u32,
    pub impressions: u16,
    pub encoded_bytes: u64,
    pub decoded_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeCriteria {
    pub min_offered: u64,
    pub min_admitted: u64,
    pub min_success: u64,
    pub min_rejected: u64,
    pub min_errors: u64,
    pub min_timeouts: u64,
    pub max_rejected: u64,
    pub max_errors: u64,
    pub max_timeouts: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatencyCriteria {
    pub start: LatencyStart,
    pub end: LatencyEnd,
    pub population: Population,
    pub percentile: u8,
    pub threshold_us: u64,
    pub max_histogram_buckets: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub family: Family,
    pub subcase: Subcase,
    pub variant_id: u16,
    pub fixture: Fixture,
    pub seed: u64,
    pub repetitions: u8,
    pub warmup_ms: u64,
    pub duration_ms: u64,
    pub meter_window_ms: u64,
    pub load: Load,
    pub effects: Effects,
    pub outcomes: OutcomeCriteria,
    pub latency: LatencyCriteria,
    pub max_peak_bytes: u64,
    pub max_retained_growth_bytes: u64,
    pub maximum_mode: MaximumMode,
    pub justification_sha256: Option<Digest>,
    pub debug_workload: Option<DebugWorkload>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    pub gvl_parses: u64,
    pub fpd_expansions: u64,
    pub decoded_responses: u64,
    pub slow_reads: u64,
    pub debug_captures: u64,
    pub debug_denials: u64,
    pub debug_output_bytes: u64,
    pub auxiliary_requests: u64,
    pub config_extractions: u64,
    pub queued_calls: u64,
    pub cancellations: u64,
    pub stalled_reads: u64,
    pub recovery_successes: u64,
}

impl Effects {
    pub fn meets(self, minimum: Self) -> bool {
        self.gvl_parses >= minimum.gvl_parses
            && self.fpd_expansions >= minimum.fpd_expansions
            && self.decoded_responses >= minimum.decoded_responses
            && self.slow_reads >= minimum.slow_reads
            && self.debug_captures >= minimum.debug_captures
            && self.debug_denials >= minimum.debug_denials
            && self.debug_output_bytes >= minimum.debug_output_bytes
            && self.auxiliary_requests >= minimum.auxiliary_requests
            && self.config_extractions >= minimum.config_extractions
            && self.queued_calls >= minimum.queued_calls
            && self.cancellations >= minimum.cancellations
            && self.stalled_reads >= minimum.stalled_reads
            && self.recovery_successes >= minimum.recovery_successes
    }

    fn declares(self, subcase: Subcase, profile: &Profile) -> bool {
        match subcase {
            Subcase::ColdStartup
            | Subcase::ConfigSnapshot
            | Subcase::ConfigVariant
            | Subcase::ConfigRestoration => self.config_extractions > 0,
            Subcase::GvlColdParse => self.gvl_parses > 0,
            Subcase::GvlMissOverlap => {
                self.gvl_parses > u64::from(profile.resources.actual_gvl_cache_entries)
            }
            Subcase::FpdExpansion => self.fpd_expansions > 0,
            Subcase::EncodedDecoded | Subcase::MixedOutcomes => self.decoded_responses > 0,
            Subcase::SlowReader => self.slow_reads > 0,
            Subcase::Unrequested => self.debug_captures == 0 && self.debug_output_bytes == 0,
            Subcase::Denied => {
                self.debug_denials > 0 && self.debug_captures == 0 && self.debug_output_bytes == 0
            }
            Subcase::Permitted => self.debug_captures > 0 && self.debug_output_bytes > 0,
            Subcase::ConcurrentCapture => {
                self.debug_captures >= u64::from(profile.resources.limits.max_concurrent_auctions)
                    && self.debug_output_bytes > 0
            }
            Subcase::AuxiliaryOverlap => self.auxiliary_requests > 0,
            Subcase::OutboundQueue => self.queued_calls > 0,
            Subcase::StalledIo => self.stalled_reads > 0,
            Subcase::Cancellation => self.cancellations > 0,
            Subcase::CaptureRelease => {
                self.recovery_successes > 0
                    && self.debug_captures > 0
                    && self.debug_output_bytes > 0
            }
            Subcase::WarmBaseline | Subcase::HealthyRecovery => self.recovery_successes > 0,
            Subcase::MaximumShape | Subcase::ExcessAdmission | Subcase::Timeout => true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema_version: u16,
    pub scope: Scope,
    pub identity: Identity,
    pub resources: ResourceExport,
    pub platform: Platform,
    pub hardware: Hardware,
    pub headroom_bytes: u64,
    pub backend: Backend,
    pub variants: Vec<Variant>,
    pub required_capabilities: Vec<Capability>,
    pub scenarios: Vec<Scenario>,
}

pub fn parse(bytes: &[u8]) -> Result<Profile, Error> {
    let profile: Profile = decode(bytes)?;
    profile.validate()?;
    Ok(profile)
}

impl Profile {
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema_version != RESOURCE_SCHEMA_VERSION
            || self.identity.build_features.len() > 3
            || !unique(&self.identity.build_features)
            || self.platform.memory_bytes == 0
            || self.platform.concurrent_requests == 0
            || self.platform.wall_time_ms == 0
            || self.platform.cpu_time_ms == 0
            || self.hardware.logical_cores == 0
            || self.hardware.memory_bytes < self.platform.memory_bytes
            || !self.hardware.page_bytes.is_power_of_two()
            || self.hardware.page_bytes < 1024
            || usize::try_from(self.resources.actual_gvl_cache_entries).ok()
                != Some(GVL_PARSE_CACHE_MAX_ENTRIES)
            || self.resources.limits.gvl_cache_entries < self.resources.actual_gvl_cache_entries
            || self.resources.baked_config_bytes == 0
            || self.resources.outbound_meta_bytes == 0
            || self.resources.limits.isolate_total_bytes > self.platform.memory_bytes
            || self.resources.target_constants.platform_concurrent_ceiling
                > self.platform.concurrent_requests
        {
            return Err(Error::InvalidProfile);
        }
        let ceiling = NonZeroU64::new(
            self.platform
                .memory_bytes
                .min(self.resources.limits.isolate_total_bytes),
        )
        .ok_or(Error::InvalidProfile)?;
        let headroom_class = match self.identity.target {
            Target::Axum => HeadroomClass::Native,
            Target::Cloudflare | Target::Fastly | Target::Spin => HeadroomClass::Wasm,
        };
        if self.headroom_bytes < minimum_headroom_bytes(ceiling, headroom_class) {
            return Err(Error::InvalidProfile);
        }
        self.validate_sdk()?;
        if self.identity.target == Target::Axum
            && (self.backend != Backend::StartupSnapshot
                || self.hardware.meter != Meter::ProcessRss)
        {
            return Err(Error::InvalidProfile);
        }
        if self.scenarios.len() > MAX_ROWS
            || self.variants.len() > MAX_ROWS
            || self.variants.is_empty()
            || self.required_capabilities.len() != REQUIRED_CAPABILITIES.len()
            || !REQUIRED_CAPABILITIES
                .iter()
                .all(|c| self.required_capabilities.contains(c))
            || !REQUIRED_SUBCASES
                .iter()
                .all(|c| self.scenarios.iter().any(|s| s.subcase == *c))
        {
            return Err(Error::InvalidProfile);
        }
        let mut total_rows = 0_usize;
        for (index, variant) in self.variants.iter().enumerate() {
            if variant.id == 0
                || variant.cold_window_ms == 0
                || variant.warm_window_ms == 0
                || variant.max_cold_peak_bytes == 0
                || variant.max_cold_peak_bytes > self.platform.memory_bytes
                || variant.max_warm_peak_bytes == 0
                || variant.max_warm_peak_bytes > self.platform.memory_bytes
                || self
                    .variants
                    .iter()
                    .skip(index.saturating_add(1))
                    .any(|v| v.id == variant.id)
                || !self.scenarios.iter().any(|s| s.variant_id == variant.id)
            {
                return Err(Error::InvalidProfile);
            }
        }
        for (index, scenario) in self.scenarios.iter().enumerate() {
            scenario.validate(self)?;
            total_rows = total_rows
                .checked_add(usize::from(scenario.repetitions))
                .ok_or(Error::InvalidProfile)?;
            if self
                .scenarios
                .iter()
                .skip(index.saturating_add(1))
                .any(|s| s.subcase == scenario.subcase && s.variant_id == scenario.variant_id)
            {
                return Err(Error::InvalidProfile);
            }
        }
        if total_rows > MAX_ROWS {
            return Err(Error::InvalidProfile);
        }
        self.validate_variant_matrix()?;
        // Validate every structural/funding constraint even when the boot ceiling
        // is exceeded. Demand plus selected headroom remains an evaluation
        // check; the minimum headroom policy is validated above, not at boot.
        self.derive()?;
        super::report::ensure_capacity(self)?;
        Ok(())
    }

    pub fn derive(&self) -> Result<auctioneer_runtime::Derived, Error> {
        self.resources.derive()
    }

    fn validate_variant_matrix(&self) -> Result<(), Error> {
        let find = |subcase| {
            self.scenarios
                .iter()
                .find(|s| s.subcase == subcase)
                .and_then(|s| self.variants.iter().find(|v| v.id == s.variant_id))
                .ok_or(Error::InvalidProfile)
        };
        let baseline = find(Subcase::ColdStartup)?;
        for scenario in self.scenarios.iter().filter(|s| {
            matches!(
                s.subcase,
                Subcase::ConfigVariant | Subcase::ConfigRestoration
            )
        }) {
            let variant = self
                .variants
                .iter()
                .find(|v| v.id == scenario.variant_id)
                .ok_or(Error::InvalidProfile)?;
            let unchanged_config = baseline.config_sha256 == variant.config_sha256;
            let unchanged_settings = baseline.settings_sha256 == variant.settings_sha256;
            let valid = if scenario.subcase == Subcase::ConfigVariant {
                !unchanged_config && !unchanged_settings
            } else {
                unchanged_config && unchanged_settings
            };
            if !valid || (self.backend == Backend::StartupSnapshot && baseline.id == variant.id) {
                return Err(Error::InvalidProfile);
            }
        }
        Ok(())
    }

    fn validate_sdk(&self) -> Result<(), Error> {
        let sdk = &self.resources.sdk;
        if self.identity.target != Target::Axum {
            return Ok(());
        }
        let ingress = sdk.ingress.as_ref().ok_or(Error::InvalidProfile)?;
        let store = sdk.config_store.as_ref().ok_or(Error::InvalidProfile)?;
        let to_usize = |n| usize::try_from(n).map_err(|_error| Error::InvalidProfile);
        AxumIngressConfig::new(
            to_usize(u64::from(ingress.max_connections))?,
            to_usize(u64::from(ingress.max_raw_head_bytes))?,
            to_usize(u64::from(ingress.max_header_count))?,
            Duration::from_millis(ingress.head_read_timeout_ms),
        )
        .map_err(|_error| Error::InvalidProfile)?;
        ConfigStoreLimits::new(
            to_usize(store.max_file_bytes)?,
            to_usize(u64::from(store.max_entries))?,
            to_usize(u64::from(store.max_key_bytes))?,
            to_usize(store.max_value_bytes)?,
            to_usize(store.max_resident_bytes)?,
            to_usize(store.max_startup_bytes)?,
        )
        .map_err(|_error| Error::InvalidProfile)?;
        let target = self.resources.target_constants;
        if sdk.http2_enabled
            || ingress.max_connections > target.platform_concurrent_ceiling
            || ingress.max_raw_head_bytes > target.h1_max_buf_size_bytes
            || ingress.max_header_count > target.h1_max_headers
            || store.max_startup_bytes > self.resources.limits.baseline_bytes
        {
            return Err(Error::InvalidProfile);
        }
        Ok(())
    }
}

fn unique<T: PartialEq>(values: &[T]) -> bool {
    values.iter().enumerate().all(|(i, v)| {
        !values
            .iter()
            .skip(i.saturating_add(1))
            .any(|next| next == v)
    })
}

impl Scenario {
    fn validate(&self, profile: &Profile) -> Result<(), Error> {
        let l = &profile.resources.limits;
        let load = self.load;
        let o = &self.outcomes;
        let latency = &self.latency;
        if self.family != self.subcase.family()
            || self.fixture.id == 0
            || self.repetitions == 0
            || !profile.variants.iter().any(|v| v.id == self.variant_id)
            || !(1..=86_400_000).contains(&self.duration_ms)
            || !(1..=86_400_000).contains(&self.warmup_ms)
            || self.meter_window_ms == 0
            || self.meter_window_ms > self.duration_ms
            || load.concurrency == 0
            || load.concurrency > l.max_concurrent_auctions
            || load.calls == 0
            || load.calls > l.max_outbound_requests
            || load.request_bytes == 0
            || load.request_bytes > l.inbound_request_bytes
            || load.nodes == 0
            || load.nodes > l.max_parsed_nodes
            || load.impressions == 0
            || load.impressions > l.max_impressions_per_auction
            || load.encoded_bytes == 0
            || load.encoded_bytes > l.max_transport_bytes
            || load.decoded_bytes == 0
            || load.decoded_bytes > l.max_decoded_bytes
            || o.min_offered == 0
            || o.min_admitted == 0
            || o.min_admitted > o.min_offered
            || o.min_success > o.min_admitted
            || o.min_rejected > o.max_rejected
            || o.min_errors > o.max_errors
            || o.min_timeouts > o.max_timeouts
            || !self.effects.declares(self.subcase, profile)
            || (self.subcase == Subcase::ExcessAdmission && o.min_rejected == 0)
            || (self.subcase == Subcase::Cancellation && o.min_errors == 0)
            || (self.subcase == Subcase::Timeout && o.min_timeouts == 0)
            || (self.subcase == Subcase::MixedOutcomes
                && (o.min_success == 0 || o.min_errors == 0 || o.min_timeouts == 0))
            || (self.family != Family::SaturationRecovery
                && self.subcase != Subcase::MixedOutcomes
                && o.min_success == 0)
            || !(1..=100).contains(&latency.percentile)
            || latency.threshold_us == 0
            || !(1..=32).contains(&latency.max_histogram_buckets)
            || latency.population == Population::SuccessOnly
            || (latency.start == LatencyStart::Admission
                && latency.population != Population::AllAdmitted)
            || self.max_peak_bytes == 0
            || self.max_peak_bytes > profile.platform.memory_bytes
            || self.max_retained_growth_bytes > self.max_peak_bytes
            || (self.maximum_mode == MaximumMode::ReviewedNonSimultaneous)
                != self.justification_sha256.is_some()
        {
            return Err(Error::InvalidProfile);
        }
        self.validate_maxima(profile)
    }

    fn validate_maxima(&self, profile: &Profile) -> Result<(), Error> {
        let l = &profile.resources.limits;
        let load = self.load;
        if self.subcase == Subcase::MaximumShape
            && (load.concurrency != l.max_concurrent_auctions
                || load.calls != l.max_outbound_requests
                || load.request_bytes != l.inbound_request_bytes
                || load.nodes != l.max_parsed_nodes
                || load.impressions != l.max_impressions_per_auction)
        {
            return Err(Error::InvalidProfile);
        }
        if self.subcase == Subcase::EncodedDecoded
            && (load.encoded_bytes != l.max_transport_bytes
                || load.decoded_bytes != l.max_decoded_bytes)
        {
            return Err(Error::InvalidProfile);
        }
        if matches!(
            self.subcase,
            Subcase::Permitted | Subcase::ConcurrentCapture | Subcase::CaptureRelease
        ) && (load.calls != l.max_outbound_requests
            || load.decoded_bytes
                != l.max_decoded_bytes
                    .min(auctioneer_runtime::MAX_DEBUG_INPUT_BYTES))
        {
            return Err(Error::InvalidProfile);
        }
        match (&self.debug_workload, self.subcase) {
            (
                Some(debug),
                Subcase::Permitted | Subcase::ConcurrentCapture | Subcase::CaptureRelease,
            ) => debug.validate(self, profile)?,
            (None, Subcase::Permitted | Subcase::ConcurrentCapture | Subcase::CaptureRelease)
            | (Some(_), _) => return Err(Error::InvalidProfile),
            (None, _) => {}
        }
        if self.subcase == Subcase::ConcurrentCapture
            && load.concurrency != l.max_concurrent_auctions
        {
            return Err(Error::InvalidProfile);
        }
        // Restart repetitions use distinct variant IDs so every cold/config
        // sample binds a fresh startup identity and independent meter windows.
        if profile.backend == Backend::StartupSnapshot
            && matches!(
                self.subcase,
                Subcase::ColdStartup | Subcase::ConfigVariant | Subcase::ConfigRestoration
            )
            && self.repetitions != 1
        {
            return Err(Error::InvalidProfile);
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use serde_json::{Value, json};

    pub(crate) const SUBCASES: &[(&str, &str)] = &[
        ("baseline", "cold_startup"),
        ("baseline", "warm_baseline"),
        ("baseline", "gvl_cold_parse"),
        ("maximum_admitted", "maximum_shape"),
        ("maximum_admitted", "fpd_expansion"),
        ("decode_overlap", "encoded_decoded"),
        ("decode_overlap", "slow_reader"),
        ("decode_overlap", "mixed_outcomes"),
        ("debug", "unrequested"),
        ("debug", "denied"),
        ("debug", "permitted"),
        ("debug", "concurrent_capture"),
        ("debug", "capture_release"),
        ("auxiliary_config", "auxiliary_overlap"),
        ("auxiliary_config", "config_snapshot"),
        ("auxiliary_config", "gvl_miss_overlap"),
        ("auxiliary_config", "config_variant"),
        ("auxiliary_config", "config_restoration"),
        ("saturation_recovery", "excess_admission"),
        ("saturation_recovery", "outbound_queue"),
        ("saturation_recovery", "stalled_io"),
        ("saturation_recovery", "cancellation"),
        ("saturation_recovery", "timeout"),
        ("saturation_recovery", "healthy_recovery"),
    ];

    pub(crate) fn digest() -> String {
        "a".repeat(64)
    }

    /// Construct current test input, not a recapture or a production migration.
    pub(crate) fn synthetic_profile(bytes: &[u8]) -> super::Profile {
        let mut value: Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(value["schema_version"], 1_u16);
        value["schema_version"] = json!(2_u16);
        assert!(
            value["resources"]
                .as_object_mut()
                .unwrap()
                .insert(
                    "catalog_costs".to_owned(),
                    json!({"resident_bytes":0_u64,"initialization_bytes":0_u64,
                "schema_validation_per_auction_bytes":0_u64,"context_per_auction_bytes":0_u64}),
                )
                .is_none()
        );
        let resources: super::ResourceExport =
            serde_json::from_value(value["resources"].clone()).unwrap();
        let digest = super::Digest::canonical(&resources).unwrap();
        for scenario in value["scenarios"].as_array_mut().unwrap() {
            if let Some(workload) = scenario["debug_workload"].as_object_mut() {
                workload.get_mut("reference").unwrap()["resources_sha256"] = json!(digest);
                let rebound = super::Digest::canonical(&workload["reference"]).unwrap();
                workload.insert("reference_sha256".to_owned(), json!(rebound));
            }
        }
        serde_json::from_value(value).unwrap()
    }

    fn catalog_candidate(costs: Value) -> Value {
        let mut value = candidate();
        value["schema_version"] = json!(2_u16);
        value["resources"]["catalog_costs"] = costs;
        debug_references(&mut value);
        value
    }

    #[test]
    fn resource_protocol_round_trips_each_catalog_cost_and_its_owning_term() {
        let zero = json!({"resident_bytes": 0_u64, "initialization_bytes": 0_u64,
            "schema_validation_per_auction_bytes": 0_u64, "context_per_auction_bytes": 0_u64});
        let baseline =
            super::parse(&serde_json::to_vec(&catalog_candidate(zero.clone())).unwrap()).unwrap();
        let baseline_derived = baseline.derive().unwrap();
        let baseline_digest = super::Digest::canonical(&baseline.resources).unwrap();
        for (field, bytes) in [
            ("resident_bytes", 101_u64),
            ("initialization_bytes", 211),
            ("schema_validation_per_auction_bytes", 307),
            ("context_per_auction_bytes", 401),
        ] {
            let mut costs = zero.clone();
            costs[field] = json!(bytes);
            let value = catalog_candidate(costs.clone());
            let parsed = super::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
            assert_eq!(serde_json::to_value(&parsed).unwrap(), value);
            assert_ne!(
                super::Digest::canonical(&parsed.resources).unwrap(),
                baseline_digest
            );
            let derived = parsed.derive().unwrap();
            assert_eq!(derived.total_bytes, baseline_derived.total_bytes + bytes);
            assert_eq!(derived.catalog_resident_term, costs["resident_bytes"]);
            assert_eq!(
                derived.catalog_initialization_term,
                costs["initialization_bytes"]
            );
            assert_eq!(
                derived.schema_validation_term,
                costs["schema_validation_per_auction_bytes"]
            );
            assert_eq!(
                derived.catalog_context_term,
                costs["context_per_auction_bytes"]
            );
        }
    }

    #[test]
    fn resource_protocol_requires_the_complete_explicit_catalog_descriptor() {
        let value = catalog_candidate(
            json!({"resident_bytes": 101_u64, "initialization_bytes": 211_u64,
            "schema_validation_per_auction_bytes": 307_u64, "context_per_auction_bytes": 401_u64}),
        );
        super::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        for field in [
            "resident_bytes",
            "initialization_bytes",
            "schema_validation_per_auction_bytes",
            "context_per_auction_bytes",
        ] {
            let mut missing = value.clone();
            assert!(
                missing["resources"]["catalog_costs"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field)
                    .is_some()
            );
            assert_eq!(
                super::parse(&serde_json::to_vec(&missing).unwrap()).unwrap_err(),
                super::Error::InvalidJson
            );
            for wrong in [Value::Null, json!(-1_i64), json!(1.0_f64), json!("1")] {
                let mut malformed = value.clone();
                malformed["resources"]["catalog_costs"][field] = wrong;
                assert_eq!(
                    super::parse(&serde_json::to_vec(&malformed).unwrap()).unwrap_err(),
                    super::Error::InvalidJson
                );
            }
            let encoded = serde_json::to_string(&value).unwrap();
            let key = format!("\"{field}\":");
            let duplicate = encoded.replacen(&key, &format!("\"{field}\":0,{key}"), 1);
            assert_ne!(duplicate, encoded);
            assert_eq!(
                super::parse(duplicate.as_bytes()).unwrap_err(),
                super::Error::InvalidJson
            );
        }
        for version in [0_u16, 1, 3, u16::MAX] {
            let mut old = value.clone();
            old["schema_version"] = json!(version);
            assert_eq!(
                super::parse(&serde_json::to_vec(&old).unwrap()).unwrap_err(),
                super::Error::InvalidProfile
            );
        }
        let mut missing = value.clone();
        assert!(
            missing["resources"]
                .as_object_mut()
                .unwrap()
                .remove("catalog_costs")
                .is_some()
        );
        assert_eq!(
            super::parse(&serde_json::to_vec(&missing).unwrap()).unwrap_err(),
            super::Error::InvalidJson
        );
        let mut unknown = value.clone();
        unknown["resources"]["catalog_costs"]["unknown"] = json!(0_u64);
        assert_eq!(
            super::parse(&serde_json::to_vec(&unknown).unwrap()).unwrap_err(),
            super::Error::InvalidJson
        );
        let mut positional = value.clone();
        positional["resources"]["catalog_costs"] = json!([101_u64, 211_u64, 307_u64, 401_u64]);
        assert_eq!(
            super::parse(&serde_json::to_vec(&positional).unwrap()).unwrap_err(),
            super::Error::InvalidJson
        );
        let mut overflow = value;
        overflow["resources"]["catalog_costs"]["resident_bytes"] = json!(u64::MAX);
        assert_eq!(
            super::parse(&serde_json::to_vec(&overflow).unwrap()).unwrap_err(),
            super::Error::InvalidProfile
        );
    }

    pub(crate) fn candidate() -> Value {
        let d = digest();
        let version = json!({"major": 1_i32, "minor": 0_i32, "patch": 0_i32});
        let load = json!({"concurrency":1_i32,"calls":1_i32,"request_bytes":1_024_i32,"nodes":32_i32,
            "impressions":1_i32,"encoded_bytes":1_024_i32,"decoded_bytes":1_024_i32});
        let scenarios: Vec<_> = SUBCASES.iter().map(|(family, subcase)| json!({
            "family":family,"subcase":subcase,"variant_id": if *subcase == "config_variant" {2_i32} else if *subcase == "config_restoration" {3_i32} else {1_i32},
            "fixture":{"id":1_i32,"sha256":d},"seed":7_i32,"repetitions":1_i32,
            "warmup_ms":10_i32,"duration_ms":100_i32,"meter_window_ms":100_i32,"load":load,
            "outcomes":{"min_offered":10_i32,"min_admitted":10_i32,"min_success":10_i32,
                "max_rejected":0_i32,"max_errors":0_i32,"max_timeouts":0_i32},
            "latency":{"start":"client_dispatch","end":"terminal_outcome", "population":"all_offered", "percentile":99_i32,"threshold_us":10_000_i32},
            "effects":effects(subcase),"max_peak_bytes":100_000_000_i32,"max_retained_growth_bytes":1_024_i32,
            "maximum_mode":"simultaneous","justification_sha256":null
        })).collect();
        let mut p = json!({"schema_version":2_i32,"scope":"deployment",
            "identity":{"auctioneer_commit":"b".repeat(40),"artifact_sha256":d,"build_features":[],
                "edgezero_pin":"c".repeat(40),"target":"axum","runtime_version":version,
                "harness_sha256":d,"fixtures_sha256":d,"reference_data_sha256":d,
                "baked_config_sha256":d},
            "resources":{"limits":{
                "isolate_total_bytes":5_000_000_000_u64,"baseline_bytes":160_000_000_i32,
                "inbound_request_bytes":1_024_i32,"inbound_parse_factor":36_i32,
                "max_outbound_request_bytes":1_024_i32,"max_outbound_raw_bytes":1_024_i32,
                "max_transport_bytes":1_024_i32,"max_decoded_bytes":1_024_i32,
                "response_parse_factor":256_i32,"response_node_peak_bytes":320_i32,"final_response_bytes":3_072_i32,
                "egress_permits":1_i32,"response_write_deadline_ms":30_000_i32,"guest_decoder_bound_bytes":0_i32,
                "aux_route_permits":1_i32,"aux_route_peak_bytes":800_000_000_i32,
                "max_brotli_wbits":10_i32,"max_brotli_layers":1_i32,"gvl_cache_entries":4_i32,"gvl_parsed_bytes":512_000_i32,
                "max_bidders_per_auction":1_i32,"max_outbound_requests":1_i32,"max_concurrent_auctions":1_i32,
                "max_active_body_streams":1_i32,"max_runtime_config_bytes":0x0010_0000_i32,"max_runtime_config_blob_bytes":0x0010_0000_i32,
                "max_parsed_nodes":32_i32,"max_copied_header_bytes":4_096_i32,"max_admitted_header_bytes":0x4000_i32,
                "max_admitted_header_count":128_i32,"max_impressions_per_auction":1_i32,"max_config_json_nodes":8_192_i32,
                "terminal_response_permits":384_i32,"refusal_body_bytes":256_i32,"transmit_state_bytes":4_096_i32},
                "target_constants":{"platform_concurrent_ceiling":384_i32,"header_entry_overhead_bytes":64_i32,
                    "parser_header_ceiling_bytes":21_632_i32,"parser_header_knob_bytes":21_633_i32,"max_request_target_bytes":768_i32,
                    "h1_max_buf_size_bytes":24_576_i32,"h1_max_headers":128_i32,"parser_max_header_entries":676_i32,
                    "pseudoheader_allowance_bytes":1_024_i32,"parser_transient_bytes":0x4000_i32,"egress_handoff_factor":2_i32},
                "construction_costs":{"fixed_per_auction_bytes":100_i32,"reusable_scratch_bytes":200_i32,"retained_per_scope_bytes":78_i32},
                "catalog_costs":{"resident_bytes":0_u64,"initialization_bytes":0_u64,
                    "schema_validation_per_auction_bytes":0_u64,"context_per_auction_bytes":0_u64},
                "outbound_meta_bytes":4_384_i32,"baked_config_bytes":4_096_i32,"actual_gvl_cache_entries":4_i32,
                "sdk":{"ingress":{"max_connections":256_i32,"max_raw_head_bytes":0x4000_i32,"max_header_count":100_i32,"head_read_timeout_ms":10_000_i32},
                    "config_store":{"max_entries":1_024_i32,"max_file_bytes":0x0100_0000_i32,"max_key_bytes":1_024_i32,
                        "max_value_bytes":0x0080_0000_i32,"max_resident_bytes":0x0200_0000_i32,"max_startup_bytes":0x0700_0000_i32},"http2_enabled":false}},
            "platform":{"memory_bytes":5_000_000_000_u64,"concurrent_requests":384_i32,"wall_time_ms":30_000_i32,"cpu_time_ms":30_000_i32},
            "hardware":{"architecture":"aarch64","cpu":"apple_m_series","logical_cores":8_i32,"memory_bytes":0x0002_0000_0000_u64,
                "page_bytes":0x4000_i32,"os":"macos","os_version":version,"meter":"process_rss","meter_version":version,"machine_sha256":d},
            "headroom_bytes":312_500_000_u64,"backend":"startup_snapshot",
            "variants":[{"id":1_i32,"config_sha256":d,"settings_sha256":d},{"id":2_i32,"config_sha256":"b".repeat(64),"settings_sha256":"b".repeat(64)},
                {"id":3_i32,"config_sha256":d,"settings_sha256":d}],
            "required_capabilities":["ingress_admission","config_pre_extraction","transport_lifecycle","delivery_lifetime","deadline","cancellation"],
            "scenarios":scenarios});
        for s in p["scenarios"].as_array_mut().unwrap() {
            s["outcomes"]["min_rejected"] = json!(0_i32);
            s["outcomes"]["min_errors"] = json!(0_i32);
            s["outcomes"]["min_timeouts"] = json!(0_i32);
            match s["subcase"].as_str().unwrap() {
                "excess_admission" => {
                    s["outcomes"]["min_offered"] = json!(11_i32);
                    s["outcomes"]["min_rejected"] = json!(1_i32);
                    s["outcomes"]["max_rejected"] = json!(1_i32);
                }
                "cancellation" | "mixed_outcomes" => {
                    s["outcomes"]["min_success"] = json!(8_i32);
                    s["outcomes"]["min_errors"] = json!(1_i32);
                    s["outcomes"]["max_errors"] = json!(1_i32);
                    if s["subcase"] == "mixed_outcomes" {
                        s["outcomes"]["min_timeouts"] = json!(1_i32);
                        s["outcomes"]["max_timeouts"] = json!(1_i32);
                    }
                }
                "timeout" => {
                    s["outcomes"]["min_success"] = json!(9_i32);
                    s["outcomes"]["min_timeouts"] = json!(1_i32);
                    s["outcomes"]["max_timeouts"] = json!(1_i32);
                }
                _ => {}
            }
        }
        for v in p["variants"].as_array_mut().unwrap() {
            v["cold_window_ms"] = json!(100_i32);
            v["warm_window_ms"] = json!(100_i32);
            v["max_cold_peak_bytes"] = json!(100_000_000_i32);
            v["max_warm_peak_bytes"] = json!(100_000_000_i32);
        }
        for s in p["scenarios"].as_array_mut().unwrap() {
            s["latency"]["max_histogram_buckets"] = json!(1_u8);
            s["debug_workload"] = Value::Null;
        }
        debug_references(&mut p);
        p
    }

    pub(crate) fn candidate_with_call_limit(limit: u32) -> Value {
        let mut p = candidate();
        p["resources"]["limits"]["max_outbound_requests"] = json!(limit);
        // Each declared call needs the synthetic fixture's 3-KiB assembly floor.
        p["resources"]["limits"]["final_response_bytes"] =
            json!(3_072_u64.checked_mul(u64::from(limit)).unwrap());
        for scenario in p["scenarios"].as_array_mut().unwrap() {
            if matches!(
                scenario["subcase"].as_str().unwrap(),
                "maximum_shape" | "permitted" | "concurrent_capture" | "capture_release"
            ) {
                scenario["load"]["calls"] = json!(limit);
            }
        }
        debug_references(&mut p);
        p
    }

    pub(crate) fn debug_references(p: &mut Value) {
        let resources: super::ResourceExport =
            serde_json::from_value(p["resources"].clone()).unwrap();
        let resource_digest = super::Digest::canonical(&resources).unwrap();
        for s in p["scenarios"].as_array_mut().unwrap() {
            if !matches!(
                s["subcase"].as_str().unwrap(),
                "permitted" | "concurrent_capture" | "capture_release"
            ) {
                continue;
            }
            let reference = json!({"fixture":s["fixture"],"resources_sha256":resource_digest,
                "review_sha256":digest(),"limiting_factor":"fixture_shape",
                "limits":super::DebugLimits::effective(&resources),"min_call_rows":1_u32,"max_call_rows":1_u32,
                "capture_bytes":{"min":512_u64,"max":512_u64},"output_bytes":{"min":1_024_u64,"max":1_024_u64}});
            s["debug_workload"] = json!({"reference_sha256":super::Digest::canonical(&reference).unwrap(),"reference":reference});
            let concurrency = s["load"]["concurrency"].as_u64().unwrap();
            s["effects"]["debug_captures"] = json!(concurrency);
            s["effects"]["debug_output_bytes"] = json!(1_024_u64.saturating_mul(concurrency));
        }
    }

    #[test]
    fn qualification_validates_complete_candidate() {
        super::parse(&serde_json::to_vec(&candidate()).unwrap()).unwrap();
    }

    #[test]
    fn qualification_profiles_require_target_specific_headroom_at_the_effective_ceiling() {
        for (target, configured, platform, floor) in [
            (
                "axum",
                4_000_000_000_u64,
                5_000_000_000_u64,
                0x1000_0000_u64,
            ),
            ("axum", 0x0001_0000_0001, 4_294_967_329, 0x1000_0001),
            ("axum", 1_000_000_000_000, 1_000_000_000_000, 62_500_000_000),
            ("cloudflare", 100_000_001, 100_000_017, 6_250_001),
            ("fastly", 100_000_001, 100_000_017, 6_250_001),
            ("spin", 100_000_001, 100_000_017, 6_250_001),
        ] {
            let mut value = candidate();
            value["identity"]["target"] = json!(target);
            value["resources"]["limits"]["isolate_total_bytes"] = json!(configured);
            value["platform"]["memory_bytes"] = json!(platform);
            value["hardware"]["memory_bytes"] = json!(platform.max(0x0002_0000_0000));
            if target != "axum" {
                value["hardware"]["architecture"] = json!("wasm32");
                value["hardware"]["cpu"] = json!("provider_managed");
                value["hardware"]["os"] = json!("provider");
                value["hardware"]["meter"] = json!("wasm_guest");
            }
            debug_references(&mut value);
            for (headroom, expected) in [
                (floor - 1, Err(super::Error::InvalidProfile)),
                (floor, Ok(())),
                (floor + 1, Ok(())),
            ] {
                value["headroom_bytes"] = json!(headroom);
                assert_eq!(
                    super::parse(&serde_json::to_vec(&value).unwrap()).map(|_| ()),
                    expected,
                    "{target}, configured {configured}, platform {platform}, headroom {headroom}"
                );
            }
        }
    }

    #[test]
    fn qualification_planned_calls_have_an_independent_installed_ceiling() {
        let p = candidate_with_call_limit(4);
        super::parse(&serde_json::to_vec(&p).unwrap()).unwrap();
        for (calls, expected) in [
            (0_u32, Err(super::Error::InvalidProfile)),
            (1, Ok(())),
            (2, Ok(())),
            (3, Ok(())),
            (4, Ok(())),
            (5, Err(super::Error::InvalidProfile)),
        ] {
            let mut changed = p.clone();
            changed["scenarios"][0]["load"]["calls"] = json!(calls);
            assert_eq!(
                super::parse(&serde_json::to_vec(&changed).unwrap()).map(|_profile| ()),
                expected,
                "calls {calls}, installed ceiling 4",
            );
        }
    }

    #[test]
    fn qualification_build_features_accept_only_unique_known_modes() {
        let modes = [
            "insecure_bidders",
            "conformance_suite",
            "test_harness_override",
        ];
        for mask in 0_u8..8 {
            let features: Vec<_> = modes
                .iter()
                .enumerate()
                .filter_map(|(bit, mode)| ((mask & (1 << bit)) != 0).then_some(*mode))
                .collect();
            let mut p = candidate();
            p["identity"]["build_features"] = json!(features);
            let parsed = super::parse(&serde_json::to_vec(&p).unwrap()).unwrap();
            assert_eq!(
                serde_json::to_value(parsed.identity.build_features).unwrap(),
                json!(features)
            );
            for mode in features {
                let mut duplicate = p.clone();
                duplicate["identity"]["build_features"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(mode));
                assert!(matches!(
                    super::parse(&serde_json::to_vec(&duplicate).unwrap()),
                    Err(super::Error::InvalidProfile)
                ));
            }
        }
        let mut p = candidate();
        p["identity"]["build_features"] = json!(["future_mode"]);
        assert!(matches!(
            super::parse(&serde_json::to_vec(&p).unwrap()),
            Err(super::Error::InvalidJson)
        ));
    }

    #[test]
    fn qualification_effective_debug_call_rows_respects_resource_limit_and_cap() {
        let mut resources: super::ResourceExport =
            serde_json::from_value(candidate()["resources"].clone()).unwrap();
        for (limit, expected) in [(63, 63), (64, 64), (65, 64)] {
            resources.limits.max_outbound_requests = limit;
            assert_eq!(
                super::DebugLimits::effective(&resources),
                super::DebugLimits {
                    call_rows: expected,
                    input_bytes: 1_024,
                    body_nodes: 32,
                    body_bytes: 0x4000,
                    capture_bytes: 0x0001_0000,
                    output_bytes: 3_072,
                },
                "max_outbound_requests={limit}"
            );
        }
    }

    #[test]
    fn qualification_effective_debug_input_bytes_respects_resource_limit_and_cap() {
        let mut resources: super::ResourceExport =
            serde_json::from_value(candidate()["resources"].clone()).unwrap();
        for (limit, expected) in [
            (0x0000_ffff, 0x0000_ffff),
            (0x0001_0000, 0x0001_0000),
            (0x0001_0001, 0x0001_0000),
        ] {
            resources.limits.max_decoded_bytes = limit;
            assert_eq!(
                super::DebugLimits::effective(&resources),
                super::DebugLimits {
                    call_rows: 1,
                    input_bytes: expected,
                    body_nodes: 32,
                    body_bytes: 0x4000,
                    capture_bytes: 0x0001_0000,
                    output_bytes: 3_072,
                },
                "max_decoded_bytes={limit}"
            );
        }
    }

    #[test]
    fn qualification_effective_debug_body_nodes_respects_resource_limit_and_cap() {
        let mut resources: super::ResourceExport =
            serde_json::from_value(candidate()["resources"].clone()).unwrap();
        for (limit, expected) in [(4_095, 4_095), (4_096, 4_096), (4_097, 4_096)] {
            resources.limits.max_parsed_nodes = limit;
            assert_eq!(
                super::DebugLimits::effective(&resources),
                super::DebugLimits {
                    call_rows: 1,
                    input_bytes: 1_024,
                    body_nodes: expected,
                    body_bytes: 0x4000,
                    capture_bytes: 0x0001_0000,
                    output_bytes: 3_072,
                },
                "max_parsed_nodes={limit}"
            );
        }
    }

    #[test]
    fn qualification_effective_debug_output_bytes_respects_resource_limit_and_cap() {
        let mut resources: super::ResourceExport =
            serde_json::from_value(candidate()["resources"].clone()).unwrap();
        for (limit, expected) in [
            (0x0000_ffff, 0x0000_ffff),
            (0x0001_0000, 0x0001_0000),
            (0x0001_0001, 0x0001_0000),
        ] {
            resources.limits.final_response_bytes = limit;
            assert_eq!(
                super::DebugLimits::effective(&resources),
                super::DebugLimits {
                    call_rows: 1,
                    input_bytes: 1_024,
                    body_nodes: 32,
                    body_bytes: 0x4000,
                    capture_bytes: 0x0001_0000,
                    output_bytes: expected,
                },
                "final_response_bytes={limit}"
            );
        }
    }

    #[test]
    fn qualification_maximum_debug_requires_a_predeclared_fixture_bound_reference() {
        let mut p = candidate();
        for s in p["scenarios"].as_array_mut().unwrap() {
            s.as_object_mut().unwrap().remove("debug_workload");
        }
        super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err();
    }

    #[test]
    fn qualification_scheduled_rows_are_bounded_at_sixty_four() {
        let mut p = candidate();
        p["scenarios"][1]["repetitions"] = json!(41_u8);
        assert_eq!(
            super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err(),
            super::Error::ProfileCapacity
        );
        p["scenarios"][1]["repetitions"] = json!(42_u8);
        assert_eq!(
            super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err(),
            super::Error::InvalidProfile
        );
    }

    pub(crate) fn repeated_candidate() -> Value {
        let mut p = candidate();
        let mut restarts = Vec::new();
        let mut variants = Vec::new();
        for s in p["scenarios"].as_array_mut().unwrap() {
            if matches!(
                s["subcase"].as_str().unwrap(),
                "cold_startup" | "config_variant" | "config_restoration"
            ) {
                let mut next = s.clone();
                let id = s["variant_id"].as_u64().unwrap().saturating_add(3);
                next["variant_id"] = json!(id);
                restarts.push(next);
                variants.push((s["variant_id"].as_u64().unwrap(), id));
            } else {
                s["repetitions"] = json!(2_u8);
            }
        }
        for (source, id) in variants {
            let mut v = p["variants"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == source)
                .unwrap()
                .clone();
            v["id"] = json!(id);
            p["variants"].as_array_mut().unwrap().push(v);
        }
        p["scenarios"].as_array_mut().unwrap().extend(restarts);
        p
    }

    #[test]
    fn qualification_maximum_debug_references_reject_tiny_or_unbound_substitutes() {
        for path in [
            "/fixture/sha256",
            "/resources_sha256",
            "/limits/input_bytes",
            "/output_bytes/min",
            "/capture_bytes/max",
        ] {
            let mut p = candidate();
            let reference = &mut p["scenarios"][10]["debug_workload"]["reference"];
            *reference.pointer_mut(path).unwrap() = match path {
                "/fixture/sha256" | "/resources_sha256" => json!("b".repeat(64)),
                "/capture_bytes/max" => json!(u64::MAX),
                _ => json!(1_u64),
            };
            let hash = super::Digest::canonical(reference).unwrap();
            p["scenarios"][10]["debug_workload"]["reference_sha256"] = json!(hash);
            assert!(
                super::parse(&serde_json::to_vec(&p).unwrap()).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn qualification_maximum_debug_can_have_no_body_content_under_narrow_limits() {
        let mut p = candidate();
        p["resources"]["limits"]["max_decoded_bytes"] = json!(1_u64);
        for s in p["scenarios"].as_array_mut().unwrap() {
            s["load"]["decoded_bytes"] = json!(1_u64);
        }
        debug_references(&mut p);
        for s in p["scenarios"].as_array_mut().unwrap() {
            if let Some(workload) = s.get_mut("debug_workload").filter(|value| !value.is_null()) {
                workload["reference"]["capture_bytes"] = json!({"min":0_u64,"max":0_u64});
                workload["reference"]["output_bytes"] = json!({"min":256_u64,"max":256_u64});
                workload["reference_sha256"] =
                    json!(super::Digest::canonical(&workload["reference"]).unwrap());
                s["effects"]["debug_output_bytes"] = json!(256_u64);
            }
        }
        super::parse(&serde_json::to_vec(&p).unwrap()).unwrap();
    }

    #[test]
    fn qualification_restart_subcases_bind_one_fresh_process_per_variant() {
        for subcase in ["cold_startup", "config_variant", "config_restoration"] {
            let mut p = candidate();
            let row = p["scenarios"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|s| s["subcase"] == subcase)
                .unwrap();
            row["repetitions"] = json!(2_u8);
            super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err();
        }
    }

    #[test]
    fn qualification_every_restoration_targets_the_known_good_snapshot() {
        let mut p = candidate();
        let mut variant = p["variants"][1].clone();
        variant["id"] = json!(4_u16);
        p["variants"].as_array_mut().unwrap().push(variant);
        let mut restoration = p["scenarios"][17].clone();
        restoration["variant_id"] = json!(4_u16);
        p["scenarios"].as_array_mut().unwrap().push(restoration);
        super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err();
    }

    #[test]
    fn qualification_allocator_usage_cannot_stand_in_for_native_process_memory() {
        let mut p = candidate();
        p["hardware"]["meter"] = json!("allocator");
        super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err();
    }

    #[test]
    fn qualification_debug_capture_targets_the_funded_call_and_concurrency_width() {
        let mut p = super::parse(&serde_json::to_vec(&candidate()).unwrap()).unwrap();
        p.resources.limits.max_outbound_requests = 2;
        let permitted = p
            .scenarios
            .iter()
            .find(|s| s.subcase == super::Subcase::Permitted)
            .unwrap();
        permitted.validate(&p).unwrap_err();
        p.resources.limits.max_outbound_requests = 1;
        p.resources.limits.max_concurrent_auctions = 2;
        let mut concurrent = p
            .scenarios
            .iter()
            .find(|s| s.subcase == super::Subcase::ConcurrentCapture)
            .unwrap()
            .clone();
        concurrent.effects.debug_captures = 2;
        concurrent.validate(&p).unwrap_err();
    }

    #[test]
    fn qualification_rejects_unsafe_identities_and_invalid_thresholds() {
        for (path, value) in [
            ("/identity/artifact_sha256", json!("SECRET_CANARY")),
            ("/identity/build_features", json!(["SECRET_CANARY"])),
            ("/hardware/cpu", json!("SECRET_CANARY")),
            ("/hardware/architecture", json!("SECRET_CANARY")),
            ("/hardware/meter", json!("SECRET_CANARY")),
            ("/scenarios/0/latency/threshold_us", json!(0_i32)),
            ("/scenarios/0/latency/threshold_us", json!(-1_i32)),
            ("/scenarios/0/latency/threshold_us", json!(1.5_f64)),
            ("/scenarios/0/latency/population", json!("success_only")),
            ("/scenarios/0/latency/percentile", json!(101_i32)),
            ("/scenarios/0/duration_ms", json!(0_i32)),
            ("/scenarios/0/repetitions", json!(0_i32)),
            ("/resources/limits/gvl_cache_entries", json!(3_i32)),
            ("/resources/actual_gvl_cache_entries", json!(3_i32)),
        ] {
            let mut p = candidate();
            *p.pointer_mut(path).unwrap() = value;
            assert!(
                super::parse(&serde_json::to_vec(&p).unwrap()).is_err(),
                "{path}"
            );
        }
        for value in ["NaN", "Infinity", "1e999"] {
            let bytes = serde_json::to_vec(&candidate()).unwrap();
            let text = String::from_utf8(bytes).unwrap().replace(
                "\"threshold_us\":10000",
                &format!("\"threshold_us\":{value}"),
            );
            super::parse(text.as_bytes()).unwrap_err();
        }
    }

    #[test]
    fn qualification_rejects_missing_and_duplicate_scenarios() {
        for index in 0..SUBCASES.len() {
            let mut p = candidate();
            p["scenarios"].as_array_mut().unwrap().remove(index);
            super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err();
        }
        let mut p = candidate();
        let row = p["scenarios"][0].clone();
        p["scenarios"].as_array_mut().unwrap().push(row);
        super::parse(&serde_json::to_vec(&p).unwrap()).unwrap_err();
    }

    #[test]
    fn qualification_forbids_raw_payload_fields_at_every_input_boundary() {
        for path in [
            "",
            "/identity",
            "/hardware",
            "/resources",
            "/scenarios/10/debug_workload",
            "/scenarios/10/debug_workload/reference",
            "/scenarios/10/debug_workload/reference/limits",
            "/scenarios/10/debug_workload/reference/output_bytes",
            "/resources/limits",
            "/resources/sdk",
            "/scenarios/0",
            "/scenarios/0/fixture",
        ] {
            for field in [
                "raw_config",
                "body",
                "debug_payload",
                "error_message",
                "environment",
                "url",
                "SECRET_CANARY",
            ] {
                let mut p = candidate();
                p.pointer_mut(path)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(field.to_owned(), json!("SECRET_CANARY"));
                assert!(
                    super::parse(&serde_json::to_vec(&p).unwrap()).is_err(),
                    "{path}/{field}"
                );
            }
        }
    }

    #[test]
    fn qualification_requires_observable_native_snapshots_and_valid_sdk_settings() {
        for (path, value) in [
            ("/backend", json!("live_refresh")),
            ("/resources/sdk/ingress/max_connections", json!(0_i32)),
            ("/resources/sdk/ingress/max_connections", json!(385_i32)),
            (
                "/resources/sdk/ingress/max_raw_head_bytes",
                json!(0x0010_0001_i32),
            ),
            ("/resources/sdk/config_store/max_entries", json!(0_i32)),
            ("/resources/sdk/http2_enabled", json!(true)),
            ("/resources/sdk/ingress", Value::Null),
            ("/resources/sdk/config_store", Value::Null),
            ("/variants/1/config_sha256", json!(digest())),
        ] {
            let mut p = candidate();
            *p.pointer_mut(path).unwrap() = value;
            assert!(
                super::parse(&serde_json::to_vec(&p).unwrap()).is_err(),
                "{path}"
            );
        }
    }

    pub(crate) fn effects(subcase: &str) -> Value {
        let mut e = json!({"gvl_parses":0_i32,"fpd_expansions":0_i32,"decoded_responses":0_i32,
            "slow_reads":0_i32,"debug_captures":0_i32,"debug_denials":0_i32,"debug_output_bytes":0_i32,
            "auxiliary_requests":0_i32,"config_extractions":0_i32,"queued_calls":0_i32,"cancellations":0_i32,
            "stalled_reads":0_i32,"recovery_successes":0_i32});
        let key = match subcase {
            "cold_startup" | "config_snapshot" | "config_variant" | "config_restoration" => {
                "config_extractions"
            }
            "gvl_cold_parse" | "gvl_miss_overlap" => "gvl_parses",
            "fpd_expansion" => "fpd_expansions",
            "encoded_decoded" | "mixed_outcomes" => "decoded_responses",
            "slow_reader" => "slow_reads",
            "denied" => "debug_denials",
            "permitted" | "concurrent_capture" => "debug_captures",
            "auxiliary_overlap" => "auxiliary_requests",
            "outbound_queue" => "queued_calls",
            "stalled_io" => "stalled_reads",
            "cancellation" => "cancellations",
            "warm_baseline" | "capture_release" | "healthy_recovery" => "recovery_successes",
            _ => return e,
        };
        e[key] = json!(if subcase == "gvl_miss_overlap" {
            5_i32
        } else {
            1_i32
        });
        if subcase == "permitted" || subcase == "concurrent_capture" {
            e["debug_output_bytes"] = json!(1_i32);
        }
        e
    }
}
