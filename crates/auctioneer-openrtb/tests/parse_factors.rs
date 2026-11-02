//! What a typed `BidRequest` parse costs, measured against the floor
//! the isolate budget refuses to charge less than.
//!
//! `auctioneer-runtime` states the charge as `inbound_bytes x
//! inbound_parse_factor` but cannot measure it: that crate depends on
//! no auctioneer crate, so it can never name `BidRequest`. The measurement
//! therefore lives here, beside the type, and asserts the OBSERVED
//! allocation fits `MIN_INBOUND_PARSE_FACTOR` — the smallest factor an
//! operator may configure.
//!
//! The fixtures are adversarial by design. A benign round-trip body
//! understates the factor badly: what pushes a parse past its
//! allowance is structural, not the payload's size — dense arrays,
//! floods of tiny objects, deep nesting, and retained unknown fields.
//!
//! # Why the floor is 36
//!
//! The cost is bounded by the TYPES, not by the body: `Imp`, `Banner`
//! and their siblings each carry a `#[serde(flatten)]`
//! `serde_json::Map`, so a forty-byte impression retains a map header
//! and its buckets whatever the input density. The ratio therefore
//! does not fall as bodies grow, which is what disproved the old
//! compile-time factor of 2.
//!
//! The original three fixtures measure 18.6x, 22.4x and 29.4x cumulative;
//! `many small strings` binds at 29.4x. The floor is that worst case
//! with the codebase's standard 20% margin, `ceil(29.4 * 1.2) = 36`.
//! Cumulative rather than retained because the parser's transient
//! scratch is genuinely live at the parse's peak — an OOM does not
//! care that an allocation was about to be freed.
//!
//! The guard asserts against BOTH numbers. The floor's allowance, 36x,
//! is the hard bound: past it the shipped envelopes are understated.
//! The un-margined worst case, 30x — `ceil(29.4)` — is the tighter
//! one, and it is what keeps the floor's derivation honest: a type
//! change that climbed to 33x would still fit the floor while having
//! eaten the whole 20% margin, and the next rederivation would have
//! nothing left to spend. Breaching the tighter bound means the floor
//! needs recomputing even though nothing is yet unfunded.
//!
//! Retained bytes are reported alongside so the gap between what the
//! parse holds and what it touches stays visible.
//!
//! # Reobserving Structured User Agents
//!
//! Run `cargo test -p auctioneer-openrtb --test parse_factors --locked --offline
//! -- --nocapture` to record every fixture, including SUA browser counts
//! 64, 512 and 513 (just past vector growth). Output includes cumulative
//! and retained bytes and integer milli-ratios: 1,000 means 1x wire bytes.
//! Record fresh observations before updating the historical derivation
//! above; those three original ratios are not measurements of SUA. The
//! 30x un-margined guard and 36x funded floor remain unchanged.
//!
//! Observed on 2026-10-10, Linux aarch64, Rust 1.95.0 (ratios truncated to
//! three decimal places; cumulative/retained bytes are allocator requests):
//! | Fixture | Wire B | Cumulative B | Retained B | Cumulative ratio |
//! | --- | ---: | ---: | ---: | ---: |
//! | Dense impressions | 17,554 | 326,294 | 326,294 | 18.588x |
//! | Unknown extension | 23,514 | 526,458 | 280,698 | 22.389x |
//! | Many small strings | 11,644 | 342,392 | 291,192 | 29.405x |
//! | SUA, 64 empty brands | 245 | 691 | 691 | 2.820x |
//! | SUA, 512 empty brands | 1,589 | 4,275 | 4,275 | 2.690x |
//! | SUA, 513 empty brands | 1,592 | 8,371 | 8,371 | 5.258x |
//! `BrandVersion` is 8 bytes on this target. SUA's vector-growth boundary
//! remains well below the original binding fixture, which still determines
//! the 30x guard and 36x funded floor; these are not portable exact-byte pins.

/// The measured worst case before the margin is applied: `ceil(29.4)`.
/// `MIN_INBOUND_PARSE_FACTOR` is this plus 20%, so a fixture past this
/// bound has consumed margin the floor is supposed to hold in reserve.
const UNMARGINED_RATIO_CEILING: u64 = 30;

/// Test-scoped counting allocator (per-thread, cumulative) backing the
/// parse-factor measurements. Delegates every operation to `System`.
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the inbound parse-factor bound"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ALLOCATED: Cell<u64> = const { Cell::new(0) };
        static LIVE: Cell<i64> = const { Cell::new(0) };
    }

    pub fn allocated_on_this_thread() -> u64 {
        ALLOCATED.try_with(Cell::get).unwrap_or(0)
    }

    /// Bytes currently held by this thread's live allocations. The
    /// difference across a parse is what the parse RETAINED, as
    /// distinct from the cumulative total, which also counts every
    /// transient buffer `#[serde(flatten)]` allocates and frees.
    pub fn live_on_this_thread() -> i64 {
        LIVE.try_with(Cell::get).unwrap_or(0)
    }

    fn live_add(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    fn live_sub(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_sub(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    fn count(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only
        // relaxes the measurement, never breaks the allocator.
        let _outcome = ALLOCATED.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX)),
            );
        });
    }

    pub struct CountingAllocator;

    // SAFETY: every operation delegates verbatim to `System`; the
    // counting side effect touches only a thread-local counter and
    // never the returned memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            count(layout.size());
            live_add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            live_sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            count(layout.size());
            live_add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            count(new_size.saturating_sub(layout.size()));
            live_add(new_size);
            live_sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc;
            // new_size is the caller's requested size.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }
}

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::mem::size_of;
    use std::thread;

    use super::UNMARGINED_RATIO_CEILING;
    use super::alloc_track::{allocated_on_this_thread, live_on_this_thread};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::device::BrandVersion;
    use auctioneer_runtime::budget::MIN_INBOUND_PARSE_FACTOR;

    /// Bytes the parse of `body` allocated cumulatively, and bytes it
    /// retained, plus the request's id so the caller can confirm the
    /// fixture parsed to the right thing.
    ///
    /// The measurement runs on a THREAD OF ITS OWN. Both counters are
    /// thread-local and a fresh thread starts at zero, so the readings
    /// cover this parse and nothing else — no harness bookkeeping and
    /// no neighbouring test can land inside the window. Measuring on
    /// the harness's own thread made the result depend on whatever ran
    /// before it, which is not a property of the parse being measured.
    fn parse_cost(body: &[u8]) -> (u64, u64, String) {
        let owned = body.to_vec();
        thread::spawn(move || {
            let before = allocated_on_this_thread();
            let live_before = live_on_this_thread();
            let parsed: BidRequest = serde_json::from_slice(&owned).expect("fixture parses");
            let cumulative = allocated_on_this_thread().saturating_sub(before);
            let retained =
                u64::try_from(live_on_this_thread().saturating_sub(live_before)).unwrap_or(0);
            // Read the id before the request drops, so the readings and
            // the identity check describe the same value.
            let id = parsed.id.clone();
            drop(parsed);
            (cumulative, retained, id)
        })
        .join()
        .expect("the measuring thread does not panic")
    }

    /// An imp carrying `ext` content the typed model does not name, so
    /// the parse retains it as a `Value` subtree.
    fn imp_with_unknown_ext(index: usize, unknown_fields: usize) -> String {
        let mut ext = String::from("{");
        for field in 0_usize..unknown_fields {
            if field > 0 {
                ext.push(',');
            }
            let _written = write!(ext, "\"u{field}\":{field}");
        }
        ext.push('}');
        format!("{{\"id\":\"imp-{index}\",\"banner\":{{\"w\":300,\"h\":250}},\"ext\":{ext}}}")
    }

    fn request_with(imps: &[String]) -> String {
        format!(
            "{{\"id\":\"req\",\"imp\":[{}],\"site\":{{\"id\":\"s\",\"page\":\"https://e.test/p\"}}}}",
            imps.join(",")
        )
    }

    /// Dense imp array: many small typed objects, the shape that
    /// maximizes per-element allocation against total body size.
    fn dense_imp_array() -> String {
        let imps: Vec<String> = (0_usize..400_usize)
            .map(|index| format!("{{\"id\":\"imp-{index}\",\"banner\":{{\"w\":300,\"h\":250}}}}"))
            .collect();
        request_with(&imps)
    }

    /// Retained unknowns: `ext` subtrees the typed model keeps as
    /// `Value`, the case that disproved the naive factor.
    fn retained_unknown_ext() -> String {
        let imps: Vec<String> = (0_usize..60_usize)
            .map(|index| imp_with_unknown_ext(index, 40))
            .collect();
        request_with(&imps)
    }

    /// Many small strings: string headers and their contents dominate.
    fn many_small_strings() -> String {
        let imps: Vec<String> = (0_usize..200_usize)
            .map(|index| {
                format!(
                    "{{\"id\":\"imp-{index}\",\"tagid\":\"t{index}\",\
                      \"banner\":{{\"w\":300,\"h\":250}}}}"
                )
            })
            .collect();
        request_with(&imps)
    }

    fn dense_structured_user_agent(count: usize) -> String {
        serde_json::json!({
            "id": "req", "imp": [],
            "device": {"sua": {"browsers": vec![serde_json::json!({}); count]}}
        })
        .to_string()
    }

    fn dense_video_mimes(mimes: Option<serde_json::Value>) -> String {
        let mut video = serde_json::json!({});
        if let Some(value) = mimes {
            video["mimes"] = value;
        }
        let imps: Vec<String> = (0_usize..400_usize)
            .map(|index| {
                serde_json::json!({"id": format!("imp-{index}"), "video": video}).to_string()
            })
            .collect();
        request_with(&imps)
    }

    fn dense_imp_ext_bidder(bidder: Option<serde_json::Value>) -> String {
        let mut ext = serde_json::json!({});
        if let Some(value) = bidder {
            ext["bidder"] = value;
        }
        let imps: Vec<String> = (0_usize..400_usize)
            .map(|index| {
                serde_json::json!({
                    "id": format!("imp-{index}"),
                    "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": ext,
                })
                .to_string()
            })
            .collect();
        request_with(&imps)
    }

    fn inbound_parse_fixtures() -> [(&'static str, String); 16] {
        [
            ("dense imp array", dense_imp_array()),
            ("retained unknown ext", retained_unknown_ext()),
            ("many small strings", many_small_strings()),
            (
                "small structured user agent",
                dense_structured_user_agent(64),
            ),
            (
                "large structured user agent",
                dense_structured_user_agent(512),
            ),
            (
                "structured user agent after vector growth",
                dense_structured_user_agent(513),
            ),
            // Charge each field and presence state independently: a larger
            // neighbouring fixture must not subsidize its parse ratio.
            ("dense video missing mimes", dense_video_mimes(None)),
            (
                "dense video null mimes",
                dense_video_mimes(Some(serde_json::Value::Null)),
            ),
            (
                "dense video empty mimes",
                dense_video_mimes(Some(serde_json::json!([]))),
            ),
            (
                "dense video small mimes strings",
                dense_video_mimes(Some(serde_json::json!(vec!["v"; 512]))),
            ),
            (
                "dense video mimes after vector growth",
                dense_video_mimes(Some(serde_json::json!(vec!["v"; 513]))),
            ),
            ("dense imp ext missing bidder", dense_imp_ext_bidder(None)),
            (
                "dense imp ext null bidder",
                dense_imp_ext_bidder(Some(serde_json::Value::Null)),
            ),
            (
                "dense imp ext empty bidder",
                dense_imp_ext_bidder(Some(serde_json::json!({}))),
            ),
            (
                "dense imp ext bidder tiny fields",
                dense_imp_ext_bidder(Some(serde_json::Value::Object(
                    (0_usize..40_usize)
                        .map(|index| (format!("u{index}"), serde_json::json!(index)))
                        .collect(),
                ))),
            ),
            (
                "dense imp ext bidder after vector growth",
                dense_imp_ext_bidder(Some(serde_json::json!(vec!["v"; 513]))),
            ),
        ]
    }

    #[test]
    #[expect(
        clippy::print_stderr,
        reason = "nocapture records cumulative and retained parse ratios for review"
    )]
    fn inbound_parse_factor_guard() {
        for (name, body) in &inbound_parse_fixtures() {
            let bytes = body.as_bytes();
            let body_len = u64::try_from(bytes.len()).expect("fixture length fits u64");
            let allowance = body_len
                .checked_mul(MIN_INBOUND_PARSE_FACTOR)
                .expect("the allowance does not overflow");
            let (cumulative, retained, parsed_id) = parse_cost(bytes);
            let cumulative_milli_ratio = cumulative
                .checked_mul(1_000)
                .and_then(|scaled| scaled.checked_div(body_len))
                .expect("fixture ratio fits and wire length is nonzero");
            let retained_milli_ratio = retained
                .checked_mul(1_000)
                .and_then(|scaled| scaled.checked_div(body_len))
                .expect("fixture ratio fits and wire length is nonzero");
            eprintln!(
                "{name}: wire={body_len} cumulative={cumulative} retained={retained} \
                 cumulative_milli_ratio={cumulative_milli_ratio} \
                 retained_milli_ratio={retained_milli_ratio} \
                 unmargined_ceiling={UNMARGINED_RATIO_CEILING} \
                 funded_floor={MIN_INBOUND_PARSE_FACTOR} \
                 brand_version_bytes={}",
                size_of::<BrandVersion>(),
            );

            assert!(
                retained > 0,
                "{name}: the parse must retain something measurable"
            );
            assert!(
                cumulative <= allowance,
                "{name}: a {body_len} B body retained {retained} B and allocated \
                 {cumulative} B in total, past the {allowance} B the floor \
                 allows at MIN_INBOUND_PARSE_FACTOR = \
                 {MIN_INBOUND_PARSE_FACTOR}. Every shipped envelope charges \
                 that floor, so these types no longer fit what is funded."
            );
            // The tighter bound: the worst case the floor was derived
            // from, before its 20% margin. Breaching this is not yet an
            // unfunded parse, but it has spent margin the next
            // rederivation would need.
            let unmargined = body_len
                .checked_mul(UNMARGINED_RATIO_CEILING)
                .expect("the unmargined bound does not overflow");
            assert!(
                cumulative <= unmargined,
                "{name}: a {body_len} B body allocated {cumulative} B, past the \
                 {unmargined} B of the un-margined worst case \
                 (x{UNMARGINED_RATIO_CEILING}) that \
                 MIN_INBOUND_PARSE_FACTOR = {MIN_INBOUND_PARSE_FACTOR} was \
                 derived from. BrandVersion occupies {} B. Still inside the floor, \
                 but the 20% margin is gone: rederive the floor.",
                size_of::<BrandVersion>()
            );
            assert_eq!(
                parsed_id, "req",
                "{name}: fixture parsed to the wrong request"
            );
        }
    }
}
