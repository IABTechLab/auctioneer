//! Whole-isolate memory budget: the arithmetic and the boot validator.
//!
//! An isolate has one memory ceiling and many independent consumers:
//! the inbound body and everything the parse expands it into,
//! retained split trees and one sequential bidder working set, measured
//! projection and privacy allowances, request-derived bookkeeping, one
//! outbound request and one retained
//! response body per call, the decoder state of every bidder response
//! a fanout wave decodes at once, the pre-admission header parse of
//! every connection the platform will admit, and the egress buffers.
//! This module states that sum once, as checked arithmetic over a
//! normalized limit set, and reports a configuration whose sum exceeds
//! the ceiling.
//!
//! What the CALLER does with that report is the caller's policy, not
//! this module's: `auctioneer-core`'s boot sequence warns on
//! [`BudgetError::BudgetOverCeiling`] and boots, because an
//! over-ceiling total says the declared caps permit a state that would
//! not fit rather than that the configuration is inconsistent. Every
//! other variant it treats as fatal. This module keeps returning the
//! same `Err` for both; it draws no such distinction itself.
//!
//! Everything here is pure: no auctioneer type crosses the boundary and
//! no platform API is called, so the same arithmetic runs on every
//! target. The measurements that justify the expansion factors live
//! with the types they measure — each factor constant below is
//! exported so the owning crate's allocator guard can assert its
//! observed value against the charged one.
//!
//! The two bidder-derived inputs arrive as scalars. `outbound_meta_bytes`
//! is computed by `auctioneer-bidder`, which owns the byte-pool layout, and
//! passed into [`validate_isolate_limits`] already finished: this crate
//! depends on no auctioneer crate and must never re-derive a layout it does
//! not own.

use crate::auction_budget::AuctionCharges;
use crate::aux_work::{
    AuxWorkDerived, AuxWorkError, AuxWorkLimits, MAX_CATALOG_IDENTITIES, MAX_SYNC_BODY_BYTES,
    MAX_SYNC_DEBUG_ROWS, MAX_SYNC_FILTER_NAMES_PER_LIST, MAX_SYNC_OUTPUT_BYTES,
    MAX_SYNC_PRIORITY_ENTRIES, MAX_SYNC_PRIORITY_GROUPS, MAX_SYNC_QUERY_BYTES,
    MAX_SYNC_QUERY_PARAMETERS, MAX_SYNC_REQUESTED_NAMES, MAX_SYNC_RESPONSE_HEADER_BYTES,
    MAX_SYNC_RESPONSE_HEADER_COUNT, MAX_SYNC_SET_COOKIE_BYTES, MAX_SYNC_URL_BYTES, derive_aux_work,
};
use crate::bounded_json::NODE_RETAINED_BYTES;
use crate::debug_budget::debug_peak_bytes;
use std::error::Error as StdError;
use std::fmt;
use std::num::NonZeroU64;

/// Target class for the shared minimum-headroom policy, not boot admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadroomClass {
    Native,
    Wasm,
}

/// Minimum headroom for a known, nonzero memory ceiling.
/// Native targets reserve at least 256 MiB; both classes round one sixteenth
/// of the ceiling upward without adding to the ceiling itself.
#[must_use]
pub const fn minimum_headroom_bytes(ceiling: NonZeroU64, class: HeadroomClass) -> u64 {
    let fraction = ceiling.get().div_ceil(16);
    match class {
        HeadroomClass::Native if fraction < 0x1000_0000 => 0x1000_0000,
        HeadroomClass::Native | HeadroomClass::Wasm => fraction,
    }
}

/// Fixed heap a `brotli` decoder instance charges beyond its ring
/// buffer, from upstream's audited figure. The window is charged
/// separately by [`brotli_state_bytes`].
pub const BROTLI_DECODER_FIXED_CHARGE_BYTES: u64 = 0x0100_0000;

/// Smallest RFC 7932 window exponent. Below this a stream is not a
/// valid brotli stream, so a budget derived from it would be fiction.
pub const MIN_BROTLI_WBITS: u8 = 10;

/// Largest window exponent auctioneer decodes. Large-window streams
/// (RFC 7932 §9.2, wbits > 24) are rejected before decoder allocation.
pub const MAX_BROTLI_WBITS: u8 = 24;

/// Decode layers a stacked `Content-Encoding` may carry.
pub const LAYERS_MAX: u8 = 3;

/// The same limit in the width the charge arithmetic speaks. `u64::from`
/// is not callable in a `const fn` on stable, so the twin is written
/// once and pinned to its `u8` original below.
const LAYERS_MAX_U64: u64 = 3;

/// How many of those layers may be brotli. The chain charge allocates
/// one brotli state and fills the remaining layers with the far
/// cheaper flate2 state.
pub const MAX_BROTLI_LAYERS: u8 = 1;

/// Layers the chain funds with flate2 state: every slot brotli does not
/// take, at the widths the arithmetic uses.
const FLATE2_SLOTS_U64: u64 = 2;

const _: () = assert!(
    LAYERS_MAX_U64 == 3 && LAYERS_MAX == 3 && MAX_BROTLI_LAYERS == 1 && FLATE2_SLOTS_U64 == 2,
    "a width-specific layer constant drifted from its u8 twin"
);

/// Heap one `flate2` inflate state charges.
pub const FLATE2_STATE_BYTES: u64 = 0x0002_0000;

/// Largest chunk the transport hands the decoder at once.
pub const MAX_CHUNK_SIZE: u64 = 8_192;

/// Staging buffers held across the decode chain: one chunk per layer.
pub const CHUNK_STAGING_BYTES: u64 = MAX_CHUNK_SIZE * LAYERS_MAX_U64;

/// Retained header map of one in-flight body stream, plus the
/// parser-worst transient of a trailer block that can be live at the
/// same time.
pub const HEADER_STREAM_CHARGE: u64 = 42_656;

/// Smallest bytes-of-RAM-per-byte-of-body an operator may charge for
/// the inbound parse. The factor itself is
/// [`ResolvedIsolateLimits::inbound_parse_factor`], because the true
/// cost depends on the workload as well as on the types: a publisher
/// sending dense small-key maps allocates far more per wire byte than
/// one sending sparse, well-shaped `OpenRTB`. A deployment may charge
/// MORE than this for hostile traffic; it may never charge less,
/// because below this the types provably do not fit.
///
/// # Derivation
///
/// `inbound_parse_factor_guard` in `auctioneer-openrtb`, which owns
/// `BidRequest`, parses three adversarial fixtures under a counting
/// allocator and reports both retained and cumulative bytes:
///
/// | fixture              | body     | retained  | cumulative | ratio |
/// |----------------------|----------|-----------|------------|-------|
/// | dense imp array      | `17_554` | `326_294` | `326_294`  | 18.6x |
/// | retained unknown ext | `23_514` | `280_698` | `526_458`  | 22.4x |
/// | many small strings   | `11_644` | `291_192` | `342_392`  | 29.4x |
///
/// `many small strings` binds at 29.4x cumulative. Cumulative rather
/// than retained because the parser's transient scratch is genuinely
/// live at the parse's peak, and an OOM does not care that an
/// allocation was about to be freed — every other factor here is a
/// worst-case charge and this matches. Applying the codebase's
/// standard 20% margin gives `ceil(29.4 * 1.2) = 36`.
///
/// The cost is bounded by the TYPES, not by the body: `Imp`, `Banner`
/// and their siblings each carry a `#[serde(flatten)]`
/// `serde_json::Map`, so a forty-byte impression retains a map header
/// and its buckets whatever the input density. The ratio therefore
/// does not fall as bodies grow.
pub const MIN_INBOUND_PARSE_FACTOR: u64 = 36;

/// Smallest heap an operator may reserve per resident parsed GVL — the
/// floor [`ResolvedIsolateLimits::gvl_parsed_bytes`] must meet or
/// exceed. `Gvl` is `Arc<GvlInner>`, so this is the cost of a DISTINCT
/// resident vendor list; additional handles to the same list are
/// refcount bumps and cost nothing this term needs to fund.
///
/// # Derivation
///
/// `gvl_retained_charge` in `auctioneer-privacy`, which owns `Gvl`, parses
/// every entry of the baked archive under a counting allocator and
/// reports the live bytes the parsed value retains. The archive is the
/// production shape — one `440_614`-byte spec-2 list and twenty-two
/// spec-3 lists from `888_217` to `933_077` bytes:
///
/// | list                  | raw       | retained  |
/// |-----------------------|-----------|-----------|
/// | spec 2, list 224      | `440_614` | `344_893` |
/// | spec 3, list 157      | `888_217` | `404_275` |
/// | spec 3, list 178      | `933_077` | `424_677` |
///
/// The largest bundled list binds at `424_677` retained. Retained
/// rather than cumulative, because a resident vendor map is what this
/// term funds. Cold parse scratch is transient and separately funded by
/// [`PRIVACY_FIXED_BYTES`], not by the unrelated inbound parse.
/// Applying the codebase's standard 20% margin gives
/// `ceil(424_677 * 1.2) = 509_613`, rounded up to `512_000`.
///
/// Retention tracks the vendor count, not the wire bytes: the spec-2
/// list is half the raw size of a spec-3 list and still retains 81% as
/// much, because the per-vendor `BTreeSet` and `String` allocations
/// dominate and a denser encoding does not shrink them. A future list
/// that grows the vendor population past the margin fails the guard by
/// name rather than silently overrunning the budget.
pub const MAX_GVL_PARSED_BYTES: u64 = 512_000;

/// Bytes the grouping and resolved-alias bookkeeping costs per byte
/// of inbound body, at input-derived worst-case name cardinality.
pub const GROUPING_FACTOR: u64 = 8;

/// The smallest `terminal_response_permits` an adapter row can declare
/// without a saturated instance answering a refusal with a connection
/// abort.
///
/// Nothing in [`validate_isolate_limits`] enforces a relation between
/// these three fields — `check_scalar_limits` compares each against its
/// own floor and never against the others — so every adapter row takes
/// its value from this function instead of writing one down. That is
/// deliberate: the previous revision kept the relation in a doc comment
/// on each of the four rows, and when the acquisition model changed
/// underneath them all four comments went stale while the numbers stayed
/// put. A derived value cannot.
///
/// How that is enforced differs by row, and only one row has a guard
/// that can fail. Cloudflare, Fastly and Spin declare the constant AS
/// this call, so the derivation is the enforcement and a `const _`
/// comparing the result against the same call would hold for every
/// possible operand. Axum declares `platform_concurrent_ceiling`
/// directly, because that term binds there, so its `const _` compares
/// two independently written quantities and does fail when the
/// admission sum outgrows the ceiling.
///
/// # Derivation
///
/// A static-class permit is held to terminal egress by three distinct
/// per-request populations, which can be live simultaneously:
///
/// 1. **Every admitted auction.** `ingress::admit_auction` reserves one
///    at admission and holds it through the handler, so this population
///    is exactly `max_concurrent_auctions`.
/// 2. **Every request rendering a refusal.** `ingress::refuse` draws one
///    for the bounded 503 it writes when a class gate is exhausted or
///    absent. An aux arrival that cannot get an aux permit takes this
///    path, so `aux_route_permits` further arrivals can be refusing at
///    once while the aux class is full.
/// 3. **Pre-admission head rejections.**
///    `host_primitives::static_egress::detached_egress_decision` draws
///    one for a 414 or 431. That path runs BEFORE any class gate, so it
///    is bounded only by the platform's own concurrency.
///
/// Populations 1 and 2 are admission-bounded and sum, giving
/// `max_concurrent_auctions + aux_route_permits`. Population 3 is
/// bounded by `platform_concurrent_ceiling` instead. A row must fund
/// whichever is larger, hence the `max`.
///
/// Both terms matter on shipped rows, which is why neither can be
/// dropped as redundant: on Cloudflare the admission sum (`6 + 4 = 10`)
/// exceeds the ceiling (6), while on Fastly and Spin the ceiling term is
/// what the published platform fact pins.
#[must_use]
pub const fn min_terminal_response_permits(
    max_concurrent_auctions: u32,
    aux_route_permits: u32,
    platform_concurrent_ceiling: u32,
) -> u32 {
    let admission_bounded = max_concurrent_auctions.saturating_add(aux_route_permits);
    if admission_bounded > platform_concurrent_ceiling {
        admission_bounded
    } else {
        platform_concurrent_ceiling
    }
}

/// Bytes the manifest an aux route renders may occupy, plus the catalog
/// it is built from.
///
/// The aux class's largest response is `/info/bidders`: one JSON record
/// per registered bidder, plus the overlay-aware catalog
/// (`InfoCatalog`) the handler derives from the resolved config — a
/// disabled-name set and a validated alias map — which is live while
/// the body is rendered.
///
/// Sized as a flat reservation rather than per-bidder, because the
/// registry is baked at compile time: it is not caller-influenced, and
/// a deployment cannot grow it at runtime. 1 MiB is far above the
/// rendered manifest for any plausible registry (the shipped one is a
/// few KiB) and is the term to revisit if the registry ever becomes
/// config-driven.
pub const AUX_MANIFEST_BYTES: u64 = 1 << 20;

/// The smallest `aux_route_peak_bytes` a row may declare.
///
/// # Why this needs a derivation at all
///
/// Every aux route runs `resolve_runtime_config`, exactly as the
/// auction route does: `/info/bidders` and `/info/bidders/{name}` build
/// their catalog from it, and `/_auctioneer/health/gvl` reads it too. So
/// an aux route in flight retains the whole config extraction — the raw
/// envelope, the extractor's whole-envelope `serde_json::Value`
/// precheck, and the typed parse of the root blob — and then the
/// manifest it renders on top.
///
/// That extraction charge entered the budget only through
/// `runtime_config_term` inside `per_auction_peak`, which is multiplied
/// by `max_concurrent_auctions` and funds AUCTIONS. Nothing funded the
/// same retention on the aux path, although the aux routes use the same
/// extractor.
///
/// **Replaces a flat `7_725_056`**, which carried no derivation on any
/// row in this tree and said so in its own doc comment
/// ("**UNVERIFIED**", "its basis matters"). It was also simply too
/// small: the extraction alone is 257 MiB at the shipped 1 MiB config
/// caps, so the old figure funded only a fraction of the charge its own
/// routes incur, with no statement anywhere of what the rest was.
///
/// # The derivation
///
/// `max_runtime_config_bytes` (the raw envelope, held while it is
/// walked) + `max_runtime_config_bytes x CONFIG_ENVELOPE_VALUE_FACTOR`
/// (the extractor's precheck materializes the whole envelope as a
/// `Value` before any schema narrows it, so this is charged on the RAW
/// cap) + `max_runtime_config_blob_bytes x CONFIG_PARSE_FACTOR` (only
/// the root blob is parsed into the typed config) +
/// [`AUX_MANIFEST_BYTES`].
///
/// The first three terms are `runtime_config_term` without the independent
/// baked-config allowance. They are spelled out here
/// rather than shared with it because that function computes from a
/// resolved row and this is a `const fn` an adapter calls to DECLARE
/// one; `every_aux_route_peak_funds_the_config_extraction` asserts the
/// two agree on every shipped row, so the duplication cannot drift.
#[must_use]
pub const fn min_aux_route_peak_bytes(
    max_runtime_config_bytes: u64,
    max_runtime_config_blob_bytes: u64,
    max_config_json_nodes: u64,
) -> u64 {
    max_runtime_config_bytes
        .saturating_add(max_runtime_config_bytes.saturating_mul(CONFIG_ENVELOPE_VALUE_FACTOR))
        .saturating_add(max_runtime_config_blob_bytes.saturating_mul(CONFIG_PARSE_FACTOR))
        .saturating_add(max_config_json_nodes.saturating_mul(CONFIG_NODE_RETAINED_BYTES))
        .saturating_add(AUX_MANIFEST_BYTES)
}

/// Peak bytes for one survivor's projection per byte of inbound body.
///
/// Includes the temporary scoped Value tree and typed-deserialization overlap,
/// not just the final request. `auctioneer-exchange/tests/projection_charge.rs`
/// measures both one and eight survivors across dense maps, nested forests,
/// small typed objects, aliases, and FPD. Repeated supplements and transient
/// FPD patch copies are charged as additional serialized bytes BEFORE applying
/// this factor; original wire size alone does not bound amplified projections.
/// The non-amplified FPD fixture needs
/// 1,833,723 bytes including a 20% margin for 16,476 wire bytes (111.3x).
/// Round upward to 128; the former factor 2 underfunded every fixture.
pub const TYPED_CLONE_FACTOR: u64 = 128;

/// Wire-byte contents one request copy retains. Container heap is
/// charged separately with [`NODE_RETAINED_BYTES`], including the
/// nested `Value` trees inside typed requests.
const REQUEST_COPY_WIRE_FACTOR: u64 = 2;

/// Cold GVL parsing, TCF parser/result (including DV), bounded GPP slots,
/// and warning scratch. The privacy allocation guard measures 7,796,005
/// bytes at the cold-list peak and 104,949 for TCF including inline state.
/// Their sum plus 20% is 9,481,145 bytes; round upward to 10 MiB.
pub const PRIVACY_FIXED_BYTES: u64 = 10 * 1024 * 1024;

/// Operator response-byte charges may grow, but cannot underfund the dense
/// nested-map adapter path measured by the response allocation guards.
pub const MIN_RESPONSE_PARSE_FACTOR: u64 = RESPONSE_PARSE_FACTOR;

/// Request-dependent names, selected signals, and per-bidder privacy state.
/// Applied independently from projection and ingress charges because their
/// representations can coexist throughout an auction.
pub const PRIVACY_REQUEST_FACTOR: u64 = 128;

/// Transient gzip encoder heap, separate from raw and encoded bodies.
/// The live finalizer guard measures 352,104 bytes, 422,525 with 20% margin.
/// Encoding is sequential, so one 512 KiB allowance covers the auction.
pub const OUTBOUND_FINALIZE_FIXED_BYTES: u64 = 512 * 1024;

/// Peak live adapter decoding per decoded byte, including the parsed tree
/// and copied typed bids. The dense nested-map guard needs 202.2x including
/// a 20% margin; round to 256. Guarded in `exchange/tests/response_charge.rs`.
pub const RESPONSE_PARSE_FACTOR: u64 = 256;

/// Peak typed response assembly, separate from the serialized output floor.
/// The dense nested-map guard needs 99.6x including a 20% margin.
pub const RESPONSE_ASSEMBLY_FACTOR: u64 = 128;

/// Serialized-output sizing floor per decoded byte. Typed assembly and
/// serialization scratch are separately charged by [`RESPONSE_ASSEMBLY_FACTOR`].
/// Keep the upstream 3x assembly floor as well as that independent peak charge.
pub const ASSEMBLY_FACTOR: u64 = 3;

/// Smallest bytes-per-SYNTACTIC-JSON-NODE an operator may charge for
/// the response parse, alongside the per-byte term. The figure itself
/// is [`ResolvedIsolateLimits::response_node_peak_bytes`].
///
/// Same rule as the per-byte factor: more is always allowed, less never
/// is, and validation refuses at boot below this. A node's heap cost is
/// a measurement of what `serde_json` allocates per container, not a
/// preference, so an operator who sets it under the measured figure
/// gets a boot check that passes while the isolate OOMs.
///
/// The response side needs both terms for the same reason the request
/// side does, and `parse_expansion` has charged both there all along:
/// a JSON body's heap cost depends on the CONTAINERS it materializes,
/// and the container count has an independent node budget. The node
/// charge scales with that budget even when a conservative byte factor
/// already funds a particular response shape.
///
/// # Independent Node Charge
///
/// `Bid` carries a `#[serde(flatten)]` catch-all that no schema
/// narrows, so `{"a":{"a":...}}` under `bid[]` retains one heap node
/// per level while costing seven wire bytes per two nodes. Holding the
/// node count at the shipped 65 536 cap while varying the wire size by
/// 23%, measured through the real `GenericBidder::make_bids`:
///
/// | depth | bids    | decoded   | peak         | B/node | milli-B/byte |
/// |-------|---------|-----------|--------------|--------|--------------|
/// | 5     | `3_448` | `263_331` | `16_173_275` | `246`  | `61_418`     |
/// | 10    | `2_259` | `239_548` | `18_310_186` | `279`  | `76_436`     |
/// | 20    | `1_337` | `221_114` | `19_148_914` | `292`  | `86_601`     |
/// | 50    | `601`   | `207_180` | `20_061_806` | `306`  | `96_832`     |
/// | 100   | `313`   | `201_720` | `20_346_144` | `310`  | `100_863`    |
/// | 120   | `263`   | `201_030` | `20_458_274` | `312`  | `101_767`    |
///
/// Across that sweep bytes-per-node moves 66 B while milli-bytes per
/// wire byte moves 40 349. Charging nodes independently keeps this term
/// tied to the admitted structure rather than its wire size. The dense-map
/// byte floor remains an additional, separately measured allowance.
///
/// Linear in nodes, confirmed by raising the budget rather than only
/// the body: a minimal-bid body at one, two and four times the node
/// cap reads exactly 159 B/node at all three, so the term scales with
/// the budget it is charged against.
///
/// 320 funds the worst measured 312 with ~2.5% headroom for allocator
/// differences between platforms — enough for a platform, not enough
/// to hide a shape change. Lower than [`NODE_RETAINED_BYTES`]'s 384
/// because the typed `Bid` assembly reuses allocations the request
/// side's retained `Value` tree keeps.
///
/// Pinned by `adapter_path_node_term_guard` in `auctioneer-exchange`,
/// which drives the real `Bidder::make_bids` on the catch-all shape
/// and asserts the PEAK against the funded node term.
pub const MIN_RESPONSE_NODE_PEAK_BYTES: u64 = 320;

/// Bytes deserializing the runtime-config blob costs per byte of
/// blob, including the verified JSON envelope, validation and merged policy
/// clones. The live core guard covers activity-rule limits and rejected dense
/// unknown fields; its largest margined ratio is 106.1x. Baked config clones
/// require an additional allowance because their size is independent of the
/// runtime blob. Guarded by `core/tests/config_peak_charge.rs`.
pub const CONFIG_PARSE_FACTOR: u64 = 128;

/// Bytes the envelope materialization costs per byte of raw envelope.
/// Independent of the typed root-blob parse: the extractor materializes the
/// whole envelope before schema narrowing, so the blob cap cannot fund it.
///
/// At the shipped structural caps (64 levels and 8,192 values), the worst
/// admitted envelope retains 6,067,476 bytes at the shipped 1 MiB cap;
/// its measured factor 17 remains covered. Declared node caps are configurable
/// and independently funded by [`CONFIG_NODE_RETAINED_BYTES`]. Keep M4's
/// conservative 128-byte charge for this representation, including rejected
/// dense unknown fields, separately from typed config and independent
/// baked-policy clones.
/// Guarded by core's config envelope and live config peak allocation tests.
pub const CONFIG_ENVELOPE_VALUE_FACTOR: u64 = 128;

/// Retained bytes per SDK JSON value of the extractor's whole-envelope
/// `serde_json::Value`, at the measured worst shape. Object keys do not count
/// as separate values in the SDK limit.
///
/// Higher than [`NODE_RETAINED_BYTES`]'s 384 because a config envelope
/// is map-shaped: a one-entry `serde_json::Map` takes a whole
/// `BTreeMap` leaf node, so the per-node cost is the container's, not
/// the value's.
///
/// Charged against the extraction NODE CAP rather than against the
/// byte cap, because the two are independent. `auctioneer-core` installs each
/// nonzero `max_config_json_nodes` declaration as `max_json_nodes`; zero
/// resolves to upstream's default. Boot normalizes the row to that effective
/// cap before charging it and checking the aux extraction floor. A smaller
/// `max_runtime_config_bytes` does not automatically shrink the node cap.
///
/// Deriving the node count from the byte cap does NOT close this: at
/// the cheapest admissible shape a node costs 2 wire bytes (`1,` in a
/// number array, or one level of `[`/`]` nesting), so even a 40 KB
/// envelope encodes 20,000 nodes -- above the shipped 8,192 cap, which
/// therefore keeps binding. Only charging the effective node ceiling covers it.
pub const CONFIG_NODE_RETAINED_BYTES: u64 = 623;

/// Retained bidder-response header map: the admitted header bytes plus
/// the per-entry map overhead at the response header caps.
pub const HEADER_MAP_CHARGE: u64 = 12_288;

/// The platform `http::HeaderMap` the transport conversion fills from
/// the outbound byte pool. It overlaps the pool, which stays alive for
/// the response phase, so both are charged.
pub const OUTBOUND_HEADER_MAP_CHARGE: u64 = 7_680;

/// Per-field overhead the pseudoheader allowance must cover beyond the
/// request target: the four pseudo names, the `:method` and `:scheme`
/// values at their enforced bounds, and four entries of field overhead.
pub const PSEUDOHEADER_FIXED_OVERHEAD_BYTES: u32 = 192;

/// Head slack an h1 read buffer must carry beyond the admitted header
/// block and the request target.
pub const H1_HEAD_SLACK_BYTES: u32 = 1_024;

/// Encoded bytes an h2 field-section entry is assumed to cost when
/// deriving the parser-time entry maximum. h2 exposes no header-count
/// knob, so the entry bound is the ceiling divided by this.
pub const H2_MIN_FIELD_ENTRY_BYTES: u32 = 32;

/// Which configured limit failed a check. Carried by the error rather
/// than baked into a variant so a new limit does not need a new
/// variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitField {
    MaxOutboundRawBytes,
    FinalResponseBytes,
    ResponseWriteDeadlineMs,
    MaxBrotliLayers,
    MaxActiveBodyStreams,
    MaxConcurrentAuctions,
    MaxBiddersPerAuction,
    InboundParseFactor,
    ResponseParseFactor,
    ResponseNodePeakBytes,
    GvlParsedBytes,
}

impl LimitField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MaxOutboundRawBytes => "max_outbound_raw_bytes",
            Self::FinalResponseBytes => "final_response_bytes",
            Self::ResponseWriteDeadlineMs => "response_write_deadline_ms",
            Self::MaxBrotliLayers => "max_brotli_layers",
            Self::MaxActiveBodyStreams => "max_active_body_streams",
            Self::MaxConcurrentAuctions => "max_concurrent_auctions",
            Self::MaxBiddersPerAuction => "max_bidders_per_auction",
            Self::InboundParseFactor => "inbound_parse_factor",
            Self::ResponseParseFactor => "response_parse_factor",
            Self::ResponseNodePeakBytes => "response_node_peak_bytes",
            Self::GvlParsedBytes => "gvl_parsed_bytes",
        }
    }
}

impl fmt::Display for LimitField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why an isolate budget could not be established.
///
/// Every variant reports a configuration whose memory demand cannot be
/// shown to fit the ceiling. Whether that ends the boot is the
/// caller's decision and not this enum's: `auctioneer-core`'s
/// `install_boot_envelope` classifies each variant, and only
/// [`Self::BudgetOverCeiling`] warns and boots. The rest — a cap below
/// a measured floor above all — are internally inconsistent
/// configurations that under-reserve memory the code needs on every
/// request, so they stay fatal there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetError {
    /// Inspection-only validators have no qualified catalog contribution.
    UnqualifiedSchemaArtifact,
    /// A term of the budget equation overflowed `u64`. Every
    /// multiplication and addition below is checked, so this is a
    /// configuration whose product is not representable, never a
    /// wrapped total presented as a valid one.
    BudgetArithmeticOverflow,
    /// The auxiliary-work derivation rejected a malformed or unrepresentable
    /// profile term before any auxiliary request can allocate it.
    AuxWork(AuxWorkError),
    /// The configured per-aux peak cannot fund the checked simultaneous work.
    AuxRoutePeakBelowDerived { configured: u64, required: u64 },
    /// A brotli window exponent outside the validated RFC 7932 range
    /// `10..=24`. Shifting by it would be undefined and the resulting
    /// budget meaningless.
    BrotliWbitsOutOfRange { wbits: u8 },
    /// The summed demand exceeds the isolate's memory ceiling.
    BudgetOverCeiling { required: u64, ceiling: u64 },
    /// A configured `u64` limit that this target's `usize` cannot
    /// represent. Caught at boot rather than truncating or panicking
    /// at the allocation site.
    LimitExceedsUsize { field: LimitField, value: u64 },
    /// A limit below the minimum that makes it meaningful.
    LimitBelowMinimum {
        field: LimitField,
        value: u64,
        minimum: u64,
    },
    /// A limit above the maximum that makes it meaningful.
    LimitAboveMaximum {
        field: LimitField,
        value: u64,
        maximum: u64,
    },
    /// `final_response_bytes` is below the assembly-scratch term it
    /// must be able to hold. A hard response cap smaller than the
    /// scratch a within-budget auction produces would truncate.
    FinalResponseBelowAssemblyFloor { configured: u64, required: u64 },
    /// `aux_route_peak_bytes` is below what one aux route actually
    /// retains. Every aux route resolves the runtime config through the
    /// same extractor the auction route uses, so a peak that does not
    /// fund the extraction under-reserves memory the route needs on
    /// every request.
    AuxRoutePeakBelowExtractionFloor { configured: u64, required: u64 },
    /// The h2 header-list knob is not exactly one above the admitted
    /// ceiling. h2 rejects at `headers_size >= max`, so a knob equal
    /// to the ceiling rejects a field section policy admits.
    HeaderKnobNotCeilingPlusOne { knob: u32, ceiling: u32 },
    /// The pseudoheader allowance cannot cover the request target plus
    /// the four-pseudoheader fixed overhead.
    PseudoheaderAllowanceTooSmall { allowance: u32, required: u32 },
    /// The h1 read buffer cannot hold an admitted header block plus
    /// its request target and head slack, so a policy-valid h1 head
    /// would be parser-rejected.
    H1BufSizeTooSmall { configured: u32, required: u32 },
    /// The h1 header-count knob does not match the admitted count.
    H1HeaderCountMismatch { knob: u32, admitted: u32 },
    /// The derived parser entry maximum does not match the configured
    /// one. It is `floor(ceiling / 32)` by construction; a differing
    /// value means the row was hand-edited away from the equation.
    ParserEntryCountMismatch { configured: u32, derived: u32 },
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnqualifiedSchemaArtifact => f.write_str("schema artifact is unqualified"),
            Self::BudgetArithmeticOverflow => {
                f.write_str("isolate budget arithmetic overflowed u64")
            }
            Self::AuxWork(error) => write!(f, "auxiliary work derivation failed: {error}"),
            Self::AuxRoutePeakBelowDerived {
                configured,
                required,
            } => write!(
                f,
                "aux_route_peak_bytes = {configured} is below the derived {required}-byte auxiliary peak"
            ),
            Self::BrotliWbitsOutOfRange { wbits } => write!(
                f,
                "brotli window exponent {wbits} is outside the supported range \
                 {MIN_BROTLI_WBITS}..={MAX_BROTLI_WBITS}"
            ),
            Self::BudgetOverCeiling { required, ceiling } => write!(
                f,
                "isolate budget requires {required} bytes, over the {ceiling}-byte ceiling"
            ),
            Self::LimitExceedsUsize { field, value } => write!(
                f,
                "{field} = {value} cannot be represented as usize on this target"
            ),
            Self::LimitBelowMinimum {
                field,
                value,
                minimum,
            } => write!(f, "{field} = {value} is below the minimum {minimum}"),
            Self::LimitAboveMaximum {
                field,
                value,
                maximum,
            } => write!(f, "{field} = {value} is above the maximum {maximum}"),
            Self::FinalResponseBelowAssemblyFloor {
                configured,
                required,
            } => write!(
                f,
                "final_response_bytes = {configured} is below the {required}-byte \
                 assembly scratch it must hold"
            ),
            Self::AuxRoutePeakBelowExtractionFloor {
                configured,
                required,
            } => write!(
                f,
                "aux_route_peak_bytes = {configured} is below the {required} bytes an \
                 aux route retains: the runtime-config extraction it performs, plus the \
                 manifest it renders"
            ),
            Self::HeaderKnobNotCeilingPlusOne { knob, ceiling } => write!(
                f,
                "parser_header_knob_bytes = {knob} must be exactly one above the \
                 admitted ceiling {ceiling}"
            ),
            Self::PseudoheaderAllowanceTooSmall {
                allowance,
                required,
            } => write!(
                f,
                "pseudoheader_allowance_bytes = {allowance} is below the required {required}"
            ),
            Self::H1BufSizeTooSmall {
                configured,
                required,
            } => write!(
                f,
                "h1_max_buf_size_bytes = {configured} is below the required {required}"
            ),
            Self::H1HeaderCountMismatch { knob, admitted } => write!(
                f,
                "h1_max_headers = {knob} must equal max_admitted_header_count = {admitted}"
            ),
            Self::ParserEntryCountMismatch {
                configured,
                derived,
            } => write!(
                f,
                "parser_max_header_entries = {configured} must equal the derived {derived}"
            ),
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the remaining provided methods cannot be written on stable: \
              `provide` is nightly-only (error_generic_member_access), \
              `description` and `cause` are deprecated, and `type_id` is \
              sealed by `Any`"
)]
impl StdError for BudgetError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::AuxWork(error) => Some(error),
            Self::UnqualifiedSchemaArtifact
            | Self::BudgetArithmeticOverflow
            | Self::AuxRoutePeakBelowDerived { .. }
            | Self::AuxRoutePeakBelowExtractionFloor { .. }
            | Self::BrotliWbitsOutOfRange { .. }
            | Self::BudgetOverCeiling { .. }
            | Self::LimitExceedsUsize { .. }
            | Self::LimitBelowMinimum { .. }
            | Self::LimitAboveMaximum { .. }
            | Self::FinalResponseBelowAssemblyFloor { .. }
            | Self::HeaderKnobNotCeilingPlusOne { .. }
            | Self::PseudoheaderAllowanceTooSmall { .. }
            | Self::H1BufSizeTooSmall { .. }
            | Self::H1HeaderCountMismatch { .. }
            | Self::ParserEntryCountMismatch { .. } => None,
        }
    }
}

/// Normalized per-isolate limits: the SOURCE values consumers read.
///
/// Aggregate totals are deliberately absent — a consumer cannot
/// recover a cap from a sum, so totals are derived and checked by
/// [`validate_isolate_limits`] into [`Derived`] rather than stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedIsolateLimits {
    /// The isolate's whole memory ceiling.
    pub isolate_total_bytes: u64,
    /// Whole-isolate baseline allowance: the measured baseline and its safety
    /// margin, plus any separately documented adapter startup/residency reserve.
    /// Validation consumes it as is; an allocation reserve is not measured RSS.
    pub baseline_bytes: u64,
    /// Auction body cap.
    pub inbound_request_bytes: u64,
    /// Bytes of RAM one byte of inbound body becomes once parsed into
    /// the typed request. Operator-settable so a deployment can charge
    /// its own traffic's shape, but floored at
    /// [`MIN_INBOUND_PARSE_FACTOR`]: below that the types provably do
    /// not fit, and validation refuses at boot.
    pub inbound_parse_factor: u64,
    /// Encoded outbound request cap.
    pub max_outbound_request_bytes: u64,
    /// Pre-encoding outbound construction cap.
    pub max_outbound_raw_bytes: u64,
    /// Compressed on-the-wire response cap.
    pub max_transport_bytes: u64,
    /// Post-decode response cap.
    pub max_decoded_bytes: u64,
    /// Bytes of RAM one byte of decoded bidder body becomes across the
    /// complete adapter path. Operator-settable so a deployment can
    /// charge its own bidders' response shapes, but floored at
    /// [`MIN_RESPONSE_PARSE_FACTOR`]: below that the shapes the
    /// response-factor guards measure provably do not fit, and
    /// validation refuses at boot.
    pub response_parse_factor: u64,
    /// Bytes of RAM one SYNTACTIC JSON NODE of a bidder response peaks
    /// at, charged alongside [`Self::response_parse_factor`] because a
    /// body's heap cost is part per-byte and part per-container. Floored
    /// at [`MIN_RESPONSE_NODE_PEAK_BYTES`] under the same rule.
    ///
    /// Charged at [`Self::max_parsed_nodes`], which bounds the response
    /// parse as well as the inbound one.
    pub response_node_peak_bytes: u64,
    /// Hard cap on the serialized response: the egress-buffer bound.
    pub final_response_bytes: u64,
    /// Egress-gate capacity.
    pub egress_permits: u32,
    /// The response-write deadline, range-checked only — it is a time
    /// source and charges no memory.
    pub response_write_deadline_ms: u64,
    /// Decoder heap the adapter reports for its own in-guest decode,
    /// zero where the adapter never decodes in-guest. The per-stream
    /// decoder charge is the max of this and auctioneer's own chain.
    pub guest_decoder_bound_bytes: u64,
    /// Aux-route gate capacity.
    pub aux_route_permits: u32,
    /// Peak bytes one aux route costs.
    pub aux_route_peak_bytes: u64,
    /// Largest brotli window auctioneer will decode.
    pub max_brotli_wbits: u8,
    /// Decode layers a stacked `Content-Encoding` may carry.
    pub max_brotli_layers: u8,
    /// Distinct parsed vendor lists the store's cache owns at once.
    /// `auctioneer-privacy`'s `GVL_PARSE_CACHE_MAX_ENTRIES` is the cap the
    /// store actually enforces; configuring more only over-charges.
    pub gvl_cache_entries: u32,
    /// Heap ONE DISTINCT parsed vendor list retains. Additional handles
    /// to the same list are `Arc` refcount bumps and cost nothing here;
    /// the per-auction term funds distinct lists, not copies. Floored at
    /// [`MAX_GVL_PARSED_BYTES`].
    pub gvl_parsed_bytes: u64,
    /// Bidders one auction may fan out to.
    pub max_bidders_per_auction: u32,
    /// Outbound calls one auction may make.
    pub max_outbound_requests: u32,
    /// `AdmissionGate` capacity.
    pub max_concurrent_auctions: u32,
    /// `StreamGate` capacity: concurrent outbound bidder legs one
    /// auction's fanout wave may decode at once.
    ///
    /// The name counts no inbound request bodies. `auctioneer-core`'s
    /// `drive_fanout` is the gate's only acquirer and takes a whole
    /// wave of permits at a time, one per bidder call; the per-stream
    /// charge is a bidder RESPONSE being decoded, which is what
    /// [`per_stream_bytes`] computes.
    pub max_active_body_streams: u32,
    /// Pre-deserialization cap on the raw runtime-config envelope.
    pub max_runtime_config_bytes: u64,
    /// Largest single runtime-config blob, the one parsed into the
    /// typed config.
    pub max_runtime_config_blob_bytes: u64,
    /// Syntactic JSON node budget for the inbound parse.
    pub max_parsed_nodes: u32,
    /// Advisory header bytes copied into the typed request and carried
    /// by every bidder clone.
    pub max_copied_header_bytes: u32,
    /// The whole inbound header block, admitted at the ingress seam.
    pub max_admitted_header_bytes: u32,
    /// Inbound header entries admitted at the ingress seam.
    pub max_admitted_header_count: u16,
    /// Impressions one auction may carry.
    pub max_impressions_per_auction: u16,
    /// Nodes the config extractor will materialize from one runtime
    /// envelope.
    ///
    /// Independent of `max_runtime_config_bytes`: the extraction caps
    /// bound the wire and the node population separately. `auctioneer-core`
    /// installs nonzero declarations unchanged and normalizes zero to
    /// upstream's default before charging and checking floors. Charged in
    /// its own right because a smaller byte cap does not shrink it.
    pub max_config_json_nodes: u32,
    /// Static-class response permits.
    pub terminal_response_permits: u32,
    /// The one static-permit render buffer size.
    pub refusal_body_bytes: u32,
    /// Measured per-response transmit state.
    pub transmit_state_bytes: u32,
}

/// Per-target platform facts the equation charges that are not
/// operator-tunable limits. Every value is cited to platform
/// documentation or measured, per target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetPlatformConstants {
    /// Concurrently-live requests the platform admits per isolate
    /// before any auctioneer gate. Headers materialize before admission,
    /// so this is what bounds the pre-admission parser population.
    ///
    /// The unit this counts is the platform's memory-accounting scope,
    /// not a portable notion of concurrency. On a `PerExecution`
    /// target a fresh instance serves one request out of its own
    /// ceiling, so the population is 1; on a `PerInstance` target
    /// concurrent requests share one budget and the population is
    /// whatever that platform admits at once. Each adapter derives it
    /// from its own platform's documentation and states the derivation
    /// beside the constant, because this crate cannot check it.
    pub platform_concurrent_ceiling: u32,
    /// Measured per-entry header-map cost.
    pub header_entry_overhead_bytes: u32,
    /// The largest field section admitted, in field-section
    /// accounting with pseudoheaders included. This is the charged
    /// bound.
    pub parser_header_ceiling_bytes: u32,
    /// The value actually configured into the h2 header-list knob. A
    /// separate field so the exclusive-boundary `+1` cannot be
    /// silently dropped.
    pub parser_header_knob_bytes: u32,
    /// The request-target cap, normalized per protocol.
    pub max_request_target_bytes: u32,
    /// The h1 head-buffer knob.
    pub h1_max_buf_size_bytes: u32,
    /// The h1 header-count knob.
    pub h1_max_headers: u32,
    /// Parser-time entry maximum. Derived, not configured: h2 exposes
    /// no header-count knob, so validation recomputes and asserts it.
    pub parser_max_header_entries: u32,
    /// Byte allowance for the four request pseudoheaders.
    pub pseudoheader_allowance_bytes: u32,
    /// Hpack dynamic table plus decode scratch, measured per target.
    pub parser_transient_bytes: u32,
    /// Buffers a response handoff holds simultaneously. One only with
    /// a verified owned zero-copy handoff, else two or the measured
    /// value.
    pub egress_handoff_factor: u32,
}

/// Owning crates report their actual construction layouts as neutral byte costs.
/// All costs are prepaid for each admitted auction before their storage exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequestConstructionCosts {
    /// Audit/observation state retained for the entire auction; charged to privacy.
    pub fixed_per_auction_bytes: u64,
    /// Bounded scratch reused between survivor constructions; charged to projection.
    pub reusable_scratch_bytes: u64,
    /// Destination growth retained per survivor scope; charged to projection.
    pub retained_per_scope_bytes: u64,
}

/// Disjoint catalog additions beyond the unchanged baseline and existing charges.
/// Owning crates must document whether charges are proved bounds or explicitly
/// approved tested reservations; these scalars cannot establish qualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogResourceCosts {
    /// Additional catalog, generated validator and shared-cache residency.
    pub resident_bytes: u64,
    /// Temporary catalog initialization and failure-cleanup scratch, counted once.
    pub initialization_bytes: u64,
    /// Bounded schema validation scratch for one admitted auction.
    pub schema_validation_per_auction_bytes: u64,
    /// Additional settings, correlation and diagnostic work per admitted auction.
    pub context_per_auction_bytes: u64,
}

impl CatalogResourceCosts {
    /// No new catalog behavior or residency relative to the matched baseline.
    pub const UNCHANGED: Self = Self {
        resident_bytes: 0,
        initialization_bytes: 0,
        schema_validation_per_auction_bytes: 0,
        context_per_auction_bytes: 0,
    };
}

/// Stable identities for the additive whole-isolate terms, in equation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolateTerm {
    Baseline,
    Gvl,
    Auctions,
    AuctionEgress,
    Auxiliary,
    TerminalEgress,
    ProbeOutput,
    PreAdmissionParser,
    BodyStreams,
    CatalogResident,
    CatalogInitialization,
}

impl IsolateTerm {
    /// Complete runtime-owned enumeration in `B+G+Q+E+A+T+P+H+S+C+I` order.
    pub const ALL: &'static [Self] = &[
        Self::Baseline,
        Self::Gvl,
        Self::Auctions,
        Self::AuctionEgress,
        Self::Auxiliary,
        Self::TerminalEgress,
        Self::ProbeOutput,
        Self::PreAdmissionParser,
        Self::BodyStreams,
        Self::CatalogResident,
        Self::CatalogInitialization,
    ];

    /// Number of rows, including terms whose charge is zero.
    pub const COUNT: usize = Self::ALL.len();

    /// Stable report name, independent of request or configuration text.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Gvl => "gvl",
            Self::Auctions => "auctions",
            Self::AuctionEgress => "auction_egress",
            Self::Auxiliary => "auxiliary",
            Self::TerminalEgress => "terminal_egress",
            Self::ProbeOutput => "probe_output",
            Self::PreAdmissionParser => "pre_admission_parser",
            Self::BodyStreams => "body_streams",
            Self::CatalogResident => "catalog_resident",
            Self::CatalogInitialization => "catalog_initialization",
        }
    }
}

/// Actual population-scaled summands of the whole-isolate bound.
/// Per-auction and per-stream values have already been multiplied here;
/// consumers need no raw limits to report or sum the complete charge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IsolateTerms {
    /// B: baseline residency.
    pub baseline_bytes: u64,
    /// G: every distinct resident parsed vendor list.
    pub gvl_bytes: u64,
    /// Q: all concurrently admitted auctions.
    pub auctions_bytes: u64,
    /// E: auction egress buffers and transmit state.
    pub auction_egress_bytes: u64,
    /// A: aux routes and independently gated probe-config work.
    pub auxiliary_bytes: u64,
    /// T: static-class terminal egress.
    pub terminal_egress_bytes: u64,
    /// P: probe cache and independently retained output bodies.
    pub probe_output_bytes: u64,
    /// H: all pre-admission parsers at the dominating protocol peak.
    pub pre_admission_parser_bytes: u64,
    /// S: all concurrently active bidder-response body streams.
    pub body_streams_bytes: u64,
    /// C: additional catalog residency, counted once.
    pub catalog_resident_bytes: u64,
    /// I: additional catalog initialization scratch, counted once.
    pub catalog_initialization_bytes: u64,
}

impl IsolateTerms {
    /// Fold every summand in the original equation order with checked addition.
    ///
    /// # Errors
    /// Returns [`BudgetError::BudgetArithmeticOverflow`] if the sum overflows.
    pub fn checked_total(&self) -> Result<u64, BudgetError> {
        let Self {
            baseline_bytes,
            gvl_bytes,
            auctions_bytes,
            auction_egress_bytes,
            auxiliary_bytes,
            terminal_egress_bytes,
            probe_output_bytes,
            pre_admission_parser_bytes,
            body_streams_bytes,
            catalog_resident_bytes,
            catalog_initialization_bytes,
        } = *self;
        [
            baseline_bytes,
            gvl_bytes,
            auctions_bytes,
            auction_egress_bytes,
            auxiliary_bytes,
            terminal_egress_bytes,
            probe_output_bytes,
            pre_admission_parser_bytes,
            body_streams_bytes,
            catalog_resident_bytes,
            catalog_initialization_bytes,
        ]
        .into_iter()
        .try_fold(0, add)
    }

    /// Complete named rows in equation order, without filtering zero charges.
    #[must_use]
    pub const fn entries(&self) -> [(IsolateTerm, u64); IsolateTerm::COUNT] {
        let Self {
            baseline_bytes,
            gvl_bytes,
            auctions_bytes,
            auction_egress_bytes,
            auxiliary_bytes,
            terminal_egress_bytes,
            probe_output_bytes,
            pre_admission_parser_bytes,
            body_streams_bytes,
            catalog_resident_bytes,
            catalog_initialization_bytes,
        } = *self;
        [
            (IsolateTerm::Baseline, baseline_bytes),
            (IsolateTerm::Gvl, gvl_bytes),
            (IsolateTerm::Auctions, auctions_bytes),
            (IsolateTerm::AuctionEgress, auction_egress_bytes),
            (IsolateTerm::Auxiliary, auxiliary_bytes),
            (IsolateTerm::TerminalEgress, terminal_egress_bytes),
            (IsolateTerm::ProbeOutput, probe_output_bytes),
            (IsolateTerm::PreAdmissionParser, pre_admission_parser_bytes),
            (IsolateTerm::BodyStreams, body_streams_bytes),
            (IsolateTerm::CatalogResident, catalog_resident_bytes),
            (
                IsolateTerm::CatalogInitialization,
                catalog_initialization_bytes,
            ),
        ]
    }
}

/// Every term the validator computed, so a caller can diff a proof row
/// against the code rather than trusting a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Derived {
    /// Complete, already population-scaled whole-isolate summands.
    pub isolate_terms: IsolateTerms,
    /// Disjoint reservations whose sum is exactly `per_auction_peak`.
    pub auction_charges: AuctionCharges,
    /// The owning-layout descriptor used by this derivation.
    pub construction_costs: RequestConstructionCosts,
    /// Additional isolate residency, not multiplied by admission or config caps.
    pub catalog_resident_term: u64,
    /// Additional initialization scratch, retained in the additive bound once.
    pub catalog_initialization_term: u64,
    /// Per-auction schema scratch included in the projection receipt.
    pub schema_validation_term: u64,
    /// Per-auction context work included in the projection receipt.
    pub catalog_context_term: u64,
    /// Runtime-config envelope plus its deserialized form.
    pub runtime_config_term: u64,
    /// Inbound body, admitted headers, copied headers, parse
    /// expansion, retained and working copies, grouping, and the measured
    /// projection allowance, including retained growth and reusable scratch.
    pub request_side_term: u64,
    /// Outbound requests, retained bodies, response headers, response
    /// parse, request metadata and maps, and the assembly scratch.
    pub call_side_term: u64,
    /// Request-derived diagnostics/status rows, including their assembly copies.
    /// Included in `auction_charges.response`, separately from `call_side_term`.
    pub bookkeeping_term: u64,
    /// Unconditional prepaid debug peak, separate from existing response work.
    pub debug_term: u64,
    /// The assembly scratch alone — `final_response_bytes` must be
    /// able to hold it.
    pub assembly_scratch_bytes: u64,
    /// Everything one concurrent auction costs.
    pub per_auction_peak: u64,
    /// One concurrently-decoded bidder response: transport staging,
    /// decoded body, the max-rule decoder charge, and the
    /// header-stream charge.
    pub per_stream_peak: u64,
    /// The decoder arm alone, before the max against the guest bound.
    pub decoder_chain_bytes: u64,
    /// Distinct resident vendor lists: the ones the cache owns, plus
    /// one evicted-but-still-referenced version per live auction or aux
    /// privacy request.
    pub gvl_terms: u64,
    /// Auction egress buffers plus transmit state.
    pub egress_term: u64,
    /// Aux routes and independently gated probe-config work.
    pub aux_term: u64,
    /// Static-class egress.
    pub terminal_term: u64,
    /// Resident probe cache and bodies retained outside the config-work gate.
    pub probe_output_term: u64,
    /// One h2 connection slot's pre-admission parser peak.
    pub h2_slot_peak: u64,
    /// One h1 connection slot's pre-admission parser peak.
    pub h1_slot_peak: u64,
    /// The dominating protocol peak, times the platform ceiling.
    pub pre_admission_parser_term: u64,
    /// Everything the isolate must hold at once.
    pub total_bytes: u64,
    /// Headroom left under the ceiling, zero for an over-ceiling derivation.
    pub margin_bytes: u64,
    /// `max_outbound_raw_bytes`, proven representable on this target.
    pub max_outbound_raw_bytes_usize: usize,
}

const fn mul(a: u64, b: u64) -> Result<u64, BudgetError> {
    match a.checked_mul(b) {
        Some(v) => Ok(v),
        None => Err(BudgetError::BudgetArithmeticOverflow),
    }
}

const fn add(a: u64, b: u64) -> Result<u64, BudgetError> {
    match a.checked_add(b) {
        Some(v) => Ok(v),
        None => Err(BudgetError::BudgetArithmeticOverflow),
    }
}

/// Maximum added syntactic nodes from implicit fill, counting keys and values.
/// Site/page/domain/publisher/domain/ext/amp add 14; device/IP/UA/DNT add 8;
/// at adds 2; regs/ext/GPC add 6. The mutually exclusive app channel/LMT
/// branch adds fewer nodes. This is enrichment, not a higher parser cap.
pub const NORMALIZATION_IMPLICIT_NODES: u64 = 30;

/// Maximum pinned `serde_json` wire width across `i64`, `u64` and finite `f64` numbers.
/// `serde_json` 1.0.150 writes floats through `zmij` 1.0.21's 24-byte buffer;
/// serializer dependency changes require requalification of this lexical bound.
pub const MAX_NUMBER_WIRE_BYTES: u64 = 24;

/// JSON syntax and canonical IP bytes added by the largest implicit-fill branch.
/// Each literal includes a comma to cover insertion into an existing object.
const NORMALIZATION_FIXED_WIRE_BYTES: usize =
    b",\"site\":{},\"page\":\"\",\"domain\":\"\",\"publisher\":{},\"domain\":\"\",\"ext\":{},\"amp\":0".len()
    + b",\"device\":{},\"ipv6\":\"\",\"ua\":\"\",\"dnt\":1".len()
    + 39 // Longest canonical IPv6 string; one family is filled, never both.
    + b",\"at\":1".len()
    + b",\"regs\":{},\"ext\":{},\"gpc\":\"1\"".len();

/// Checked wire allowance after bounded implicit/header enrichment.
/// Copied UA/page bytes may each require a six-byte JSON escape. Each of the
/// two derived domains must be bounded by the body/capped-Referer source
/// allowance, including IDNA expansion; this function does not bound a caller
/// that enriches from an unrestricted header. App channel/LMT syntax fits
/// within the larger site branch's fixed charge.
/// Each input node funds a numeric token's possible canonicalization to 24
/// bytes, less the one source byte already funded by `inbound`.
///
/// # Errors
/// Returns [`BudgetError::BudgetArithmeticOverflow`] rather than granting an
/// unrepresentable source/projection allowance.
pub fn normalization_wire_allowance(
    inbound: u64,
    copied_headers: u64,
    input_nodes: u64,
) -> Result<u64, BudgetError> {
    let source = add(inbound, copied_headers)?;
    let escaped = mul(copied_headers, 6)?;
    let domains = mul(source, 2)?;
    let normalized = add(inbound, add(escaped, domains)?)?;
    let fixed = u64::try_from(NORMALIZATION_FIXED_WIRE_BYTES)
        .map_err(|_overflow| BudgetError::BudgetArithmeticOverflow)?;
    let numeric_growth = mul(input_nodes, MAX_NUMBER_WIRE_BYTES - 1)?;
    add(add(normalized, fixed)?, numeric_growth)
}

/// Heap one brotli decoder instance charges at `wbits`.
///
/// This is the DECODER STATE alone: upstream's audited fixed charge
/// plus the `2^wbits` ring buffer. It is not a per-stream total —
/// [`per_stream_bytes`] adds the transport and decoded buffers and the
/// chunk staging on top.
///
/// # Errors
///
/// [`BudgetError::BrotliWbitsOutOfRange`] for any `wbits` outside
/// `10..=24`. The shift is undefined past 63 and meaningless outside
/// the RFC 7932 range, so a caller that skipped validation cannot
/// shift its way to a bogus budget.
pub const fn brotli_state_bytes(wbits: u8) -> Result<u64, BudgetError> {
    if wbits < MIN_BROTLI_WBITS || wbits > MAX_BROTLI_WBITS {
        return Err(BudgetError::BrotliWbitsOutOfRange { wbits });
    }
    // In range by the guard above, so the shift is defined and the sum
    // is far from `u64::MAX`; the checked add states that rather than
    // relying on the reader to verify it.
    add(BROTLI_DECODER_FIXED_CHARGE_BYTES, 1_u64 << wbits)
}

/// Heap one live body stream charges at its peak.
///
/// The decoder arm is a MAX rule, not a sum: auctioneer's own passthrough
/// chain and the adapter's reported in-guest decoder are alternative
/// decoders of the same body, so the charge is whichever is larger.
/// auctioneer's chain allocates one brotli state (at most
/// [`MAX_BROTLI_LAYERS`]) and fills the remaining layers with the
/// cheaper flate2 state, plus one staging chunk per layer.
///
/// # Errors
///
/// [`BudgetError::BrotliWbitsOutOfRange`] for an out-of-range window,
/// [`BudgetError::LimitAboveMaximum`] for more layers than
/// [`LAYERS_MAX`], and [`BudgetError::BudgetArithmeticOverflow`] if any
/// term overflows.
pub fn per_stream_bytes(
    max_transport_bytes: u64,
    max_decoded_bytes: u64,
    max_brotli_wbits: u8,
    max_brotli_layers: u8,
    guest_decoder_bound_bytes: u64,
) -> Result<u64, BudgetError> {
    let chain = decoder_chain_bytes(max_brotli_wbits, max_brotli_layers)?;
    let decoder = guest_decoder_bound_bytes.max(chain);
    add(add(max_transport_bytes, max_decoded_bytes)?, decoder)
}

/// auctioneer's own decode chain at `wbits` over `layers`: the staging
/// chunks, one brotli state, and flate2 state for the layers brotli
/// does not take.
fn decoder_chain_bytes(wbits: u8, layers: u8) -> Result<u64, BudgetError> {
    if layers > LAYERS_MAX {
        return Err(BudgetError::LimitAboveMaximum {
            field: LimitField::MaxBrotliLayers,
            value: u64::from(layers),
            maximum: LAYERS_MAX_U64,
        });
    }
    let brotli = brotli_state_bytes(wbits)?;
    // The chain is charged at its worst shape, not at `layers`: a
    // stacked encoding can present brotli in any position, so the
    // budget funds the full `LAYERS_MAX` staging and every non-brotli
    // slot. Charging only the configured `layers` would leave a
    // three-layer stack unfunded the moment the limit is raised.
    let flate = mul(FLATE2_STATE_BYTES, FLATE2_SLOTS_U64)?;
    add(CHUNK_STAGING_BYTES, add(brotli, flate)?)
}

/// Normalize the existing isolate envelope into M5 auxiliary work caps.
/// Baked config is independent of the runtime-config blob and may coexist
/// with it during config extraction. The caller must use the same normalized
/// envelope snapshot for this function and for isolate derivation.
///
/// # Errors
/// Refuses overflow while deriving body/cookie syntactic-node byte allowances.
pub fn aux_work_limits_from_envelope(
    limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
    baked_config_bytes: u64,
) -> Result<AuxWorkLimits, BudgetError> {
    // Config counts SDK JSON values, not syntactic keys. The sync body and
    // cookie keep their separate syntactic-node byte allowances.
    let body_node_bytes = mul(MAX_SYNC_BODY_BYTES, NODE_RETAINED_BYTES)?;
    let cookie_node_bytes = mul(4_096, NODE_RETAINED_BYTES)?;
    let privacy_input_bytes = MAX_SYNC_BODY_BYTES.max(MAX_SYNC_QUERY_BYTES);
    Ok(AuxWorkLimits {
        config_envelope_bytes: limits.max_runtime_config_bytes,
        config_blob_bytes: limits.max_runtime_config_blob_bytes,
        baked_config_bytes,
        config_json_values: limits.max_config_json_nodes,
        body_bytes: MAX_SYNC_BODY_BYTES,
        body_node_bytes,
        query_bytes: MAX_SYNC_QUERY_BYTES,
        query_parameters: MAX_SYNC_QUERY_PARAMETERS,
        admitted_header_bytes: u64::from(limits.max_admitted_header_bytes),
        admitted_header_count: u32::from(limits.max_admitted_header_count),
        header_entry_overhead_bytes: target.header_entry_overhead_bytes,
        cookie_bytes: u64::from(limits.max_admitted_header_bytes),
        cookie_node_bytes,
        privacy_input_bytes,
        selected_tcf_bytes: privacy_input_bytes,
        gvl_parsed_bytes: limits.gvl_parsed_bytes,
        catalog_names: MAX_CATALOG_IDENTITIES,
        priority_groups: MAX_SYNC_PRIORITY_GROUPS,
        priority_entries: MAX_SYNC_PRIORITY_ENTRIES,
        requested_names: MAX_SYNC_REQUESTED_NAMES,
        filter_names_per_list: MAX_SYNC_FILTER_NAMES_PER_LIST,
        debug_rows: MAX_SYNC_DEBUG_ROWS,
        url_bytes: MAX_SYNC_URL_BYTES,
        output_body_bytes: MAX_SYNC_OUTPUT_BYTES,
        // Sync output has independent ceilings even when an operator narrows
        // request headers. Keep the larger profile allowance when present.
        response_header_bytes: u64::from(limits.max_admitted_header_bytes)
            .max(MAX_SYNC_RESPONSE_HEADER_BYTES),
        response_header_count: u32::from(limits.max_admitted_header_count)
            .max(MAX_SYNC_RESPONSE_HEADER_COUNT),
        set_cookie_bytes: MAX_SYNC_SET_COOKIE_BYTES,
    })
}

/// Derive the non-GVL auxiliary peak from an isolate envelope. The pinned
/// GVL allowance remains separate and is already funded by `gvl_terms`.
///
/// # Errors
/// Refuses malformed, unrepresentable, or overflowing auxiliary terms.
pub fn derive_aux_work_for_profile(
    limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
    baked_config_bytes: u64,
) -> Result<AuxWorkDerived, BudgetError> {
    let inputs = aux_work_limits_from_envelope(limits, target, baked_config_bytes)?;
    derive_aux_work(inputs).map_err(BudgetError::AuxWork)
}

/// Validate that the declared aux-route peak funds the checked non-GVL work.
///
/// # Errors
/// Refuses a profile whose aux peak is below the derived allowance.
pub fn validate_aux_route_peak(
    limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
    baked_config_bytes: u64,
) -> Result<AuxWorkDerived, BudgetError> {
    let derived = derive_aux_work_for_profile(limits, target, baked_config_bytes)?;
    if limits.aux_route_peak_bytes < derived.peak_bytes() {
        return Err(BudgetError::AuxRoutePeakBelowDerived {
            configured: limits.aux_route_peak_bytes,
            required: derived.peak_bytes(),
        });
    }
    Ok(derived)
}

/// Prove a configuration fits its isolate, and hand back every term.
///
/// `outbound_meta_bytes` is the finished per-call metadata charge from
/// `auctioneer_bidder::outbound_meta_bytes`. It arrives as a scalar
/// precisely so no byte-pool layout constant crosses the crate
/// boundary: a second copy of that layout here would silently diverge
/// from the pool the sink actually allocates.
/// `construction_costs` similarly comes from the owning exchange/core layouts,
/// without importing those crates or copying their layout constants.
/// `catalog_costs` supplies disjoint additions to the matched baseline; it is
/// mandatory even when no new catalog behavior has been introduced.
///
/// # Errors
///
/// [`BudgetError`] — one variant per check. Overflow anywhere is
/// [`BudgetError::BudgetArithmeticOverflow`]; a demand over the
/// ceiling is [`BudgetError::BudgetOverCeiling`]; the knob, allowance
/// and buffer-sizing assertions each have their own variant.
pub fn validate_isolate_limits(
    limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
    outbound_meta_bytes: u32,
    construction_costs: RequestConstructionCosts,
    catalog_costs: CatalogResourceCosts,
) -> Result<Derived, BudgetError> {
    let derived = derive_isolate_budget(
        limits,
        target,
        outbound_meta_bytes,
        construction_costs,
        catalog_costs,
    )?;
    if derived.total_bytes > limits.isolate_total_bytes {
        return Err(BudgetError::BudgetOverCeiling {
            required: derived.total_bytes,
            ceiling: limits.isolate_total_bytes,
        });
    }
    Ok(derived)
}

/// Compute checked budget terms without treating the whole-isolate ceiling as
/// an admission policy. Over-ceiling boot can retain these terms for per-auction
/// accounting and diagnostics; [`validate_isolate_limits`] still rejects that
/// configuration, leaving the warn-and-boot decision to its existing caller.
///
/// # Errors
/// Returns every arithmetic, scalar, layout, and target-knob error reported by
/// [`validate_isolate_limits`], except [`BudgetError::BudgetOverCeiling`]. An
/// over-ceiling derivation reports zero `margin_bytes`, never wrapped headroom.
pub fn derive_isolate_budget(
    limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
    outbound_meta_bytes: u32,
    construction_costs: RequestConstructionCosts,
    catalog_costs: CatalogResourceCosts,
) -> Result<Derived, BudgetError> {
    derive_isolate_budget_with_baked_config(
        limits,
        target,
        outbound_meta_bytes,
        construction_costs,
        0,
        catalog_costs,
    )
}

/// Derive the same budget with an independent baked-config cloning allowance.
/// `baked_config_bytes` is its serialized size, measured by the owning crate
/// without materializing another copy. It is not part of the runtime blob cap.
///
/// # Errors
/// Same checks as [`derive_isolate_budget`], including overflow in the added
/// allowance and its per-isolate population multiplier.
#[expect(
    clippy::too_many_lines,
    reason = "keeping the checked equation together makes its terms auditable"
)]
pub fn derive_isolate_budget_with_baked_config(
    limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
    outbound_meta_bytes: u32,
    construction_costs: RequestConstructionCosts,
    baked_config_bytes: u64,
    catalog_costs: CatalogResourceCosts,
) -> Result<Derived, BudgetError> {
    check_scalar_limits(limits)?;
    check_target_knobs(limits, target)?;

    let inbound = limits.inbound_request_bytes;
    let normalized_wire = normalization_wire_allowance(
        inbound,
        u64::from(limits.max_copied_header_bytes),
        u64::from(limits.max_parsed_nodes),
    )?;
    let decoded = limits.max_decoded_bytes;
    let bidders = u64::from(limits.max_bidders_per_auction);
    let calls = u64::from(limits.max_outbound_requests);

    // The envelope is charged raw; it is then
    // MATERIALIZED whole as a `serde_json::Value` by the extractor's
    // version precheck, which is what the envelope factor pays for;
    // and only the root blob is parsed into the typed config, so the
    // parse factor scales the blob and not the total.
    //
    // Baked policy clones have an independent allowance: their size is not
    // bounded by either runtime cap.
    // The precheck charge is on the RAW cap because that is the input
    // it walks — it runs before any schema narrows the envelope, so the
    // blob cap does not bound it.
    // Four charges. The first three scale with a byte cap; the NODE
    // term does not, and that asymmetry is the point. The extraction
    // node cap is independent of `max_runtime_config_bytes`, so a
    // smaller byte cap shrinks the first three and leaves the node
    // population untouched — the shape that made the per-byte charge
    // alone insufficient below the shipped 1 MiB row.
    let runtime_config_term = add(
        add(
            add(
                limits.max_runtime_config_bytes,
                mul(
                    limits.max_runtime_config_bytes,
                    CONFIG_ENVELOPE_VALUE_FACTOR,
                )?,
            )?,
            mul(
                add(limits.max_runtime_config_blob_bytes, baked_config_bytes)?,
                CONFIG_PARSE_FACTOR,
            )?,
        )?,
        mul(
            u64::from(limits.max_config_json_nodes),
            CONFIG_NODE_RETAINED_BYTES,
        )?,
    )?;

    let admitted_headers = add(
        u64::from(limits.max_admitted_header_bytes),
        mul(
            u64::from(limits.max_admitted_header_count),
            u64::from(target.header_entry_overhead_bytes),
        )?,
    )?;
    // Transient header/URL work is independent of the normalized request
    // lifetimes below; advisory strings do not undergo supplement amplification.
    let header_copies = mul(u64::from(limits.max_copied_header_bytes), 5)?;
    let node_tree = mul(u64::from(limits.max_parsed_nodes), NODE_RETAINED_BYTES)?;
    let normalized_node_tree = mul(
        add(
            u64::from(limits.max_parsed_nodes),
            NORMALIZATION_IMPLICIT_NODES,
        )?,
        NODE_RETAINED_BYTES,
    )?;
    let parse_expansion = add(
        mul(normalized_wire, limits.inbound_parse_factor)?,
        normalized_node_tree,
    )?;
    let request_copy = add(
        mul(normalized_wire, REQUEST_COPY_WIRE_FACTOR)?,
        normalized_node_tree,
    )?;
    // Alongside the normalized tree funded by parse_expansion, the
    // underlying typed clone and the splitter's shared Value persist.
    let retained_copies = mul(2, request_copy)?;
    // Group plans retain imp bidder params and the shared request-level
    // supplement block. These are disjoint input subtrees: together at
    // most one extra input-sized node tree, not one tree per bidder.
    let grouping = add(mul(normalized_wire, GROUPING_FACTOR)?, normalized_node_tree)?;
    // The splitter preflights the TOTAL supplement expansion across
    // this bidder's imps against max_parsed_nodes and the raw-body cap.
    // Thus a scope retains at most the input plus one capped supplement,
    // not max_impressions copies of the whole input. Two representations
    // fund materialization or the scoped request plus bidder-owned transform.
    // These materialization/transform temporaries are sequential; completed
    // bidder request trees remain retained through make_bids below.
    let scoped_supplement = add(
        mul(limits.max_outbound_raw_bytes, REQUEST_COPY_WIRE_FACTOR)?,
        node_tree,
    )?;
    let working_copies = mul(2, add(request_copy, scoped_supplement)?)?;
    // M4's projection receipt checks measured work across all survivors,
    // including FPD scratch. It covers bidder request trees retained in
    // run.capped through make_bids as well as sequential projection temporaries;
    // keep that allowance separate from the copy model.
    let projection_scopes = mul(
        bidders,
        add(
            mul(normalized_wire, TYPED_CLONE_FACTOR)?,
            construction_costs.retained_per_scope_bytes,
        )?,
    )?;

    let ingress = add(
        add(add(inbound, admitted_headers)?, header_copies)?,
        add(
            add(parse_expansion, grouping)?,
            add(retained_copies, working_copies)?,
        )?,
    )?;
    // Validation and context construction use the existing prepaid projection
    // receipt. Their inspectable terms are not added again at isolate level.
    let schema_validation_term = catalog_costs.schema_validation_per_auction_bytes;
    let catalog_context_term = catalog_costs.context_per_auction_bytes;
    let projection = add(
        add(projection_scopes, construction_costs.reusable_scratch_bytes)?,
        add(schema_validation_term, catalog_context_term)?,
    )?;
    let request_side_term = add(ingress, projection)?;

    let outbound_bodies = mul(
        calls,
        add(
            limits.max_outbound_raw_bytes,
            limits.max_outbound_request_bytes,
        )?,
    )?;
    let retained_bodies = mul(calls, decoded)?;
    let response_headers = mul(calls, HEADER_MAP_CHARGE)?;
    // Two terms, mirroring `parse_expansion` on the request side: a
    // body's heap cost is part per-byte and part per-container, and the
    // container count is bounded by the node budget rather than by the
    // wire size. Per CALL, unlike the request side's single inbound
    // parse — each bidder response is parsed on its own.
    let response_parse = mul(
        calls,
        add(
            mul(decoded, limits.response_parse_factor)?,
            mul(
                u64::from(limits.max_parsed_nodes),
                limits.response_node_peak_bytes,
            )?,
        )?,
    )?;
    let request_meta = mul(calls, u64::from(outbound_meta_bytes))?;
    let request_maps = mul(calls, OUTBOUND_HEADER_MAP_CHARGE)?;
    let assembly_scratch_bytes = mul(mul(calls, decoded)?, ASSEMBLY_FACTOR)?;
    let response_assembly = mul(mul(calls, decoded)?, RESPONSE_ASSEMBLY_FACTOR)?;

    let outbound = add(
        OUTBOUND_FINALIZE_FIXED_BYTES,
        add(outbound_bodies, add(request_meta, request_maps)?)?,
    )?;
    let response = add(
        add(retained_bodies, response_headers)?,
        add(
            response_parse,
            add(response_assembly, assembly_scratch_bytes)?,
        )?,
    )?;
    let call_side_term = add(outbound, response)?;

    // The configured hard response cap must be able to hold the
    // scratch a within-budget auction produces. This is a sizing
    // floor, not an output-size proof: JSON escaping can expand wire
    // bytes, and the bounded serializer is what enforces the wire
    // bound.
    if limits.final_response_bytes < assembly_scratch_bytes {
        return Err(BudgetError::FinalResponseBelowAssemblyFloor {
            configured: limits.final_response_bytes,
            required: assembly_scratch_bytes,
        });
    }

    // The aux class funds the SAME config extraction the auction class
    // does, because the aux routes call the same extractor. Checked
    // rather than left to a doc comment: the figure this replaces
    // carried no derivation and funded under a third of the charge.
    let aux_floor = min_aux_route_peak_bytes(
        limits.max_runtime_config_bytes,
        limits.max_runtime_config_blob_bytes,
        u64::from(limits.max_config_json_nodes),
    );
    if limits.aux_route_peak_bytes < aux_floor {
        return Err(BudgetError::AuxRoutePeakBelowExtractionFloor {
            configured: limits.aux_route_peak_bytes,
            required: aux_floor,
        });
    }

    // Privacy denials precede the bidder cap. Their identity/status
    // metadata is independently preflighted against materialization caps,
    // not funded by the population of calls that survive those denials.
    let bookkeeping_term = mul(4, add(limits.max_outbound_raw_bytes, node_tree)?)?;
    let debug_term = debug_peak_bytes(limits.max_outbound_requests)?;
    let auction_charges = AuctionCharges {
        config: runtime_config_term,
        ingress,
        privacy: add(
            add(PRIVACY_FIXED_BYTES, mul(inbound, PRIVACY_REQUEST_FACTOR)?)?,
            construction_costs.fixed_per_auction_bytes,
        )?,
        projection,
        outbound,
        response: add(add(response, bookkeeping_term)?, debug_term)?,
    };
    let per_auction_peak = auction_charges
        .total()
        .map_err(|_overflow| BudgetError::BudgetArithmeticOverflow)?;

    let decoder_chain_bytes =
        decoder_chain_bytes(limits.max_brotli_wbits, limits.max_brotli_layers)?;
    let per_stream_peak = add(
        per_stream_bytes(
            limits.max_transport_bytes,
            decoded,
            limits.max_brotli_wbits,
            limits.max_brotli_layers,
            limits.guest_decoder_bound_bytes,
        )?,
        HEADER_STREAM_CHARGE,
    )?;

    // Resident GVL memory is counted in DISTINCT PARSED LISTS, never in
    // handles. `Gvl` is an `Arc<GvlInner>`: the store's cache owns one
    // handle per entry and an auction clones a second one out of it for
    // the auction's lifetime, which is a refcount bump — measured at a
    // pointer width per extra handle by `gvl_retained_charge` in
    // `auctioneer-privacy`.
    //
    // The per-request term survives that anyway, because it funds a
    // different thing: the `(spec, list)` key comes from the request's
    // own TC string, so concurrent auctions and aux privacy requests
    // can pin different versions. A version evicted from the cache
    // while a request still holds its handle stays resident until that
    // request ends. Each live request can therefore keep one distinct
    // list alive beyond the cache's own entries, which is charged as one.
    let gvl_terms = mul(
        add(
            add(
                u64::from(limits.gvl_cache_entries),
                u64::from(limits.max_concurrent_auctions),
            )?,
            u64::from(limits.aux_route_permits),
        )?,
        limits.gvl_parsed_bytes,
    )?;

    let handoff = u64::from(target.egress_handoff_factor);
    let transmit = u64::from(limits.transmit_state_bytes);
    let egress_term = mul(
        u64::from(limits.egress_permits),
        add(mul(limits.final_response_bytes, handoff)?, transmit)?,
    )?;
    // Separate counters let aux routes and probe-config work coexist.
    // Probes keep a minimum work slot even when the aux class is
    // disabled; the shared floor must fund that slot too.
    let aux_permits = u64::from(limits.aux_route_permits);
    let probe_config_permits = u64::from(
        limits
            .aux_route_permits
            .max(crate::PROBE_CONFIG_MIN_PERMITS),
    );
    let aux_term = mul(
        add(aux_permits, probe_config_permits)?,
        add(mul(limits.aux_route_peak_bytes, handoff)?, transmit)?,
    )?;
    let terminal_term = mul(
        u64::from(limits.terminal_response_permits),
        add(
            mul(u64::from(limits.refusal_body_bytes), handoff)?,
            transmit,
        )?,
    )?;
    // The independent probe output gate holds permits through terminal
    // egress, including before bootstrap. Config-work permits and unknown
    // provider ingress populations cannot bound these retained bodies.
    // The cached verdict can remain resident beside that population.
    let probe_output_term = add(
        crate::PROBE_RESPONSE_BODY_MAX_BYTES,
        mul(
            u64::from(crate::PROBE_RESPONSE_PERMITS),
            add(
                mul(crate::PROBE_RESPONSE_BODY_MAX_BYTES, handoff)?,
                transmit,
            )?,
        )?,
    )?;

    // Headers materialize before admission, so the parser population
    // is bounded by the platform's own concurrency ceiling rather than
    // by any auctioneer gate. Both protocol peaks are computed and the
    // larger charged: substituting the h2 ceiling everywhere would be
    // unsound on a target where h1 dominates.
    let entry_overhead = u64::from(target.header_entry_overhead_bytes);
    let ceiling_bytes = u64::from(target.parser_header_ceiling_bytes);
    let h2_slot_peak = add(
        add(ceiling_bytes, u64::from(target.parser_transient_bytes))?,
        add(
            ceiling_bytes,
            mul(u64::from(target.parser_max_header_entries), entry_overhead)?,
        )?,
    )?;
    let h1_slot_peak = add(
        u64::from(target.h1_max_buf_size_bytes),
        add(
            u64::from(limits.max_admitted_header_bytes),
            mul(u64::from(limits.max_admitted_header_count), entry_overhead)?,
        )?,
    )?;
    let pre_admission_parser_term = mul(
        h2_slot_peak.max(h1_slot_peak),
        u64::from(target.platform_concurrent_ceiling),
    )?;

    let catalog_resident_term = catalog_costs.resident_bytes;
    let catalog_initialization_term = catalog_costs.initialization_bytes;
    let isolate_terms = IsolateTerms {
        baseline_bytes: limits.baseline_bytes,
        gvl_bytes: gvl_terms,
        auctions_bytes: mul(u64::from(limits.max_concurrent_auctions), per_auction_peak)?,
        auction_egress_bytes: egress_term,
        auxiliary_bytes: aux_term,
        terminal_egress_bytes: terminal_term,
        probe_output_bytes: probe_output_term,
        pre_admission_parser_bytes: pre_admission_parser_term,
        body_streams_bytes: mul(u64::from(limits.max_active_body_streams), per_stream_peak)?,
        catalog_resident_bytes: catalog_resident_term,
        catalog_initialization_bytes: catalog_initialization_term,
    };
    let total_bytes = isolate_terms.checked_total()?;

    validate_aux_route_peak(limits, target, baked_config_bytes)?;
    let margin_bytes = limits.isolate_total_bytes.saturating_sub(total_bytes);

    Ok(Derived {
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
        max_outbound_raw_bytes_usize: usize::try_from(limits.max_outbound_raw_bytes).map_err(
            |_ignored| BudgetError::LimitExceedsUsize {
                field: LimitField::MaxOutboundRawBytes,
                value: limits.max_outbound_raw_bytes,
            },
        )?,
    })
}

/// Range checks on the limits that charge no memory term, plus the
/// `usize` representability of the one limit that reaches an
/// allocation site as a capacity.
fn check_scalar_limits(limits: &ResolvedIsolateLimits) -> Result<(), BudgetError> {
    if usize::try_from(limits.max_outbound_raw_bytes).is_err() {
        return Err(BudgetError::LimitExceedsUsize {
            field: LimitField::MaxOutboundRawBytes,
            value: limits.max_outbound_raw_bytes,
        });
    }
    if limits.response_write_deadline_ms < 100 {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::ResponseWriteDeadlineMs,
            value: limits.response_write_deadline_ms,
            minimum: 100,
        });
    }
    if limits.response_write_deadline_ms > 120_000 {
        return Err(BudgetError::LimitAboveMaximum {
            field: LimitField::ResponseWriteDeadlineMs,
            value: limits.response_write_deadline_ms,
            maximum: 120_000,
        });
    }
    // A zero-capacity gate admits nothing: the isolate would validate
    // by serving no traffic at all.
    if limits.max_concurrent_auctions == 0 {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::MaxConcurrentAuctions,
            value: 0,
            minimum: 1,
        });
    }
    if limits.max_active_body_streams == 0 {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::MaxActiveBodyStreams,
            value: 0,
            minimum: 1,
        });
    }
    if limits.max_bidders_per_auction == 0 {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::MaxBiddersPerAuction,
            value: 0,
            minimum: 1,
        });
    }
    // The parse factor is a floor, not a default: raising it for
    // hostile traffic is always allowed, going under the measured
    // minimum never is.
    if limits.inbound_parse_factor < MIN_INBOUND_PARSE_FACTOR {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::InboundParseFactor,
            value: limits.inbound_parse_factor,
            minimum: MIN_INBOUND_PARSE_FACTOR,
        });
    }
    // The response side's two halves take the same rule, for the same
    // reason: each is a MEASUREMENT of what a decoded byte or a parsed
    // node costs the heap, not a preference. An operator who sets
    // either under the measured figure gets a boot check that passes
    // while the isolate overruns on the first bidder response.
    if limits.response_parse_factor < MIN_RESPONSE_PARSE_FACTOR {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::ResponseParseFactor,
            value: limits.response_parse_factor,
            minimum: MIN_RESPONSE_PARSE_FACTOR,
        });
    }
    if limits.response_node_peak_bytes < MIN_RESPONSE_NODE_PEAK_BYTES {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::ResponseNodePeakBytes,
            value: limits.response_node_peak_bytes,
            minimum: MIN_RESPONSE_NODE_PEAK_BYTES,
        });
    }
    // Same rule as the parse factor: a deployment may reserve more per
    // resident vendor list than the archive's largest entry measured,
    // never less. Under-charging a privacy-critical path means the
    // isolate validates and then overruns at the first consent request.
    if limits.gvl_parsed_bytes < MAX_GVL_PARSED_BYTES {
        return Err(BudgetError::LimitBelowMinimum {
            field: LimitField::GvlParsedBytes,
            value: limits.gvl_parsed_bytes,
            minimum: MAX_GVL_PARSED_BYTES,
        });
    }
    if limits.max_brotli_layers > LAYERS_MAX {
        return Err(BudgetError::LimitAboveMaximum {
            field: LimitField::MaxBrotliLayers,
            value: u64::from(limits.max_brotli_layers),
            maximum: u64::from(LAYERS_MAX),
        });
    }
    Ok(())
}

/// The parser-knob assertions: each one closes a gap where a
/// policy-valid request would be rejected by the parser, or a
/// parser-admitted request would exceed what policy charges.
fn check_target_knobs(
    limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
) -> Result<(), BudgetError> {
    // h2 rejects at `headers_size >= max`, so the configured knob sits
    // exactly one above the admitted ceiling.
    let expected_knob = target
        .parser_header_ceiling_bytes
        .checked_add(1)
        .ok_or(BudgetError::BudgetArithmeticOverflow)?;
    if target.parser_header_knob_bytes != expected_knob {
        return Err(BudgetError::HeaderKnobNotCeilingPlusOne {
            knob: target.parser_header_knob_bytes,
            ceiling: target.parser_header_ceiling_bytes,
        });
    }

    let required_allowance = target
        .max_request_target_bytes
        .checked_add(PSEUDOHEADER_FIXED_OVERHEAD_BYTES)
        .ok_or(BudgetError::BudgetArithmeticOverflow)?;
    if target.pseudoheader_allowance_bytes < required_allowance {
        return Err(BudgetError::PseudoheaderAllowanceTooSmall {
            allowance: target.pseudoheader_allowance_bytes,
            required: required_allowance,
        });
    }

    let required_buf = limits
        .max_admitted_header_bytes
        .checked_add(target.max_request_target_bytes)
        .and_then(|v| v.checked_add(H1_HEAD_SLACK_BYTES))
        .ok_or(BudgetError::BudgetArithmeticOverflow)?;
    if target.h1_max_buf_size_bytes < required_buf {
        return Err(BudgetError::H1BufSizeTooSmall {
            configured: target.h1_max_buf_size_bytes,
            required: required_buf,
        });
    }

    let admitted_count = u32::from(limits.max_admitted_header_count);
    if target.h1_max_headers != admitted_count {
        return Err(BudgetError::H1HeaderCountMismatch {
            knob: target.h1_max_headers,
            admitted: admitted_count,
        });
    }

    // h2 exposes no header-count knob, so the parser-time entry
    // maximum is derived from the byte ceiling rather than configured.
    let derived_entries = target
        .parser_header_ceiling_bytes
        .checked_div(H2_MIN_FIELD_ENTRY_BYTES)
        .ok_or(BudgetError::BudgetArithmeticOverflow)?;
    if target.parser_max_header_entries != derived_entries {
        return Err(BudgetError::ParserEntryCountMismatch {
            configured: target.parser_max_header_entries,
            derived: derived_entries,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::aux_work::{
        AuxNodeSet, AuxWorkBudget, AuxWorkCharges, AuxWorkError, derive_aux_work,
    };
    use crate::bounded_json::NODE_RETAINED_BYTES;
    use crate::{HeadroomClass, IsolateTerm, IsolateTerms, minimum_headroom_bytes};
    use std::num::NonZeroU64;

    use super::{
        ASSEMBLY_FACTOR, BROTLI_DECODER_FIXED_CHARGE_BYTES, BudgetError,
        CONFIG_ENVELOPE_VALUE_FACTOR, CONFIG_NODE_RETAINED_BYTES, CONFIG_PARSE_FACTOR, Derived,
        LimitField, MAX_BROTLI_WBITS, MAX_GVL_PARSED_BYTES, MIN_BROTLI_WBITS,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits, TargetPlatformConstants, brotli_state_bytes,
        derive_aux_work_for_profile, derive_isolate_budget, min_aux_route_peak_bytes,
        min_terminal_response_permits, per_stream_bytes, validate_isolate_limits,
    };

    #[test]
    fn wasm_headroom_uses_ceiling_division() {
        for (ceiling, expected) in [
            (128_000_000, 8_000_000),
            (0x0800_0000, 0x0080_0000),
            (100_000_001, 6_250_001),
        ] {
            assert_eq!(
                minimum_headroom_bytes(NonZeroU64::new(ceiling).unwrap(), HeadroomClass::Wasm),
                expected,
                "ceiling={ceiling}"
            );
        }
    }

    #[test]
    fn wasm_headroom_rounds_small_nonzero_ceilings_up() {
        for (ceiling, expected) in [(1, 1), (15, 1), (16, 1), (17, 2)] {
            assert_eq!(
                minimum_headroom_bytes(NonZeroU64::new(ceiling).unwrap(), HeadroomClass::Wasm),
                expected,
                "ceiling={ceiling}"
            );
        }
    }

    #[test]
    fn native_headroom_applies_its_floor_and_fractional_boundary() {
        for (ceiling, expected) in [
            (1, 0x1000_0000),
            (128_000_000, 0x1000_0000),
            (0x0800_0000, 0x1000_0000),
            (100_000_001, 0x1000_0000),
            (0xFFFF_FFFF, 0x1000_0000),
            (0x0001_0000_0000, 0x1000_0000),
            (0x0001_0000_0001, 0x1000_0001),
        ] {
            assert_eq!(
                minimum_headroom_bytes(NonZeroU64::new(ceiling).unwrap(), HeadroomClass::Native),
                expected,
                "ceiling={ceiling}"
            );
        }
    }

    #[test]
    fn headroom_supports_the_maximum_ceiling_without_overflow() {
        for class in [HeadroomClass::Native, HeadroomClass::Wasm] {
            assert_eq!(
                minimum_headroom_bytes(NonZeroU64::MAX, class),
                0x1000_0000_0000_0000
            );
        }
    }

    /// The synthetic fixture's body/transport/decoded cap. This module
    /// tests the platform-independent equation, not any adapter's defaults.
    const CAP_256_KIB: u64 = 1 << 18;
    /// The 16 MiB brotli fixed charge.
    const BYTES_16_MIB: u64 = 1 << 24;
    /// The synthetic row's response cap, DERIVED from the assembly scratch
    /// the validator requires it to hold. A literal here went stale when
    /// `ASSEMBLY_FACTOR` was corrected from 1, and every test below that
    /// expects a DIFFERENT refusal then got
    /// `FinalResponseBelowAssemblyFloor` instead.
    const FINAL_RESPONSE_BYTES: u64 = 64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    /// The row's 1 MiB runtime-config envelope cap.
    const BYTES_1_MIB: u64 = 1 << 20;
    /// The row's 64 KiB node budget and 16 KiB header block.
    const BYTES_64_KIB: u32 = 1 << 16;
    /// The same 64 KiB, as a byte cap rather than a node count. Used as
    /// a runtime-config blob cap DIFFERENT from the envelope cap, so
    /// each runtime-config charge is pinned against the cap it names.
    const BYTES_64_KIB_U64: u64 = 1 << 16;
    const BYTES_16_KIB: u32 = 1 << 14;

    /// Synthetic inputs for arithmetic, sensitivity, and refusal tests.
    /// Historically copied from an older native row, these values are
    /// deliberately fixed test data, not production limits or measurements.
    /// The authoritative Axum proof lives beside the actual `limits()` and
    /// `target()` constructors in `auctioneer-adapter-axum/src/envelope.rs`.
    fn fixture_limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 512_000_000,
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
            egress_permits: 64,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 16,
            // Synthetic M5 aux-work floor, not the shipped 768 MiB reservation.
            aux_route_peak_bytes: 700_114_944,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 50,
            max_outbound_requests: 64,
            max_concurrent_auctions: 64,
            max_active_body_streams: 32,
            max_runtime_config_bytes: BYTES_1_MIB,
            max_runtime_config_blob_bytes: BYTES_1_MIB,
            max_parsed_nodes: BYTES_64_KIB,
            max_copied_header_bytes: 4_096_u32,
            max_admitted_header_bytes: BYTES_16_KIB,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            // `DEFAULT_CONFIG_JSON_NODES`, inlined: this crate carries
            // no `edgezero-core` dependency on purpose, and the
            // adapters that build a real envelope read the constant.
            max_config_json_nodes: 8_192,
            terminal_response_permits: 384,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096_u32,
        }
    }

    fn fixture_target() -> TargetPlatformConstants {
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

    /// Fixed synthetic per-call metadata charge. Real adapters supply
    /// their own charge; this crate depends on no bidder or adapter crate.
    const FIXTURE_OUTBOUND_META_BYTES: u32 = 4_384;

    /// The synthetic fixture with its ceiling lifted, so equation tests
    /// exercise one relationship at a time without an unrelated overshoot.
    /// Ceiling tests use the original fixture or set their own boundary.
    fn roomy_fixture_limits() -> ResolvedIsolateLimits {
        let mut limits = fixture_limits();
        limits.isolate_total_bytes = u64::MAX;
        limits
    }

    /// Read the synthetic fixture's terms on the validator's success path.
    /// Only the ceiling changes; this is not a shipped-adapter proof.
    fn fixture_derived() -> Derived {
        validate_isolate_limits(
            &roomy_fixture_limits(),
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the synthetic terms are computable once the ceiling is lifted")
    }

    #[test]
    fn catalog_costs_follow_disjoint_isolate_and_auction_populations() {
        let costs = super::CatalogResourceCosts {
            resident_bytes: 23,
            initialization_bytes: 29,
            schema_validation_per_auction_bytes: 31,
            context_per_auction_bytes: 37,
        };
        for auctions in [1_u32, 3, 7] {
            for auxiliaries in [0_u32, 5, 9] {
                let mut limits = roomy_fixture_limits();
                limits.max_concurrent_auctions = auctions;
                limits.aux_route_permits = auxiliaries;
                limits.max_bidders_per_auction = 11;
                limits.max_outbound_requests = 17;
                // Fund the largest baked sample in auxiliary scratch as well.
                limits.aux_route_peak_bytes += 113 * 128;
                for baked in [0_u64, 79] {
                    let derive = |catalog| {
                        super::derive_isolate_budget_with_baked_config(
                            &limits,
                            &fixture_target(),
                            FIXTURE_OUTBOUND_META_BYTES,
                            super::RequestConstructionCosts::default(),
                            baked,
                            catalog,
                        )
                        .unwrap()
                    };
                    let plain = derive(super::CatalogResourceCosts::UNCHANGED);
                    let funded = derive(costs);
                    assert_eq!(funded.catalog_resident_term, 23);
                    assert_eq!(funded.catalog_initialization_term, 29);
                    assert_eq!(funded.schema_validation_term, 31);
                    assert_eq!(funded.catalog_context_term, 37);
                    assert_eq!(funded.runtime_config_term, plain.runtime_config_term);
                    assert_eq!(funded.auction_charges.config, plain.auction_charges.config);
                    assert_eq!(
                        funded.auction_charges.projection - plain.auction_charges.projection,
                        68
                    );
                    assert_eq!(funded.request_side_term - plain.request_side_term, 68);
                    assert_eq!(funded.per_auction_peak - plain.per_auction_peak, 68);
                    assert_eq!(
                        funded.total_bytes - plain.total_bytes,
                        52 + u64::from(auctions) * 68
                    );
                    for (catalog, projection_delta, total_delta) in [
                        (
                            super::CatalogResourceCosts {
                                resident_bytes: 23,
                                ..super::CatalogResourceCosts::UNCHANGED
                            },
                            0,
                            23,
                        ),
                        (
                            super::CatalogResourceCosts {
                                initialization_bytes: 29,
                                ..super::CatalogResourceCosts::UNCHANGED
                            },
                            0,
                            29,
                        ),
                        (
                            super::CatalogResourceCosts {
                                schema_validation_per_auction_bytes: 31,
                                ..super::CatalogResourceCosts::UNCHANGED
                            },
                            31,
                            u64::from(auctions) * 31,
                        ),
                        (
                            super::CatalogResourceCosts {
                                context_per_auction_bytes: 37,
                                ..super::CatalogResourceCosts::UNCHANGED
                            },
                            37,
                            u64::from(auctions) * 37,
                        ),
                    ] {
                        let single = derive(catalog);
                        assert_eq!(
                            single.auction_charges.projection - plain.auction_charges.projection,
                            projection_delta
                        );
                        assert_eq!(single.total_bytes - plain.total_bytes, total_delta);
                    }
                }
            }
        }
    }

    #[test]
    fn catalog_scratch_is_prepaid_and_refusal_releases_partial_charges() {
        use crate::{AuctionBudget, ChargeKind, ResourceError};
        let mut limits = roomy_fixture_limits();
        limits.max_concurrent_auctions = 3;
        let derive = |catalog| {
            super::derive_isolate_budget(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                catalog,
            )
            .unwrap()
        };
        let plain = derive(super::CatalogResourceCosts::UNCHANGED);
        let funded = derive(super::CatalogResourceCosts {
            resident_bytes: 23,
            initialization_bytes: 29,
            schema_validation_per_auction_bytes: 31,
            context_per_auction_bytes: 37,
        });
        assert_eq!(funded.per_auction_peak, plain.per_auction_peak + 68);
        let short = AuctionBudget::new(plain.per_auction_peak + 67);
        assert_eq!(
            funded.auction_charges.prepay(&short).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Response
            }
        );
        assert_eq!(short.in_use(), 0);
        let exact = AuctionBudget::new(plain.per_auction_peak + 68);
        let receipts = funded.auction_charges.prepay(&exact).unwrap();
        assert_eq!(exact.in_use(), plain.per_auction_peak + 68);
        let projection = receipts
            .iter()
            .find(|receipt| receipt.kind() == ChargeKind::Projection)
            .unwrap();
        assert_eq!(
            projection.ensure_capacity(
                ChargeKind::Projection,
                plain.auction_charges.projection + 68
            ),
            Ok(())
        );
        assert_eq!(
            projection.ensure_capacity(
                ChargeKind::Projection,
                plain.auction_charges.projection + 69
            ),
            Err(ResourceError::Exhausted {
                kind: ChargeKind::Projection
            })
        );
        drop(receipts);
        assert_eq!(exact.in_use(), 0);
    }

    #[test]
    fn catalog_costs_reach_strict_ceiling_checks_and_baked_config_derivation() {
        let costs = super::CatalogResourceCosts {
            resident_bytes: 23,
            initialization_bytes: 29,
            schema_validation_per_auction_bytes: 31,
            context_per_auction_bytes: 37,
        };
        let mut limits = roomy_fixture_limits();
        limits.max_concurrent_auctions = 3;
        limits.aux_route_peak_bytes += 113 * 128;
        let construction = super::RequestConstructionCosts::default();
        let plain = super::derive_isolate_budget(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            construction,
            super::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let funded = super::derive_isolate_budget(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            construction,
            costs,
        )
        .unwrap();
        assert_eq!(funded.total_bytes, plain.total_bytes + 256);
        limits.isolate_total_bytes = plain.total_bytes + 255;
        assert_eq!(
            super::validate_isolate_limits(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                construction,
                costs
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: plain.total_bytes + 256,
                ceiling: plain.total_bytes + 255
            })
        );
        let over = super::derive_isolate_budget(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            construction,
            costs,
        )
        .unwrap();
        assert_eq!(over.total_bytes, plain.total_bytes + 256);
        assert_eq!(over.margin_bytes, 0);
        limits.isolate_total_bytes += 1;
        let exact = super::validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            construction,
            costs,
        )
        .unwrap();
        assert_eq!(exact.total_bytes, plain.total_bytes + 256);
        assert_eq!(exact.margin_bytes, 0);
        let baked_plain = super::derive_isolate_budget_with_baked_config(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            construction,
            113,
            super::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let baked_funded = super::derive_isolate_budget_with_baked_config(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            construction,
            113,
            costs,
        )
        .unwrap();
        assert_eq!(
            baked_funded.runtime_config_term,
            baked_plain.runtime_config_term
        );
        assert_eq!(
            baked_funded.runtime_config_term - funded.runtime_config_term,
            113 * 128
        );
        assert_eq!(baked_funded.total_bytes - baked_plain.total_bytes, 256);
    }

    #[test]
    fn catalog_costs_refuse_individual_sum_and_population_overflow() {
        let mut limits = roomy_fixture_limits();
        limits.max_concurrent_auctions = 3;
        for costs in [
            super::CatalogResourceCosts {
                resident_bytes: u64::MAX,
                ..super::CatalogResourceCosts::UNCHANGED
            },
            super::CatalogResourceCosts {
                initialization_bytes: u64::MAX,
                ..super::CatalogResourceCosts::UNCHANGED
            },
            super::CatalogResourceCosts {
                schema_validation_per_auction_bytes: u64::MAX,
                ..super::CatalogResourceCosts::UNCHANGED
            },
            super::CatalogResourceCosts {
                context_per_auction_bytes: u64::MAX,
                ..super::CatalogResourceCosts::UNCHANGED
            },
            super::CatalogResourceCosts {
                schema_validation_per_auction_bytes: u64::MAX.saturating_div(2),
                ..super::CatalogResourceCosts::UNCHANGED
            },
            super::CatalogResourceCosts {
                context_per_auction_bytes: u64::MAX.saturating_div(2),
                ..super::CatalogResourceCosts::UNCHANGED
            },
        ] {
            assert_eq!(
                super::derive_isolate_budget(
                    &limits,
                    &fixture_target(),
                    FIXTURE_OUTBOUND_META_BYTES,
                    super::RequestConstructionCosts::default(),
                    costs
                ),
                Err(BudgetError::BudgetArithmeticOverflow)
            );
        }
    }

    #[test]
    fn additive_isolate_sum_preserves_every_existing_and_catalog_term() {
        // Q includes 3 baseline bytes plus two auctions each paying V=31,K=37.
        let terms = literal_isolate_terms();
        assert_eq!(terms.checked_total(), Ok(266));
        for index in 0..IsolateTerm::COUNT {
            let mut increased = terms;
            *isolate_term_fields(&mut increased)[index] += 1;
            assert_eq!(increased.checked_total(), Ok(267), "term {index}");
        }
    }

    fn literal_isolate_terms() -> IsolateTerms {
        IsolateTerms {
            baseline_bytes: 1,
            gvl_bytes: 2,
            auctions_bytes: 139,
            auction_egress_bytes: 5,
            auxiliary_bytes: 7,
            terminal_egress_bytes: 11,
            probe_output_bytes: 13,
            pre_admission_parser_bytes: 17,
            body_streams_bytes: 19,
            catalog_resident_bytes: 23,
            catalog_initialization_bytes: 29,
        }
    }

    fn isolate_term_fields(terms: &mut IsolateTerms) -> [&mut u64; IsolateTerm::COUNT] {
        let IsolateTerms {
            baseline_bytes,
            gvl_bytes,
            auctions_bytes,
            auction_egress_bytes,
            auxiliary_bytes,
            terminal_egress_bytes,
            probe_output_bytes,
            pre_admission_parser_bytes,
            body_streams_bytes,
            catalog_resident_bytes,
            catalog_initialization_bytes,
        } = terms;
        [
            baseline_bytes,
            gvl_bytes,
            auctions_bytes,
            auction_egress_bytes,
            auxiliary_bytes,
            terminal_egress_bytes,
            probe_output_bytes,
            pre_admission_parser_bytes,
            body_streams_bytes,
            catalog_resident_bytes,
            catalog_initialization_bytes,
        ]
    }

    #[test]
    fn isolate_sum_checks_overflow_in_every_named_term() {
        for index in 0..IsolateTerm::COUNT {
            let mut terms = IsolateTerms::default();
            *isolate_term_fields(&mut terms)[index] = u64::MAX;
            assert_eq!(terms.checked_total(), Ok(u64::MAX), "term {index}");
            *isolate_term_fields(&mut terms)[(index + 1).rem_euclid(IsolateTerm::COUNT)] = 1;
            assert_eq!(
                terms.checked_total(),
                Err(BudgetError::BudgetArithmeticOverflow),
                "term {index}"
            );
        }
    }

    #[test]
    fn isolate_entries_preserve_term_identity_names_and_order() {
        let entries = literal_isolate_terms().entries();
        assert_eq!(
            entries,
            [
                (IsolateTerm::Baseline, 1),
                (IsolateTerm::Gvl, 2),
                (IsolateTerm::Auctions, 139),
                (IsolateTerm::AuctionEgress, 5),
                (IsolateTerm::Auxiliary, 7),
                (IsolateTerm::TerminalEgress, 11),
                (IsolateTerm::ProbeOutput, 13),
                (IsolateTerm::PreAdmissionParser, 17),
                (IsolateTerm::BodyStreams, 19),
                (IsolateTerm::CatalogResident, 23),
                (IsolateTerm::CatalogInitialization, 29),
            ]
        );
        assert_eq!(IsolateTerm::COUNT, 11);
        assert_eq!(
            entries.map(|(term, _bytes)| term).as_slice(),
            IsolateTerm::ALL
        );
        assert_eq!(
            entries.map(|(term, _bytes)| term.name()),
            [
                "baseline",
                "gvl",
                "auctions",
                "auction_egress",
                "auxiliary",
                "terminal_egress",
                "probe_output",
                "pre_admission_parser",
                "body_streams",
                "catalog_resident",
                "catalog_initialization",
            ]
        );
    }

    #[test]
    fn zero_isolate_terms_keep_every_report_row() {
        let terms = IsolateTerms::default();
        assert_eq!(terms.checked_total(), Ok(0));
        assert_eq!(terms.entries().len(), 11);
        assert_eq!(
            terms.entries().map(|(term, _bytes)| term).as_slice(),
            IsolateTerm::ALL
        );
        assert_eq!(terms.entries().map(|(_term, bytes)| bytes), [0; 11]);
    }

    #[test]
    fn derived_retains_the_actual_population_scaled_isolate_terms() {
        let expected = IsolateTerms {
            baseline_bytes: 27_328_512,
            gvl_bytes: 43_008_000,
            auctions_bytes: 1_506_107_949_312,
            auction_egress_bytes: 6_442_713_088,
            auxiliary_bytes: 44_807_487_488,
            terminal_egress_bytes: 1_769_472,
            probe_output_bytes: 90_112,
            pre_admission_parser_bytes: 39_518_208,
            body_streams_bytes: 698_405_888,
            catalog_resident_bytes: 0,
            catalog_initialization_bytes: 0,
        };
        for ceiling in [512_000_000, u64::MAX] {
            let mut limits = fixture_limits();
            limits.isolate_total_bytes = ceiling;
            let derived = derive_isolate_budget(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap();
            assert_eq!(derived.isolate_terms, expected);
            assert_eq!(derived.total_bytes, 1_558_168_270_080);
            assert_eq!(
                derived.isolate_terms.checked_total(),
                Ok(derived.total_bytes)
            );
        }
    }

    #[test]
    fn derived_isolate_rows_retain_catalog_costs_and_independent_populations() {
        let mut limits = roomy_fixture_limits();
        limits.max_concurrent_auctions = 3;
        limits.max_active_body_streams = 7;
        let derived = derive_isolate_budget(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts {
                resident_bytes: 23,
                initialization_bytes: 29,
                schema_validation_per_auction_bytes: 31,
                context_per_auction_bytes: 37,
            },
        )
        .unwrap();
        assert_eq!(
            derived.isolate_terms,
            IsolateTerms {
                baseline_bytes: 27_328_512,
                gvl_bytes: 11_776_000,
                auctions_bytes: 70_598_810_328,
                auction_egress_bytes: 6_442_713_088,
                auxiliary_bytes: 44_807_487_488,
                terminal_egress_bytes: 1_769_472,
                probe_output_bytes: 90_112,
                pre_admission_parser_bytes: 39_518_208,
                body_streams_bytes: 152_776_288,
                catalog_resident_bytes: 23,
                catalog_initialization_bytes: 29,
            }
        );
        assert_eq!(derived.isolate_terms.checked_total(), Ok(122_082_269_548));
        assert_eq!(derived.total_bytes, 122_082_269_548);
    }

    #[test]
    fn debug_budget_is_unconditionally_prepaid_in_the_existing_response_phase() {
        let derived = fixture_derived();
        // Independent pre-debug response golden plus the hand-derived debug peak.
        assert_eq!(derived.debug_term, 10_493_088);
        assert_eq!(derived.auction_charges.response, 7_954_235_392 + 10_493_088);
        let unfunded = crate::AuctionBudget::new(23_522_443_620);
        assert_eq!(
            derived.auction_charges.prepay(&unfunded).unwrap_err(),
            crate::ResourceError::Exhausted {
                kind: crate::ChargeKind::Response
            }
        );
        assert_eq!(
            unfunded.in_use(),
            0,
            "failed prepay releases earlier phases"
        );
        let budget = crate::AuctionBudget::new(23_532_936_708);
        let receipts = derived.auction_charges.prepay(&budget).unwrap();
        assert_eq!(receipts.len(), 6);
        assert_eq!(budget.in_use(), 23_532_936_708);
        receipts[5]
            .ensure_capacity(crate::ChargeKind::Response, 7_964_728_480)
            .unwrap();
        drop(receipts);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn debug_budget_uses_funded_call_rows_instead_of_permission_or_shipped_caps() {
        for (calls, debug) in [
            (0, 8_916_128),
            (1, 8_940_768),
            (2, 8_965_408),
            (63, 10_468_448),
            (64, 10_493_088),
            (65, 10_493_088),
            (96, 10_493_088),
        ] {
            let mut limits = roomy_fixture_limits();
            limits.max_outbound_requests = calls;
            // Synthetic funding, not a change to any shipped ceiling.
            limits.final_response_bytes = 96 * 0x0004_0000 * 3;
            let derived = validate_isolate_limits(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap();
            let old_response = 101_711_872 + u64::from(calls) * 122_695_680;
            assert_eq!(derived.debug_term, debug, "calls={calls}");
            assert_eq!(
                derived.auction_charges.response,
                old_response + debug,
                "calls={calls}"
            );
            assert_eq!(
                derived.call_side_term,
                0x0008_0000 + u64::from(calls) * 123_232_032
            );
        }
    }

    #[test]
    fn debug_budget_preserves_nondebug_terms_and_scales_with_auction_concurrency() {
        for concurrency in [1, 2, 64] {
            let mut limits = roomy_fixture_limits();
            limits.max_concurrent_auctions = concurrency;
            let derived = validate_isolate_limits(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts {
                    fixed_per_auction_bytes: 103,
                    reusable_scratch_bytes: 257,
                    retained_per_scope_bytes: 509,
                },
                crate::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap();
            let charges = derived.auction_charges;
            assert_eq!(derived.debug_term, 10_493_088);
            assert_eq!(charges.config, 274_587_648);
            assert_eq!(charges.ingress, 323_740_772);
            assert_eq!(charges.privacy, 44_040_192 + 103);
            assert_eq!(charges.projection, 14_890_988_800 + 257 + 50 * 509);
            assert_eq!(charges.outbound, 34_850_816);
            assert_eq!(charges.response, 7_954_235_392 + 10_493_088);
            assert_eq!(derived.request_side_term, 15_214_729_572 + 257 + 50 * 509);
            assert_eq!(derived.call_side_term, 7_887_374_336);
            assert_eq!(derived.bookkeeping_term, 101_711_872);
            assert_eq!(derived.assembly_scratch_bytes, 50_331_648);
            assert_eq!(derived.per_stream_peak, 21_825_184);
            assert_eq!(derived.egress_term, 6_442_713_088);
            assert_eq!(derived.aux_term, 44_807_487_488);
            assert_eq!(derived.terminal_term, 1_769_472);
            assert_eq!(derived.probe_output_term, 90_112);
            assert_eq!(derived.pre_admission_parser_term, 39_518_208);
            assert_eq!(
                derived.gvl_terms,
                (4 + u64::from(concurrency) + 16) * 512_000
            );
            let old_total =
                1_557_496_712_448 - (64 - u64::from(concurrency)) * (23_522_443_620 + 512_000);
            let construction = 103 + 257 + 50 * 509;
            assert_eq!(
                derived.per_auction_peak,
                23_522_443_620 + construction + 10_493_088
            );
            assert_eq!(
                derived.total_bytes,
                old_total + u64::from(concurrency) * (construction + 10_493_088)
            );
        }
    }

    #[test]
    fn derivation_keeps_terms_for_an_over_ceiling_boot_without_changing_validation() {
        let limits = fixture_limits();
        let target = fixture_target();
        let terms = derive_isolate_budget(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let roomy = fixture_derived();
        assert_eq!(terms.per_auction_peak, roomy.per_auction_peak);
        assert_eq!(terms.total_bytes, roomy.total_bytes);
        assert_eq!(terms.margin_bytes, 0);
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: terms.total_bytes,
                ceiling: limits.isolate_total_bytes
            })
        );
    }

    #[test]
    fn disjoint_phase_charges_fund_the_complete_auction() {
        let limits = fixture_limits();
        let derived = derive_isolate_budget(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let charges = derived.auction_charges;
        assert_eq!(charges.total().unwrap(), derived.per_auction_peak);
        assert_eq!(charges.config, derived.runtime_config_term);
        assert_eq!(
            charges.ingress + charges.projection,
            derived.request_side_term
        );
        assert_eq!(
            charges.outbound + charges.response,
            derived.call_side_term + derived.bookkeeping_term + derived.debug_term
        );
        assert_eq!(
            charges.privacy,
            super::PRIVACY_FIXED_BYTES
                + limits.inbound_request_bytes * super::PRIVACY_REQUEST_FACTOR
        );
        assert!(
            charges.response
                >= u64::from(limits.max_outbound_requests)
                    * limits.max_decoded_bytes
                    * (super::RESPONSE_PARSE_FACTOR + super::RESPONSE_ASSEMBLY_FACTOR)
        );
        let budget = crate::AuctionBudget::new(derived.per_auction_peak);
        let held = charges.prepay(&budget).unwrap();
        assert_eq!(budget.in_use(), derived.per_auction_peak);
        drop(held);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn construction_costs_are_disjoint_and_scale_with_owning_populations() {
        let limits = roomy_fixture_limits();
        let target = fixture_target();
        let zero = super::RequestConstructionCosts::default();
        let derive = |row: &ResolvedIsolateLimits, costs| {
            derive_isolate_budget(
                row,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                costs,
                crate::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap()
        };
        let base = derive(&limits, zero);
        for costs in [
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: 103,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: 0,
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: 0,
                reusable_scratch_bytes: 1009,
                retained_per_scope_bytes: 0,
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: 0,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: 78,
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: 103,
                reusable_scratch_bytes: 1009,
                retained_per_scope_bytes: 78,
            },
        ] {
            for bidders in [1, 2, limits.max_bidders_per_auction] {
                let mut row = limits;
                row.max_bidders_per_auction = bidders;
                let unfunded = derive(&row, zero);
                let funded = derive(&row, costs);
                assert_eq!(funded.construction_costs, costs);
                let projection_added = u64::from(bidders)
                    .checked_mul(costs.retained_per_scope_bytes)
                    .unwrap()
                    .checked_add(costs.reusable_scratch_bytes)
                    .unwrap();
                assert_eq!(
                    funded.auction_charges.privacy - unfunded.auction_charges.privacy,
                    costs.fixed_per_auction_bytes
                );
                assert_eq!(
                    funded.auction_charges.projection - unfunded.auction_charges.projection,
                    projection_added
                );
                assert_eq!(
                    funded.request_side_term - unfunded.request_side_term,
                    projection_added
                );
                assert_eq!(
                    funded.auction_charges.config,
                    unfunded.auction_charges.config
                );
                assert_eq!(
                    funded.auction_charges.ingress,
                    unfunded.auction_charges.ingress
                );
                assert_eq!(
                    funded.auction_charges.outbound,
                    unfunded.auction_charges.outbound
                );
                assert_eq!(
                    funded.auction_charges.response,
                    unfunded.auction_charges.response
                );
                assert_eq!(funded.aux_term, unfunded.aux_term);
                assert_eq!(funded.gvl_terms, unfunded.gvl_terms);
                let added = costs
                    .fixed_per_auction_bytes
                    .checked_add(projection_added)
                    .unwrap();
                assert_eq!(funded.per_auction_peak - unfunded.per_auction_peak, added);
                assert_eq!(
                    funded.total_bytes - unfunded.total_bytes,
                    added
                        .checked_mul(u64::from(row.max_concurrent_auctions))
                        .unwrap()
                );
                assert_eq!(
                    funded.auction_charges.total().unwrap(),
                    funded.per_auction_peak
                );
            }
        }
        assert_eq!(base, fixture_derived());
    }

    #[test]
    fn construction_costs_are_prepaid_and_partial_refusal_refunds_every_phase() {
        let costs = super::RequestConstructionCosts {
            fixed_per_auction_bytes: 103,
            reusable_scratch_bytes: 1009,
            retained_per_scope_bytes: 78,
        };
        let derived = derive_isolate_budget(
            &roomy_fixture_limits(),
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let charges = derived.auction_charges;
        let budget = crate::AuctionBudget::new(derived.per_auction_peak);
        let receipts = charges.prepay(&budget).unwrap();
        receipts[2]
            .ensure_capacity(crate::ChargeKind::Privacy, charges.privacy)
            .unwrap();
        receipts[3]
            .ensure_capacity(crate::ChargeKind::Projection, charges.projection)
            .unwrap();
        assert_eq!(budget.in_use(), derived.per_auction_peak);
        drop(receipts);
        assert_eq!(budget.in_use(), 0);

        let projection_short =
            charges.config + charges.ingress + charges.privacy + charges.projection - 1;
        let small = crate::AuctionBudget::new(projection_short);
        assert_eq!(
            charges.prepay(&small).unwrap_err(),
            crate::ResourceError::Exhausted {
                kind: crate::ChargeKind::Projection,
            }
        );
        assert_eq!(small.in_use(), 0);
    }

    #[test]
    fn construction_costs_survive_baked_config_derivation_and_exact_ceiling_validation() {
        let costs = super::RequestConstructionCosts {
            fixed_per_auction_bytes: 103,
            reusable_scratch_bytes: 1009,
            retained_per_scope_bytes: 78,
        };
        let mut limits = roomy_fixture_limits();
        let target = fixture_target();
        let plain = derive_isolate_budget(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let allowance = 1007_u64.checked_mul(super::CONFIG_PARSE_FACTOR).unwrap();
        let mut baked_limits = limits;
        baked_limits.aux_route_peak_bytes = baked_limits
            .aux_route_peak_bytes
            .checked_add(allowance)
            .unwrap();
        let baked = super::derive_isolate_budget_with_baked_config(
            &baked_limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            1007,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(baked.construction_costs, costs);
        assert_eq!(baked.auction_charges.privacy, plain.auction_charges.privacy);
        assert_eq!(
            baked.auction_charges.projection,
            plain.auction_charges.projection
        );
        assert_eq!(
            baked.per_auction_peak - plain.per_auction_peak,
            1007 * super::CONFIG_PARSE_FACTOR
        );
        limits.isolate_total_bytes = plain.total_bytes;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                costs,
                crate::CatalogResourceCosts::UNCHANGED
            )
            .unwrap()
            .margin_bytes,
            0
        );
        limits.isolate_total_bytes -= 1;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                costs,
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: plain.total_bytes,
                ceiling: limits.isolate_total_bytes,
            })
        );
    }

    #[test]
    fn construction_costs_refuse_phase_total_and_population_overflow() {
        let limits = roomy_fixture_limits();
        let target = fixture_target();
        let zero = super::RequestConstructionCosts::default();
        let base = derive_isolate_budget(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            zero,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let headroom = u64::MAX - base.per_auction_peak;
        for costs in [
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: u64::MAX,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: 0,
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: 0,
                reusable_scratch_bytes: u64::MAX,
                retained_per_scope_bytes: 0,
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: 0,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: u64::MAX,
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: 0,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: u64::MAX
                    .checked_div(u64::from(limits.max_bidders_per_auction))
                    .unwrap(),
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: headroom,
                reusable_scratch_bytes: 1,
                retained_per_scope_bytes: 0,
            },
            super::RequestConstructionCosts {
                fixed_per_auction_bytes: headroom,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: 0,
            },
        ] {
            assert_eq!(
                derive_isolate_budget(
                    &limits,
                    &target,
                    FIXTURE_OUTBOUND_META_BYTES,
                    costs,
                    crate::CatalogResourceCosts::UNCHANGED
                ),
                Err(BudgetError::BudgetArithmeticOverflow)
            );
            assert_eq!(
                validate_isolate_limits(
                    &limits,
                    &target,
                    FIXTURE_OUTBOUND_META_BYTES,
                    costs,
                    crate::CatalogResourceCosts::UNCHANGED
                ),
                Err(BudgetError::BudgetArithmeticOverflow)
            );
            assert_eq!(
                super::derive_isolate_budget_with_baked_config(
                    &limits,
                    &target,
                    FIXTURE_OUTBOUND_META_BYTES,
                    costs,
                    1007,
                    crate::CatalogResourceCosts::UNCHANGED
                ),
                Err(BudgetError::BudgetArithmeticOverflow)
            );
        }
    }

    #[test]
    fn pinned_json_number_wire_width_covers_finite_exponents_and_mantissas() {
        // Exercise the pinned serializer, not a copied formatter implementation.
        const NUMBER_WIRE_MAX: usize = 24;
        const MANTISSA_MASK: u64 = (1_u64 << 52_u32) - 1;
        assert_eq!(super::MAX_NUMBER_WIRE_BYTES, 24);
        let mantissas = [
            0,
            1,
            (1_u64 << 51_u32) - 1,
            1_u64 << 51_u32,
            0x0005_5555_5555_5555,
            0x000a_aaaa_aaaa_aaaa,
            MANTISSA_MASK - 1,
            MANTISSA_MASK,
        ];
        let mut widest = 0;
        for exponent in 0_u64..=0x07fe {
            for mantissa in mantissas {
                for sign in [0, 1_u64 << 63_u32] {
                    let value = f64::from_bits(sign | (exponent << 52) | mantissa);
                    assert!(value.is_finite());
                    let encoded = serde_json::to_vec(&value).unwrap();
                    assert!(
                        encoded.len() <= NUMBER_WIRE_MAX,
                        "finite bits {:016x} need {} wire bytes",
                        value.to_bits(),
                        encoded.len()
                    );
                    widest = widest.max(encoded.len());
                }
            }
        }
        assert_eq!(
            widest, NUMBER_WIRE_MAX,
            "the maximum is observed, not assumed"
        );
        assert!(serde_json::to_vec(&u64::MAX).unwrap().len() <= NUMBER_WIRE_MAX);
        assert!(serde_json::to_vec(&i64::MIN).unwrap().len() <= NUMBER_WIRE_MAX);
        let compact: serde_json::Value = serde_json::from_slice(b"1e15").unwrap();
        assert_eq!(serde_json::to_vec(&compact).unwrap(), b"1000000000000000.0");
    }

    #[test]
    fn normalization_budget_funds_number_canonicalization_per_input_node() {
        let mut limits = roomy_fixture_limits();
        limits.inbound_request_bytes = 0x0004_0000;
        limits.max_copied_header_bytes = 4096;
        limits.max_parsed_nodes = 0x0001_0000;
        // Independent: each input node already has at least one source byte;
        // a pinned numeric token can need 24, so reserve 23 additional bytes.
        let normalized = 3 * limits.inbound_request_bytes
            + 8 * u64::from(limits.max_copied_header_bytes)
            + 189
            + 23 * u64::from(limits.max_parsed_nodes);
        for bidders in [1, 2, 50] {
            limits.max_bidders_per_auction = bidders;
            let derived = derive_isolate_budget(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap();
            assert_eq!(
                derived.auction_charges.projection,
                u64::from(bidders) * normalized * 128
            );
        }
    }

    #[test]
    fn normalization_allowance_funds_small_body_with_larger_copied_headers() {
        let mut limits = roomy_fixture_limits();
        limits.inbound_request_bytes = 1024;
        limits.max_copied_header_bytes = 4096;
        let normalized =
            super::normalization_wire_allowance(1024, 4096, u64::from(limits.max_parsed_nodes))
                .unwrap();
        // Independent: six-byte JSON escapes for the copied pair, two
        // bounded domain copies, and the literal implicit-field syntax/IP.
        assert_eq!(
            normalized,
            1024 + 6 * 4096 + 2 * (1024 + 4096) + 189 + 23 * u64::from(limits.max_parsed_nodes)
        );
        assert_eq!(super::NORMALIZATION_IMPLICIT_NODES, 30);
        for bidders in [1, 2, 50] {
            limits.max_bidders_per_auction = bidders;
            let derived = derive_isolate_budget(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap();
            assert_eq!(
                derived.auction_charges.projection,
                u64::from(bidders) * normalized * 128
            );
            let normalized_nodes = (u64::from(limits.max_parsed_nodes) + 30) * NODE_RETAINED_BYTES;
            let supplement = limits.max_outbound_raw_bytes * 2
                + u64::from(limits.max_parsed_nodes) * NODE_RETAINED_BYTES;
            let tree = normalized * 2 + normalized_nodes;
            let expected_ingress = limits.inbound_request_bytes
                + u64::from(limits.max_admitted_header_bytes)
                + u64::from(limits.max_admitted_header_count)
                    * u64::from(fixture_target().header_entry_overhead_bytes)
                + 4096 * 5
                + normalized * limits.inbound_parse_factor
                + normalized_nodes
                + 2 * tree
                + normalized * 8
                + normalized_nodes
                + 2 * (tree + supplement);
            assert_eq!(derived.auction_charges.ingress, expected_ingress);
            assert_eq!(
                derived.request_side_term,
                expected_ingress + derived.auction_charges.projection
            );
        }
    }

    #[test]
    fn normalization_allowance_checks_every_growth_operation() {
        assert_eq!(super::normalization_wire_allowance(0, 0, 0).unwrap(), 189);
        for (inbound, copied, nodes) in [
            (u64::MAX, 0, 0),
            (0, u64::MAX, 0),
            (u64::MAX.div_euclid(3), 0, 0),
            (u64::MAX.div_euclid(3) - 189, 4096, 0),
            (0, 0, u64::MAX.div_euclid(23) + 1),
            (0, 0, u64::MAX.div_euclid(23)),
        ] {
            assert_eq!(
                super::normalization_wire_allowance(inbound, copied, nodes),
                Err(BudgetError::BudgetArithmeticOverflow)
            );
        }
        let nodes = (u64::MAX - 189).div_euclid(23);
        assert_eq!(
            super::normalization_wire_allowance(0, 0, nodes).unwrap(),
            nodes.checked_mul(23).unwrap().checked_add(189).unwrap()
        );
    }

    #[test]
    fn baked_config_allowance_is_checked_and_scales_with_auction_population() {
        let limits = fixture_limits();
        let target = fixture_target();
        let plain = derive_isolate_budget(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let allowance = 1007_u64.checked_mul(super::CONFIG_PARSE_FACTOR).unwrap();
        let mut baked_limits = limits;
        baked_limits.aux_route_peak_bytes = baked_limits
            .aux_route_peak_bytes
            .checked_add(allowance)
            .unwrap();
        let baked = super::derive_isolate_budget_with_baked_config(
            &baked_limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            1007,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(
            baked.auction_charges.config - plain.auction_charges.config,
            allowance
        );
        assert_eq!(baked.per_auction_peak - plain.per_auction_peak, allowance);
        assert_eq!(
            baked.aux_term - plain.aux_term,
            (u64::from(limits.aux_route_permits)
                + u64::from(
                    limits
                        .aux_route_permits
                        .max(crate::PROBE_CONFIG_MIN_PERMITS)
                ))
                * allowance
                * u64::from(target.egress_handoff_factor)
        );
        assert_eq!(
            baked.total_bytes - plain.total_bytes,
            allowance * u64::from(limits.max_concurrent_auctions) + baked.aux_term - plain.aux_term
        );
        assert_eq!(
            baked.auction_charges.total().unwrap(),
            baked.per_auction_peak
        );
        assert_eq!(baked.margin_bytes, 0);
        assert_eq!(
            super::derive_isolate_budget_with_baked_config(
                &baked_limits,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                u64::MAX,
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetArithmeticOverflow)
        );
    }

    #[test]
    fn derivation_does_not_relax_arithmetic_scalar_or_layout_validation() {
        let target = fixture_target();
        let mut overflow = fixture_limits();
        overflow.max_decoded_bytes = u64::MAX;
        assert_eq!(
            derive_isolate_budget(
                &overflow,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetArithmeticOverflow)
        );
        let mut invalid = fixture_limits();
        invalid.max_concurrent_auctions = 0;
        assert_eq!(
            derive_isolate_budget(
                &invalid,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::MaxConcurrentAuctions,
                value: 0,
                minimum: 1
            })
        );
        let mut small_response = fixture_limits();
        small_response.final_response_bytes = 1;
        assert!(matches!(
            derive_isolate_budget(
                &small_response,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::FinalResponseBelowAssemblyFloor { .. })
        ));
    }

    /// The synthetic fixture reports overcommit with exact operands.
    #[test]
    fn synthetic_row_is_over_its_own_ceiling() {
        let limits = fixture_limits();
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: 1_558_168_270_080,
                ceiling: limits.isolate_total_bytes,
            }),
        );
    }

    #[test]
    fn brotli_state_bytes_is_fixed_charge_plus_window() {
        assert_eq!(brotli_state_bytes(10), Ok(16_778_240));
        assert_eq!(brotli_state_bytes(22), Ok(20_971_520));
        // The decoder STATE alone. The per-stream total at this window
        // is 34_365_440, which adds the transport and decoded buffers
        // and the chunk staging on top.
        assert_eq!(brotli_state_bytes(24), Ok(2 * BYTES_16_MIB));
        assert_eq!(
            brotli_state_bytes(24),
            Ok(BROTLI_DECODER_FIXED_CHARGE_BYTES + BYTES_16_MIB)
        );
    }

    #[test]
    fn brotli_state_bytes_rejects_out_of_range_wbits() {
        for wbits in [9_u8, 25, 64, 255] {
            assert_eq!(
                brotli_state_bytes(wbits),
                Err(BudgetError::BrotliWbitsOutOfRange { wbits }),
                "wbits {wbits} must be rejected"
            );
        }
        assert_eq!(brotli_state_bytes(MIN_BROTLI_WBITS), Ok(16_778_240));
        assert_eq!(brotli_state_bytes(MAX_BROTLI_WBITS), Ok(2 * BYTES_16_MIB));
    }

    #[test]
    fn per_stream_bytes_matches_the_worked_values() {
        // The fixture's 256 KiB caps at the default window, guest bound
        // zero so auctioneer's own chain is the charge.
        assert_eq!(
            per_stream_bytes(CAP_256_KIB, CAP_256_KIB, 22, 1, 0),
            Ok(21_782_528)
        );
        // At the maximum window: the 33_554_432 decoder state plus
        // 811_008 of transport/decoded buffers and chunk staging.
        assert_eq!(
            per_stream_bytes(CAP_256_KIB, CAP_256_KIB, 24, 1, 0),
            Ok(34_365_440)
        );
        assert_eq!(
            per_stream_bytes(CAP_256_KIB, CAP_256_KIB, 24, 1, 0),
            brotli_state_bytes(24).map(|state| state + 811_008)
        );
    }

    #[test]
    fn per_stream_bytes_charges_the_larger_of_chain_and_guest_bound() {
        let chain_wins = per_stream_bytes(CAP_256_KIB, CAP_256_KIB, 22, 1, 1_024)
            .expect("a small guest bound loses the max");
        assert_eq!(chain_wins, 21_782_528);

        let guest_wins = per_stream_bytes(CAP_256_KIB, CAP_256_KIB, 22, 1, 40_000_000)
            .expect("a large guest bound wins the max");
        assert_eq!(guest_wins, CAP_256_KIB + CAP_256_KIB + 40_000_000);
    }

    #[test]
    fn per_stream_bytes_rejects_bad_wbits_and_layers() {
        assert_eq!(
            per_stream_bytes(0, 0, 25, 1, 0),
            Err(BudgetError::BrotliWbitsOutOfRange { wbits: 25 })
        );
        assert_eq!(
            per_stream_bytes(0, 0, 22, 4, 0),
            Err(BudgetError::LimitAboveMaximum {
                field: LimitField::MaxBrotliLayers,
                value: 4,
                maximum: 3,
            })
        );
    }

    #[test]
    fn per_stream_bytes_reports_overflow_rather_than_wrapping() {
        assert_eq!(
            per_stream_bytes(u64::MAX, 1, 22, 1, 0),
            Err(BudgetError::BudgetArithmeticOverflow)
        );
    }

    /// The aux floor and `runtime_config_term` charge the SAME
    /// extraction, so the aux peak must cover that term.
    ///
    /// The two are computed in different places — `min_aux_route_peak_bytes`
    /// is a `const fn` an adapter calls to DECLARE a row, and
    /// `runtime_config_term` is computed by the validator FROM a
    /// resolved row — so this is what stops them drifting apart. It is
    /// not a tautology: it asserts a relation between two separate
    /// computations, and it fails if either changes without the other.
    #[test]
    fn every_aux_route_peak_funds_the_config_extraction() {
        let limits = roomy_fixture_limits();
        let derived = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the row validates once the ceiling is lifted");
        let floor = min_aux_route_peak_bytes(
            limits.max_runtime_config_bytes,
            limits.max_runtime_config_blob_bytes,
            u64::from(limits.max_config_json_nodes),
        );
        assert!(
            floor >= derived.runtime_config_term,
            "the aux floor is {floor} B but one config extraction is \
             {} B: an aux route performs that extraction, so a peak \
             below it under-reserves memory the route needs on every request",
            derived.runtime_config_term,
        );
        // And the synthetic fixture satisfies the floor, which is what the
        // validator enforces.
        assert!(
            limits.aux_route_peak_bytes >= floor,
            "the row declares {} B against a {floor} B floor",
            limits.aux_route_peak_bytes,
        );
    }

    /// A row declaring less than the floor is REFUSED, not warned
    /// about. Under-reserving memory the code needs on every request is
    /// the fatal class, unlike the ceiling overshoot.
    #[test]
    fn an_aux_peak_below_the_extraction_floor_is_refused() {
        let mut limits = roomy_fixture_limits();
        let floor = min_aux_route_peak_bytes(
            limits.max_runtime_config_bytes,
            limits.max_runtime_config_blob_bytes,
            u64::from(limits.max_config_json_nodes),
        );
        // The figure this replaced, which funded under a third of the
        // extraction its own routes perform.
        limits.aux_route_peak_bytes = 7_725_056;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::AuxRoutePeakBelowExtractionFloor {
                configured: 7_725_056,
                required: floor,
            }),
            "the previous UNVERIFIED figure must now be refused by name",
        );
    }

    #[test]
    fn validator_regenerates_the_synthetic_worked_row() {
        let derived = fixture_derived();

        assert_eq!(
            derived.runtime_config_term,
            257 * BYTES_1_MIB + 8_192 * CONFIG_NODE_RETAINED_BYTES,
        );
        assert_eq!(derived.request_side_term, 15_214_729_572);
        assert_eq!(derived.call_side_term, 7_887_374_336);
        assert_eq!(derived.bookkeeping_term, 101_711_872);
        assert_eq!(derived.per_auction_peak, 23_532_936_708);
        assert_eq!(derived.assembly_scratch_bytes, FINAL_RESPONSE_BYTES);
        assert_eq!(derived.decoder_chain_bytes, 21_258_240);
        assert_eq!(derived.per_stream_peak, 21_825_184);
        assert_eq!(derived.h2_slot_peak, 102_912);
        assert_eq!(derived.h1_slot_peak, 49_152);
        assert_eq!(derived.pre_admission_parser_term, 39_518_208);

        // Synthetic populations: 64 concurrent auctions, 64 egress
        // permits, 16 aux permits, and 384 static permits. Each scales
        // its own unit cost; the fixture does not describe a deployment.
        assert_eq!(derived.egress_term, 6_442_713_088);
        assert_eq!(derived.aux_term, 44_807_487_488);
        assert_eq!(derived.terminal_term, 1_769_472);
        assert_eq!(derived.probe_output_term, 90_112);

        // Resident GVL is charged per DISTINCT parsed list: the cache's
        // four entries plus one each concurrent auction and aux request
        // can pin past eviction: `(4 + 64 + 16) x 512_000`.
        assert_eq!(derived.gvl_terms, 43_008_000);

        // The synthetic total and its margin. `checked_sub` underflows on
        // the fixture's original `512_000_000`, the `BudgetOverCeiling` that
        // `synthetic_row_is_over_its_own_ceiling` asserts; here the
        // ceiling is lifted, so the margin is the lifted ceiling's
        // headroom and carries no meaning of its own.
        assert_eq!(derived.total_bytes, 1_558_168_270_080);
        assert_eq!(derived.margin_bytes, u64::MAX - 1_558_168_270_080);

        // Population-scaled ordering, not just pinned constants.
        assert!(
            u64::from(fixture_limits().max_concurrent_auctions) * derived.per_auction_peak
                > derived.egress_term,
            "concurrent auctions are the largest charge on this row",
        );
        assert!(
            derived.egress_term
                > u64::from(fixture_limits().max_active_body_streams) * derived.per_stream_peak,
            "egress stays ahead of the fanout wave gate",
        );
        assert_eq!(
            u64::try_from(derived.max_outbound_raw_bytes_usize),
            Ok(CAP_256_KIB)
        );
    }

    /// The static-class egress term is CHARGED INTO the total, not
    /// merely computed beside it.
    ///
    /// `validator_regenerates_the_synthetic_worked_row` pins `terminal_term`
    /// and `total_bytes` as two independent constants, which is exactly
    /// what a term computed correctly and then dropped from the sum
    /// would still satisfy. This is a differential instead: the same
    /// row derived twice, once at the synthetic terminal-class inputs and
    /// once with them zeroed, and the totals must differ by exactly the
    /// term. Remove `terminal_term` from the addition in
    /// `validate_isolate_limits` and the two totals become equal while
    /// the term itself still reads `313_344` — this fails, that row does
    /// not.
    #[test]
    fn static_egress_term_is_charged_into_the_total() {
        let limits = roomy_fixture_limits();
        let target = fixture_target();
        let charged = fixture_derived();

        // Zeroing the permit count removes the whole term: the charge is
        // `permits x (refusal_body x handoff + transmit)`, so no permits
        // means no refusal buffers and no transmit state either.
        let mut no_terminal = limits;
        no_terminal.terminal_response_permits = 0;
        let without = validate_isolate_limits(
            &no_terminal,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a row that reserves no static-egress slots still validates");

        assert_eq!(
            without.terminal_term, 0,
            "zero permits reserve no static-class egress",
        );
        assert_eq!(
            charged.total_bytes - without.total_bytes,
            charged.terminal_term,
            "the static-egress term enters the isolate total once, at full width",
        );
        // Stated from the other side too, so a total that moved by the
        // right amount for the wrong reason is still caught: nothing
        // else in the row may move when only the terminal inputs do.
        assert_eq!(without.egress_term, charged.egress_term);
        assert_eq!(without.aux_term, charged.aux_term);
        assert_eq!(without.per_auction_peak, charged.per_auction_peak);
        assert_eq!(without.per_stream_peak, charged.per_stream_peak);
        assert_eq!(without.gvl_terms, charged.gvl_terms);
        assert_eq!(
            without.pre_admission_parser_term,
            charged.pre_admission_parser_term
        );
        // The freed bytes land in the margin, which is what makes the
        // term's presence in the total observable to the ceiling check
        // rather than only to this arithmetic.
        assert_eq!(
            without.margin_bytes - charged.margin_bytes,
            charged.terminal_term,
        );

        // A second differential on the body size alone, so a term wired
        // in through the permit count but with a constant body would
        // also fail. One more refusal byte costs one handoff factor per
        // permit, and that too reaches the total.
        let mut wider_body = limits;
        wider_body.refusal_body_bytes += 1;
        let wider = validate_isolate_limits(
            &wider_body,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a one-byte-wider refusal body still fits");
        let one_byte_per_permit =
            u64::from(limits.terminal_response_permits) * u64::from(target.egress_handoff_factor);
        assert_eq!(
            wider.terminal_term - charged.terminal_term,
            one_byte_per_permit,
        );
        assert_eq!(wider.total_bytes - charged.total_bytes, one_byte_per_permit,);
    }

    /// All THREE runtime-config charges are present in
    /// `runtime_config_term`, and the term reaches the isolate total.
    ///
    /// `validator_regenerates_the_synthetic_worked_row` pins the term at
    /// `257 * BYTES_1_MIB` plus the independent node charge, and pins the
    /// total at one more literal. This is exactly what a term computed
    /// with a charge MISSING would still
    /// satisfy once both literals were re-pinned — and a merge on this
    /// branch did drop a charge with the suite staying green, because
    /// the row sets `max_runtime_config_bytes` and
    /// `max_runtime_config_blob_bytes` to the SAME `BYTES_1_MIB`, so no
    /// pinned total distinguishes which cap a charge scaled with.
    ///
    /// This is differential instead, and the two caps are moved
    /// SEPARATELY so each of the three charges is isolated by a
    /// perturbation only it responds to:
    ///
    /// - the raw envelope, charged once at `max_runtime_config_bytes`,
    /// - the extractor's whole-envelope `serde_json::Value` precheck,
    ///   charged at `max_runtime_config_bytes x
    ///   CONFIG_ENVELOPE_VALUE_FACTOR`,
    /// - the typed parse, charged at `max_runtime_config_blob_bytes x
    ///   CONFIG_PARSE_FACTOR`.
    ///
    /// The differentials run FIRST, before the symbolic restatement of
    /// the whole sum, so that deleting a charge fails on the assertion
    /// that NAMES it rather than on a total that only disagrees by a
    /// number. Moving the raw cap alone moves the term by `1 +
    /// CONFIG_ENVELOPE_VALUE_FACTOR`, which separates the first two
    /// charges from the third; zeroing that cap then splits the two
    /// from each other. Delete any one `add`/`mul` from the formula in
    /// `validate_isolate_limits` and one of the assertions below
    /// reports WHICH charge went missing by name.
    #[test]
    fn every_runtime_config_charge_is_in_the_term_and_in_the_total() {
        let (limits, target, charged) = differential_row();
        let raw_cap = limits.max_runtime_config_bytes;

        // Differential one: the RAW cap alone, which the raw charge and
        // the precheck charge both scale with and the parse charge does
        // not. One extra raw byte must cost `1 + factor`. A dropped
        // precheck charge leaves this at 1; a dropped raw charge leaves
        // it at the factor.
        let mut wider_raw = limits;
        wider_raw.max_runtime_config_bytes = raw_cap + 1;
        let wider = validate_isolate_limits(
            &wider_raw,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a one-byte-wider runtime-config envelope cap still validates");
        let per_raw_byte = 1 + CONFIG_ENVELOPE_VALUE_FACTOR;
        assert_eq!(
            wider.runtime_config_term - charged.runtime_config_term,
            per_raw_byte,
            "one more raw envelope byte must cost 1 B for the raw envelope itself plus \
             {CONFIG_ENVELOPE_VALUE_FACTOR} B for the precheck Value; it cost \
             {} B, so one of those two charges is MISSING from the sum (a sole raw \
             charge gives 1, a sole precheck charge gives {CONFIG_ENVELOPE_VALUE_FACTOR})",
            wider.runtime_config_term - charged.runtime_config_term,
        );
        // Nothing else in the row may move when only this cap does, so a
        // term that moved by the right amount for the wrong reason is
        // still caught.
        assert_eq!(wider.request_side_term, charged.request_side_term);
        assert_eq!(wider.call_side_term, charged.call_side_term);
        assert_eq!(wider.egress_term, charged.egress_term);
        assert_eq!(wider.gvl_terms, charged.gvl_terms);

        // The remaining charges are asserted in
        // `the_blob_charge_and_the_symbolic_sum`, which shares this
        // row through `differential_row()`. Split only to keep each
        // function readable; both halves must hold.
    }

    /// The blob-cap differential and the symbolic restatement of the
    /// whole sum, continuing
    /// [`every_runtime_config_charge_is_in_the_term_and_in_the_total`].
    #[test]
    fn the_blob_charge_and_the_symbolic_sum() {
        let (limits, target, charged) = differential_row();
        let raw_cap = limits.max_runtime_config_bytes;
        let blob_cap = limits.max_runtime_config_blob_bytes;

        // Differential two: the BLOB cap alone, which only the parse
        // charge scales with. A dropped parse charge leaves this at 0.
        let mut wider_blob = limits;
        wider_blob.max_runtime_config_blob_bytes = blob_cap + 1;
        let blobbier = validate_isolate_limits(
            &wider_blob,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a one-byte-wider runtime-config blob cap still validates");
        assert_eq!(
            blobbier.runtime_config_term - charged.runtime_config_term,
            CONFIG_PARSE_FACTOR,
            "one more blob byte must cost {CONFIG_PARSE_FACTOR} B for the typed parse; \
             it cost {} B, so the CONFIG_PARSE_FACTOR charge is MISSING from the sum",
            blobbier.runtime_config_term - charged.runtime_config_term,
        );

        // Symbolic, not a literal: the whole term is the sum of exactly
        // these four charges, each named. It backstops the
        // differentials above — those pin each charge's RATE, this pins
        // the sum's WIDTH, so a charge present at the right rate but
        // scaled from the wrong base also fails.
        //
        // The node charge is the one that does NOT scale with a byte
        // cap: the extraction node population is capped independently,
        // so it is an absolute ceiling the per-byte charges cannot
        // cover once the byte caps drop below the shipped row.
        let raw_charge = raw_cap;
        let precheck_charge = raw_cap * CONFIG_ENVELOPE_VALUE_FACTOR;
        let parse_charge = blob_cap * CONFIG_PARSE_FACTOR;
        let node_charge = u64::from(limits.max_config_json_nodes) * CONFIG_NODE_RETAINED_BYTES;
        assert_eq!(
            charged.runtime_config_term,
            raw_charge + precheck_charge + parse_charge + node_charge,
            "runtime_config_term must be the raw envelope ({raw_charge} B) plus the \
             extractor's whole-envelope Value precheck ({precheck_charge} B at \
             CONFIG_ENVELOPE_VALUE_FACTOR = {CONFIG_ENVELOPE_VALUE_FACTOR}) plus the \
             typed parse ({parse_charge} B at CONFIG_PARSE_FACTOR = \
             {CONFIG_PARSE_FACTOR}) plus the node ceiling ({node_charge} B at \
             CONFIG_NODE_RETAINED_BYTES = {CONFIG_NODE_RETAINED_BYTES})",
        );

        // The two caps are moved to DIFFERENT values, which is what no
        // other test on this row does: with both at `BYTES_1_MIB` a
        // charge scaled from the wrong cap is invisible, and that is
        // precisely the ambiguity under which a charge was dropped here
        // once already. At a narrower blob cap each charge is pinned
        // against the cap it NAMES, so a precheck charge misattributed
        // to the blob cap fails.
        let narrow_blob_cap = BYTES_64_KIB_U64;
        assert_ne!(
            narrow_blob_cap, raw_cap,
            "the split row is only informative while the two caps differ",
        );
        let mut split = limits;
        split.max_runtime_config_blob_bytes = narrow_blob_cap;
        let split_derived = validate_isolate_limits(
            &split,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a narrower blob cap than envelope cap still validates");
        assert_eq!(
            split_derived.runtime_config_term,
            raw_cap
                + raw_cap * CONFIG_ENVELOPE_VALUE_FACTOR
                + narrow_blob_cap * CONFIG_PARSE_FACTOR
                // Unchanged by either cap, which is this charge's whole
                // character: narrowing the blob cap to 64 KiB leaves the
                // node ceiling exactly where it was. That is why the
                // per-byte charges alone stopped covering it.
                + node_charge,
            "with the blob cap narrower than the envelope cap, each charge must scale \
             with the cap it names: the raw envelope and the precheck Value with the \
             envelope cap, the typed parse with the blob cap",
        );

        // Zeroing the raw cap removes both charges that scale with it,
        // leaving the parse charge and the node ceiling — a second,
        // independent way to read the precheck charge's width out of
        // the term.
        //
        // That the node charge SURVIVES a zero byte cap is the defect's
        // whole character stated as an assertion: the extraction node
        // population is capped independently of the bytes, so no byte
        // cap — however small — reduces it. Charging it per byte alone
        // therefore under-reserved on any row below the shipped 1 MiB.
        let mut no_raw = limits;
        no_raw.max_runtime_config_bytes = 0;
        let without_raw = validate_isolate_limits(
            &no_raw,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a zero runtime-config envelope cap still validates");
        assert_eq!(
            without_raw.runtime_config_term,
            parse_charge + node_charge,
            "a zero envelope cap must leave the typed parse charge and the node \
             ceiling, which no byte cap reduces",
        );
        assert_eq!(
            charged.runtime_config_term - without_raw.runtime_config_term,
            raw_charge + precheck_charge,
            "the raw envelope and precheck charges together are what the envelope cap \
             funds; the raw envelope charge is MISSING if this is only \
             {precheck_charge} B and the precheck charge is MISSING if it is only \
             {raw_charge} B",
        );

        // And the term is CHARGED INTO the total, once per concurrent
        // auction: it enters through `per_auction_peak`, so a term
        // computed correctly and dropped from that sum fails here.
        let auctions = u64::from(limits.max_concurrent_auctions);
        assert_eq!(
            charged.total_bytes - without_raw.total_bytes,
            auctions * (raw_charge + precheck_charge),
            "the runtime-config term enters the isolate total once per concurrent \
             auction, at full width",
        );
        assert_eq!(
            without_raw.margin_bytes - charged.margin_bytes,
            auctions * (raw_charge + precheck_charge),
            "the bytes the envelope cap funds must be visible to the ceiling check",
        );
        assert_eq!(
            blobbier.total_bytes - charged.total_bytes,
            auctions * CONFIG_PARSE_FACTOR,
            "the typed-parse charge reaches the isolate total too, once per auction",
        );
    }

    /// The row both runtime-config differentials perturb.
    ///
    /// Not the shared proof row: the aux-route peak is funded from the
    /// runtime-config charges, so a one-byte widening would otherwise
    /// push the row under the derived aux floor and be refused before
    /// measuring anything. Funding one widening's worth of headroom
    /// here keeps the aux term CONSTANT across every perturbation,
    /// which is what lets each differential attribute a change to the
    /// config charge it names. The shared row stays untouched, so the
    /// pinned proof rows do not move.
    fn differential_row() -> (ResolvedIsolateLimits, TargetPlatformConstants, Derived) {
        let mut limits = roomy_fixture_limits();
        let target = fixture_target();
        let mut widened = limits;
        widened.max_runtime_config_bytes = widened.max_runtime_config_bytes.saturating_add(1);
        widened.max_runtime_config_blob_bytes =
            widened.max_runtime_config_blob_bytes.saturating_add(1);
        limits.aux_route_peak_bytes = min_aux_route_peak_bytes(
            widened.max_runtime_config_bytes,
            widened.max_runtime_config_blob_bytes,
            u64::from(widened.max_config_json_nodes),
        )
        .max(
            derive_aux_work_for_profile(&widened, &target, 0)
                .expect("the widened aux work derives")
                .peak_bytes(),
        );
        let charged = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the differential row validates");
        (limits, target, charged)
    }

    /// Both terms of the terminal-permit floor have to bind on their
    /// own, because each is the one that binds on a shipped row: the
    /// admission sum on cloudflare, the platform ceiling on
    /// fastly/spin/axum. A floor that dropped either would under-fund
    /// half the targets.
    #[test]
    fn the_terminal_permit_floor_takes_whichever_population_is_larger() {
        // Cloudflare's shape: the admission sum exceeds the ceiling,
        // because an auction holds its permit for the whole auction.
        // This is the row that was under-provisioned at 6.
        assert_eq!(min_terminal_response_permits(6, 4, 6), 10);

        // Axum's shape: the pre-admission rejection path runs before
        // any class gate, so the ceiling outruns the admission sum.
        assert_eq!(min_terminal_response_permits(64, 16, 384), 384);

        // Fastly and Spin: a published per-execution population of 1,
        // where the admission sum is what reaches 2.
        assert_eq!(min_terminal_response_permits(1, 1, 1), 2);

        // Equal terms resolve to the shared value rather than doubling.
        assert_eq!(min_terminal_response_permits(1, 1, 2), 2);

        // The sum saturates rather than wrapping, so an absurd pair
        // cannot roll under the ceiling and silently pass an adapter's
        // `const _`.
        assert_eq!(
            min_terminal_response_permits(u32::MAX, u32::MAX, 1),
            u32::MAX,
        );
    }

    #[test]
    fn request_copies_follow_sequential_scopes_and_bounded_supplements() {
        let mut limits = roomy_fixture_limits();
        limits.max_bidders_per_auction = 1;
        limits.max_impressions_per_auction = 1;
        let charged = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the minimum shape validates");
        let normalized = super::normalization_wire_allowance(
            limits.inbound_request_bytes,
            u64::from(limits.max_copied_header_bytes),
            u64::from(limits.max_parsed_nodes),
        )
        .unwrap();
        let normalized_nodes = (u64::from(limits.max_parsed_nodes)
            + super::NORMALIZATION_IMPLICIT_NODES)
            * NODE_RETAINED_BYTES;
        let one_tree = normalized * 2 + normalized_nodes;
        let supplement = limits.max_outbound_raw_bytes * 2
            + u64::from(limits.max_parsed_nodes) * NODE_RETAINED_BYTES;
        let expected = limits.inbound_request_bytes
            + u64::from(limits.max_admitted_header_bytes)
            + u64::from(limits.max_admitted_header_count)
                * u64::from(fixture_target().header_entry_overhead_bytes)
            + u64::from(limits.max_copied_header_bytes) * 5
            + normalized * MIN_INBOUND_PARSE_FACTOR
            + normalized_nodes
            + 2 * one_tree
            + normalized * super::GROUPING_FACTOR
            + normalized_nodes
            + 2 * (one_tree + supplement);
        assert_eq!(charged.auction_charges.ingress, expected);
        assert_eq!(
            charged.request_side_term,
            expected + charged.auction_charges.projection
        );

        let mut many_bidders = limits;
        many_bidders.max_bidders_per_auction = 50;
        let wider = validate_isolate_limits(
            &many_bidders,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("more sequential bidders validates");
        assert_eq!(
            wider.auction_charges.ingress, charged.auction_charges.ingress,
            "returned scopes drop after make_requests, not after every bidder is built"
        );
        assert_eq!(
            wider.auction_charges.projection - charged.auction_charges.projection,
            49 * normalized * super::TYPED_CLONE_FACTOR,
            "M4's independent measured projection allowance still follows survivors"
        );

        limits.max_impressions_per_auction += 1;
        let more_imps = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("one more supplemented imp validates");
        assert_eq!(
            more_imps.request_side_term, charged.request_side_term,
            "preflight caps the total supplement expansion, not a whole tree per imp"
        );
    }

    #[test]
    fn request_derived_response_bookkeeping_has_its_own_charge() {
        let limits = roomy_fixture_limits();
        let derived = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the bounded envelope validates");
        let bookkeeping = 4
            * (limits.max_outbound_raw_bytes
                + u64::from(limits.max_parsed_nodes) * NODE_RETAINED_BYTES);
        assert_eq!(
            derived.per_auction_peak,
            derived.runtime_config_term
                + derived.request_side_term
                + derived.call_side_term
                + derived.auction_charges.privacy
                + bookkeeping
                + derived.debug_term,
            "denial/status rows coexist with the request and bidder response populations",
        );
        assert_eq!(derived.bookkeeping_term, bookkeeping);
        assert_eq!(
            derived.auction_charges.outbound + derived.auction_charges.response,
            derived.call_side_term + bookkeeping + derived.debug_term,
            "bookkeeping is prepaid in the response phase, not merely reported"
        );
        let mut more_bytes = limits;
        more_bytes.max_outbound_raw_bytes += 1;
        let bytes = validate_isolate_limits(
            &more_bytes,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(bytes.bookkeeping_term - derived.bookkeeping_term, 4);
        assert_eq!(
            bytes.auction_charges.response - derived.auction_charges.response,
            4
        );
        let mut more_nodes = limits;
        more_nodes.max_parsed_nodes += 1;
        let nodes = validate_isolate_limits(
            &more_nodes,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(
            nodes.bookkeeping_term - derived.bookkeeping_term,
            4 * NODE_RETAINED_BYTES
        );
    }

    #[test]
    fn zero_aux_permits_still_fund_probe_config_work() {
        let mut limits = roomy_fixture_limits();
        limits.aux_route_permits = 0;
        let zero_aux = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let one_config_slot = limits.aux_route_peak_bytes
            * u64::from(fixture_target().egress_handoff_factor)
            + u64::from(limits.transmit_state_bytes);
        assert_eq!(
            zero_aux.aux_term,
            u64::from(crate::PROBE_CONFIG_MIN_PERMITS) * one_config_slot,
            "zero aux permits must not remove the unsheddable probe's config work charge"
        );
        assert!(zero_aux.aux_term >= zero_aux.runtime_config_term);

        limits.aux_route_permits = crate::PROBE_CONFIG_MIN_PERMITS;
        let both = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(
            both.aux_term - zero_aux.aux_term,
            u64::from(crate::PROBE_CONFIG_MIN_PERMITS) * one_config_slot
        );
        assert_eq!(both.probe_output_term, zero_aux.probe_output_term);
        assert_eq!(
            both.gvl_terms - zero_aux.gvl_terms,
            u64::from(crate::PROBE_CONFIG_MIN_PERMITS) * limits.gvl_parsed_bytes
        );
        assert_eq!(
            both.total_bytes - zero_aux.total_bytes,
            both.aux_term - zero_aux.aux_term + both.gvl_terms - zero_aux.gvl_terms
        );
    }

    #[test]
    fn probe_output_funding_uses_reserved_application_slots() {
        let limits = roomy_fixture_limits();
        let target = fixture_target();
        let derived = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(
            derived.probe_output_term,
            crate::PROBE_RESPONSE_BODY_MAX_BYTES
                + 4 * (crate::PROBE_RESPONSE_BODY_MAX_BYTES
                    * u64::from(target.egress_handoff_factor)
                    + u64::from(limits.transmit_state_bytes)),
            "probe outputs must not depend on an unenforced inbound population"
        );
    }

    #[test]
    fn probe_output_population_does_not_follow_aux_work_permits() {
        let limits = roomy_fixture_limits();
        let charged = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let expected = crate::PROBE_RESPONSE_BODY_MAX_BYTES
            + u64::from(crate::PROBE_RESPONSE_PERMITS)
                * (crate::PROBE_RESPONSE_BODY_MAX_BYTES
                    * u64::from(fixture_target().egress_handoff_factor)
                    + u64::from(limits.transmit_state_bytes));
        assert_eq!(charged.probe_output_term, expected);
        let non_probe = limits.baseline_bytes
            + charged.gvl_terms
            + u64::from(limits.max_concurrent_auctions) * charged.per_auction_peak
            + charged.egress_term
            + charged.aux_term
            + charged.terminal_term
            + charged.pre_admission_parser_term
            + u64::from(limits.max_active_body_streams) * charged.per_stream_peak;
        assert_eq!(charged.total_bytes - non_probe, expected);

        let mut reduced = limits;
        reduced.aux_route_permits = 1;
        let fewer = validate_isolate_limits(
            &reduced,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(fewer.probe_output_term, charged.probe_output_term);
        assert_eq!(
            charged.total_bytes - fewer.total_bytes,
            charged.aux_term - fewer.aux_term + charged.gvl_terms - fewer.gvl_terms,
            "reducing config work permits must not reduce probe response funding"
        );

        let mut less_platform = fixture_target();
        less_platform.platform_concurrent_ceiling -= 1;
        let smaller = validate_isolate_limits(
            &limits,
            &less_platform,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(charged.probe_output_term, smaller.probe_output_term);
    }

    #[test]
    fn request_working_copy_charge_reports_checked_overflow() {
        let mut limits = roomy_fixture_limits();
        limits.max_bidders_per_auction = 1;
        limits.max_outbound_raw_bytes = u64::MAX.div_euclid(2) + 1;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &fixture_target(),
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetArithmeticOverflow),
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "independent per-term accounting deltas share one baseline and retain every charge assertion"
    )]
    fn per_auction_formula_charges_clones_outbound_and_retained_responses() {
        let limits = roomy_fixture_limits();
        let derived = fixture_derived();
        let normalized = super::normalization_wire_allowance(
            limits.inbound_request_bytes,
            u64::from(limits.max_copied_header_bytes),
            u64::from(limits.max_parsed_nodes),
        )
        .unwrap();

        // Bidder scopes are sequential, so the copy-model ingress stays
        // fixed. M4's independent measured projection receipt still sums
        // work across survivors and needs its existing per-bidder allowance.
        let mut more_bidders = limits;
        more_bidders.max_bidders_per_auction += 1;
        let widened = validate_isolate_limits(
            &more_bidders,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("one more bidder still fits");
        assert_eq!(
            widened.request_side_term - derived.request_side_term,
            normalized * super::TYPED_CLONE_FACTOR
        );
        assert_eq!(
            widened.auction_charges.ingress,
            derived.auction_charges.ingress
        );
        assert_eq!(widened.call_side_term, derived.call_side_term);

        // Raising the call count moves only the call-side terms, by
        // one call's worth of outbound, retained body, headers,
        // response parse, metadata, maps and assembly scratch. The
        // response cap rises with it, because the added scratch would
        // otherwise trip the sizing floor — which is itself the point
        // of that floor.
        let mut more_calls = limits;
        more_calls.max_outbound_requests += 1;
        more_calls.final_response_bytes += limits.max_decoded_bytes * ASSEMBLY_FACTOR;
        let longer = validate_isolate_limits(
            &more_calls,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("one more call still fits");
        let one_call = (limits.max_outbound_raw_bytes + limits.max_outbound_request_bytes)
            + limits.max_decoded_bytes
            + 12_288
            + limits.max_decoded_bytes * (RESPONSE_PARSE_FACTOR + super::RESPONSE_ASSEMBLY_FACTOR)
            + u64::from(limits.max_parsed_nodes) * MIN_RESPONSE_NODE_PEAK_BYTES
            + u64::from(FIXTURE_OUTBOUND_META_BYTES)
            + 7_680
            + limits.max_decoded_bytes * ASSEMBLY_FACTOR;
        assert_eq!(
            longer.call_side_term - derived.call_side_term,
            one_call,
            "one more call must cost the whole per-call charge. The response parse is \
             TWO terms — decoded x MIN_RESPONSE_PARSE_FACTOR = {MIN_RESPONSE_PARSE_FACTOR} plus \
             max_parsed_nodes x MIN_RESPONSE_NODE_PEAK_BYTES = {MIN_RESPONSE_NODE_PEAK_BYTES} — \
             and a delta short by {} B is the node half missing from response_parse",
            u64::from(limits.max_parsed_nodes) * MIN_RESPONSE_NODE_PEAK_BYTES,
        );
        assert_eq!(longer.request_side_term, derived.request_side_term);

        // The original, two retained copies, grouping subtrees, and
        // both amplified working copies each fund their own node heap.
        let mut no_nodes = limits;
        no_nodes.max_parsed_nodes = 0;
        let without = validate_isolate_limits(
            &no_nodes,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("zero nodes validates");
        // Numeric canonicalization expands each normalized-wire representation:
        // parse, two retained copies, grouping, two working copies and scopes.
        let wire_copies = limits
            .inbound_parse_factor
            .checked_add(16)
            .unwrap()
            .checked_add(
                u64::from(limits.max_bidders_per_auction)
                    .checked_mul(128)
                    .unwrap(),
            )
            .unwrap();
        let numeric_growth = u64::from(limits.max_parsed_nodes)
            .checked_mul(23)
            .unwrap()
            .checked_mul(wire_copies)
            .unwrap();
        let request_node_bytes = u64::from(limits.max_parsed_nodes)
            .checked_mul(NODE_RETAINED_BYTES)
            .unwrap()
            .checked_mul(8)
            .unwrap()
            .checked_add(numeric_growth)
            .unwrap();
        let auction_node_bytes = u64::from(limits.max_parsed_nodes)
            .checked_mul(
                NODE_RETAINED_BYTES
                    .checked_mul(12)
                    .unwrap()
                    .checked_add(
                        u64::from(limits.max_outbound_requests)
                            .checked_mul(limits.response_node_peak_bytes)
                            .unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap()
            .checked_add(numeric_growth)
            .unwrap();
        assert_eq!(
            derived.request_side_term - without.request_side_term,
            request_node_bytes
        );
        assert_eq!(
            derived.per_auction_peak - without.per_auction_peak,
            auction_node_bytes
        );
        assert_eq!(
            derived.total_bytes - without.total_bytes,
            auction_node_bytes
                .checked_mul(u64::from(limits.max_concurrent_auctions))
                .unwrap()
        );
        assert_eq!(derived.egress_term, without.egress_term);

        // And the RESPONSE side carries a node charge of its own, once
        // per call. Asserted on the same zero-node row, against the
        // call-side term rather than the request-side one, so the two
        // node charges cannot cover for each other: request copies and
        // bookkeeping are independent of calls, while the call-side node
        // term is charged `max_outbound_requests` times. Dropping the
        // response node term entirely leaves this delta at zero.
        assert_eq!(
            derived.call_side_term - without.call_side_term,
            u64::from(limits.max_outbound_requests)
                * u64::from(limits.max_parsed_nodes)
                * MIN_RESPONSE_NODE_PEAK_BYTES,
            "zeroing the node budget must remove the response-side node charge from \
             every call; a call-side term that does not move with the node budget is \
             charging bytes only, which is the asymmetry the response node term exists \
             to close"
        );

        // The metadata charge crosses the boundary as a scalar and is
        // charged once per call.
        let bigger_meta = validate_isolate_limits(
            &limits,
            &fixture_target(),
            FIXTURE_OUTBOUND_META_BYTES + 1,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a wider meta charge still fits");
        assert_eq!(
            bigger_meta.call_side_term - derived.call_side_term,
            u64::from(limits.max_outbound_requests)
        );
    }

    #[test]
    fn max_bidders_per_auction_requires_at_least_one_bidder() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        limits.max_bidders_per_auction = 0;
        let result = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        );
        assert!(
            matches!(
                result,
                Err(BudgetError::LimitBelowMinimum {
                    field: LimitField::MaxBiddersPerAuction,
                    value: 0,
                    minimum: 1,
                })
            ),
            "zero bidders must fail their scalar minimum check: {result:?}",
        );

        limits.max_bidders_per_auction = 1;
        validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("one bidder meets the minimum");
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture matrix pins exact-ceiling acceptance and every named validation refusal"
    )]
    fn validate_isolate_limits_rejects_overcommit_overflow_and_bad_defaults() {
        /// The synthetic fixture's independently worked demand. The ceiling
        /// boundary is exclusive at exactly one byte, which is the
        /// whole point of the `checked_sub`, so the two cases below set
        /// their own ceiling against this rather than using
        /// `roomy_fixture_limits`: the ceiling is what they are testing.
        const FIXTURE_DEMAND: u64 = 1_558_168_270_080;

        let target = fixture_target();

        // Overcommit: a ceiling one byte under the computed demand.
        let mut tight = fixture_limits();
        tight.isolate_total_bytes = FIXTURE_DEMAND - 1;
        assert_eq!(
            validate_isolate_limits(
                &tight,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: FIXTURE_DEMAND,
                ceiling: FIXTURE_DEMAND - 1,
            })
        );
        // The exact demand is accepted, with zero margin.
        let mut exact = fixture_limits();
        exact.isolate_total_bytes = FIXTURE_DEMAND;
        assert_eq!(
            validate_isolate_limits(
                &exact,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            )
            .expect("the exact demand fits")
            .margin_bytes,
            0
        );

        // Overflow: a term whose product is not representable.
        let mut huge = roomy_fixture_limits();
        huge.max_decoded_bytes = u64::MAX;
        assert_eq!(
            validate_isolate_limits(
                &huge,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetArithmeticOverflow)
        );

        // A gate that admits nothing is not a valid configuration.
        let mut no_auctions = roomy_fixture_limits();
        no_auctions.max_concurrent_auctions = 0;
        assert_eq!(
            validate_isolate_limits(
                &no_auctions,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::MaxConcurrentAuctions,
                value: 0,
                minimum: 1,
            })
        );
        let mut no_streams = roomy_fixture_limits();
        no_streams.max_active_body_streams = 0;
        assert_eq!(
            validate_isolate_limits(
                &no_streams,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::MaxActiveBodyStreams,
                value: 0,
                minimum: 1,
            })
        );

        // The write deadline is range-checked at both ends.
        let mut quick = roomy_fixture_limits();
        quick.response_write_deadline_ms = 99;
        assert_eq!(
            validate_isolate_limits(
                &quick,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::ResponseWriteDeadlineMs,
                value: 99,
                minimum: 100,
            })
        );
        let mut slow = roomy_fixture_limits();
        slow.response_write_deadline_ms = 120_001;
        assert_eq!(
            validate_isolate_limits(
                &slow,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitAboveMaximum {
                field: LimitField::ResponseWriteDeadlineMs,
                value: 120_001,
                maximum: 120_000,
            })
        );

        // A window outside the validated range never reaches a shift.
        let mut wide = roomy_fixture_limits();
        wide.max_brotli_wbits = 25;
        assert_eq!(
            validate_isolate_limits(
                &wide,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BrotliWbitsOutOfRange { wbits: 25 })
        );

        // A hard response cap below the assembly scratch would
        // truncate a within-budget auction.
        let mut small_response = roomy_fixture_limits();
        small_response.final_response_bytes = FINAL_RESPONSE_BYTES - 1;
        assert_eq!(
            validate_isolate_limits(
                &small_response,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::FinalResponseBelowAssemblyFloor {
                configured: FINAL_RESPONSE_BYTES - 1,
                required: FINAL_RESPONSE_BYTES,
            })
        );
    }

    /// Both halves of the response charge are floored, not defaulted,
    /// and ABOVE the floor is always allowed.
    ///
    /// Written as the inbound factor's test is written, because the rule
    /// is the same rule: a factor is a measurement of what a byte or a
    /// node costs the heap, not a preference, so config may only make it
    /// more conservative. An operator who sets either under the measured
    /// figure would otherwise get a boot check that passes while the
    /// isolate overruns on the first bidder response.
    ///
    /// The adjacent raised-factor test asserts the extra lands where charged
    /// — once per CALL, and once per call per CONCURRENT AUCTION in the
    /// total — which is what separates these two fields from a knob the
    /// equation reads and then ignores.
    #[test]
    fn the_response_factors_are_floored_not_defaulted() {
        let target = fixture_target();
        let limits = roomy_fixture_limits();

        // One under either floor is refused by name.
        let mut under_bytes = limits;
        under_bytes.response_parse_factor = MIN_RESPONSE_PARSE_FACTOR - 1;
        assert_eq!(
            validate_isolate_limits(
                &under_bytes,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::ResponseParseFactor,
                value: MIN_RESPONSE_PARSE_FACTOR - 1,
                minimum: MIN_RESPONSE_PARSE_FACTOR,
            })
        );
        let mut under_nodes = limits;
        under_nodes.response_node_peak_bytes = MIN_RESPONSE_NODE_PEAK_BYTES - 1;
        assert_eq!(
            validate_isolate_limits(
                &under_nodes,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::ResponseNodePeakBytes,
                value: MIN_RESPONSE_NODE_PEAK_BYTES - 1,
                minimum: MIN_RESPONSE_NODE_PEAK_BYTES,
            })
        );

        // Zero is the configuration that would silently delete the term
        // altogether, which is the whole reason these are floors rather
        // than defaults, and it is refused by name like any other
        // under-floor value.
        let mut no_node_charge = limits;
        no_node_charge.response_node_peak_bytes = 0;
        assert_eq!(
            validate_isolate_limits(
                &no_node_charge,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::ResponseNodePeakBytes,
                value: 0,
                minimum: MIN_RESPONSE_NODE_PEAK_BYTES,
            })
        );

        // At the floors the synthetic fixture validates.
        validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the floors themselves validate");
        assert_eq!(limits.response_parse_factor, MIN_RESPONSE_PARSE_FACTOR);
        assert_eq!(
            limits.response_node_peak_bytes,
            MIN_RESPONSE_NODE_PEAK_BYTES
        );
    }

    #[test]
    fn higher_response_factors_charge_owning_populations() {
        let target = fixture_target();
        let limits = roomy_fixture_limits();
        let calls = u64::from(limits.max_outbound_requests);
        let auctions = u64::from(limits.max_concurrent_auctions);
        let at_floor = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();

        // Above the per-byte floor charges one more byte per decoded
        // byte, per call.
        let mut raised_bytes = limits;
        raised_bytes.response_parse_factor = MIN_RESPONSE_PARSE_FACTOR + 1;
        let harsher_bytes = validate_isolate_limits(
            &raised_bytes,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("an operator may charge more than the floor");
        assert_eq!(
            harsher_bytes.call_side_term - at_floor.call_side_term,
            calls * limits.max_decoded_bytes
        );
        assert_eq!(
            harsher_bytes.total_bytes - at_floor.total_bytes,
            calls * limits.max_decoded_bytes * auctions
        );

        // And above the per-node floor charges one more byte per node of
        // the response budget, per call.
        let mut raised_nodes = limits;
        raised_nodes.response_node_peak_bytes = MIN_RESPONSE_NODE_PEAK_BYTES + 1;
        let harsher_nodes = validate_isolate_limits(
            &raised_nodes,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("an operator may charge more than the floor");
        assert_eq!(
            harsher_nodes.call_side_term - at_floor.call_side_term,
            calls * u64::from(limits.max_parsed_nodes)
        );
        assert_eq!(
            harsher_nodes.total_bytes - at_floor.total_bytes,
            calls * u64::from(limits.max_parsed_nodes) * auctions
        );
    }

    #[test]
    fn inbound_parse_factor_is_floored_not_defaulted() {
        let target = fixture_target();
        let limits = roomy_fixture_limits();

        // One under the floor is refused by name, exactly as the other
        // range checks refuse theirs.
        let mut under = limits;
        under.inbound_parse_factor = MIN_INBOUND_PARSE_FACTOR - 1;
        assert_eq!(
            validate_isolate_limits(
                &under,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::InboundParseFactor,
                value: MIN_INBOUND_PARSE_FACTOR - 1,
                minimum: MIN_INBOUND_PARSE_FACTOR,
            })
        );
        // The old compile-time constant is far under the floor, so a
        // row carrying it can no longer boot.
        let mut disproven = limits;
        disproven.inbound_parse_factor = 2;
        assert_eq!(
            validate_isolate_limits(
                &disproven,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::InboundParseFactor,
                value: 2,
                minimum: MIN_INBOUND_PARSE_FACTOR,
            })
        );

        // At the floor the synthetic fixture validates.
        let at_floor = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the floor itself validates");
        assert_eq!(limits.inbound_parse_factor, MIN_INBOUND_PARSE_FACTOR);

        // Above the floor is always allowed, and charges the extra: one
        // more byte per inbound byte, landing once in the request side
        // and once per CONCURRENT AUCTION in the total, because the
        // per-auction peak is what the total multiplies.
        let mut raised = limits;
        raised.inbound_parse_factor = MIN_INBOUND_PARSE_FACTOR + 1;
        let harsher = validate_isolate_limits(
            &raised,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("an operator may charge more than the floor");
        assert_eq!(
            harsher.request_side_term - at_floor.request_side_term,
            super::normalization_wire_allowance(
                limits.inbound_request_bytes,
                u64::from(limits.max_copied_header_bytes),
                u64::from(limits.max_parsed_nodes),
            )
            .unwrap()
        );
        assert_eq!(
            harsher.total_bytes - at_floor.total_bytes,
            super::normalization_wire_allowance(
                limits.inbound_request_bytes,
                u64::from(limits.max_copied_header_bytes),
                u64::from(limits.max_parsed_nodes),
            )
            .unwrap()
                * u64::from(limits.max_concurrent_auctions)
        );
    }

    /// The resident GVL charge is counted in distinct lists, floored at
    /// the measured retention, and every handle beyond the first costs
    /// nothing.
    ///
    /// The old row assumed nine independent 4 MB copies — one per cache
    /// entry plus one per concurrent auction. `Gvl` is an `Arc`, so the
    /// per-auction handle is a refcount bump; what that term really
    /// funds is a DISTINCT list an auction can pin past its eviction,
    /// which is why the shape survives the correction while the
    /// per-list figure does not.
    #[test]
    fn gvl_terms_charge_distinct_lists_at_the_measured_floor() {
        let target = fixture_target();
        let limits = roomy_fixture_limits();

        // One under the floor is refused by name, exactly as the
        // inbound parse factor's floor refuses its own.
        let mut under = limits;
        under.gvl_parsed_bytes = MAX_GVL_PARSED_BYTES - 1;
        assert_eq!(
            validate_isolate_limits(
                &under,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::LimitBelowMinimum {
                field: LimitField::GvlParsedBytes,
                value: MAX_GVL_PARSED_BYTES - 1,
                minimum: MAX_GVL_PARSED_BYTES,
            })
        );

        // The row charges one list per cache entry plus one per
        // concurrent auction and aux privacy request, at the floor.
        let at_floor = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the floor itself validates");
        assert_eq!(limits.gvl_parsed_bytes, MAX_GVL_PARSED_BYTES);
        assert_eq!(
            at_floor.gvl_terms,
            u64::from(
                limits.gvl_cache_entries
                    + limits.max_concurrent_auctions
                    + limits.aux_route_permits
            ) * MAX_GVL_PARSED_BYTES
        );

        // Each additional cache entry is one more distinct list, and it
        // reaches the total — a term computed beside the sum rather
        // than into it would leave the total unmoved.
        let mut wider_cache = limits;
        wider_cache.gvl_cache_entries += 1;
        let wider = validate_isolate_limits(
            &wider_cache,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("one more cache entry still fits");
        assert_eq!(wider.gvl_terms - at_floor.gvl_terms, MAX_GVL_PARSED_BYTES);
        assert_eq!(
            wider.total_bytes - at_floor.total_bytes,
            MAX_GVL_PARSED_BYTES
        );

        // An auction can pin a list the cache has evicted, so the
        // concurrency term charges one distinct list too — the same
        // width as a cache entry, for a different reason. A second
        // auction also brings a second `per_auction_peak`, which this
        // row's ceiling does not fund, so the comparison is drawn
        // against a lifted ceiling on both sides rather than against a
        // refusal.
        let mut roomy = limits;
        roomy.isolate_total_bytes = u64::MAX;
        let one_auction = validate_isolate_limits(
            &roomy,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the row's terms are computable once the ceiling is lifted");
        let mut busier = roomy;
        busier.max_concurrent_auctions += 1;
        let busy = validate_isolate_limits(
            &busier,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("a second concurrent auction is computable");
        assert_eq!(busy.gvl_terms - one_auction.gvl_terms, MAX_GVL_PARSED_BYTES);

        // An operator may reserve more per list; never less.
        let mut generous = limits;
        generous.gvl_parsed_bytes = MAX_GVL_PARSED_BYTES * 2;
        let roomier = validate_isolate_limits(
            &generous,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("an operator may charge more than the floor");
        assert_eq!(roomier.gvl_terms, at_floor.gvl_terms * 2);
    }

    #[test]
    fn aux_requests_pin_distinct_gvl_versions_after_cache_eviction() {
        let target = fixture_target();
        let limits = roomy_fixture_limits();
        let costs = super::RequestConstructionCosts::default();
        let base = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("base budget");
        let mut one_more = limits;
        one_more.aux_route_permits += 1;
        let widened = validate_isolate_limits(
            &one_more,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("one more aux request");
        assert_eq!(widened.gvl_terms - base.gvl_terms, limits.gvl_parsed_bytes);
        let work_permits = |aux_permits: u32| {
            u64::from(aux_permits) + u64::from(aux_permits.max(crate::PROBE_CONFIG_MIN_PERMITS))
        };
        let added_work_permits =
            work_permits(one_more.aux_route_permits) - work_permits(limits.aux_route_permits);
        assert_eq!(
            widened.total_bytes - base.total_bytes,
            limits.gvl_parsed_bytes
                + added_work_permits
                    * (limits.aux_route_peak_bytes * u64::from(target.egress_handoff_factor)
                        + u64::from(limits.transmit_state_bytes))
        );
    }

    #[test]
    fn aux_shipped_count_profile_has_independent_literal_owners() {
        let limits = roomy_fixture_limits();
        let target = fixture_target();
        let mapped = super::aux_work_limits_from_envelope(&limits, &target, 0).unwrap();
        let derived = derive_aux_work_for_profile(&limits, &target, 0).unwrap();
        for counts in [mapped, derived.limits()] {
            assert_eq!(
                (
                    counts.catalog_names,
                    counts.priority_entries,
                    counts.requested_names,
                    counts.filter_names_per_list,
                    counts.debug_rows,
                ),
                (512, 256, 256, 256, 256)
            );
            assert_eq!(counts.priority_groups, 32);
        }
        assert_eq!(derived.charges().chooser, 462_848);
    }

    #[test]
    fn aux_node_caps_follow_sdk_values_and_independent_syntactic_input_bounds() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        let caps = derive_aux_work_for_profile(&limits, &target, 0)
            .unwrap()
            .caps();
        assert_eq!(caps.config_nodes, 8_192);
        assert_eq!(caps.body_nodes, 32 * 1024);
        assert_eq!(caps.cookie_nodes, 4_096);

        limits.max_parsed_nodes = 1;
        let reduced_auction_nodes = derive_aux_work_for_profile(&limits, &target, 0)
            .unwrap()
            .caps();
        assert_eq!(reduced_auction_nodes.config_nodes, caps.config_nodes);

        limits.max_runtime_config_blob_bytes = 1_024;
        let smaller = derive_aux_work_for_profile(&limits, &target, 0)
            .unwrap()
            .caps();
        assert_eq!(smaller.config_nodes, caps.config_nodes);
        assert_eq!(smaller.body_nodes, caps.body_nodes);
        assert_eq!(smaller.cookie_nodes, caps.cookie_nodes);
    }

    #[test]
    fn aux_config_value_count_changes_only_node_charge_by_literal_per_value_bytes() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        limits.max_config_json_nodes = 37;
        let baseline = derive_aux_work_for_profile(&limits, &target, 256).unwrap();
        limits.max_config_json_nodes += 1;
        let larger = derive_aux_work_for_profile(&limits, &target, 256).unwrap();

        assert_eq!(larger.charges().nodes - baseline.charges().nodes, 623);
        assert_eq!(larger.peak_bytes() - baseline.peak_bytes(), 623);
        assert_eq!(
            larger.charges(),
            AuxWorkCharges {
                nodes: baseline.charges().nodes + 623,
                ..baseline.charges()
            }
        );
        assert_eq!(baseline.caps().config_nodes, 37);
        assert_eq!(larger.caps().config_nodes, 38);
        assert_eq!(larger.caps().body_nodes, baseline.caps().body_nodes);
        assert_eq!(larger.caps().cookie_nodes, baseline.caps().cookie_nodes);
    }

    #[test]
    fn aux_config_byte_terms_vary_independently_without_changing_value_charge() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        limits.max_runtime_config_bytes = 1_536;
        limits.max_runtime_config_blob_bytes = 512;
        limits.max_config_json_nodes = 37;
        let baked = 256;
        let baseline = derive_aux_work_for_profile(&limits, &target, baked).unwrap();
        assert_eq!(baseline.charges().config, 1_536 + (1_536 + 512 + 256) * 128);

        for (envelope_delta, blob_delta, baked_delta, expected) in
            [(1, 0, 0, 129), (0, 1, 0, 128), (0, 0, 1, 128)]
        {
            let mut changed = limits;
            changed.max_runtime_config_bytes += envelope_delta;
            changed.max_runtime_config_blob_bytes += blob_delta;
            let work = derive_aux_work_for_profile(&changed, &target, baked + baked_delta).unwrap();
            assert_eq!(work.charges().config - baseline.charges().config, expected);
            assert_eq!(work.charges().nodes, baseline.charges().nodes);
            assert_eq!(work.caps(), baseline.caps());
            assert_eq!(work.peak_bytes() - baseline.peak_bytes(), expected);
        }
    }

    #[test]
    fn aux_zero_config_values_leave_byte_terms_and_syntactic_nodes_intact() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        limits.max_config_json_nodes = 0;
        let work = derive_aux_work_for_profile(&limits, &target, 256).unwrap();
        assert_eq!(work.caps().config_nodes, 0);
        assert_eq!(work.charges().nodes, (32 * 1_024 + 4_096) * 384);
        assert_eq!(
            work.charges().config,
            limits.max_runtime_config_bytes * 129
                + (limits.max_runtime_config_blob_bytes + 256) * 128
        );
        let budget = AuxWorkBudget::new(work.peak_bytes());
        let receipt = work.reserve(&budget).unwrap();
        assert_eq!(receipt.ensure_nodes(AuxNodeSet::Config, 0), Ok(()));
        assert_eq!(
            receipt.ensure_nodes(AuxNodeSet::Config, 1),
            Err(AuxWorkError::NodeLimit(AuxNodeSet::Config))
        );
    }

    #[test]
    fn aux_config_value_count_supports_the_full_u32_range() {
        let target = fixture_target();
        for count in [1, u32::MAX] {
            let mut limits = roomy_fixture_limits();
            limits.max_config_json_nodes = count;
            let work = derive_aux_work_for_profile(&limits, &target, 0).unwrap();
            assert_eq!(work.caps().config_nodes, count);
            assert_eq!(
                work.charges().nodes,
                u64::from(count) * 623 + (32 * 1_024 + 4_096) * 384
            );
            assert_eq!(work.concurrent_peak(1), Ok(work.peak_bytes()));
            let budget = AuxWorkBudget::new(work.peak_bytes());
            let receipt = work.reserve(&budget).unwrap();
            assert_eq!(receipt.ensure_nodes(AuxNodeSet::Config, count), Ok(()));
            if let Some(over) = count.checked_add(1) {
                assert_eq!(
                    receipt.ensure_nodes(AuxNodeSet::Config, over),
                    Err(AuxWorkError::NodeLimit(AuxNodeSet::Config))
                );
            }
        }
    }

    #[test]
    fn aux_config_value_charge_keeps_node_sum_overflow_checked() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        limits.max_config_json_nodes = u32::MAX;
        let mut inputs = super::aux_work_limits_from_envelope(&limits, &target, 0).unwrap();
        inputs.body_node_bytes = u64::MAX - u64::from(u32::MAX) * 623 + 1;
        inputs.cookie_node_bytes = 0;
        assert_eq!(
            derive_aux_work(inputs),
            Err(AuxWorkError::ArithmeticOverflow)
        );
    }

    #[test]
    fn aux_blob_bytes_do_not_multiply_the_config_value_allowance() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        limits.max_runtime_config_blob_bytes = u64::MAX.checked_div(384).unwrap() + 1;
        let work = derive_aux_work_for_profile(&limits, &target, 0).unwrap();
        assert_eq!(work.caps().config_nodes, limits.max_config_json_nodes);
        assert_eq!(
            work.charges().config,
            limits.max_runtime_config_bytes * 129 + limits.max_runtime_config_blob_bytes * 128
        );
    }

    #[test]
    fn aux_response_header_allowances_cover_small_and_large_ingress_profiles() {
        let target = fixture_target();
        for (admitted_bytes, reserved_bytes) in [
            (256, 16 * 1024),
            (16 * 1024, 16 * 1024),
            (64 * 1024, 64 * 1024),
        ] {
            for (admitted_count, reserved_count) in [(1, 16), (16, 16), (128, 128)] {
                let mut limits = roomy_fixture_limits();
                limits.max_admitted_header_bytes = admitted_bytes;
                limits.max_admitted_header_count = admitted_count;
                let work = derive_aux_work_for_profile(&limits, &target, 0).unwrap();
                let inputs = work.limits();
                assert_eq!(inputs.admitted_header_bytes, u64::from(admitted_bytes));
                assert_eq!(inputs.admitted_header_count, u32::from(admitted_count));
                assert_eq!(inputs.response_header_bytes, reserved_bytes);
                assert_eq!(inputs.response_header_count, reserved_count);
                assert_eq!(inputs.set_cookie_bytes, 4 * 1024);
            }
        }
    }

    #[test]
    fn aux_peak_is_derived_from_profile_and_baked_config_before_boot() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        let costs = super::RequestConstructionCosts::default();
        let bare = derive_aux_work_for_profile(&limits, &target, 0).unwrap();
        assert_eq!(bare.pinned_gvl_bytes(), limits.gvl_parsed_bytes);
        assert_eq!(bare.charges().retained_urls, 256 * 1024);

        limits.aux_route_peak_bytes = bare.peak_bytes() - 1;
        assert_eq!(
            derive_isolate_budget(
                &limits,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                costs,
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::AuxRoutePeakBelowDerived {
                configured: limits.aux_route_peak_bytes,
                required: bare.peak_bytes(),
            })
        );
        limits.aux_route_peak_bytes = bare.peak_bytes();
        derive_isolate_budget(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();

        let baked = derive_aux_work_for_profile(&limits, &target, 1_024).unwrap();
        assert_eq!(
            baked.peak_bytes() - bare.peak_bytes(),
            1_024 * super::CONFIG_PARSE_FACTOR
        );
        assert_eq!(baked.pinned_gvl_bytes(), bare.pinned_gvl_bytes());
        assert_eq!(
            super::derive_isolate_budget_with_baked_config(
                &limits,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                costs,
                1_024,
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::AuxRoutePeakBelowDerived {
                configured: limits.aux_route_peak_bytes,
                required: baked.peak_bytes(),
            })
        );
    }

    #[test]
    fn pinned_gvl_scales_only_the_global_gvl_term() {
        let target = fixture_target();
        let mut limits = roomy_fixture_limits();
        let costs = super::RequestConstructionCosts::default();
        let floor = derive_aux_work_for_profile(&limits, &target, 0).unwrap();
        limits.aux_route_peak_bytes = floor.peak_bytes();
        let base = derive_isolate_budget(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();

        limits.gvl_parsed_bytes *= 2;
        let doubled = derive_aux_work_for_profile(&limits, &target, 0).unwrap();
        let wider = derive_isolate_budget(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            costs,
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        assert_eq!(doubled.peak_bytes(), floor.peak_bytes());
        assert_eq!(doubled.pinned_gvl_bytes(), floor.pinned_gvl_bytes() * 2);
        let distinct_lists = u64::from(
            limits.gvl_cache_entries + limits.max_concurrent_auctions + limits.aux_route_permits,
        );
        assert_eq!(
            wider.gvl_terms - base.gvl_terms,
            distinct_lists * floor.pinned_gvl_bytes()
        );
        assert_eq!(wider.aux_term, base.aux_term);
        assert_eq!(
            wider.total_bytes - base.total_bytes,
            wider.gvl_terms - base.gvl_terms
        );
    }

    #[test]
    fn validate_isolate_limits_rejects_bad_parser_knobs() {
        let limits = roomy_fixture_limits();

        // h2 rejects at `>= max`, so a knob equal to the ceiling
        // rejects a field section policy admits.
        let mut equal_knob = fixture_target();
        equal_knob.parser_header_knob_bytes = 21_632;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &equal_knob,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::HeaderKnobNotCeilingPlusOne {
                knob: 21_632,
                ceiling: 21_632,
            })
        );

        // The pseudoheader allowance must cover the target plus the
        // four-pseudoheader fixed overhead.
        let mut tight_allowance = fixture_target();
        tight_allowance.max_request_target_bytes = 833;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &tight_allowance,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::PseudoheaderAllowanceTooSmall {
                allowance: 1_024,
                required: 1_025,
            })
        );

        // The h1 read buffer must hold an admitted head.
        let mut small_buf = fixture_target();
        small_buf.h1_max_buf_size_bytes = 18_175;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &small_buf,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::H1BufSizeTooSmall {
                configured: 18_175,
                required: 18_176,
            })
        );

        // hyper's h1 count default sits below what auctioneer admits.
        let mut default_h1_count = fixture_target();
        default_h1_count.h1_max_headers = 100;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &default_h1_count,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::H1HeaderCountMismatch {
                knob: 100,
                admitted: 128,
            })
        );

        // The entry maximum is derived, so a hand-edited value fails.
        let mut hand_edited = fixture_target();
        hand_edited.parser_max_header_entries = 128;
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &hand_edited,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::ParserEntryCountMismatch {
                configured: 128,
                derived: 676,
            })
        );
    }

    #[test]
    fn validate_isolate_limits_charges_the_dominating_protocol_peak() {
        let mut limits = roomy_fixture_limits();
        let mut target = fixture_target();

        // A target whose h1 head buffer dwarfs its h2 field section:
        // the h1 peak must be the one charged.
        target.parser_header_ceiling_bytes = 4_096_u32;
        target.parser_header_knob_bytes = 4_097_u32;
        target.parser_max_header_entries = 128;
        target.max_request_target_bytes = 768;
        target.h1_max_buf_size_bytes = 0x0004_0000;
        limits.max_admitted_header_bytes = 2_048_u32;
        limits.max_admitted_header_count = 64;
        target.h1_max_headers = 64;
        // `roomy_fixture_limits` already lifts the ceiling, which this
        // fixture needs: a quarter-megabyte read buffer on every one of
        // 384 slots is 100 MB of parser peak on top of a row that is
        // already over its own ceiling. The ceiling is not what this
        // test is about.

        let derived = validate_isolate_limits(
            &limits,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the h1-dominant row validates");
        assert!(
            derived.h1_slot_peak > derived.h2_slot_peak,
            "fixture must make h1 dominate: h1={}, h2={}",
            derived.h1_slot_peak,
            derived.h2_slot_peak
        );
        assert_eq!(
            derived.pre_admission_parser_term,
            derived.h1_slot_peak * u64::from(target.platform_concurrent_ceiling)
        );
    }

    #[test]
    fn validate_isolate_limits_rejects_raw_bytes_above_usize_max() {
        let target = fixture_target();

        // One above what this target's `usize` can hold. On a 64-bit
        // target every `u64` is representable, so the conversion
        // cannot fail there and the check is vacuous by construction —
        // the assertion is written to hold on both widths rather than
        // to pass only where it can fire.
        let over_usize = u64::try_from(usize::MAX)
            .ok()
            .and_then(|max| max.checked_add(1));
        if let Some(value) = over_usize {
            let mut unrepresentable = roomy_fixture_limits();
            unrepresentable.max_outbound_raw_bytes = value;
            assert_eq!(
                validate_isolate_limits(
                    &unrepresentable,
                    &target,
                    FIXTURE_OUTBOUND_META_BYTES,
                    super::RequestConstructionCosts::default(),
                    crate::CatalogResourceCosts::UNCHANGED
                ),
                Err(BudgetError::LimitExceedsUsize {
                    field: LimitField::MaxOutboundRawBytes,
                    value,
                })
            );
        }

        // The boundary value itself passes the conversion.
        let boundary = u64::try_from(usize::MAX).unwrap_or(u64::MAX);
        let mut at_boundary = roomy_fixture_limits();
        at_boundary.max_outbound_raw_bytes = boundary;
        let at_boundary_outcome = validate_isolate_limits(
            &at_boundary,
            &target,
            FIXTURE_OUTBOUND_META_BYTES,
            super::RequestConstructionCosts::default(),
            crate::CatalogResourceCosts::UNCHANGED,
        );
        // `usize::MAX` is representable as usize by definition, so the
        // boundary never fails the conversion. It still fails the
        // budget, because a cap that large cannot fit any ceiling —
        // an arithmetic refusal, not a conversion one.
        assert!(
            !matches!(
                at_boundary_outcome,
                Err(BudgetError::LimitExceedsUsize { .. })
            ),
            "the usize boundary must pass the conversion, got {at_boundary_outcome:?}"
        );
        let mut representable = roomy_fixture_limits();
        representable.max_outbound_raw_bytes = CAP_256_KIB;
        assert_eq!(
            validate_isolate_limits(
                &representable,
                &target,
                FIXTURE_OUTBOUND_META_BYTES,
                super::RequestConstructionCosts::default(),
                crate::CatalogResourceCosts::UNCHANGED
            )
            .expect("a representable value validates")
            .max_outbound_raw_bytes_usize,
            usize::try_from(CAP_256_KIB).unwrap_or(usize::MAX)
        );
    }
}
