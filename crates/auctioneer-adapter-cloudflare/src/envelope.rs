//! Cloudflare's boot envelope: the per-isolate memory budget this
//! target attempts to install before it serves a route.
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
//! concurrent native auction reaches that transient peak simultaneously.
//!
//! # Where the ceiling comes from
//!
//! [`edgezero_adapter_cloudflare::CLOUDFLARE_PLATFORM`], read through
//! its const accessors rather than copied as a literal. It publishes
//! `128_000_000` bytes with
//! `PlatformResourceSource::PlatformLimit { provider: "Cloudflare Workers" }`
//! and, importantly, `MemoryCeilingScope::PerInstance`.
//!
//! # What `PerInstance` means here, and what the validator cannot say
//!
//! `PerInstance` means concurrent requests served by one runtime
//! instance SHARE one budget. So every population term in this row is
//! an instance-wide population: the pre-admission parser slots, the
//! live body streams and the concurrent auctions are all counted
//! across the whole isolate, not per request.
//!
//! [`auctioneer_runtime::validate_isolate_limits`] has no field for that
//! distinction — it takes one `isolate_total_bytes` and charges every
//! population against it, which happens to be the correct reading for a
//! `PerInstance` target. On a `PerExecution` target the same equation
//! silently means something else. The scope is honoured here by how the
//! population terms are chosen and stated; it is not something the
//! validator can check, and that gap is a property of the validator
//! rather than of this row.
//!
//! # Where each value comes from
//!
//! Four provenances, and they are not interchangeable:
//!
//! - **Published.** [`ISOLATE_TOTAL_BYTES`] alone, read from the
//!   adapter constant above.
//! - **Derived.** auctioneer's own knobs, with the arithmetic written
//!   beside each one.
//! - **Substituted.** Values carried over from
//!   `auctioneer-adapter-axum`'s measured row because they cannot be
//!   measured on this target. Each one says so at its definition. A
//!   substitute is NOT a measurement and is not presented as one.
//! - **Borrowed from a published limit on something else.**
//!   [`PLATFORM_CONCURRENT_CEILING`] alone. Workers publishes no
//!   inbound-concurrency figure, so the term is charged at the only
//!   per-invocation population it does publish — a bound on outbound
//!   connections. Its definition quotes both passages, says what they
//!   settle and what they do not, and names the borrowing as this
//!   row's weakest link rather than presenting it as a derivation.

use crate::boot_log::BOOT_LOG;
use edgezero_adapter_cloudflare::CLOUDFLARE_PLATFORM;
use edgezero_core::PlatformFact;
use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;

/// `DEFAULT_CONFIG_JSON_NODES` in the width the budget charges it at.
///
/// A twin rather than a cast: `u64::from` is not callable in a `const`
/// initializer on the pinned toolchain, and the workspace denies `as`.
/// The assertion below is what keeps the two from drifting.
const CONFIG_JSON_NODES_U64: u64 = 8_192;
use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
use auctioneer_core::host_primitives::construction_diagnostic_costs;
use auctioneer_core::host_primitives::{BootEnvelope, ResourceProfile};
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
/// PUBLISHED, not chosen here: `CLOUDFLARE_PLATFORM` carries it with
/// its scope and its provenance, so a ceiling moved upstream moves this
/// row rather than leaving a stale literal behind.
///
/// The `panic!` cannot fire: the constant is built with
/// `PlatformFact::known(MemoryCeiling::new(..), ..)` in the adapter,
/// and both accessors on the path are `const fn`, so the whole
/// expression is evaluated at compile time and an `Unknown` upstream
/// would be a build failure rather than a boot panic.
const ISOLATE_TOTAL_BYTES: u64 = match CLOUDFLARE_PLATFORM.memory_ceiling() {
    PlatformFact::Known { value: ceiling, .. } => ceiling.total_bytes(),
    PlatformFact::Unknown { .. } | _ => panic!("CLOUDFLARE_PLATFORM publishes a memory ceiling"),
};

/// The shipped body / transport / decoded cap.
const CAP_256_KIB: u64 = 1 << 18;

/// The response cap follows the serialized-output floor without lowering
/// M4's existing 16 MiB cap. Typed assembly has its own auction charge.
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
/// `20_971_520 x 6 = 125_829_120` bytes. The charge bounds retained
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
/// so the capacity is the width of a single auction's fanout wave, not
/// a count of requests. `per_stream_bytes` charges the matching shape:
/// a transport buffer, a decoded buffer and one decoder state, which is
/// a bidder RESPONSE being read.
///
/// # Why `PerInstance` does NOT multiply this
///
/// The gate is process-global and sized once, and only an AUCTION
/// draws on it. The population it bounds is one auction's fanout
/// width, so it does not scale with the isolate's request population
/// the way this row's other terms do. What it scales with is
/// [`MAX_CALLS`].
///
/// Note the consequence the budget equation does not model: with
/// [`MAX_CONCURRENT_AUCTIONS`] above 1, several auctions can be inside
/// `drive_fanout` at once, and they contend for ONE gate of this
/// capacity. So this is the instance-wide concurrent-decode
/// population, shared, not a per-auction allowance — which means
/// auctions beyond the first queue for wave permits. The budget's flat
/// `max_active_body_streams x per_stream_peak` charges the memory
/// correctly; it simply says nothing about that queuing.
///
/// # Why 32
///
/// The gate is QUEUED, not shedding: `acquire_wave` makes a wave wait
/// its turn rather than refusing it, so a capacity below [`MAX_CALLS`]
/// rejects nothing and instead splits a fanout into serial waves
/// inside the same auction deadline. At 4 a 32-call auction ran eight
/// waves back to back, and a Prebid auction's whole timeout is
/// typically 100-300 ms, so bidders in the later waves would routinely
/// time out having never been called.
///
/// 32 is [`MAX_CALLS`]: one wave, every bidder called in parallel, no
/// bidder penalised for its position in the list.
///
/// # The cost, stated plainly
///
/// The per-stream peak is `21_825_184` bytes, nearly all of it one
/// brotli decoder state at this row's 22-bit window, so 32 of them
/// charge `698_405_888` — more than five times this platform's entire
/// ceiling. Pooling decoder
/// state across a wave, or a smaller `max_brotli_wbits`, are the two
/// changes that would move it; cutting the population back would only
/// move the cost onto the deadline.
///
/// # What a reader should re-check
///
/// If `drive_fanout` stops being the sole acquirer, this becomes a
/// mixed population and must be re-derived.
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

/// Concurrently-live requests this platform admits per isolate before
/// any auctioneer gate.
///
/// **A PROVISIONAL CHARGE, NOT A PLATFORM INBOUND LIMIT.** Workers
/// publishes no inbound concurrency figure. This is the one
/// field of the row where the `PerInstance` scope does real work, so
/// the reasoning is written out rather than summarized.
///
/// # What the platform documents
///
/// Workers' "Limits" page, section "Memory", states the ceiling and its
/// scope and nothing about population: *"Each isolate can consume up to
/// 128 MB of memory, including the JavaScript heap and WebAssembly
/// allocations. This limit is per-isolate, not per-invocation. A single
/// isolate can handle many concurrent requests."* "Many" is the whole
/// published figure — no number, no bound, no way to derive one.
///
/// The same page does publish one per-invocation population, and it is
/// about OUTBOUND connections rather than inbound heads: *"Each Worker
/// invocation can have up to six connections simultaneously waiting for
/// response headers"*, and a seventh *"is queued until one of the
/// existing connections receives its response headers"*.
///
/// # What this concludes
///
/// Two separate questions, and only the second is answerable.
///
/// Whether the host's h2 and hpack state is inside the 128 MB at all
/// CANNOT be determined from the published documentation. The memory
/// section enumerates "the JavaScript heap and WebAssembly
/// allocations", and the edge proxy's framing state is neither — it
/// sits in the runtime ahead of the isolate. But Cloudflare never says
/// the enumeration is exhaustive, and `PerInstance` scope means
/// per-request host cost could land in the instance's accounting
/// without ever entering guest linear memory. Absent a statement either
/// way, this row keeps a charge rather than zeroing one.
///
/// Upstream has since made that uncertainty a first-class fact rather
/// than leaving it to each caller's prose.
/// `HostIngressMemoryAccounting` distinguishes `CountsTowardCeiling`
/// from `OutsideCeiling`, and every adapter publishes it as
/// `Unknown` — `ProviderUnpublished` on Cloudflare, Fastly and Akamai,
/// `RuntimeConfigured` on generic Spin. So the question this row could
/// not answer from vendor documentation is one upstream cannot answer
/// either, and it is now recorded as unanswered rather than merely
/// undiscussed. The conservative charge therefore stands on every
/// target, and `the_host_ingress_accounting_is_unpublished_upstream`
/// pins the unknown so a future answer surfaces as a test failure.
///
/// The multiplier is not answerable from provider documentation. The
/// historical Axum example, `96 connections x 4 h2 streams`, is not
/// enforced by its pinned HTTP/1 runner either. Neither knob exists here: the
/// Workers runtime owns admission and auctioneer never sees a connection,
/// so nothing in this deployment makes 384 true and nothing published
/// makes it true either. It was a number carried across rows, not a
/// bound.
///
/// Six is charged in its place: `6 x 238_080 = 1_428_480` bytes, about
/// 1.1% of the ceiling. The wider target allowance increases each slot.
///
/// Six is a BORROWED bound and the borrowing is the weak point of this
/// row — more so now that [`MAX_CONCURRENT_AUCTIONS`] and
/// [`TERMINAL_RESPONSE_PERMITS`] are both derived from it, so a wrong
/// borrowing is wrong in three places. It is named rather than buried. Cloudflare's six is a limit
/// on outbound connections awaiting response headers, not on inbound
/// requests an isolate holds parsed heads for; nothing published ties
/// the two together. It is retained only as a nominal parser-population
/// charge. The application auction gate actually enforces its own six
/// slots; the probe output gate has independently funded fixed capacity.
/// Neither turns this provisional estimate into a provider guarantee.
///
/// It is deliberately NOT one. A `PerInstance` isolate documented to
/// serve "many concurrent requests" plainly holds more than one head at
/// a time, and one would be the per-execution reading that belongs to
/// Fastly and Spin, not to this scope. Upstream reaches the same
/// reading of the same page — its capability guide states that
/// Cloudflare's limit "is shared by concurrent requests in one isolate,
/// so concurrency divides the available application envelope", against
/// Fastly giving "each execution its own heap and stack allowances".
/// A population of 1 here would contradict the scope the ceiling is
/// published with.
///
/// # Upstream now publishes this fact as UNKNOWN
///
/// `CLOUDFLARE_PLATFORM` carries an
/// `InboundRequestPopulationBound` fact, and for Workers it is
/// `PlatformFact::unknown(PlatformUnknownReason::ProviderUnpublished)`
/// — upstream declines to invent a figure Cloudflare does not
/// document. Fastly and Spin publish a known `1`, and those rows read
/// it; this one cannot, because there is nothing to read.
///
/// That is a confirmation of the reading above, not a licence to drop
/// the charge. An unpublished population is not a population of zero:
/// the isolate demonstrably holds parsed heads for the concurrent
/// requests its `PerInstance` ceiling is shared by, so a term must be
/// charged and auctioneer must choose the figure itself. Six remains that
/// figure, and `the_population_bound_is_unpublished_upstream` pins the
/// unknown so that a future upstream publication fails the test rather
/// than leaving this constant quietly stale.
///
/// What changes is the provenance, not the number: this is auctioneer's
/// own borrowing standing in for a fact upstream has now formally
/// recorded as unavailable, rather than a figure auctioneer derived.
///
/// # What a reader should re-check
///
/// If Workers ever publishes a concurrent-request bound per isolate,
/// upstream's fact becomes `Known` and that number replaces this one
/// directly — the substitution is the whole change. If Cloudflare
/// states that host-side framing memory is excluded from the 128 MB,
/// the term becomes zero on this target and the row should say so
/// rather than carry a phantom; upstream's
/// `HostIngressMemoryAccounting` is the place that statement would
/// appear, and it is still `Unknown(ProviderUnpublished)`. If the
/// six-connection limit moves, this moves with it.
///
/// This population follows the target scope, independently of whether
/// the resulting memory budget fits the target ceiling.
const PLATFORM_CONCURRENT_CEILING: u32 = 6;

/// Whole-isolate resident memory after boot.
///
/// **A SUBSTITUTE, NOT A MEASUREMENT.** This is
/// `auctioneer-adapter-axum`'s value, measured on native arm64, standing in
/// for a Cloudflare figure that cannot be obtained here.
///
/// What a real measurement of this field requires on this target: the
/// SUM of the Wasm linear memory and the JS-heap total, the latter over
/// the `DevTools` protocol against `wrangler dev`. `wrangler` is not
/// available in this environment and no other local tool yields the
/// JS-heap half, so no figure can be measured.
///
/// It is carried rather than omitted because the field has no
/// "unknown" representation — it is a `u64` the equation adds
/// unconditionally — and because a native no-op boot is the closest
/// honest lower bound available. A Workers isolate boots a JS runtime
/// this figure does not account for, so the real value is expected to
/// be LARGER and this substitute makes the budget look better than it
/// is. That direction matters: the row is over its ceiling anyway, and
/// a truthful baseline would only widen the overshoot.
const BASELINE_BYTES: u64 = 27_328_512;

/// Per-response transmit state.
///
/// **A SUBSTITUTE, NOT A MEASUREMENT.** On axum this is host-runtime
/// bookkeeping on the write path. This target cannot measure it at all:
/// the bytes live in the Workers host, outside the guest's linear
/// memory, where no guest-side allocator can observe them. Axum's
/// figure is carried so the term is not silently zero.
const TRANSMIT_STATE_BYTES: u32 = 4_096;

/// Hpack dynamic table plus the platform parser's decode scratch, per
/// connection slot.
///
/// **A SUBSTITUTE, NOT A MEASUREMENT, and unmeasurable in principle
/// here.** No h2 or hpack code runs in this guest — the host hands it
/// an already-parsed request — so these bytes never enter guest linear
/// memory and no guest-side allocator can observe them. They are
/// nevertheless real host-side cost under a `PerInstance` budget, so
/// axum's `16_384 = 4_096 hpack table + 12_288 decode scratch` is
/// carried rather than zeroed.
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

/// Auctions this isolate admits at once, instance-wide.
///
/// **Raised from 1.** `PerInstance` scope is the whole argument: this
/// ceiling is shared by the concurrent requests one runtime instance
/// serves, and Workers documents that "a single isolate can handle
/// many concurrent requests". An admission capacity of 1 in that
/// setting means the isolate holds several requests and serves one
/// auction, queuing the rest behind the slowest bidder of the first —
/// which is not what a production exchange does with an isolate it is
/// already paying to keep warm.
///
/// # Why 6
///
/// It is [`PLATFORM_CONCURRENT_CEILING`], the only per-invocation
/// population Workers publishes at all, and the same figure this row
/// already uses for its request population. Admitting every request
/// the isolate can hold is the right shape HERE specifically because
/// the auction route is what this deployment exists to serve: a
/// concurrent request that is not an auction is a manifest or a probe,
/// and those draw on their own classes.
///
/// It inherits that constant's weakness and makes it worse, which is
/// the honest reading. Six is a BORROWED bound — Workers' limit on
/// outbound connections awaiting response headers, not on inbound
/// requests — and it now backs THREE populations on this row: the
/// pre-admission request population, the static refusal class, and now
/// the auction class. A wrong borrowing is therefore wrong in three
/// places rather than one. If Workers ever publishes a concurrent
/// inbound bound, all three move together.
///
/// # What a reader should re-check
///
/// The budget charges each admitted auction's declared peak. This is
/// a conservative population bound, not a measurement of simultaneous
/// transient peaks; production retention must be measured separately.
const MAX_CONCURRENT_AUCTIONS: u32 = PLATFORM_CONCURRENT_CEILING;

/// Auction responses in flight on the egress path, instance-wide.
///
/// One per admitted auction, for the same reason the axum row pairs
/// them: an egress permit is held while a finished auction's response
/// is serialized and written, so sizing this below
/// [`MAX_CONCURRENT_AUCTIONS`] would let admitted auctions complete and
/// then queue at the egress gate still holding all their per-auction
/// memory. Tracking the admission capacity is what keeps that state
/// unreachable.
const EGRESS_PERMITS: u32 = MAX_CONCURRENT_AUCTIONS;

/// Aux-route gate capacity, instance-wide.
///
/// **KEPT AT 4, and this is where `PerInstance` does the opposite of
/// what it does on the per-execution rows.** An aux permit is a
/// per-REQUEST lease: `auctioneer-core`'s ingress policy takes exactly one
/// in `admit_aux` and moves it into the response completion, held
/// until terminal egress. So the live population is the number of
/// aux-class requests in flight at once.
///
/// # What the scope permits
///
/// This ceiling is shared by concurrent requests in one isolate, and
/// [`PLATFORM_CONCURRENT_CEILING`] puts that population at 6. Four
/// concurrent aux requests are therefore plainly reachable here, which
/// is exactly what is NOT true on Fastly and Spin: their
/// `PerExecution` budgets hold one request each, so their rows carry 1
/// and this one does not follow them. Flattening this to 1 by analogy
/// would contradict the scope the ceiling is published with.
///
/// # What auctioneer's gates permit
///
/// The aux class covers three manifest routes (`/info/bidders`,
/// `/info/bidders/{name}`, `/_auctioneer/health/gvl`) and the 404/405
/// fallback arms, which are answered under an aux permit so a
/// canonical response is written while a permit is still held.
/// `/status` is the `probe` class and deliberately takes no aux permit,
/// so it cannot be shed by an aux flood. With six requests admissible
/// and every non-auction, non-probe route drawing here, a capacity of
/// 4 sits below the platform population rather than above it.
///
/// # Why it is not raised to 6
///
/// The gate exists to stop an aux flood consuming auction admission,
/// so its capacity is a deliberate reservation, not a fit to the
/// arrival rate. Four leaves room for an auction and a probe to make
/// progress while the aux class is saturated. Raising it to the full
/// request population would leave nothing reserved and weaken the
/// separation the class exists for.
///
/// # What a reader should re-check
///
/// This tracks [`PLATFORM_CONCURRENT_CEILING`]: if Workers publishes a
/// real concurrent-request bound per isolate, re-derive both together.
///
/// Keep M5's 768 MiB reservation for checked user-sync/privacy work,
/// not just M4's config-extraction and manifest floor. Boot validates
/// the full aux-work peak, including the serialized baked configuration.
/// This is an upper-bound reservation, not a measured resident heap peak.
const AUX_ROUTE_PERMITS: u32 = 4;
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

/// Static-class response permits, instance-wide.
///
/// **DERIVED, not chosen: `6 + 4 = 10`.** The value was 68, then 6, and
/// both were wrong in the same way — a literal standing in for a
/// relation nothing checked.
///
/// # Where the earlier numbers came from
///
/// 68 was `64 + 4`, from a design-time rule
/// `terminal_response_permits >= max_concurrent_auctions +
/// aux_route_permits` whose first operand was filled in with
/// `max_outbound_requests` — an auction's outbound CALL count —
/// instead of `max_concurrent_auctions`. Cutting it to 6 fixed that
/// operand but dropped the rule itself, on the premise that no admitted
/// request holds a permit. That premise was true when the cut landed
/// and false once admission began reserving one, which left the number
/// under-provisioned and the comment asserting the condition that
/// invalidated it.
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
/// again. On this row the admission-bounded sum binds:
/// [`MAX_CONCURRENT_AUCTIONS`] is 6 and [`AUX_ROUTE_PERMITS`] is 4, so
/// ten are required — above the six that
/// [`PLATFORM_CONCURRENT_CEILING`] would give on its own, because an
/// auction's permit is now held for the whole auction rather than only
/// while a refusal renders.
///
/// # Why it is not cut lower
///
/// Exhausting this class turns a refusal into a connection abort, and a
/// saturated isolate is exactly when these permits are drawn. At six,
/// six concurrent auctions consumed the entire class and the seventh
/// arrival — or any aux arrival — reached `ingress::refuse` with
/// nothing left to draw, so it was aborted rather than answered with a
/// bounded 503. Ten makes that state unreachable.
///
/// It is cheap: `10 x (256 x 3 + 4_096) = 48_640` bytes, because a
/// refusal body is 256 bytes. The four added permits cost `19_456`
/// bytes against a row already over its ceiling by more than 1.5 GB,
/// so this is not a term worth trimming.
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

/// This target's resolved per-isolate limits.
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
        // Keep M4's 256-byte parse floor and fund the independent node peak.
        // Both charges are configurable upward, never below their floors.
        response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
        response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
        // The cap covers the serialized-output floor and retains M4's cap.
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
        // The SDK keeps the source Bytes and a Rust Vec while WorkersIo
        // copies that Vec into a JavaScript Uint8Array for a pending write.
        // Both heaps count toward the isolate ceiling: fund all three.
        egress_handoff_factor: 3,
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
        // Installed BEFORE the envelope is validated, which is the
        // whole point: this row is over its ceiling, the warning is the
        // only safety signal on that path, and it fires once per
        // process. The pinned `EdgeZero`'s Cloudflare `init_logger` is
        // a documented no-op, so without this the isolate has no `log`
        // sink at any point and that warning is lost permanently.
        //
        // The channel is the WORKER CONSOLE, selected by target in
        // `crate::boot_log`. It used to be stderr, on the reasoning that
        // `worker-build`'s `wasm-bindgen` shim routes guest stderr to
        // the console — the one link that comment admitted was
        // unverified, and the one that does not hold.
        // `wasm32-unknown-unknown` has no WASI, so there is no
        // `fd_write` import for a shim to route: the writes succeeded
        // into nothing and the warning was lost on exactly the target
        // whose row is over its ceiling.
        //
        // This function stays host-visible, which is why the sink is a
        // static from another module rather than a `worker::` call here:
        // `worker` is a wasm32-only dependency and every row in this
        // envelope is proved by a host test.
        log_sink: BOOT_LOG,
    }
}

/// The platform passed by this adapter's actual dispatch path.
pub const DISPATCH_PLATFORM: edgezero_core::PlatformMetadata = CLOUDFLARE_PLATFORM;

/// Source declarations consumed by both bootstrap and the resource inspector.
#[must_use]
pub fn resource_profile() -> ResourceProfile {
    let limits = limits();
    let target = target();
    let boot = envelope(&limits, &target);
    ResourceProfile {
        name: "cloudflare",
        dispatch_name: "cloudflare",
        target_abi: "wasm32-unknown-unknown",
        headroom_class: auctioneer_runtime::HeadroomClass::Wasm,
        memory_scope: edgezero_core::MemoryCeilingScope::PerInstance,
        platform: CLOUDFLARE_PLATFORM,
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
        assert_eq!(profile.name, "cloudflare");
        assert_eq!(profile.target_abi, "wasm32-unknown-unknown");
        assert_eq!(
            profile.headroom_class,
            auctioneer_runtime::HeadroomClass::Wasm
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
        assert_eq!(profile.catalog_costs.context_per_auction_bytes, 4_919_296);
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
    use edgezero_adapter_cloudflare::CLOUDFLARE_PLATFORM;
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
        // Stage-2 M5 funded raw B. M4 adds domains, escaped headers, syntax, and 23N.
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
        // Stage-2 M5 golden already includes auxiliary work and its pinned GVLs.
        58_064_844_288_u64
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
    /// scope and provenance are pinned too: `PerInstance` is what makes
    /// every population term in this row instance-wide, and a silent
    /// flip to `PerExecution` upstream would change what the row means
    /// without changing a single number in it.
    #[test]
    fn the_ceiling_is_read_from_the_adapter_constant() {
        let ceiling_fact = CLOUDFLARE_PLATFORM.memory_ceiling();
        let source = ceiling_fact.source();
        let ceiling = ceiling_fact
            .value()
            .expect("Cloudflare publishes a ceiling");
        assert_eq!(ISOLATE_TOTAL_BYTES, ceiling.total_bytes());
        assert_eq!(limits().isolate_total_bytes, ceiling.total_bytes());
        assert_eq!(ceiling.scope(), MemoryCeilingScope::PerInstance);
        assert_eq!(
            source,
            Some(PlatformResourceSource::PlatformLimit {
                provider: "Cloudflare Workers",
            }),
        );
        // Workers publishes no separate stack allowance, so nothing
        // sits outside `total_bytes` on this target.
        assert_eq!(ceiling.stack_bytes(), None);
        // Decimal, not `128 * 1024 * 1024`. Pinned because Spin
        // publishes the binary figure and the two must not be conflated.
        assert_eq!(ceiling.total_bytes(), 128_000_000);
    }

    /// Workers publishes no inbound request population, and
    /// [`PLATFORM_CONCURRENT_CEILING`] is auctioneer's own borrowing
    /// standing in for it. Pinning the unknown is what makes that
    /// substitution visible: if upstream ever publishes the figure,
    /// this test fails and the borrowed 6 is replaced by the published
    /// value rather than surviving as a stale local constant.
    #[test]
    fn the_population_bound_is_unpublished_upstream() {
        assert_eq!(
            CLOUDFLARE_PLATFORM
                .inbound_request_population_bound()
                .unknown_reason(),
            Some(PlatformUnknownReason::ProviderUnpublished),
        );
        // The charge is auctioneer's, not upstream's, and it is not zero.
        assert_eq!(PLATFORM_CONCURRENT_CEILING, 6);
    }

    /// Whether host-owned ingress framing memory is charged to the
    /// 128 MB ceiling is the question this row could not answer from
    /// vendor documentation. Upstream models it now and answers
    /// `Unknown`, so the conservative charge stands. If Cloudflare ever
    /// states that the framing state sits outside the ceiling, this
    /// test fails and `parser_transient_bytes` becomes zero here.
    #[test]
    fn the_host_ingress_accounting_is_unpublished_upstream() {
        assert_eq!(
            CLOUDFLARE_PLATFORM
                .host_ingress_memory_accounting()
                .unknown_reason(),
            Some(PlatformUnknownReason::ProviderUnpublished),
        );
        // The pre-admission parser charge is therefore still funded.
        assert_ne!(target().parser_transient_bytes, 0);
    }

    #[test]
    fn egress_handoff_funds_both_rust_buffers_and_the_javascript_copy() {
        assert_eq!(target().egress_handoff_factor, 3);
    }

    /// The shipped row reports the ceiling-warning variant with exact
    /// operands. A different error would be a fatal configuration fault.
    #[test]
    fn envelope_warns_over_ceiling_on_cloudflare() {
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
    /// shipped row above still carries the real `128_000_000`.
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
        // one auction. The 32-call policy cap is unchanged.
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
        assert!(
            derived.per_auction_peak > ISOLATE_TOTAL_BYTES,
            "the measured projection charge exposes single-auction overcommit",
        );

        // The fanout wave gate funds one auction's full call width,
        // behind the concurrent-auction and aux-extraction populations.
        assert_eq!(derived.per_stream_peak, 21_825_184);
        assert_eq!(
            u64::from(limits().max_active_body_streams) * derived.per_stream_peak,
            698_405_888,
        );

        // Concurrent auctions dominate with M4's charges plus per-call nodes.
        // This is the cap `PerInstance`
        // scope argues for and the old row set to 1: six requests
        // sharing one isolate, every one of them admissible as an
        // auction.
        assert_eq!(
            u64::from(limits().max_concurrent_auctions) * derived.per_auction_peak,
            u64::from(limits().max_concurrent_auctions)
                * (6_258_324_480
                    + request_growth_delta()
                    + construction_charge()
                    + 9_704_608
                    + correlation_delta()
                    + 4_919_296),
        );
        assert!(
            u64::from(limits().max_concurrent_auctions) * derived.per_auction_peak
                > u64::from(limits().max_active_body_streams) * derived.per_stream_peak,
            "concurrent auctions are the largest charge on this row",
        );
        assert_eq!(
            limits().max_concurrent_auctions,
            target().platform_concurrent_ceiling,
            "the auction class is bounded by requests in flight on this scope",
        );

        assert_eq!(derived.egress_term, 453_009_408);
        assert_eq!(derived.aux_term, 19_327_385_600);
        assert_eq!(derived.probe_output_term, 122_880);

        // The static class funds every admitted auction plus a
        // concurrent refusal, and stays cheap: a refusal body is 256
        // bytes. Ten permits against the six this row's platform
        // ceiling would give on its own, because an auction holds its
        // permit for the whole auction rather than only while a refusal
        // renders.
        assert_eq!(derived.terminal_term, 48_640);
        assert_eq!(
            limits().terminal_response_permits,
            limits().max_concurrent_auctions + limits().aux_route_permits,
            "the admission-bounded term binds this row, not the platform ceiling",
        );
        assert!(
            limits().terminal_response_permits > target().platform_concurrent_ceiling,
            "an auction's permit outlives the refusal window, so the ceiling alone \
             under-funds the class",
        );

        // The pre-admission parser term: one h2 slot peak times the
        // nominal six-slot parser population. Smaller than every term
        // above, and smaller than the GVL term below.
        assert_eq!(derived.h2_slot_peak, 238_080);
        assert_eq!(derived.pre_admission_parser_term, 1_428_480);
        assert!(
            derived.pre_admission_parser_term < derived.egress_term,
            "the parser term is not a dominant charge on this target",
        );

        // Distinct resident vendor lists at the measured
        // `MAX_GVL_PARSED_BYTES`: cache, auctions and aux requests can
        // retain distinct versions after eviction.
        assert_eq!(derived.gvl_terms, 7_168_000);
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
        assert_eq!(funded.total_bytes - without_shape_marker.total_bytes, 192);
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
