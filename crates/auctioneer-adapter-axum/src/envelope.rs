//! Axum's boot envelope: the per-isolate memory budget this target
//! installs before it serves a route.
//!
//! This adapter owns both the shipped row and its worked proof tests.
//! The platform-independent runtime validates the equation; it does not
//! carry a second copy of this adapter's inputs.
//!
//! # Ceiling policy
//!
//! Numbers below describe the pure runtime row. Core boot additionally funds
//! `serialized_baked_config_bytes * CONFIG_PARSE_FACTOR` per admitted auction;
//! the boot warning reports that exact compiled-configuration total.
//!
//! The compiled candidate selects a 4 GiB local operator budget, not a
//! hosted or 512 MB fitness claim. Its complete baked-config equation
//! leaves at least 256 MiB unallocated. This structural fit is not physical
//! qualification of the serving artifact, throughput or bidder latency.
//! Over-ceiling custom rows still warn and proceed at installation.
//!
//! Auction populations are charged conservatively at their full peaks.
//! Sequential request-construction work is not evidence that every
//! concurrent auction reaches that transient peak simultaneously.
//!
//! # Where each value comes from
//!
//! These provenances are not interchangeable:
//!
//! - **Derived.** auctioneer's own knobs. Each one's arithmetic is written
//!   beside it, and [`auctioneer_runtime::validate_isolate_limits`] asserts
//!   the relationships between them at boot, so a wrong derivation
//!   fails loudly rather than silently under-budgeting.
//! - **Measured.** The historical baseline component. It is whole-process resident
//!   memory after a no-op boot, taken as the max of 30 runs with a 20%
//!   margin carried inside the committed constant. The constant records
//!   the raw max, the margin arithmetic and the date, and
//!   `scripts/measure-baseline.sh` re-runs the procedure.
//! - **Reserved.** SDK snapshot startup/residency and parser allocations have
//!   explicit allowances, checked against the pinned SDK's allocation helpers.
//!   These are requested-allocation charges, not measured RSS. The historical
//!   baseline must be remeasured for this artifact during deployment qualification.
//! - **Pending measurement.** `transmit_state_bytes` remains provisional.
//! - **Enforced.** Native connection/head limits use the SDK's HTTP/1 runner.
//!   Startup preflight rejects environment settings wider than this funded row.

use std::time::Duration;

use edgezero_adapter_axum::config_store_limits::ConfigStoreLimits;
use edgezero_adapter_axum::ingress_config::AxumIngressConfig;
use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
use edgezero_core::env_config::EnvConfig;

/// `DEFAULT_CONFIG_JSON_NODES` in the width the budget charges it at.
///
/// A twin rather than a cast: `u64::from` is not callable in a `const`
/// initializer on the pinned toolchain, and the workspace denies `as`.
/// The assertion below is what keeps the two from drifting.
const CONFIG_JSON_NODES_U64: u64 = 8_192;
use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
use auctioneer_core::host_primitives::construction_diagnostic_costs;
use auctioneer_core::host_primitives::{BootEnvelope, ResourceProfile, StderrBootLog};
// `H2_MIN_FIELD_ENTRY_BYTES` is not in `auctioneer-runtime`'s root re-export
// list, so it is named through its own module rather than adding a
// re-export line other work in flight also touches.
use auctioneer_bidder::schema::controlled_native_costs;
use auctioneer_runtime::budget::{
    ASSEMBLY_FACTOR, H2_MIN_FIELD_ENTRY_BYTES, min_aux_route_peak_bytes,
};
use auctioneer_runtime::{
    MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES,
    MIN_RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits, TargetPlatformConstants,
    min_terminal_response_permits,
};

/// Selected local operator budget on the verified 24 GiB host. Neither this
/// selection nor the equation proves runtime residency or deployment fitness.
const ISOLATE_TOTAL_BYTES: u64 = 4 << 30;

/// Independent rejection caps for the local candidate's request/response legs.
/// Legitimate traffic outside these limits needs a separately qualified profile;
/// narrowing bytes does not reduce any measured allocation factor.
const INBOUND_REQUEST_BYTES: u64 = 32 << 10;
const MAX_OUTBOUND_RAW_BYTES: u64 = 32 << 10;
const MAX_ENCODED_REQUEST_BYTES: u64 = 32 << 10;
const MAX_RESPONSE_ENCODED_BYTES: u64 = 16 << 10;
const MAX_DECODED_RESPONSE_BYTES: u64 = 32 << 10;

/// The response cap, derived from the serialized-output floor below.
///
/// `FinalResponseBelowAssemblyFloor` requires this cap to hold
/// `max_outbound_requests x max_decoded_bytes x ASSEMBLY_FACTOR`.
/// Deriving it from that floor keeps it in sync with the runtime charge.
/// Typed assembly remains independently funded in the auction ledger.
///
/// This is the structural floor, not evidence about real response sizes.
const FINAL_RESPONSE_BYTES: u64 = MAX_CALLS_U64 * MAX_DECODED_RESPONSE_BYTES * ASSEMBLY_FACTOR;

/// [`MAX_CALLS`] as a `u64`, for the const arithmetic above.
/// `u64::from` is not callable in a const initializer, and an `as`
/// cast is refused by this crate's lint set, so the width is declared
/// and the two are pinned equal below.
const MAX_CALLS_U64: u64 = 16;
const _: () = assert!(
    MAX_CALLS == 16 && MAX_CALLS_U64 == 16,
    "the u64 copy of the call count must equal MAX_CALLS",
);

/// The syntactic node budget for the inbound parse.
///
/// `NODE_RETAINED_BYTES` now charges 384 bytes/node, not 64. The native
/// object forest retained `8_106_496 / 25_729 = ~315.073` bytes/node
/// (array forest: ~127.040); 384 covers the binding result plus 20%.
/// The candidate retains the 384-byte factor at `2_048 x 384 = 786_432`
/// bytes per input tree. Normalization's implicit nodes and all parser /
/// transformation scratch remain independently funded. The selected cap covers
/// `16 + 16 x (16 + 8 x 8) = 1_296` return-all-bid-status metadata nodes,
/// including the global response reservation; a half-node variant has 1,024
/// nodes for the 400-node one-impression baseline.
const NODE_BUDGET: u32 = 2_048;

/// The whole inbound header block admitted at the ingress seam.
const ADMITTED_HEADER_BYTES: u32 = 1 << 14;

/// Inbound header entries admitted at the ingress seam.
const ADMITTED_HEADER_COUNT: u16 = 128;

/// The same count in the width the h1 header-count knob speaks.
/// `u32::from` is not callable in a `const fn` on stable, so the twin is
/// written once and pinned to its `u16` original below. Validation
/// asserts the knob equals the admitted count, so a drift between these
/// two fails boot rather than passing silently.
const ADMITTED_HEADER_COUNT_U32: u32 = 128;

const _: () = assert!(
    ADMITTED_HEADER_COUNT == 128 && ADMITTED_HEADER_COUNT_U32 == 128,
    "the admitted header count drifted from its u32 twin"
);

/// Bidders one auction may fan out to.
const MAX_BIDDERS: u32 = 16;

/// Outbound calls one auction may make in this local candidate. The cap
/// covers a future selected 13-bidder population; qualification can reach
/// 16 through fake requested aliases to a discovered local fixture. The
/// final-response cap must fund
/// the assembly scratch for this many calls; the boot validator checks
/// that sizing floor separately from its ceiling warning.
const MAX_CALLS: u32 = 16;

/// Impressions one auction may carry. Sets the per-call metadata
/// charge, hence [`OUTBOUND_META_BYTES`].
const MAX_IMPRESSIONS: u16 = 8;

/// Concurrent outbound bidder legs one auction may have in flight.
///
/// **The name is wrong and the description this row used to carry was
/// wrong with it.** This field sizes
/// [`auctioneer_runtime::StreamGate`], and despite the "body streams"
/// spelling the gate counts no inbound request bodies at all.
/// `auctioneer-core`'s `drive_fanout` is its only acquirer: it batches ONE
/// auction's outbound calls through
/// `StreamGate::acquire_wave(wave_width(gate.capacity(), remaining))`,
/// so the capacity is the width of a single auction's fanout wave.
/// `per_stream_bytes` charges the matching shape — a transport buffer,
/// a decoded buffer and one decoder state, which is a bidder RESPONSE
/// being read.
///
/// The gate is QUEUED, not shedding: `acquire_wave` makes a wave wait
/// its turn rather than refusing it. A capacity below
/// [`MAX_CALLS`] therefore rejects nothing — it splits one auction's
/// fanout into serial waves inside the same auction deadline. This
/// candidate queues 16 calls through two slots; latency and fill rate
/// still require qualification under the selected deadline.
///
/// The per-stream peak is `21_350_048` bytes, nearly all of it one
/// brotli decoder state at [`limits`]'s 22-bit window, so this term is
/// `2 x 21_350_048 = 42_700_096`. Two things would change it: a smaller
/// `max_brotli_wbits`, which shrinks every stream, or the decoder
/// being pooled across a wave rather than allocated per leg, which
/// would make the population stop multiplying the state.
///
/// The live handler uses `EdgeZeroFanout::with_wave_scheduler` with
/// this installed gate. Explicit test fanout handles bypass transport;
/// the production `HttpClient` path cannot use an envelope-less fallback.
const MAX_ACTIVE_BODY_STREAMS: u32 = 2;

/// Per-call outbound metadata reservation, as
/// `auctioneer_bidder::outbound_meta_bytes(MAX_IMPRESSIONS)` returns it.
///
/// Use the owning crate's const helper so impression-cap changes cannot
/// leave a hand-copied metadata allowance stale.
const OUTBOUND_META_BYTES: u32 = auctioneer_bidder::outbound_meta_bytes(MAX_IMPRESSIONS);

/// Funded HTTP/1 connection ceiling. SDK defaults use 256; startup rejects
/// overrides above 384 before the SDK loads stores or binds its listener.
/// The runner acquires capacity before constructing a parser. HTTP/2 is disabled.
const PLATFORM_CONCURRENT_CEILING: u32 = 384;

/// Whole-process resident memory after boot, with the measurement
/// contract's 20% safety margin already inside it.
///
/// MEASURED on 2026-09-26, on native arm64 (16 KiB pages), replacing the
/// `96_000_000` placeholder that had never been measured.
///
/// - Metric: the probe process's OS resident-set size, read from outside
///   the process with `ps -o rss=`, after a no-op boot through the same
///   production bootstrap this binary runs and after every page of the
///   embedded GVL archive has been made resident.
/// - 30 repetitions, max taken (not mean): observations spanned
///   `22_659_072` to `22_773_760` bytes, so the raw max is `22_773_760`.
/// - Revalidated on 2026-10-01 after user-sync and archive changes: 30
///   observations spanned `22_708_224` to the same `22_773_760`-byte max.
///   The existing margined constant still covers the refreshed baseline.
/// - Margin rule: `ceil(max x 6/5)`, as the checked integer expression
///   `(22_773_760 x 6 + 4) / 5 = 27_328_512`. The 20% lives in this
///   constant and nowhere else — `validate_isolate_limits` consumes it
///   as is and derives nothing further from it.
///
/// The touch pass is what makes this figure honest rather than optimistic:
/// the archive is `include_bytes!` rodata in the executable image, and
/// this host faults those pages lazily, so an untouched boot reads about
/// `2_150` KiB and understates the baseline by the whole ~20 MB archive.
///
/// Re-run with `scripts/measure-baseline.sh`, which prints every observation
/// and this arithmetic. This is historical measurement, not qualification of
/// the updated SDK's serving artifact or populated config stores. A margined
/// current-artifact baseline exceeding it requires re-derivation and a fresh
/// check of the complete row against the selected local budget.
const BASELINE_BYTES: u64 = 27_328_512;

/// Aggregate snapshot residency plus sequential startup staging, enforced by
/// the SDK. Charged alongside the historical baseline rather than pretending
/// it was present in that no-config measurement. Startup and serving do not
/// overlap, so retaining the full startup allowance while serving overfunds
/// the smaller resident snapshot allowance. Neither term certifies allocator/RSS
/// overhead; exact-artifact baseline/load qualification remains required.
const SDK_CONFIG_STARTUP_BYTES: u64 = 112 << 20;

/// Per-response transmit state.
///
/// Host-runtime bookkeeping on the write path, charged once per egress,
/// aux and static permit. Not yet measured for this SDK revision.
const TRANSMIT_STATE_BYTES: u32 = 4_096;

/// Per-connection SDK allocation allowance: receive backing allocation,
/// trailer buffer, two parser/cache header maps and four raw-head byte copies.
/// Tests check the allowance at the widest permitted header profile using the
/// SDK's audited helpers. The generic budget charges this through its larger
/// protocol term; that conservative accounting does not enable HTTP/2.
/// Fixed tasks, allocator overhead and RSS remain deployment measurements.
const PARSER_TRANSIENT_BYTES: u32 = 1 << 20;

/// The largest field section admitted, in h2 field-section accounting
/// with pseudoheaders included: the charged bound.
///
/// `55_424 = 16_384 admitted header bytes
///         + (128 entries + 4 pseudoheaders) x 32 bytes per entry
///         + 34_816 pseudoheader allowance`.
const PARSER_HEADER_CEILING_BYTES: u32 = 55_424;

/// The request-target cap, normalized per protocol.
const MAX_REQUEST_TARGET_BYTES: u32 = 33 * 1_024;

/// The byte allowance for the four request pseudoheaders.
///
/// Must cover the request target plus the fixed per-field overhead:
/// `33_792 + 192 <= 34_816`, which validation asserts.
const PSEUDOHEADER_ALLOWANCE_BYTES: u32 = 34_816;

/// The SDK-enforced h1 head-buffer maximum. Startup rejects raw-head
/// settings above this funded 64 KiB cap.
///
/// Must hold an admitted header block plus its request target and head
/// slack: `16_384 + 33_792 + 1_024 = 51_200 <= 65_536`, which validation
/// asserts. Also bounds the SDK's joint raw-head setting; parser backing
/// allocations beyond the logical threshold are reserved separately above.
const H1_MAX_BUF_SIZE_BYTES: u32 = 1 << 16;

/// Auctions this server admits at once.
///
/// Two admitted auctions are funded at their full declared peaks. The
/// 384-connection parser population remains independent of this gate.
/// This local candidate's throughput and simultaneous retention still
/// require measurement; saturation sheds at admission.
const MAX_CONCURRENT_AUCTIONS: u32 = 2;

/// Auction responses in flight on the egress path.
///
/// One per admitted auction. An egress permit is held while a
/// completed auction's response is serialized and written, and the
/// write can block on a slow client for as long as
/// `response_write_deadline_ms` allows, so an auction that has
/// finished bidding still occupies one. Sizing this below
/// [`MAX_CONCURRENT_AUCTIONS`] would let admitted auctions finish and
/// then queue behind the egress gate holding all their per-auction
/// memory, which is the expensive state to hold.
///
/// It tracks [`MAX_CONCURRENT_AUCTIONS`] deliberately, so the two
/// cannot drift apart into that state. Re-check the pairing if the
/// egress path ever stops holding a permit for the whole write.
const EGRESS_PERMITS: u32 = MAX_CONCURRENT_AUCTIONS;

/// Pre-deserialization cap on the raw runtime-config envelope, and the
/// largest single blob inside it.
///
/// Named rather than written inline at the row, because
/// [`AUX_ROUTE_PEAK_BYTES`] is checked against both: the aux floor and the
/// row the validator checks it against must read the SAME two figures,
/// or the check compares a derivation to a different row than the one
/// that shipped.
const MAX_RUNTIME_CONFIG_BYTES: u64 = 128 << 10;
const MAX_RUNTIME_CONFIG_BLOB_BYTES: u64 = 64 << 10;

/// Aux-route gate capacity and the peak one aux route costs.
///
/// The aux class covers the manifest routes (`/info/bidders`,
/// `/info/bidders/{name}`, `/_auctioneer/health/gvl`) and the 404/405
/// fallback arms. These are small, fast, cacheable responses, and the
/// gate exists so a flood of them cannot consume auction admission —
/// so its capacity is a deliberate RESERVATION rather than a fit to an
/// arrival rate.
///
/// One aux slot and the independent probe-config slot each retain a
/// 128 MiB reservation. The full production derivation at these caps,
/// including 1,592 serialized baked-config bytes, requires 58,647,552
/// non-GVL bytes per slot. The 75,570,176-byte slack is structural sizing,
/// not a new allocation factor or measured resident peak. Boot checks
/// the complete floor, not only config extraction and manifests.
const AUX_ROUTE_PERMITS: u32 = 1;
const AUX_ROUTE_PEAK_BYTES: u64 = 128 << 20;

const _: () = assert!(
    AUX_ROUTE_PEAK_BYTES
        >= min_aux_route_peak_bytes(
            MAX_RUNTIME_CONFIG_BYTES,
            MAX_RUNTIME_CONFIG_BLOB_BYTES,
            CONFIG_JSON_NODES_U64
        ),
    "the aux reservation must also fund config extraction and the manifest",
);

/// Distinct parsed vendor lists the store's cache owns, and the heap
/// ONE of them retains. The cache cap `auctioneer-privacy` enforces is 4;
/// this reserves the measured floor per list, which is what the budget
/// requires and what `gvl_retained_charge` pins.
const GVL_CACHE_ENTRIES: u32 = 4;
const GVL_PARSED_BYTES: u64 = MAX_GVL_PARSED_BYTES;

/// Static-class response permits and the one render buffer they size.
///
/// Three per-request populations hold a static permit to terminal
/// egress, and they can be live at once: every ADMITTED auction,
/// because `ingress::admit_auction` reserves one at admission and holds
/// it through the handler; every request rendering a refusal, via
/// `ingress::refuse`; and every pre-admission head rejection such as a
/// 414 or 431, via `static_egress`. The third runs BEFORE any class
/// gate, so it is bounded by requests in flight rather than by
/// admission — which is [`PLATFORM_CONCURRENT_CEILING`], 384.
///
/// [`min_terminal_response_permits`] owns that derivation, and every
/// adapter either declares its value from it or, as here, asserts
/// against it. On this row the platform term binds: 384 exceeds the
/// admission-bounded `2 + 1 = 3`, so the ceiling is the tight bound
/// and the admission sum is satisfied with room. Exhausting
/// the class turns a refusal into a connection abort at precisely the
/// moment refusals are being drawn.
///
/// It is cheap: `384 x (256 x 2 + 4_096) = 1_769_472` bytes, because a
/// refusal body is 256 bytes. This is not a term worth trimming.
const TERMINAL_RESPONSE_PERMITS: u32 = PLATFORM_CONCURRENT_CEILING;
const REFUSAL_BODY_BYTES: u32 = 256;

/// A saturated server must be able to answer a refusal, so the class
/// has to fund every population that holds a permit at once. Checked at
/// COMPILE time, and the check is live on this row because the constant
/// is `PLATFORM_CONCURRENT_CEILING` rather than the derivation's result:
/// raising the admission sum past 384 fails the build rather than
/// leaving a doc comment to go stale.
const _: () = assert!(
    TERMINAL_RESPONSE_PERMITS
        >= min_terminal_response_permits(
            MAX_CONCURRENT_AUCTIONS,
            AUX_ROUTE_PERMITS,
            PLATFORM_CONCURRENT_CEILING,
        ),
    "terminal-response permits cannot bound every admitted auction plus a \
     concurrent refusal, so a saturated server aborts connections"
);

/// This target's resolved per-isolate limits.
#[must_use]
pub const fn limits() -> ResolvedIsolateLimits {
    ResolvedIsolateLimits {
        isolate_total_bytes: ISOLATE_TOTAL_BYTES,
        baseline_bytes: BASELINE_BYTES + SDK_CONFIG_STARTUP_BYTES,
        inbound_request_bytes: INBOUND_REQUEST_BYTES,
        // The floor itself: this target charges the measured worst case
        // and nothing above it. A deployment facing hostile traffic may
        // raise it; validation refuses anything under.
        inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
        max_outbound_request_bytes: MAX_ENCODED_REQUEST_BYTES,
        max_outbound_raw_bytes: MAX_OUTBOUND_RAW_BYTES,
        max_transport_bytes: MAX_RESPONSE_ENCODED_BYTES,
        max_decoded_bytes: MAX_DECODED_RESPONSE_BYTES,
        // Keep M4's 256-byte parse floor and fund the independent node peak.
        // Both charges are configurable upward, never below their floors.
        response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
        response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
        // The cap covers the serialized-output floor at the selected caps.
        final_response_bytes: FINAL_RESPONSE_BYTES,
        egress_permits: EGRESS_PERMITS,
        response_write_deadline_ms: 30_000,
        // Axum decodes through auctioneer's own chain, so there is no
        // separate in-guest decoder bound to take the max against.
        guest_decoder_bound_bytes: 0,
        aux_route_permits: AUX_ROUTE_PERMITS,
        aux_route_peak_bytes: AUX_ROUTE_PEAK_BYTES,
        max_brotli_wbits: 22,
        max_brotli_layers: 1,
        gvl_cache_entries: GVL_CACHE_ENTRIES,
        gvl_parsed_bytes: GVL_PARSED_BYTES,
        max_bidders_per_auction: MAX_BIDDERS,
        max_outbound_requests: MAX_CALLS,
        max_concurrent_auctions: MAX_CONCURRENT_AUCTIONS,
        max_active_body_streams: MAX_ACTIVE_BODY_STREAMS,
        max_runtime_config_bytes: MAX_RUNTIME_CONFIG_BYTES,
        max_runtime_config_blob_bytes: MAX_RUNTIME_CONFIG_BLOB_BYTES,
        max_parsed_nodes: NODE_BUDGET,
        max_copied_header_bytes: 4_096,
        max_admitted_header_bytes: ADMITTED_HEADER_BYTES,
        max_admitted_header_count: ADMITTED_HEADER_COUNT,
        max_impressions_per_auction: MAX_IMPRESSIONS,
        max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
        terminal_response_permits: TERMINAL_RESPONSE_PERMITS,
        refusal_body_bytes: REFUSAL_BODY_BYTES,
        transmit_state_bytes: TRANSMIT_STATE_BYTES,
    }
}

/// What this target's platform fixes, as this adapter configures it.
#[must_use]
pub const fn target() -> TargetPlatformConstants {
    TargetPlatformConstants {
        platform_concurrent_ceiling: PLATFORM_CONCURRENT_CEILING,
        // The `http::HeaderMap` per-entry cost, carried over from the
        // committed row as measured there rather than re-measured here.
        header_entry_overhead_bytes: 64,
        parser_header_ceiling_bytes: PARSER_HEADER_CEILING_BYTES,
        // h2 rejects at `headers_size >= max`, so the configured knob
        // sits exactly one above the admitted ceiling. Validation
        // asserts this rather than trusting the literal.
        parser_header_knob_bytes: PARSER_HEADER_CEILING_BYTES + 1,
        max_request_target_bytes: MAX_REQUEST_TARGET_BYTES,
        h1_max_buf_size_bytes: H1_MAX_BUF_SIZE_BYTES,
        // h1 exposes a header-count knob; it must equal the admitted
        // count, which validation asserts.
        h1_max_headers: ADMITTED_HEADER_COUNT_U32,
        // Derived, not configured: h2 exposes no header-count knob, so
        // the parser-time entry maximum is the byte ceiling over the
        // minimum encoded entry size, `55_424 / 32 = 1_732`. Validation
        // recomputes this from the ceiling and refuses a mismatch, so
        // the division is the derivation rather than a magic number.
        parser_max_header_entries: PARSER_HEADER_CEILING_BYTES.div_euclid(H2_MIN_FIELD_ENTRY_BYTES),
        pseudoheader_allowance_bytes: PSEUDOHEADER_ALLOWANCE_BYTES,
        parser_transient_bytes: PARSER_TRANSIENT_BYTES,
        // Two buffers per handoff: there is no verified owned zero-copy
        // handoff on this target, so both are charged.
        egress_handoff_factor: 2,
    }
}

/// Reject SDK environment settings that exceed the native row's allowances.
/// The SDK still owns parsing, store loading and serving; this is its preflight.
///
/// # Errors
/// Returns category/field-only diagnostics for invalid or unfunded settings.
pub fn validate_sdk_settings(env: &EnvConfig) -> anyhow::Result<()> {
    let ingress = AxumIngressConfig::from_env(env)?;
    let config = ConfigStoreLimits::from_env(env)?;
    let defaults = ConfigStoreLimits::default();
    for (within, field) in [
        (
            u32::try_from(ingress.max_connections())
                .is_ok_and(|value| value <= PLATFORM_CONCURRENT_CEILING),
            "max_connections",
        ),
        (
            u32::try_from(ingress.max_raw_head_bytes())
                .is_ok_and(|value| value <= H1_MAX_BUF_SIZE_BYTES),
            "max_raw_head_bytes",
        ),
        (
            ingress.max_header_count() <= usize::from(ADMITTED_HEADER_COUNT),
            "max_header_count",
        ),
        (
            ingress.head_read_timeout() <= Duration::from_secs(10),
            "head_read_timeout_ms",
        ),
        (
            config.max_entries() <= defaults.max_entries(),
            "max_entries",
        ),
        (
            config.max_file_bytes() <= defaults.max_file_bytes(),
            "max_file_bytes",
        ),
        (
            config.max_key_bytes() <= defaults.max_key_bytes(),
            "max_key_bytes",
        ),
        (
            config.max_value_bytes() <= defaults.max_value_bytes(),
            "max_value_bytes",
        ),
        (
            config.max_resident_bytes() <= defaults.max_resident_bytes(),
            "max_resident_bytes",
        ),
        (
            u64::try_from(config.max_startup_bytes())
                .is_ok_and(|value| value <= SDK_CONFIG_STARTUP_BYTES),
            "max_startup_bytes",
        ),
    ] {
        if !within {
            anyhow::bail!("Axum SDK setting `{field}` exceeds the funded native profile");
        }
    }
    Ok(())
}

/// This target's boot envelope, ready for
/// `bootstrap_adapter_with_envelope`.
#[must_use]
pub fn envelope<'a>(
    limits: &'a ResolvedIsolateLimits,
    target: &'a TargetPlatformConstants,
) -> BootEnvelope<'a> {
    BootEnvelope {
        limits,
        target,
        outbound_meta_bytes: OUTBOUND_META_BYTES,
        construction_costs: core_construction_costs(),
        catalog_costs: catalog_costs(limits.max_bidders_per_auction),
        // This platform surfaces the guest's stderr, so the shared
        // stderr sink is the real target channel. It is installed
        // BEFORE the envelope is validated, which is what makes the
        // over-ceiling warning reach an operator: the pinned EdgeZero
        // installs its own logger inside `run_app`, after boot returns.
        log_sink: &StderrBootLog,
    }
}

fn catalog_costs(bidders: u32) -> auctioneer_runtime::CatalogResourceCosts {
    controlled_native_costs(construction_diagnostic_costs(bidders))
}

/// The platform passed by this adapter's actual dispatch path.
pub const DISPATCH_PLATFORM: edgezero_core::PlatformMetadata = edgezero_adapter_axum::AXUM_PLATFORM;

/// Source declarations consumed by both bootstrap and the resource inspector.
#[must_use]
pub fn resource_profile() -> ResourceProfile {
    let limits = limits();
    let target = target();
    let boot = envelope(&limits, &target);
    ResourceProfile {
        name: "axum",
        dispatch_name: "axum",
        target_abi: "native",
        headroom_class: auctioneer_runtime::HeadroomClass::Native,
        memory_scope: edgezero_core::MemoryCeilingScope::PerInstance,
        platform: edgezero_adapter_axum::AXUM_PLATFORM,
        dispatch_platform: DISPATCH_PLATFORM,
        limits,
        target,
        outbound_meta_bytes: boot.outbound_meta_bytes,
        construction_costs: boot.construction_costs,
        catalog_costs: boot.catalog_costs,
        log_sink: boot.log_sink,
    }
}

#[cfg(test)]
mod tests {
    use super::catalog_costs;

    #[test]
    fn diagnostic_profile_follows_the_supplied_bidder_limit_not_the_adapter_default() {
        for (names, expected) in [(1, 102_400), (3, 299_008), (51, 5_017_600)] {
            let mut row = super::limits();
            row.max_bidders_per_auction = names;
            let platform = super::target();
            let boot = super::envelope(&row, &platform);
            assert_eq!(boot.catalog_costs.context_per_auction_bytes, expected);
        }
    }
    #[test]
    fn resource_profile_shares_boot_inputs_and_actual_dispatch_metadata() {
        let profile = super::resource_profile();
        let boot = super::envelope(&profile.limits, &profile.target);
        assert_eq!(profile.name, "axum");
        assert_eq!(profile.target_abi, "native");
        assert_eq!(
            profile.headroom_class,
            auctioneer_runtime::HeadroomClass::Native
        );
        assert_eq!(
            profile.memory_scope,
            edgezero_core::MemoryCeilingScope::PerInstance
        );
        assert_eq!(profile.dispatch_platform, super::DISPATCH_PLATFORM);
        assert!(profile.dispatch_matches_platform());
        assert_eq!(profile.outbound_meta_bytes, boot.outbound_meta_bytes);
        assert_eq!(profile.construction_costs, boot.construction_costs);
        assert_eq!(profile.catalog_costs, boot.catalog_costs);
        assert_eq!(profile.catalog_costs.context_per_auction_bytes, 1_576_960);
    }

    fn selected_native_candidate() -> super::ResolvedIsolateLimits {
        super::ResolvedIsolateLimits {
            isolate_total_bytes: 4 << 30,
            inbound_request_bytes: 32 << 10,
            max_outbound_raw_bytes: 32 << 10,
            max_outbound_request_bytes: 32 << 10,
            max_transport_bytes: 16 << 10,
            max_decoded_bytes: 32 << 10,
            final_response_bytes: 16 * (32 << 10) * 3,
            max_bidders_per_auction: 16,
            max_outbound_requests: 16,
            max_concurrent_auctions: 2,
            max_active_body_streams: 2,
            egress_permits: 2,
            aux_route_permits: 1,
            aux_route_peak_bytes: 128 << 20,
            max_parsed_nodes: 2_048,
            max_impressions_per_auction: 8,
            max_runtime_config_bytes: 128 << 10,
            max_runtime_config_blob_bytes: 64 << 10,
            baseline_bytes: 27_328_512 + 0x0700_0000,
            inbound_parse_factor: 36,
            response_parse_factor: 256,
            response_node_peak_bytes: 320,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: 512_000,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 0x4000,
            max_admitted_header_count: 128,
            max_config_json_nodes: 8_192,
            terminal_response_permits: 384,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        }
    }

    fn baked_config_bytes() -> u64 {
        auctioneer_runtime::serialized_json_bytes(&(
            auctioneer_config::host_default(),
            auctioneer_config::account_default(),
        ))
        .unwrap()
    }

    #[test]
    fn baked_cookie_identity_bytes_are_funded_per_admitted_auction() {
        let host = auctioneer_config::host_default();
        assert_eq!(host.host_cookie.cookie_name, "auctioneer_uid");
        assert_eq!(host.host_cookie.family, "auctioneer");
        let mut shorter = host.clone();
        shorter.host_cookie.cookie_name = "fake_a_uid".to_owned();
        shorter.host_cookie.family = "fake_a".to_owned();
        let shorter_bytes = auctioneer_runtime::serialized_json_bytes(&(
            &shorter,
            auctioneer_config::account_default(),
        ))
        .unwrap();
        assert_eq!(shorter_bytes, 1_584);
        assert_eq!(baked_config_bytes(), 1_592);
        let row = super::limits();
        let derive = |bytes| {
            auctioneer_runtime::derive_isolate_budget_with_baked_config(
                &row,
                &super::target(),
                super::OUTBOUND_META_BYTES,
                super::core_construction_costs(),
                bytes,
                catalog_costs(MAX_BIDDERS),
            )
            .unwrap()
        };
        let reference = derive(shorter_bytes);
        let selected = derive(baked_config_bytes());
        assert_eq!(
            selected.runtime_config_term - reference.runtime_config_term,
            1_024
        );
        assert_eq!(
            selected.per_auction_peak - reference.per_auction_peak,
            1_024
        );
        assert_eq!(selected.total_bytes - reference.total_bytes, 2_048);
    }

    #[test]
    fn selected_native_candidate_funds_the_full_baked_aux_floor() {
        let row = selected_native_candidate();
        let aux =
            derive_aux_work_for_profile(&row, &super::target(), baked_config_bytes()).unwrap();
        assert_eq!(baked_config_bytes(), 1_592);
        assert_eq!(aux.peak_bytes(), 58_647_552);
        assert_eq!(row.aux_route_peak_bytes, 0x0800_0000);
        assert_eq!(
            row.aux_route_peak_bytes
                .checked_sub(aux.peak_bytes())
                .unwrap(),
            75_570_176
        );
        assert_eq!(aux.pinned_gvl_bytes(), 512_000);
        assert!(aux.peak_bytes() <= row.aux_route_peak_bytes);
        let derived = auctioneer_runtime::derive_isolate_budget_with_baked_config(
            &row,
            &super::target(),
            auctioneer_bidder::outbound_meta_bytes(row.max_impressions_per_auction),
            super::core_construction_costs(),
            baked_config_bytes(),
            catalog_costs(MAX_BIDDERS),
        )
        .unwrap();
        assert_eq!(
            derived.total_bytes,
            2_536_791_800 + 2 * 128 * 1024 + 26_214_400
        );
        assert!(derived.total_bytes + (256_u64 << 20_u32) <= row.isolate_total_bytes);
        let mut underfunded = row;
        underfunded.aux_route_peak_bytes = aux.peak_bytes() - 1;
        assert_eq!(
            auctioneer_runtime::derive_isolate_budget_with_baked_config(
                &underfunded,
                &super::target(),
                super::OUTBOUND_META_BYTES,
                super::core_construction_costs(),
                baked_config_bytes(),
                catalog_costs(MAX_BIDDERS)
            ),
            Err(auctioneer_runtime::BudgetError::AuxRoutePeakBelowDerived {
                configured: aux.peak_bytes() - 1,
                required: aux.peak_bytes(),
            }),
        );
    }

    #[test]
    fn shipped_row_has_exact_selected_caps() {
        assert_eq!(super::limits(), selected_native_candidate());
    }

    #[test]
    fn selected_row_funds_max_status_metadata_nodes() {
        let row = super::limits();
        // The global response reserves 16 nodes, each bidder reserves another
        // 16, and return-all-bid-status reserves 8 per participating impression.
        let required = 16
            + row.max_bidders_per_auction * (16 + 8 * u32::from(row.max_impressions_per_auction));
        assert_eq!(required, 1_296);
        assert!(
            row.max_parsed_nodes >= required,
            "max-status metadata needs {required} nodes, cap={}",
            row.max_parsed_nodes
        );
    }

    #[test]
    fn half_node_variant_funds_baseline_metadata_nodes() {
        let row = super::limits();
        let baseline = 16 + row.max_bidders_per_auction * (16 + 8);
        let variant_nodes = row.max_parsed_nodes.div_euclid(2);
        assert_eq!(baseline, 400);
        assert!(
            variant_nodes >= baseline,
            "baseline metadata needs {baseline} nodes, variant cap={variant_nodes}"
        );
        assert_eq!(variant_nodes, 1_024);
    }

    #[test]
    fn node_growth_keeps_the_512_node_accounting_reference() {
        let row = super::limits();
        let mut reference = row;
        reference.max_parsed_nodes = 512;
        let derive = |profile: &ResolvedIsolateLimits| {
            auctioneer_runtime::derive_isolate_budget_with_baked_config(
                profile,
                &super::target(),
                super::OUTBOUND_META_BYTES,
                super::core_construction_costs(),
                baked_config_bytes(),
                catalog_costs(MAX_BIDDERS),
            )
            .unwrap()
        };
        // Budget reference only: exchange's 512-node request-preflight regression
        // must still exercise its own receipts independently of this larger cap.
        let previous = derive(&reference);
        let selected = derive(&row);
        // One sequential credential bank per admitted auction, not per call.
        assert_eq!(
            previous.total_bytes,
            2_355_375_864 + 2 * (1_576_960 + 128 * 1024) + 26_214_400
        );
        assert_eq!(
            previous.per_auction_peak,
            565_749_980 + 1_576_960 + 128 * 1024 + 0x0008_0000
        );
        assert_eq!(previous.bookkeeping_term, 917_504);
        assert_eq!(
            selected.per_auction_peak - previous.per_auction_peak,
            1_536 * 58_028
        );
        assert_eq!(
            selected.total_bytes - previous.total_bytes,
            2 * 1_536 * 58_028
        );
        assert_eq!(
            selected.auction_charges.ingress - previous.auction_charges.ingress,
            1_536 * 4_268
        );
        assert_eq!(
            selected.auction_charges.projection - previous.auction_charges.projection,
            1_536 * 47_104
        );
        assert_eq!(
            selected.call_side_term - previous.call_side_term,
            1_536 * 5_120
        );
        assert_eq!(
            selected.bookkeeping_term - previous.bookkeeping_term,
            1_536 * 1_536
        );
        assert_eq!(
            [
                selected.runtime_config_term,
                selected.auction_charges.privacy,
                selected.auction_charges.outbound,
                selected.debug_term,
                selected.assembly_scratch_bytes,
                selected.per_stream_peak,
                selected.decoder_chain_bytes,
                selected.egress_term,
                selected.aux_term,
                selected.gvl_terms,
                selected.pre_admission_parser_term,
                selected.terminal_term,
                selected.probe_output_term
            ],
            [
                previous.runtime_config_term,
                previous.auction_charges.privacy,
                previous.auction_charges.outbound,
                previous.debug_term,
                previous.assembly_scratch_bytes,
                previous.per_stream_peak,
                previous.decoder_chain_bytes,
                previous.egress_term,
                previous.aux_term,
                previous.gvl_terms,
                previous.pre_admission_parser_term,
                previous.terminal_term,
                previous.probe_output_term
            ],
        );
    }

    #[test]
    fn shipped_baked_budget_leaves_256_mib_headroom() {
        let row = super::limits();
        let derived = auctioneer_runtime::derive_isolate_budget_with_baked_config(
            &row,
            &super::target(),
            super::OUTBOUND_META_BYTES,
            super::core_construction_costs(),
            baked_config_bytes(),
            catalog_costs(MAX_BIDDERS),
        )
        .unwrap();
        assert_eq!(row.isolate_total_bytes, 4_u64 << 30_u32);
        assert_eq!(
            derived.total_bytes,
            2_536_791_800 + 2 * 128 * 1024 + 26_214_400
        );
        assert_eq!(
            derived.per_auction_peak,
            654_880_988 + 1_576_960 + 128 * 1024 + 0x0008_0000
        );
        assert_eq!(derived.bookkeeping_term, 3_276_800);
        assert_eq!(
            derived.margin_bytes,
            1_761_329_416 - 2 * (1_576_960 + 128 * 1024) - 26_214_400
        );
        assert_eq!(
            derived.total_bytes,
            expected_required_bytes() + 2 * baked_config_bytes() * 128
        );
        assert!(
            derived.total_bytes + (256_u64 << 20_u32) <= row.isolate_total_bytes,
            "accounted={} declared={}",
            derived.total_bytes,
            row.isolate_total_bytes,
        );
    }

    #[test]
    fn native_parser_and_snapshot_allowances_cover_sdk_defaults() {
        use edgezero_adapter_axum::config_store_limits::ConfigStoreLimits;
        use edgezero_adapter_axum::ingress_config::AxumIngressConfig;
        let ingress = AxumIngressConfig::from_env(&super::EnvConfig::from_vars([(
            "EDGEZERO__ADAPTER__INGRESS__MAX_HEADER_COUNT",
            "128",
        )]))
        .unwrap();
        let parser_bytes = ingress.parser_read_allocation_bytes()
            + ingress.parser_trailer_allocation_bytes()
            + 2 * ingress.parser_header_map_allocation_bytes()
            + 4 * ingress.max_raw_head_bytes();
        assert!(u64::from(super::PARSER_TRANSIENT_BYTES) >= u64::try_from(parser_bytes).unwrap());
        assert!(
            u64::from(super::H1_MAX_BUF_SIZE_BYTES)
                >= u64::try_from(ingress.max_raw_head_bytes()).unwrap()
        );
        assert!(
            u64::from(super::PLATFORM_CONCURRENT_CEILING)
                >= u64::try_from(ingress.max_connections()).unwrap()
        );
        assert!(
            super::limits().baseline_bytes
                >= 27_328_512
                    + u64::try_from(ConfigStoreLimits::default().max_startup_bytes()).unwrap()
        );
    }

    #[test]
    fn sdk_settings_are_limited_before_loading_and_listening() {
        use super::{EnvConfig, validate_sdk_settings};
        validate_sdk_settings(&EnvConfig::default()).unwrap();
        validate_sdk_settings(&EnvConfig::from_vars([
            ("EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS", "384"),
            ("EDGEZERO__ADAPTER__INGRESS__MAX_HEADER_COUNT", "128"),
        ]))
        .unwrap();
        for (key, value, field) in [
            (
                "EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS",
                "385",
                "max_connections",
            ),
            (
                "EDGEZERO__ADAPTER__INGRESS__MAX_RAW_HEAD_BYTES",
                "65537",
                "max_raw_head_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__INGRESS__MAX_HEADER_COUNT",
                "129",
                "max_header_count",
            ),
            (
                "EDGEZERO__ADAPTER__INGRESS__HEAD_READ_TIMEOUT_MS",
                "10001",
                "head_read_timeout_ms",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_ENTRIES",
                "1025",
                "max_entries",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_FILE_BYTES",
                "16777217",
                "max_file_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_KEY_BYTES",
                "1025",
                "max_key_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_VALUE_BYTES",
                "8388609",
                "max_value_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_RESIDENT_BYTES",
                "33554433",
                "max_resident_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_STARTUP_BYTES",
                "117440513",
                "max_startup_bytes",
            ),
        ] {
            let error = validate_sdk_settings(&EnvConfig::from_vars([(key, value)]))
                .unwrap_err()
                .to_string();
            assert!(error.contains(field), "{key}: {error}");
            assert!(!error.contains(value), "setting values are not diagnostics");
        }
    }

    /// The `u64` twin equals upstream's node cap.
    ///
    /// Asserted here rather than as a `const` assertion because a
    /// `const` initializer on the pinned toolchain can call neither
    /// `u64::from` nor `as`, and the workspace denies the cast.
    #[test]
    fn the_node_cap_twin_matches_upstream() {
        assert_eq!(
            super::CONFIG_JSON_NODES_U64,
            u64::from(super::DEFAULT_CONFIG_JSON_NODES),
        );
    }

    use super::{
        ASSEMBLY_FACTOR, FINAL_RESPONSE_BYTES, ISOLATE_TOTAL_BYTES, MAX_BIDDERS, MAX_CALLS,
        MAX_DECODED_RESPONSE_BYTES, MAX_IMPRESSIONS, OUTBOUND_META_BYTES, core_construction_costs,
        envelope, limits, target,
    };
    use auctioneer_runtime::budget::{
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, RESPONSE_PARSE_FACTOR,
        derive_aux_work_for_profile,
    };
    use auctioneer_runtime::{
        AuctionCharges, BudgetError, ResolvedIsolateLimits, derive_isolate_budget,
        validate_isolate_limits,
    };

    #[test]
    fn encoded_sync_query_fits_target_profile() {
        let target = target();
        assert!(target.max_request_target_bytes >= 33 * 1024);
        assert!(target.pseudoheader_allowance_bytes >= target.max_request_target_bytes + 192);
        assert!(target.h1_max_buf_size_bytes >= 0x4000 + target.max_request_target_bytes + 1_024);
        assert!(
            target.parser_header_ceiling_bytes
                >= 0x4000 + 132 * 32 + target.pseudoheader_allowance_bytes
        );
    }

    #[test]
    fn native_h1_profile_uses_the_sdk_raw_head_cap() {
        assert_eq!(target().h1_max_buf_size_bytes, 1_u32 << 16_u32);
    }

    fn retained_scope_delta() -> u64 {
        core_construction_costs()
            .retained_per_scope_bytes
            .checked_mul(u64::from(limits().max_bidders_per_auction))
            .expect("retained scope destinations fit u64")
    }

    // Independent complete equation, not a delta from the former 64-call row.
    // Literal measured factors deliberately make changes to runtime factors visible.
    fn expected_charges(row: &ResolvedIsolateLimits) -> AuctionCharges {
        const BODY: u64 = 32 << 10;
        const NODES: u64 = 2_048;
        const WIRE: u64 = 3 * BODY + 8 * 4_096 + 189 + 23 * NODES;
        const TREE: u64 = (NODES + 30) * 384;
        const REQUEST_COPY: u64 = 2 * WIRE + TREE;
        const SUPPLEMENT: u64 = 2 * BODY + NODES * 384;
        const BOOKKEEPING: u64 = 4 * (BODY + NODES * 384);
        // Fixed debug body/MIME/conversion/output allowances plus bounded call rows.
        const DEBUG_FIXED: u64 = 0x0001_0000
            + 0x0001_0000 * 4
            + 4_096 * 3 * 384
            + 64 * 256
            + 0x4000 * 3
            + 0x0001_0000 * 2
            + 0x0001_0000_u64.div_euclid(3) * 96
            + 128
            + 0x0001_0000 * 3
            + 8 * 384
            + (0x0001_0000 * 6 + 0x0001_0000) * 3
            + 64;
        const INGRESS: u64 = BODY
            + 0x4000
            + 128 * 64
            + 4_096 * 5
            + WIRE * 36
            + TREE
            + WIRE * 8
            + TREE
            + 2 * REQUEST_COPY
            + 2 * (REQUEST_COPY + SUPPLEMENT);
        const RESPONSE_PER_CALL: u64 =
            BODY + 12_288 + BODY * 256 + NODES * 320 + BODY * 128 + BODY * 3;
        let costs = core_construction_costs();
        let calls = u64::from(row.max_outbound_requests);
        let debug = calls
            .min(64)
            .checked_mul(64 * 384 + 64)
            .and_then(|bytes| bytes.checked_add(DEBUG_FIXED))
            .unwrap();
        AuctionCharges {
            config: (128_u64 << 10_u32) * 129 + (64_u64 << 10_u32) * 128 + 8_192 * 623,
            ingress: INGRESS,
            privacy: (14_u64 << 20_u32)
                .checked_add(costs.fixed_per_auction_bytes)
                .unwrap(),
            projection: costs
                .retained_per_scope_bytes
                .checked_add(WIRE * 128)
                .and_then(|bytes| bytes.checked_mul(16))
                .and_then(|bytes| bytes.checked_add(costs.reusable_scratch_bytes))
                .and_then(|bytes| bytes.checked_add(1_576_960))
                .and_then(|bytes| bytes.checked_add(0x0008_0000))
                .unwrap(),
            outbound: calls
                .checked_mul(BODY * 2 + 4_192 + 7_680)
                .and_then(|bytes| bytes.checked_add(512 * 1024))
                .unwrap(),
            response: calls
                .checked_mul(RESPONSE_PER_CALL)
                .and_then(|bytes| bytes.checked_add(BOOKKEEPING))
                .and_then(|bytes| bytes.checked_add(debug))
                .unwrap(),
        }
    }

    fn expected_total(row: &ResolvedIsolateLimits) -> u64 {
        let auction = u64::from(row.max_concurrent_auctions)
            .checked_mul(expected_charges(row).total().unwrap())
            .unwrap();
        let egress = row
            .final_response_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(4_096))
            .and_then(|bytes| bytes.checked_mul(u64::from(row.egress_permits)))
            .unwrap();
        let gvls = u64::from(row.max_concurrent_auctions)
            .checked_add(4 + 1)
            .and_then(|count| count.checked_mul(512_000))
            .unwrap();
        [
            27_328_512 + 0x0700_0000,
            auction,
            2 * (21_258_240 + (16 << 10) + (32 << 10) + 42_656),
            egress,
            2 * ((128 << 20) * 2 + 4_096),
            gvls,
            384 * (55_424 + 0x0010_0000 + 55_424 + 1_732 * 64),
            384 * (256 * 2 + 4_096),
            8_192 + 4 * (8_192 * 2 + 4_096),
            0x0100_0000 + 0x0080_0000,
        ]
        .into_iter()
        .try_fold(0_u64, u64::checked_add)
        .unwrap()
    }

    fn expected_required_bytes() -> u64 {
        expected_total(&limits())
    }

    #[test]
    fn axum_proof_uses_the_shipped_native_reservations() {
        let limits = limits();
        let target = target();
        assert_eq!(limits.baseline_bytes, 144_769_024);
        assert_eq!(target.parser_transient_bytes, 0x0010_0000);
        assert_eq!(target.h1_max_buf_size_bytes, 0x0001_0000);
        assert_eq!(target.platform_concurrent_ceiling, 384);
        assert_eq!(
            envelope(&limits, &target).outbound_meta_bytes,
            OUTBOUND_META_BYTES
        );
    }

    #[test]
    fn axum_proof_accepts_exact_demand_and_rejects_one_byte_less() {
        let mut exact = limits();
        exact.isolate_total_bytes = expected_required_bytes();
        let derived = validate_isolate_limits(
            &exact,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            catalog_costs(MAX_BIDDERS),
        )
        .expect("the independently worked demand fits exactly");
        assert_eq!(derived.total_bytes, expected_required_bytes());
        assert_eq!(derived.margin_bytes, 0);

        exact.isolate_total_bytes -= 1;
        assert_eq!(
            validate_isolate_limits(
                &exact,
                &target(),
                OUTBOUND_META_BYTES,
                core_construction_costs(),
                catalog_costs(MAX_BIDDERS)
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: expected_required_bytes(),
                ceiling: expected_required_bytes() - 1,
            }),
        );
    }

    /// Negative control independent of the fitting default: an operator's
    /// oversized admission population still reaches the ceiling warning.
    #[test]
    fn envelope_warns_over_ceiling_on_axum() {
        let mut limits = limits();
        limits.max_concurrent_auctions = 64;
        limits.egress_permits = 64;
        let target = target();
        let derived = derive_isolate_budget(
            &limits,
            &target,
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            catalog_costs(MAX_BIDDERS),
        )
        .expect("the custom oversized row derives every charge");
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &target,
                OUTBOUND_META_BYTES,
                core_construction_costs(),
                catalog_costs(MAX_BIDDERS)
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: expected_total(&limits),
                ceiling: ISOLATE_TOTAL_BYTES,
            }),
        );
        // The overshoot an operator reads in the boot warning.
        assert_eq!(
            derived.total_bytes.checked_sub(ISOLATE_TOTAL_BYTES),
            expected_total(&limits).checked_sub(ISOLATE_TOTAL_BYTES)
        );
        assert_eq!(derived.margin_bytes, 0);
    }

    /// Pin every population and the independent complete auction equation.
    #[test]
    fn the_selected_row_names_every_funded_term() {
        let roomy = limits();
        let derived = validate_isolate_limits(
            &roomy,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            catalog_costs(MAX_BIDDERS),
        )
        .expect("the selected row fits the local operator budget");

        assert_eq!(derived.total_bytes, expected_required_bytes());
        assert_eq!(
            derived.total_bytes,
            roomy.baseline_bytes
                + u64::from(roomy.max_concurrent_auctions) * derived.per_auction_peak
                + u64::from(roomy.max_active_body_streams) * derived.per_stream_peak
                + derived.egress_term
                + derived.aux_term
                + derived.gvl_terms
                + derived.pre_admission_parser_term
                + derived.terminal_term
                + derived.probe_output_term
                + 0x0100_0000
                + 0x0080_0000,
        );
        assert_eq!(
            derived.total_bytes,
            2_536_384_248 + 2 * 128 * 1024 + 26_214_400
        );
        assert_eq!(
            derived.margin_bytes,
            ISOLATE_TOTAL_BYTES - expected_required_bytes()
        );
        assert_eq!(derived.auction_charges, expected_charges(&roomy));
        assert_eq!(derived.runtime_config_term, 30_400_512);
        assert_eq!(
            derived.request_side_term,
            381_139_708 + 1_576_960 + 128 * 1024 + 0x0008_0000
        );
        assert_eq!(derived.call_side_term, 215_868_928);
        assert_eq!(derived.assembly_scratch_bytes, 1_572_864);
        assert_eq!(
            [
                derived.decoder_chain_bytes,
                derived.h2_slot_peak,
                derived.h1_slot_peak,
                derived.probe_output_term
            ],
            [21_258_240, 1_270_272, 90_112, 90_112],
        );

        assert_eq!(derived.bookkeeping_term, 3_276_800);
        assert_eq!(derived.debug_term, 9_310_368);
        assert_eq!(
            derived.per_auction_peak,
            654_677_212 + 1_576_960 + 128 * 1024 + 0x0008_0000
        );
        assert_eq!(
            u64::from(limits().max_outbound_requests)
                * MAX_DECODED_RESPONSE_BYTES
                * RESPONSE_PARSE_FACTOR,
            0x0800_0000,
            "the measured complete adapter response path remains funded",
        );
        assert_eq!(
            u64::from(limits().max_concurrent_auctions) * derived.per_auction_peak,
            1_309_354_424 + 2 * (1_576_960 + 128 * 1024 + 0x0008_0000),
        );
        // The response node charge is independent of the M4 byte charge.
        assert_eq!(
            u64::from(limits().max_outbound_requests)
                * u64::from(limits().max_parsed_nodes)
                * limits().response_node_peak_bytes,
            10_485_760,
        );

        // Egress holds one permit per admitted auction, each
        // holding two response buffers under this target's handoff
        // factor. The buffer is `FINAL_RESPONSE_BYTES`, which is
        // derived from `ASSEMBLY_FACTOR`; handoff stays independently funded.
        assert_eq!(derived.egress_term, 6_299_648);

        // Compare the auction population charge with the independent
        // decoder-wave population at the declared caps.
        assert_eq!(derived.per_stream_peak, 21_350_048);
        assert_eq!(
            u64::from(limits().max_active_body_streams) * derived.per_stream_peak,
            42_700_096,
        );

        assert_eq!(derived.aux_term, 536_879_104);
        assert_eq!(derived.gvl_terms, 3_584_000);
        assert_eq!(derived.pre_admission_parser_term, 487_784_448);
        // Cheap enough to be irrelevant, despite the largest permit
        // count on the row: a refusal body is 256 bytes.
        assert_eq!(derived.terminal_term, 1_769_472);
    }

    /// Independently pin the owning helper's charge for the selected eight imps.
    #[test]
    fn envelope_meta_charge_matches_the_row() {
        // `3_584 byte pool + offset table + 96 struct overhead + correlation`, where
        // the offset table is `2x4 + 36x8 + imps x 4` rounded up to 64:
        // `8 + 288 + 32 = 328 -> 384`; correlation adds `8 x 8 + 64 = 128`.
        let offset_table = (2 * 4 + 36 * 8 + u32::from(MAX_IMPRESSIONS) * 4).div_ceil(64) * 64;
        assert_eq!(MAX_IMPRESSIONS, 8);
        assert_eq!(offset_table, 384);
        let correlation = u32::from(MAX_IMPRESSIONS) * 8 + 64;
        assert_eq!(correlation, 128);
        assert_eq!(OUTBOUND_META_BYTES, 4_192);
        assert_eq!(3_584 + offset_table + 96 + correlation, OUTBOUND_META_BYTES);
    }

    /// A bad envelope must fail LOUDLY rather than boot half-sized.
    ///
    /// "Bad" now means INCONSISTENT, not merely ambitious: since
    /// `install_boot_envelope` warns on an over-ceiling total, the
    /// refusals that still end a boot are the ones tuning cannot fix.
    /// Both checks below are of that kind — a parser knob that would
    /// reject a request policy admits, and a response cap that cannot
    /// hold the scratch every auction of that size produces.
    #[test]
    fn a_bad_envelope_is_rejected_on_axum() {
        /// The baked host-config call default, which this envelope
        /// deliberately does not fund.
        const BAKED_CALL_DEFAULT: u32 = 100;

        // The h2 header-list knob must be exactly one above the admitted
        // ceiling: h2 rejects at `headers_size >= max`, so a knob equal
        // to the ceiling would reject a field section policy admits.
        let mut equal_knob = target();
        equal_knob.parser_header_knob_bytes = equal_knob.parser_header_ceiling_bytes;
        assert_eq!(
            validate_isolate_limits(
                &limits(),
                &equal_knob,
                OUTBOUND_META_BYTES,
                core_construction_costs(),
                catalog_costs(MAX_BIDDERS)
            ),
            Err(BudgetError::HeaderKnobNotCeilingPlusOne {
                knob: equal_knob.parser_header_ceiling_bytes,
                ceiling: equal_knob.parser_header_ceiling_bytes,
            }),
        );

        // The serialized-output sizing floor, which is why validation
        // clamps the baked 100-call input default down to MAX_CALLS.
        // The scratch is `calls x max_decoded x ASSEMBLY_FACTOR`, and at
        // the baked default that is more than the row's response cap
        // can hold — the cap is derived from the same product at
        // `MAX_CALLS`, so 100 calls exceeds it by the ratio of the two
        // call counts whatever the factor is.
        //
        // This refusal is checked BEFORE the ceiling inside
        // `validate_isolate_limits`, which is what makes it observable
        // on a row that is already over its ceiling: a truncating
        // response cap is an inconsistency on every auction, not an
        // ambition about load, so it must not be reachable behind a
        // warning.
        let scratch_at_100 =
            u64::from(BAKED_CALL_DEFAULT) * MAX_DECODED_RESPONSE_BYTES * ASSEMBLY_FACTOR;

        let mut baked_call_default = limits();
        baked_call_default.max_outbound_requests = BAKED_CALL_DEFAULT;
        assert_eq!(
            validate_isolate_limits(
                &baked_call_default,
                &target(),
                OUTBOUND_META_BYTES,
                core_construction_costs(),
                catalog_costs(MAX_BIDDERS)
            ),
            Err(BudgetError::FinalResponseBelowAssemblyFloor {
                configured: FINAL_RESPONSE_BYTES,
                required: scratch_at_100,
            }),
            "100 calls must trip the sizing floor that funds {MAX_CALLS}",
        );

        // Funding that scratch clears the sizing floor. Independently widen
        // admission too, so this remains an over-ceiling negative control.
        baked_call_default.final_response_bytes = scratch_at_100;
        baked_call_default.max_concurrent_auctions = 64;
        baked_call_default.egress_permits = 64;
        let expected_funded = expected_total(&baked_call_default);
        let funded = derive_isolate_budget(
            &baked_call_default,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            catalog_costs(MAX_BIDDERS),
        )
        .expect("funding the scratch clears the sizing floor");
        assert_eq!(funded.total_bytes, expected_funded);
        assert_eq!(
            validate_isolate_limits(
                &baked_call_default,
                &target(),
                OUTBOUND_META_BYTES,
                core_construction_costs(),
                catalog_costs(MAX_BIDDERS)
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: expected_funded,
                ceiling: ISOLATE_TOTAL_BYTES,
            }),
            "the sizing floor is cleared; what remains is the ceiling warning",
        );
    }

    #[test]
    fn configurable_response_charges_are_funded_independently() {
        let mut row = limits();
        row.isolate_total_bytes = u64::MAX;
        assert_eq!(row.response_parse_factor, MIN_RESPONSE_PARSE_FACTOR);
        assert_eq!(MIN_RESPONSE_PARSE_FACTOR, RESPONSE_PARSE_FACTOR);
        assert_eq!(MIN_RESPONSE_PARSE_FACTOR, 256);
        assert_eq!(row.response_node_peak_bytes, MIN_RESPONSE_NODE_PEAK_BYTES);
        assert_eq!(MIN_RESPONSE_NODE_PEAK_BYTES, 320);

        let at_floor = validate_isolate_limits(
            &row,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            catalog_costs(MAX_BIDDERS),
        )
        .expect("the response floors are funded");
        let calls = u64::from(row.max_outbound_requests);
        let mut raised_bytes = row;
        raised_bytes.response_parse_factor += 1;
        let mut raised_nodes = row;
        raised_nodes.response_node_peak_bytes += 1;

        for (raised, delta) in [
            (raised_bytes, calls * row.max_decoded_bytes),
            (raised_nodes, calls * u64::from(row.max_parsed_nodes)),
        ] {
            let funded = validate_isolate_limits(
                &raised,
                &target(),
                OUTBOUND_META_BYTES,
                core_construction_costs(),
                catalog_costs(MAX_BIDDERS),
            )
            .expect("a larger response charge remains computable");
            assert_eq!(funded.call_side_term - at_floor.call_side_term, delta);
            assert_eq!(
                funded.auction_charges.response - at_floor.auction_charges.response,
                delta
            );
            assert_eq!(funded.per_auction_peak - at_floor.per_auction_peak, delta);
            assert_eq!(
                funded.total_bytes - at_floor.total_bytes,
                u64::from(row.max_concurrent_auctions) * delta,
            );
        }
    }

    #[test]
    fn whole_auction_charges_are_prepaid_and_released() {
        use auctioneer_runtime::{AuctionBudget, ChargeKind, ResourceError, derive_isolate_budget};

        let derived = derive_isolate_budget(
            &limits(),
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            catalog_costs(MAX_BIDDERS),
        )
        .expect("the shipped row derives accounting");
        let charges = derived.auction_charges;
        assert_eq!(charges, expected_charges(&limits()));
        assert_eq!(charges.ingress, 15_844_452);
        assert_eq!(
            charges.total().expect("checked total"),
            derived.per_auction_peak
        );
        assert_eq!(
            charges.privacy,
            14 * (1_u64 << 20_u32) + core_construction_costs().fixed_per_auction_bytes
        );
        assert_eq!(
            charges.projection,
            365_291_520
                + retained_scope_delta()
                + core_construction_costs().reusable_scratch_bytes
                + 1_576_960
                + 0x0008_0000
        );

        let budget = AuctionBudget::new(derived.per_auction_peak);
        let receipts = charges.prepay(&budget).expect("all six phases are funded");
        assert_eq!(budget.in_use(), derived.per_auction_peak);
        drop(receipts);
        assert_eq!(budget.in_use(), 0);

        let too_small = AuctionBudget::new(derived.per_auction_peak - 1);
        assert_eq!(
            charges.prepay(&too_small).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Response
            },
        );
        assert_eq!(too_small.in_use(), 0, "refusal releases partial receipts");
    }

    /// The envelope handed to the boot sequence must be the one these
    /// tests validated — not a separately-built copy that could drift.
    #[test]
    fn envelope_carries_the_validated_row() {
        let limits = limits();
        let target = target();
        let envelope = envelope(&limits, &target);
        assert_eq!(envelope.outbound_meta_bytes, OUTBOUND_META_BYTES);
        assert_eq!(envelope.construction_costs, core_construction_costs());
        assert_eq!(*envelope.limits, limits);
        assert_eq!(*envelope.target, target);
    }

    #[test]
    fn construction_costs_use_the_core_layout_once_in_disjoint_prepaid_phases() {
        use auctioneer_exchange::construction_costs as exchange_construction_costs;
        use auctioneer_runtime::{RequestConstructionCosts, derive_isolate_budget};

        let row = limits();
        let platform = target();
        let boot = envelope(&row, &platform);
        let costs = core_construction_costs();
        assert_eq!(boot.construction_costs, costs);
        assert!(costs.fixed_per_auction_bytes > 0);
        assert!(costs.reusable_scratch_bytes > 0);
        assert_eq!(costs.retained_per_scope_bytes, 2 * 39);
        let exchange = exchange_construction_costs();
        assert!(costs.fixed_per_auction_bytes > exchange.fixed_per_auction_bytes);
        assert!(costs.reusable_scratch_bytes >= exchange.reusable_scratch_bytes);
        assert_eq!(
            costs.retained_per_scope_bytes,
            exchange.retained_per_scope_bytes
        );
        let base = derive_isolate_budget(
            &row,
            &platform,
            boot.outbound_meta_bytes,
            RequestConstructionCosts::default(),
            catalog_costs(MAX_BIDDERS),
        )
        .unwrap();
        let funded = derive_isolate_budget(
            &row,
            &platform,
            boot.outbound_meta_bytes,
            boot.construction_costs,
            catalog_costs(MAX_BIDDERS),
        )
        .unwrap();
        assert_eq!(funded.construction_costs, costs);
        let mut costs_without_shape = costs;
        costs_without_shape.fixed_per_auction_bytes -= 32;
        let without_shape_marker = derive_isolate_budget(
            &row,
            &platform,
            boot.outbound_meta_bytes,
            costs_without_shape,
            catalog_costs(MAX_BIDDERS),
        )
        .unwrap();
        assert_eq!(
            funded.auction_charges.privacy - without_shape_marker.auction_charges.privacy,
            32
        );
        assert_eq!(
            funded.per_auction_peak - without_shape_marker.per_auction_peak,
            32
        );
        assert_eq!(funded.total_bytes - without_shape_marker.total_bytes, 64);
        assert_eq!(
            funded.auction_charges.privacy - base.auction_charges.privacy,
            costs.fixed_per_auction_bytes
        );
        assert_eq!(
            funded.auction_charges.projection - base.auction_charges.projection,
            costs
                .reusable_scratch_bytes
                .checked_add(retained_scope_delta())
                .expect("projection construction charges fit u64")
        );
        let added = costs
            .fixed_per_auction_bytes
            .checked_add(costs.reusable_scratch_bytes)
            .and_then(|bytes| bytes.checked_add(retained_scope_delta()))
            .expect("construction layout charge fits u64");
        assert_eq!(funded.per_auction_peak - base.per_auction_peak, added);
        assert_eq!(
            funded.total_bytes - base.total_bytes,
            added * u64::from(row.max_concurrent_auctions)
        );
        assert_eq!(
            funded.auction_charges.total().unwrap(),
            funded.per_auction_peak
        );
    }
}
