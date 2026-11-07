//! Fastly's boot envelope: the per-execution memory budget this target
//! attempts to install before it serves a route.
//!
//! This adapter used to call the envelope-less `bootstrap_adapter`, so
//! `boot_limits()` answered `None` and every envelope-derived byte cap
//! fell back to a host default. It now calls
//! `bootstrap_adapter_with_envelope` with the row below.
//!
//! # Ceiling policy
//!
//! Numbers below describe the pure runtime row. Core boot additionally funds
//! `serialized_baked_config_bytes * CONFIG_PARSE_FACTOR` per admitted auction;
//! the boot warning reports that exact compiled-configuration total.
//!
//! The declared caps exceed this target's ceiling. The boot validator
//! reports `BudgetOverCeiling`; installation warns and proceeds. Tests
//! below pin the total and its component charges without reducing caps
//! to make the row fit.
//!
//! Auction populations are charged conservatively at their full peaks.
//! Sequential request-construction work is not evidence that every
//! concurrent auction reaches that transient peak simultaneously.
//!
//! # Where the ceiling comes from
//!
//! [`edgezero_adapter_fastly::FASTLY_PLATFORM`], read through its const
//! accessors rather than copied as a literal. It publishes
//! `128_000_000` bytes with
//! `PlatformResourceSource::PlatformLimit { provider: "Fastly Compute" }`
//! and `MemoryCeilingScope::PerExecution`.
//!
//! # The separately published stack
//!
//! Fastly is the one target of the three that also publishes a stack
//! allowance — `stack_bytes() == Some(1_000_000)` — and `MemoryCeiling`
//! documents it as NOT included in `total_bytes`. So it is deliberately
//! not added to [`ISOLATE_TOTAL_BYTES`]: the budget equation charges
//! heap populations, the stack is a separate allowance for a different
//! kind of memory, and folding it in would hand the heap a million
//! bytes the platform never granted it.
//! `the_ceiling_is_read_from_the_adapter_constant` pins both the
//! presence of that stack and its exclusion from the ceiling this row
//! is proven against.
//!
//! # What `PerExecution` means here, and what the validator cannot say
//!
//! `PerExecution` means each request execution receives an INDEPENDENT
//! budget — Fastly instantiates a fresh isolate per request, as
//! [`crate::run`] already notes. Concurrency therefore does not divide
//! this ceiling and does not multiply against it: the row below
//! describes what ONE execution may hold, and a second concurrent
//! request is funded by the platform out of its own `128_000_000` rather
//! than out of this one.
//!
//! [`PLATFORM_CONCURRENT_CEILING`] follows the published per-execution
//! scope, independently of whether the declared caps fit the ceiling.
//!
//! [`auctioneer_runtime::validate_isolate_limits`] has no field for the
//! scope. It takes one `isolate_total_bytes` and charges every
//! population against it, which is the correct reading for a
//! `PerInstance` target and only becomes correct for a `PerExecution`
//! one because this row sets its populations to per-execution values by
//! hand. Nothing in the validator checks that, and nothing in it would
//! notice a `PerInstance` row installed on a `PerExecution` target. The
//! distinction is honoured here by how the populations are chosen and
//! stated; the inability to express it is a property of the validator.
//!
//! # Where each value comes from
//!
//! Four provenances, and they are not interchangeable:
//!
//! - **Published.** [`ISOLATE_TOTAL_BYTES`] alone, read from the
//!   adapter constant above.
//! - **Derived.** auctioneer's own knobs, plus
//!   [`PLATFORM_CONCURRENT_CEILING`] from the published scope. The
//!   arithmetic is written beside each one.
//! - **Substituted.** Values carried over from
//!   `auctioneer-adapter-axum`'s measured row because they cannot be
//!   measured on this target. Each one says so at its definition. A
//!   substitute is NOT a measurement and is not presented as one.
//! - **Unmeasurable in principle.** [`PARSER_TRANSIENT_BYTES`] and
//!   [`TRANSMIT_STATE_BYTES`], which are host-side bytes no guest
//!   allocator can observe.

use edgezero_adapter_fastly::FASTLY_PLATFORM;
use edgezero_core::PlatformFact;
use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;

/// `DEFAULT_CONFIG_JSON_NODES` in the width the budget charges it at.
///
/// A twin rather than a cast: `u64::from` is not callable in a `const`
/// initializer on the pinned toolchain, and the workspace denies `as`.
/// The assertion below is what keeps the two from drifting.
const CONFIG_JSON_NODES_U64: u64 = 8_192;
use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
use auctioneer_core::host_primitives::construction_diagnostic_costs;
use auctioneer_core::host_primitives::{BootEnvelope, ResourceProfile, StderrBootLog};
use auctioneer_runtime::budget::{
    ASSEMBLY_FACTOR, H2_MIN_FIELD_ENTRY_BYTES, min_aux_route_peak_bytes,
};
use auctioneer_runtime::{
    MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES,
    MIN_RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits, TargetPlatformConstants,
    min_terminal_response_permits,
};

/// This platform's published memory ceiling, read from the adapter.
///
/// PUBLISHED, not chosen here: `FASTLY_PLATFORM` carries it with its
/// scope, its separately published stack and its provenance, so a
/// ceiling moved upstream moves this row rather than leaving a stale
/// literal behind.
///
/// This is `total_bytes()` alone. The `1_000_000`-byte stack the same
/// constant publishes is NOT added: `MemoryCeiling` documents it as
/// sitting outside `total_bytes`, and the budget equation below charges
/// heap, not stack.
///
/// The `panic!` cannot fire: the constant is built with
/// `PlatformFact::known(MemoryCeiling::new(..), ..)` in the adapter,
/// and both accessors on the path are `const fn`, so the whole
/// expression is evaluated at compile time and an `Unknown` upstream
/// would be a build failure rather than a boot panic.
const ISOLATE_TOTAL_BYTES: u64 = match FASTLY_PLATFORM.memory_ceiling() {
    PlatformFact::Known { value: ceiling, .. } => ceiling.total_bytes(),
    PlatformFact::Unknown { .. } | _ => panic!("FASTLY_PLATFORM publishes a memory ceiling"),
};

/// The shipped body / transport / decoded cap.
const CAP_256_KIB: u64 = 1 << 18;

/// The response cap follows the serialized-output floor without lowering
/// privacy enforcement's existing 16 MiB cap. Typed assembly has its own auction charge.
const FINAL_RESPONSE_BYTES: u64 = {
    let floor = MAX_CALLS_U64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    let existing_cap = 1_u64 << 24_u32;
    if floor > existing_cap {
        floor
    } else {
        existing_cap
    }
};

/// [`MAX_CALLS`] as a `u64`, for the const arithmetic above.
/// `u64::from` is not callable in a const initializer, and an `as`
/// cast is refused by this crate's lint set, so the width is declared
/// and the two are pinned equal below.
const MAX_CALLS_U64: u64 = 32;
const _: () = assert!(
    MAX_CALLS == 32 && MAX_CALLS_U64 == 32,
    "the u64 copy of the call count must equal MAX_CALLS",
);

/// The runtime-config envelope cap.
const BYTES_1_MIB: u64 = 1 << 20;

/// The syntactic node budget for the inbound parse.
///
/// `NODE_RETAINED_BYTES` now charges 384 bytes/node, not 64. The native
/// object forest retained `8_106_496 / 25_729 = ~315.073` bytes/node
/// (array forest: ~127.040); 384 covers the binding result plus 20%.
/// The retained-node term is `65_536 x 384 = 25_165_824` bytes per
/// auction, formerly `65_536 x 64 = 4_194_304`. With caps unchanged,
/// the per-auction delta is `(384 - 64) x max_parsed_nodes = 20_971_520`;
/// the isolate delta multiplies by `max_concurrent_auctions`:
/// `20_971_520 x 1 = 20_971_520` bytes. The charge bounds retained
/// `Value` trees, not all parser or transformation scratch.
const NODE_BUDGET: u32 = 1 << 16;

/// The whole inbound header block admitted at the ingress seam.
const ADMITTED_HEADER_BYTES: u32 = 1 << 14;

/// Inbound header entries admitted at the ingress seam.
const ADMITTED_HEADER_COUNT: u16 = 128;

/// The same count in the width the h1 header-count knob speaks.
/// `u32::from` is not callable in a `const fn` on stable, so the twin is
/// written once and pinned to its `u16` original below.
const ADMITTED_HEADER_COUNT_U32: u32 = 128;

const _: () = assert!(
    ADMITTED_HEADER_COUNT == 128 && ADMITTED_HEADER_COUNT_U32 == 128,
    "the admitted header count drifted from its u32 twin"
);

/// Bidders one auction may fan out to.
const MAX_BIDDERS: u32 = 50;

/// Outbound calls one auction may make: a product capacity, not a
/// number chosen to fit the ceiling. The final-response cap must fund
/// the assembly scratch for this many calls; the boot validator checks
/// that sizing floor separately from its ceiling warning.
const MAX_CALLS: u32 = 32;

/// Impressions one auction may carry. Sets the per-call metadata
/// charge, hence [`OUTBOUND_META_BYTES`].
const MAX_IMPRESSIONS: u16 = 100;

/// Concurrent outbound bidder legs one auction may have in flight.
///
/// **RAISED from 4 to 32, and the name is the reason it looked wrong.**
/// This field sizes [`auctioneer_runtime::StreamGate`], and despite the
/// "body streams" spelling the gate does not count inbound request
/// bodies. `auctioneer-core`'s `drive_fanout` is its only acquirer: it
/// batches ONE auction's outbound calls through
/// `StreamGate::acquire_wave(wave_width(gate.capacity(), remaining))`,
/// so the capacity is the width of a single auction's fanout wave —
/// how many bidder calls are decoded at once — not a count of requests.
/// `per_stream_bytes` charges the matching shape: a transport buffer, a
/// decoded buffer and one decoder state, which is a bidder RESPONSE
/// being read, not a client request being received.
///
/// # What the scope permits
///
/// `PerExecution` bounds REQUESTS, and this population is not a
/// request population. One execution running one auction still issues
/// [`MAX_CALLS`] outbound calls, and four of them decoding
/// concurrently all belong to that single admitted request. Nothing
/// about a private per-execution budget makes a second concurrent
/// bidder leg impossible; the four are funded out of the same
/// execution that owns the auction.
///
/// # What auctioneer's gates permit
///
/// `StreamGate` is a QUEUED weighted gate, not a shedding one:
/// `acquire_wave` makes a wave wait its turn in arrival order rather
/// than refusing it. A capacity below [`MAX_CALLS`] therefore rejects
/// nothing — it splits one auction's fanout into serial waves inside
/// the same auction deadline. At 4 a 32-call auction ran eight waves
/// back to back, and a Prebid auction's whole timeout is typically
/// 100-300 ms, so bidders in the later waves would routinely time out
/// having never been called. That is a fill-rate loss, not a memory
/// saving.
///
/// # Why 32
///
/// It is [`MAX_CALLS`]: one wave, every bidder called in parallel, no
/// bidder penalised for its position in the list. That is what a
/// production exchange wants from a fanout, and it is the shape the
/// auction deadline is written against — a per-bidder timeout means
/// nothing if a bidder spends most of the auction queued behind
/// others. Anything less is a deliberate latency-for-memory trade,
/// and this row now states the memory instead of hiding the latency.
///
/// # The cost, stated plainly
///
/// The per-stream peak is `21_825_184` bytes, nearly all of it one
/// brotli decoder state at this row's 22-bit window, so 32 of them
/// charge `698_405_888` — more than five times this platform's entire
/// ceiling. That is the
/// honest price of decoding a full fanout wave concurrently under a
/// worst-case model in which every leg simultaneously holds a maximum
/// decoder. Pooling decoder state across a wave, or a smaller
/// `max_brotli_wbits`, are the two changes that would move it; cutting
/// the population back would only move the cost onto the deadline.
///
/// # What a reader should re-check
///
/// If `drive_fanout` ever stops being the sole acquirer — in
/// particular if an inbound body path starts taking stream weight —
/// this becomes a mixed population and has to be re-derived.
///
/// The live handler uses `EdgeZeroFanout::with_wave_scheduler` with
/// this installed gate. Explicit test fanout handles bypass transport;
/// the production `HttpClient` path cannot use an envelope-less fallback.
const MAX_ACTIVE_BODY_STREAMS: u32 = 32;

/// Per-call outbound metadata reservation, as
/// `auctioneer_bidder::outbound_meta_bytes(MAX_IMPRESSIONS)` returns it.
///
/// Written as a literal because this crate does not depend on
/// `auctioneer-bidder`. `envelope_meta_charge_matches_the_row` below pins
/// this copy against the arithmetic the bidder crate performs.
const OUTBOUND_META_BYTES: u32 = 5_248;

/// Concurrently-live requests this platform admits per execution before
/// any auctioneer gate.
///
/// **DERIVED from the published scope**, and the one field where this
/// row differs structurally from Cloudflare's rather than just
/// numerically.
///
/// # What the platform documents
///
/// Fastly's "Compute resource limits" page publishes the ceiling as
/// *"Maximum memory consumption | 1M bytes stack, 128MB heap | per
/// execution"*, and defines an execution as *"a single instance of a
/// Compute program being executed, normally in response to a client
/// HTTP request"*. That is the strongest of the three statements: the
/// unit of accounting is named, and the instance-per-request lifecycle
/// is named in the same definition. `FASTLY_PLATFORM` carries it as
/// `MemoryCeilingScope::PerExecution`, and this adapter's own boot path
/// notes the same fresh-isolate-per-request behaviour. Upstream's
/// capability guide reads the same page the same way: Fastly "gives
/// each execution its own heap and stack allowances", against
/// Cloudflare's limit being "shared by concurrent requests in one
/// isolate".
///
/// # What this concludes
///
/// One execution therefore holds exactly one pre-admission request, so
/// the parser population is 1 and the term is one slot peak:
/// `1 x 238_080` bytes. Concurrency does not divide this ceiling and
/// does not multiply against it — a second concurrent request is funded
/// by the platform out of its own `128_000_000`.
///
/// This is not a saving chosen to help the arithmetic. The row is
/// over its ceiling at the declared caps.
///
/// # No longer derived here
///
/// `FASTLY_PLATFORM` now PUBLISHES this population as
/// `InboundRequestPopulationBound::new(NonZeroU32::MIN)` with
/// `PlatformResourceSource::PlatformLimit { provider: "Fastly Compute" }`,
/// so the value is read from the adapter rather than re-derived from
/// the scope. The reading above is preserved because it is why the
/// published figure is the right one to charge, but the number itself
/// is no longer auctioneer's to choose: a population moved upstream moves
/// this row.
///
/// # What a reader should re-check
///
/// If Fastly ever serves more than one client request from a single
/// Compute instance, the published definition of an execution has
/// changed and upstream's published population changes with it.
const PLATFORM_CONCURRENT_CEILING: u32 = match FASTLY_PLATFORM.inbound_request_population_bound() {
    PlatformFact::Known { value: bound, .. } => bound.max_live_requests().get(),
    PlatformFact::Unknown { .. } | _ => {
        panic!("FASTLY_PLATFORM publishes an inbound request population bound")
    }
};

/// Whole-isolate resident memory after boot.
///
/// **A SUBSTITUTE, NOT A MEASUREMENT.** This is
/// `auctioneer-adapter-axum`'s value, measured on native arm64, standing in
/// for a Fastly figure that cannot be obtained here.
///
/// What blocks the real measurement: the contract wants the max of N
/// boot-plus-one-no-op-request runs plus a 20% margin, and a local
/// `fastly compute serve` cannot supply it — the guest aborts during
/// boot on `config store could not be found`, so only a partial-boot
/// reading is obtainable. Viceroy does report a real guest-heap number,
/// which is the right QUANTITY, so this field becomes genuinely
/// measurable on this target as soon as a local run can complete boot.
/// Of the three wasm targets this is the one whose baseline is closest
/// to being measurable.
///
/// It is carried rather than omitted because the field has no
/// "unknown" representation — it is a `u64` the equation adds
/// unconditionally.
const BASELINE_BYTES: u64 = 27_328_512;

/// Per-response transmit state.
///
/// **UNMEASURABLE ON THIS TARGET, and axum's figure is carried in its
/// place.** These are the HOST's write-path bookkeeping bytes. They do
/// not enter the guest's linear memory, so no guest-side allocator can
/// observe them and no amount of local running would produce a figure.
/// Axum's `4_096` is carried so the term is not silently zero; it is a
/// different platform's number and is not presented as this one's.
const TRANSMIT_STATE_BYTES: u32 = 4_096;

/// Hpack dynamic table plus the platform parser's decode scratch, per
/// connection slot.
///
/// **UNMEASURABLE ON THIS TARGET, and axum's figure is carried in its
/// place.** No h2 or hpack code exists in this guest — the host hands
/// it an already-parsed request — so these bytes are never in guest
/// linear memory. Axum's `16_384 = 4_096 hpack table + 12_288 decode
/// scratch` is carried rather than zeroed, because the cost is real on
/// the host side even though the guest cannot see it.
const PARSER_TRANSIENT_BYTES: u32 = 1 << 14;

/// The largest field section admitted, with pseudoheaders included.
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

/// The h1 head-buffer knob.
///
/// Must hold an admitted header block plus its request target and head
/// slack: `16_384 + 33_792 + 1_024 = 51_200 <= 65_536`, which validation
/// asserts.
const H1_MAX_BUF_SIZE_BYTES: u32 = 0x1_0000;

/// Auctions this execution admits at once.
///
/// **STAYS 1, and unlike the row this was inherited from, that is now
/// a derivation rather than a fit.** This platform publishes its
/// ceiling with `MemoryCeilingScope::PerExecution`: one execution gets
/// its own budget and serves exactly one client request out of it. A
/// request is either an auction, a probe or an aux route, so at most
/// ONE auction is ever live inside this budget and a capacity above 1
/// would reserve memory for auctions that cannot exist here.
///
/// This is the one cap on any row that is a production-plausible
/// figure and a fit-driven figure at once, and the two agree by
/// accident of scope rather than by argument. Note what it does NOT
/// mean: the platform runs many executions concurrently, so this
/// deployment serves many simultaneous auctions — each simply carries
/// its own copy of this envelope. Concurrency lives above this row,
/// not inside it.
///
/// # What a reader should re-check
///
/// The scope. `the_ceiling_is_read_from_the_adapter_constant` pins
/// `PerExecution`, so a silent flip to `PerInstance` upstream fails
/// that test rather than leaving this derivation quietly false — and
/// if it ever does flip, this becomes an instance-wide population and
/// must be re-derived the way the Cloudflare row derives its own.
const MAX_CONCURRENT_AUCTIONS: u32 = 1;

/// Auction responses in flight on the egress path, within one
/// execution.
///
/// One per admitted auction, which on this scope is one. The permit is
/// held while the finished auction's response is serialized and
/// written; with a single auction per execution there is never a
/// second response to write concurrently.
const EGRESS_PERMITS: u32 = MAX_CONCURRENT_AUCTIONS;

/// Aux-route gate capacity within one execution.
///
/// **CORRECTED from axum's 4 to 1, on the published scope.** An aux
/// permit is a per-REQUEST lease: `auctioneer-core`'s ingress policy takes
/// exactly one in `admit_aux` and moves it into the response
/// completion, where it is held until terminal egress. So the number
/// of permits that can be live at once is the number of aux-class
/// requests that can be in flight at once, and nothing else.
///
/// # What the scope permits
///
/// `PerExecution` means one execution serves one client request. That
/// request is either an auction, a probe or an aux route — it cannot
/// be four aux routes — so at most ONE aux permit exists inside this
/// budget. Charging four would reserve three additional aux peaks for
/// leases that cannot be taken.
///
/// # What auctioneer's gates permit
///
/// The three manifest routes in the aux class (`/info/bidders`,
/// `/info/bidders/{name}`, `/_auctioneer/health/gvl`) plus the 404/405
/// fallback arms are a CATALOGUE of routes that draw on this class,
/// not a count of concurrent holders. Four distinct routes sharing one
/// class does not make four permits simultaneously live; one request
/// holds one permit whichever of them it hit. `/status` is the `probe`
/// class and deliberately takes no aux permit at all.
///
/// # What a reader should re-check
///
/// If Fastly ever serves more than one client request from a single
/// Compute instance, this rises with that concurrency — it tracks the
/// same published fact as [`PLATFORM_CONCURRENT_CEILING`].
///
/// Keep user sync's 768 MiB reservation for checked user-sync/privacy work,
/// not just privacy enforcement's config-extraction and manifest floor. Boot validates
/// the full aux-work peak, including the serialized baked configuration.
/// This is an upper-bound reservation, not a measured resident heap peak.
const AUX_ROUTE_PERMITS: u32 = 1;
const AUX_ROUTE_PEAK_BYTES: u64 = 768 * 1_024 * 1_024;

/// Pre-deserialization cap on the raw runtime-config envelope, and the
/// largest single blob inside it.
///
/// Named rather than written inline at the row, because
/// [`AUX_ROUTE_PEAK_BYTES`] is checked against both: the aux floor and the
/// row the validator checks it against must read the SAME two figures,
/// or the check compares a derivation to a different row than the one
/// that shipped.
const MAX_RUNTIME_CONFIG_BYTES: u64 = BYTES_1_MIB;
const MAX_RUNTIME_CONFIG_BLOB_BYTES: u64 = BYTES_1_MIB;

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

/// Static-class response permits within one execution.
///
/// **DERIVED, not chosen: `1 + 1 = 2`.** The value was 68, then 2 as a
/// margin over the 1 this target can hold; it is now the relation
/// itself.
///
/// # Where the earlier numbers came from
///
/// 68 was `64 + 4`, from a design-time rule
/// `terminal_response_permits >= max_concurrent_auctions +
/// aux_route_permits` whose first operand was filled in with
/// `max_outbound_requests` — an auction's outbound CALL count —
/// instead of `max_concurrent_auctions`. Cutting it fixed that operand
/// but dropped the rule itself, on the premise that no admitted request
/// holds a permit. That premise was true when the cut landed and false
/// once admission began reserving one, so the rule is now enforced by
/// the compiler rather than restated here.
///
/// # What auctioneer's gates actually permit
///
/// Three per-request populations hold a permit to terminal egress, and
/// they can be live at once: every ADMITTED auction, because
/// `ingress::admit_auction` reserves one at admission and holds it
/// through the handler; every request rendering a refusal, via
/// `ingress::refuse`; and every pre-admission head rejection, via
/// `host_primitives::static_egress::detached_egress_decision`.
///
/// [`min_terminal_response_permits`] owns that derivation and this
/// constant is its result, so the relation cannot go stale in a comment
/// again.
///
/// # What the scope permits
///
/// `PerExecution` admits one request per execution, so the platform
/// term is **1**. The admission-bounded term is what binds here:
/// [`MAX_CONCURRENT_AUCTIONS`] is 1 and [`AUX_ROUTE_PERMITS`] is 1, so
/// two are required. Two is also what this row already charged, but for
/// a different reason — it was a spare held for the pre-admission
/// rejection path, which an admitted auction's own permit now consumes.
/// The number is unchanged and its basis is not, so the value is now
/// derived rather than reasoned about in prose.
///
/// It is deliberately not cut to 1: exhausting this class turns a
/// refusal into a connection abort, and that is the failure the class
/// exists to prevent. At `256 x 2 + 4_096` bytes a permit this term is
/// `9_216` bytes, the cheapest uncertainty in this row to over-fund.
///
/// No `const _` restates the relation here: this constant IS the
/// derivation's result, so an assertion comparing the two would hold
/// for every possible operand — protection in appearance only. The
/// derivation is the enforcement on this row. Axum declares its value
/// independently, as [`PLATFORM_CONCURRENT_CEILING`], so the assertion
/// it keeps can fail.
const TERMINAL_RESPONSE_PERMITS: u32 = min_terminal_response_permits(
    MAX_CONCURRENT_AUCTIONS,
    AUX_ROUTE_PERMITS,
    PLATFORM_CONCURRENT_CEILING,
);
const REFUSAL_BODY_BYTES: u32 = 256;

/// This target's resolved per-execution limits.
#[must_use]
pub const fn limits() -> ResolvedIsolateLimits {
    ResolvedIsolateLimits {
        isolate_total_bytes: ISOLATE_TOTAL_BYTES,
        baseline_bytes: BASELINE_BYTES,
        inbound_request_bytes: CAP_256_KIB,
        // The measured floor. These targets have no traffic profile of
        // their own yet, so they take the minimum validation allows; a
        // deployment facing denser publisher payloads may raise it.
        inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
        max_outbound_request_bytes: CAP_256_KIB,
        max_outbound_raw_bytes: CAP_256_KIB,
        max_transport_bytes: CAP_256_KIB,
        max_decoded_bytes: CAP_256_KIB,
        // Keep privacy enforcement's 256-byte parse floor and fund the independent node peak.
        // Both charges are configurable upward, never below their floors.
        response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
        response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
        // The cap covers the serialized-output floor and retains privacy enforcement's cap.
        final_response_bytes: FINAL_RESPONSE_BYTES,
        egress_permits: EGRESS_PERMITS,
        response_write_deadline_ms: 30_000,
        // auctioneer decodes through its own chain here, so there is no
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
        // The `http::HeaderMap` per-entry cost, carried over from
        // axum's measured row rather than re-measured here.
        header_entry_overhead_bytes: 64,
        parser_header_ceiling_bytes: PARSER_HEADER_CEILING_BYTES,
        // h2 rejects at `headers_size >= max`, so the configured knob
        // sits exactly one above the admitted ceiling.
        parser_header_knob_bytes: PARSER_HEADER_CEILING_BYTES + 1,
        max_request_target_bytes: MAX_REQUEST_TARGET_BYTES,
        h1_max_buf_size_bytes: H1_MAX_BUF_SIZE_BYTES,
        h1_max_headers: ADMITTED_HEADER_COUNT_U32,
        // Derived, not configured: the byte ceiling over the minimum
        // encoded entry size, `55_424 / 32 = 1_732`. Validation
        // recomputes this and refuses a mismatch.
        parser_max_header_entries: PARSER_HEADER_CEILING_BYTES.div_euclid(H2_MIN_FIELD_ENTRY_BYTES),
        pseudoheader_allowance_bytes: PSEUDOHEADER_ALLOWANCE_BYTES,
        parser_transient_bytes: PARSER_TRANSIENT_BYTES,
        // Two buffers per handoff: there is no verified owned zero-copy
        // handoff on this target, so both are charged. Charging one
        // would be an unproven claim, not a saving.
        egress_handoff_factor: 2,
    }
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
        catalog_costs: construction_diagnostic_costs(limits.max_bidders_per_auction),
        // This platform surfaces the guest's stderr, so the shared
        // stderr sink is the real target channel. It is installed
        // BEFORE the envelope is validated, which is what makes the
        // over-ceiling warning reach an operator: the pinned EdgeZero
        // installs its own logger inside `run_app`, after boot returns.
        log_sink: &StderrBootLog,
    }
}

/// The platform passed by this adapter's actual dispatch path.
pub const DISPATCH_PLATFORM: edgezero_core::PlatformMetadata = FASTLY_PLATFORM;

/// Source declarations consumed by both bootstrap and the resource inspector.
#[must_use]
pub fn resource_profile() -> ResourceProfile {
    let limits = limits();
    let target = target();
    let boot = envelope(&limits, &target);
    ResourceProfile {
        name: "fastly",
        dispatch_name: "fastly",
        target_abi: "wasm32-wasip1",
        headroom_class: auctioneer_runtime::HeadroomClass::Wasm,
        memory_scope: edgezero_core::MemoryCeilingScope::PerExecution,
        platform: FASTLY_PLATFORM,
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
    use super::construction_diagnostic_costs;

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
        assert_eq!(profile.name, "fastly");
        assert_eq!(profile.target_abi, "wasm32-wasip1");
        assert_eq!(
            profile.headroom_class,
            auctioneer_runtime::HeadroomClass::Wasm
        );
        assert_eq!(
            profile.memory_scope,
            edgezero_core::MemoryCeilingScope::PerExecution
        );
        assert_eq!(profile.dispatch_platform, super::DISPATCH_PLATFORM);
        assert!(profile.dispatch_matches_platform());
        assert_eq!(profile.outbound_meta_bytes, boot.outbound_meta_bytes);
        assert_eq!(profile.construction_costs, boot.construction_costs);
        assert_eq!(profile.catalog_costs, boot.catalog_costs);
        assert_eq!(profile.catalog_costs.context_per_auction_bytes, 4_919_296);
        let baked_bytes = auctioneer_runtime::serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        // Neutral host (764) + account (680) + tuple delimiters (3).
        assert_eq!(baked_bytes, 1_447);
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
        CAP_256_KIB, ISOLATE_TOTAL_BYTES, MAX_BIDDERS, MAX_IMPRESSIONS, OUTBOUND_META_BYTES,
        PLATFORM_CONCURRENT_CEILING, core_construction_costs, envelope, limits, target,
    };
    use auctioneer_runtime::budget::{
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, RESPONSE_PARSE_FACTOR,
        TYPED_CLONE_FACTOR,
    };
    use auctioneer_runtime::{BudgetError, derive_isolate_budget, validate_isolate_limits};
    use edgezero_adapter_fastly::FASTLY_PLATFORM;
    use edgezero_core::{MemoryCeilingScope, PlatformResourceSource, PlatformUnknownReason};

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

    fn normalization_wire_delta() -> u64 {
        let row = limits();
        // Stage-2 user sync funded raw B. privacy enforcement adds domains, escaped headers, syntax, and 23N.
        let enrichment = row
            .inbound_request_bytes
            .checked_mul(2)
            .and_then(|bytes| {
                let headers = u64::from(row.max_copied_header_bytes).checked_mul(8)?;
                bytes.checked_add(headers)
            })
            .and_then(|bytes| bytes.checked_add(189))
            .expect("normalization enrichment fits u64");
        23_u64
            .checked_mul(u64::from(row.max_parsed_nodes))
            .and_then(|bytes| bytes.checked_add(enrichment))
            .expect("numeric and enrichment wire growth fits u64")
    }

    fn normalization_ingress_delta() -> u64 {
        // Parse, grouping, two retained copies, and two working copies.
        let multiplier = limits()
            .inbound_parse_factor
            .checked_add(8 + 4 + 4)
            .expect("normalized copy multiplier fits u64");
        // Each of those six trees additionally funds 30 implicit 384-byte nodes.
        let implicit_nodes = 30_u64
            .checked_mul(384)
            .and_then(|bytes| bytes.checked_mul(6))
            .expect("implicit node storage fits u64");
        normalization_wire_delta()
            .checked_mul(multiplier)
            .and_then(|bytes| bytes.checked_add(implicit_nodes))
            .expect("normalization ingress growth fits u64")
    }

    fn normalization_projection_delta() -> u64 {
        normalization_wire_delta()
            .checked_mul(TYPED_CLONE_FACTOR)
            .and_then(|bytes| bytes.checked_mul(u64::from(limits().max_bidders_per_auction)))
            .expect("normalization projection growth fits u64")
    }

    fn retained_scope_delta() -> u64 {
        core_construction_costs()
            .retained_per_scope_bytes
            .checked_mul(u64::from(limits().max_bidders_per_auction))
            .expect("retained scope destinations fit u64")
    }

    fn request_growth_delta() -> u64 {
        normalization_ingress_delta()
            .checked_add(normalization_projection_delta())
            .and_then(|bytes| bytes.checked_add(retained_scope_delta()))
            .expect("request growth delta fits u64")
    }

    fn construction_charge() -> u64 {
        let costs = core_construction_costs();
        costs
            .fixed_per_auction_bytes
            .checked_add(costs.reusable_scratch_bytes)
            .expect("construction layout charge fits u64")
    }

    fn correlation_delta() -> u64 {
        // The unchanged 32-call, 100-impression row adds 864 bytes per call.
        32 * (100 * 8 + 64)
    }

    fn expected_required_bytes() -> u64 {
        // Stage-2 user sync golden already includes auxiliary work and its pinned GVLs.
        10_259_037_696_u64
            .checked_add(
                u64::from(limits().max_concurrent_auctions)
                    .checked_mul(
                        construction_charge()
                            .checked_add(request_growth_delta())
                            .and_then(|bytes| bytes.checked_add(9_704_608))
                            .and_then(|bytes| bytes.checked_add(correlation_delta()))
                            .and_then(|bytes| bytes.checked_add(4_919_296))
                            .expect("per-auction request deltas fit u64"),
                    )
                    .expect("concurrent construction charges fit u64"),
            )
            .expect("independent required bytes fit u64")
    }

    /// The ceiling must come from the adapter constant, so an upstream
    /// change moves this row instead of leaving a stale literal. The
    /// scope, the provenance and the separately published stack are
    /// pinned too: `PerExecution` is what makes this row's populations
    /// per-execution values, and the stack's exclusion from the ceiling
    /// is a decision this test keeps visible.
    #[test]
    fn the_ceiling_is_read_from_the_adapter_constant() {
        let ceiling_fact = FASTLY_PLATFORM.memory_ceiling();
        let source = ceiling_fact.source();
        let ceiling = ceiling_fact.value().expect("Fastly publishes a ceiling");
        assert_eq!(ISOLATE_TOTAL_BYTES, ceiling.total_bytes());
        assert_eq!(limits().isolate_total_bytes, ceiling.total_bytes());
        assert_eq!(ceiling.scope(), MemoryCeilingScope::PerExecution);
        assert_eq!(
            source,
            Some(PlatformResourceSource::PlatformLimit {
                provider: "Fastly Compute",
            }),
        );

        // Fastly is the only one of the three that publishes a separate
        // stack allowance, and `MemoryCeiling` documents it as sitting
        // OUTSIDE `total_bytes`. The row is therefore proven against
        // `total_bytes` alone; folding the stack in would hand the heap
        // a million bytes the platform never granted it.
        assert_eq!(ceiling.stack_bytes(), Some(1_000_000));
        assert_eq!(ISOLATE_TOTAL_BYTES, 128_000_000);
        assert_ne!(ISOLATE_TOTAL_BYTES, 128_000_000 + 1_000_000);
    }

    /// The parser population is PUBLISHED on this target, and
    /// [`PLATFORM_CONCURRENT_CEILING`] reads it rather than restating
    /// the derivation from the scope. Pinning the value and its
    /// provenance is what keeps the read honest: a population moved
    /// upstream must move this row, and a population that turned
    /// `Unknown` upstream must fail here rather than silently fall
    /// back to a local literal.
    #[test]
    fn the_population_bound_is_read_from_the_adapter_constant() {
        let bound_fact = FASTLY_PLATFORM.inbound_request_population_bound();
        let bound = bound_fact
            .value()
            .expect("Fastly publishes an inbound request population bound");
        assert_eq!(PLATFORM_CONCURRENT_CEILING, bound.max_live_requests().get());
        assert_eq!(PLATFORM_CONCURRENT_CEILING, 1);
        assert_eq!(
            bound_fact.source(),
            Some(PlatformResourceSource::PlatformLimit {
                provider: "Fastly Compute",
            }),
        );
    }

    /// Whether host-owned ingress framing memory is charged to the
    /// published ceiling is not something this provider documents.
    /// Upstream models the question now and answers `Unknown`, so the
    /// conservative charge stands. A future answer of `OutsideCeiling`
    /// would zero the pre-admission parser term on this row, and this
    /// test is what surfaces it.
    #[test]
    fn the_host_ingress_accounting_is_unpublished_upstream() {
        assert_eq!(
            FASTLY_PLATFORM
                .host_ingress_memory_accounting()
                .unknown_reason(),
            Some(PlatformUnknownReason::ProviderUnpublished),
        );
        assert_ne!(target().parser_transient_bytes, 0);
    }

    /// The shipped row reports the ceiling-warning variant with exact
    /// operands. A different error would be a fatal configuration fault.
    #[test]
    fn envelope_warns_over_ceiling_on_fastly() {
        let limits = limits();
        let target = target();
        let derived = derive_isolate_budget(
            &limits,
            &target,
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            construction_diagnostic_costs(MAX_BIDDERS),
        )
        .expect("the shipped row derives every charge");
        assert_eq!(
            validate_isolate_limits(
                &limits,
                &target,
                OUTBOUND_META_BYTES,
                core_construction_costs(),
                construction_diagnostic_costs(MAX_BIDDERS)
            ),
            Err(BudgetError::BudgetOverCeiling {
                required: expected_required_bytes(),
                ceiling: ISOLATE_TOTAL_BYTES,
            }),
        );
        // The overshoot an operator reads in the boot warning.
        assert_eq!(
            derived.total_bytes.checked_sub(ISOLATE_TOTAL_BYTES),
            expected_required_bytes().checked_sub(ISOLATE_TOTAL_BYTES)
        );
        assert_eq!(derived.margin_bytes, 0);
    }

    /// Which terms produce that overshoot, so the warning is a worked
    /// list rather than one opaque number. Raising the ceiling to
    /// something the row does fit is the only way to read the terms
    /// back out of the validator, so that is what this does — the
    /// shipped row above still carries the real published figure.
    #[test]
    fn the_over_ceiling_row_names_the_terms_that_dominate() {
        let mut roomy = limits();
        roomy.isolate_total_bytes = u64::MAX;
        let derived = validate_isolate_limits(
            &roomy,
            &target(),
            OUTBOUND_META_BYTES,
            core_construction_costs(),
            construction_diagnostic_costs(MAX_BIDDERS),
        )
        .expect("the row's terms are computable once the ceiling is lifted");

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
                + derived.probe_output_term,
        );

        // The corrected projection charge exceeds the ceiling even for
        // one auction. This is the whole auction population on this
        // scope: an execution serves one request.
        assert_eq!(derived.bookkeeping_term, 101_711_872);
        assert_eq!(
            derived.per_auction_peak,
            6_258_324_480
                + request_growth_delta()
                + construction_charge()
                + 9_704_608
                + correlation_delta()
                + 4_919_296
        );
        assert_eq!(derived.call_side_term, 3_943_976_960);
        assert_eq!(
            u64::from(limits().max_outbound_requests) * CAP_256_KIB * RESPONSE_PARSE_FACTOR,
            0x8000_0000,
            "the measured complete adapter response path remains funded",
        );
        assert_eq!(
            u64::from(limits().max_outbound_requests)
                * u64::from(limits().max_parsed_nodes)
                * limits().response_node_peak_bytes,
            671_088_640,
            "each response parse independently funds its node budget",
        );
        assert_eq!(limits().max_concurrent_auctions, 1);
        assert!(
            derived.per_auction_peak > ISOLATE_TOTAL_BYTES,
            "the measured projection charge exposes single-auction overcommit",
        );

        // Decoder waves remain a separate, significant charge. The
        // corrected projection term is larger than the wave charge.
        assert_eq!(derived.per_stream_peak, 21_825_184);
        assert_eq!(
            u64::from(limits().max_active_body_streams) * derived.per_stream_peak,
            698_405_888,
        );
        assert!(
            derived.per_auction_peak
                > u64::from(limits().max_active_body_streams) * derived.per_stream_peak,
            "the projection-inclusive auction charge now dominates the wave gate",
        );

        // The `PerExecution` scope is worth exactly one slot peak here,
        // against the 6 Cloudflare's `PerInstance` row charges. It is
        // the structural difference between the two rows.
        assert_eq!(derived.pre_admission_parser_term, 238_080);
        assert_eq!(derived.h2_slot_peak, 238_080);

        assert_eq!(derived.egress_term, 50_335_744);

        // The row funds one permit in each independent aux and probe-config
        // gate. The terminal pool is the admission classes' `1 + 1`.
        assert_eq!(derived.aux_term, 3_221_233_664);
        assert_eq!(derived.probe_output_term, 90_112);
        assert_eq!(limits().aux_route_permits, 1);
        assert_eq!(derived.terminal_term, 9_216);
        assert_eq!(limits().terminal_response_permits, 2);

        // Six distinct resident vendor lists at the measured
        // `MAX_GVL_PARSED_BYTES`: the cache's four plus one each for
        // the single concurrent auction and auxiliary route.
        assert_eq!(derived.gvl_terms, 3_072_000);
    }

    /// Independently pin the per-call metadata charge and its layout arithmetic.
    #[test]
    fn envelope_meta_charge_matches_the_row() {
        // `3_584 byte pool + offset table + 96 struct overhead + correlation`, where
        // the offset table is `2x4 + 36x8 + imps x 4` rounded up to 64:
        // `8 + 288 + 400 = 696 -> 704`; correlation adds `100 x 8 + 64 = 864`.
        let offset_table = (2 * 4 + 36 * 8 + u32::from(MAX_IMPRESSIONS) * 4).div_ceil(64) * 64;
        assert_eq!(MAX_IMPRESSIONS, 100);
        assert_eq!(offset_table, 704);
        let correlation = u32::from(MAX_IMPRESSIONS) * 8 + 64;
        assert_eq!(correlation, 864);
        assert_eq!(OUTBOUND_META_BYTES, 5_248);
        assert_eq!(
            OUTBOUND_META_BYTES,
            auctioneer_bidder::outbound_meta_bytes(MAX_IMPRESSIONS)
        );
        assert_eq!(3_584 + offset_table + 96 + correlation, OUTBOUND_META_BYTES);
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
            construction_diagnostic_costs(MAX_BIDDERS),
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
                construction_diagnostic_costs(MAX_BIDDERS),
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
            construction_diagnostic_costs(MAX_BIDDERS),
        )
        .expect("the shipped over-ceiling row still derives accounting");
        let charges = derived.auction_charges;
        assert_eq!(charges.ingress, 216_313_856 + normalization_ingress_delta());
        assert_eq!(
            charges.total().expect("checked total"),
            derived.per_auction_peak
        );
        assert_eq!(
            charges.privacy,
            42 * (1_u64 << 20_u32) + core_construction_costs().fixed_per_auction_bytes
        );
        assert_eq!(
            charges.projection,
            1_677_721_600
                + normalization_projection_delta()
                + retained_scope_delta()
                + core_construction_costs().reusable_scratch_bytes
                + 4_919_296
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
    /// tests examined — not a separately-built copy that could drift.
    #[test]
    fn envelope_carries_the_examined_row() {
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
            construction_diagnostic_costs(MAX_BIDDERS),
        )
        .unwrap();
        let funded = derive_isolate_budget(
            &row,
            &platform,
            boot.outbound_meta_bytes,
            boot.construction_costs,
            construction_diagnostic_costs(MAX_BIDDERS),
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
            construction_diagnostic_costs(MAX_BIDDERS),
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
        assert_eq!(funded.total_bytes - without_shape_marker.total_bytes, 32);
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
