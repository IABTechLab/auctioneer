//! Host-environment seams: `Clock` and `Rng` traits + deterministic test impls.

// The whole point of this crate is its re-export surface — every public name
// lives behind a single-letter file (`clock.rs`, `rng.rs`, ...) and is
// re-exported here so callers depend only on `auctioneer_runtime::Clock` etc.

pub mod admission;
/// Test-only allocation counting. A `#[global_allocator]` is a
/// per-binary singleton, so the gates' no-allocation assertions share
/// one definition here rather than each declaring their own.
#[cfg(test)]
mod alloc_track;
pub mod auction_budget;
pub mod aux_gate;
pub mod aux_work;
pub mod bidder_credentials;
pub mod body_caps;
pub mod bounded_json;
pub mod budget;
pub mod debug_budget;
pub mod diagnostic;
pub mod egress;
pub mod json_preflight;
pub mod static_egress;

mod clock;
mod fake_clock;
mod helpers;
mod json_size;
mod rng;
mod seeded_rng;
mod timer;

#[cfg(feature = "test-support")]
pub mod test_support;

/// Maximum complete liveness response, including a degraded-config diagnostic.
/// Probe output is funded against its independent response gate.
pub const PROBE_RESPONSE_BODY_MAX_BYTES: u64 = 8_192;

/// Reserved liveness outputs per process, including responses still being written.
/// Independent of bootstrap and auction/aux capacity; saturation aborts without a body.
pub const PROBE_RESPONSE_PERMITS: u32 = 4;

/// Liveness probes retain one counted config slot even when aux routes are disabled.
pub const PROBE_CONFIG_MIN_PERMITS: u32 = 1;

pub use admission::{
    AdmissionGate, AdmissionPermit, StreamAcquire, StreamGate, WaveError, WavePermit,
};
pub use auction_budget::{
    AuctionBudget, AuctionCharges, ChargeKind, Reservation, ResourceError,
    checked_allocation_capacity,
};
pub use aux_gate::{AuxGate, AuxPermit};
pub use aux_work::{
    MAX_CATALOG_IDENTITIES, MAX_SYNC_DEBUG_ROWS, MAX_SYNC_FILTER_NAMES_PER_LIST,
    MAX_SYNC_PRIORITY_ENTRIES, MAX_SYNC_PRIORITY_GROUPS, MAX_SYNC_REQUESTED_NAMES,
};
pub use bidder_credentials::{
    BidderCredentialSource, BidderCredentials, CredentialUnavailable, MAX_BIDDER_CREDENTIAL_BYTES,
};
pub use bounded_json::{
    BoundedJsonError, NODE_RETAINED_BYTES, NodeBudgetExceeded, UnbudgetableRawValue,
    count_json_nodes, from_slice_within, value_within,
};
pub use budget::{
    BudgetError, CatalogResourceCosts, Derived, HeadroomClass, IsolateTerm, IsolateTerms,
    LimitField, MAX_GVL_PARSED_BYTES, MAX_NUMBER_WIRE_BYTES, MIN_INBOUND_PARSE_FACTOR,
    MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NORMALIZATION_IMPLICIT_NODES,
    RequestConstructionCosts, ResolvedIsolateLimits, TargetPlatformConstants, brotli_state_bytes,
    derive_isolate_budget, derive_isolate_budget_with_baked_config, min_aux_route_peak_bytes,
    min_terminal_response_permits, minimum_headroom_bytes, normalization_wire_allowance,
    per_stream_bytes, validate_isolate_limits,
};
pub use clock::{Clock, MonotonicInstant};
pub use debug_budget::{
    MAX_DEBUG_BODY_DEPTH, MAX_DEBUG_BODY_NODES, MAX_DEBUG_CALL_ROWS, MAX_DEBUG_INPUT_BYTES,
    MAX_DEBUG_RETAINED_BYTES, MAX_DEBUG_SANITIZED_BODY_BYTES, MAX_DEBUG_SERIALIZED_BYTES,
    debug_peak_bytes,
};
pub use egress::{EgressGate, EgressPermit, PermitBody};
pub use fake_clock::FakeClock;
pub use helpers::{gen_range, shuffle};
pub use json_size::serialized_json_bytes;
pub use rng::Rng;
pub use seeded_rng::SeededRng;
pub use static_egress::{StaticEgressGate, StaticEgressPermit};
pub use timer::{DecodeCooperator, ImmediateResolveTimer, NoopDecodeCooperator, Timer};
