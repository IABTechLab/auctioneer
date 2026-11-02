//! Shared response budgeting uses the canonical synthetic bidder. Shipping decoder
//! allocation obligations live in bidder-owned allocation conformance. Composed
//! protocol proofs also live beside their bidder, outside shared exchange tests.
//!
//! What the RESPONSE side of one auction costs per byte of decoded
//! bidder body, measured against what the isolate budget charges.
//!
//! Two independent factors, two independent measurements:
//!
//! * `MIN_RESPONSE_PARSE_FACTOR` covers turning one bidder's decoded body
//!   into the typed bids the exchange holds. The budget charges it once
//!   per call, so the measurement parses one body and reads what the
//!   parse RETAINED.
//! * `RESPONSE_ASSEMBLY_FACTOR` covers the scratch the assembled `BidResponse`
//!   and its serialization need. It is charged against the same decoded
//!   figure, so the measurement builds a response from bids of a known
//!   body size and reads the peak that survives.
//!
//! Both are LIVE-BYTES readings on a thread of their own. The counter is
//! thread-local and a fresh thread starts at zero, so each window covers
//! exactly the work being measured — no harness bookkeeping, and no
//! neighbouring test can land inside it. Measuring on the harness thread
//! made the result depend on whatever ran before.
//!
//! # Why the fixtures are bid-dense
//!
//! A factor is a claim about the WORST ratio, not the average one, and
//! the worst case for both is a body that is almost entirely structure:
//! many small bids, each producing its own typed node and its own seatbid
//! entry. A body dominated by one large creative string would retain
//! roughly its own size and prove nothing about either factor.
//!
//! # The per-bid floor, and why the factors are what they are
//!
//! Measurement found something the per-byte model does not express on its
//! face: the retained cost of a parsed bid is dominated by a FIXED
//! per-bid figure, not by the bytes it came from. `auctioneer_openrtb::Bid`
//! is over 200 bytes inline — five `String`/`Option<String>` handles, a
//! flattened `serde_json::Map`, and the numerics — and each of its live
//! strings is a separate allocation with its own allocator rounding.
//! Measured across 200, 1 000 and 4 000 bids the parse retains a
//! near-constant [`OBSERVED_PARSE_BYTES_PER_BID`] and the assembly a
//! near-constant [`OBSERVED_ASSEMBLY_BYTES_PER_BID`].
//!
//! A fixed per-bid cost means the per-BYTE ratio is worst when the bids
//! are smallest, so the bid-dense fixture is where each factor binds: a
//! minimal bid is about 67 bytes of JSON, so at the shipped 256 KiB
//! decoded cap a maximally bid-dense body holds roughly 3 900 bids. The
//! factors are set from that case — parse at 6.4x the cap, assembly at
//! 1.95x — and bids padded to 1 KiB retain only about 1.3x and 1.05x.
//!
//! # What these guards assert
//!
//! Every factor guard here asserts the measurement against the FUNDED
//! bound: `decoded x FACTOR`, read from the shipped constant, on the
//! bid-dense fixture. None of them asserts an observed ratio against a
//! separate observed constant.
//!
//! That distinction is the whole point of this revision. The previous
//! guards pinned the measurement instead: this module's own doc stated
//! that "the per-byte factors alone are therefore undercharged for a
//! bid-dense response", and `assembly_factor_guard` then asserted the
//! observed 3 000 milli-bytes per byte while merely NAMING
//! `RESPONSE_ASSEMBLY_FACTOR = 1` in its failure message. Those guards were green
//! across four review rounds while the budget was short by 8x, because a
//! guard that asserts what the code does cannot report that what the code
//! does is unfunded. The per-bid floors are still pinned, but as a
//! separate claim about the TYPE's size, not as a substitute for the
//! funded bound.
//!
//! Catch-all nesting is now funded by M4's corrected parse charge.
//! The M3b depth sweep, node accounting, measured ratio bracket and
//! per-node ceiling remain regression guards alongside M4's margin test.
//! The response assembly API moves bids instead of retaining cloned input.
//!
//! # The response charge is per-byte AND per-node
//!
//! M4's `RESPONSE_PARSE_FACTOR` and `MIN_RESPONSE_PARSE_FACTOR` are both
//! 256, conservatively funding even admitted catch-all nesting with the
//! required 20% margin. The measured peak still tracks nodes: holding
//! the node count at the cap while varying the wire size 23%, bytes per
//! node moves 66 B while milli-bytes per wire byte moves 40 349.
//! `auctioneer_runtime::budget` therefore also charges the independent node
//! reservation, mirroring `parse_expansion` on the request side:
//! `calls x (decoded x MIN_RESPONSE_PARSE_FACTOR + max_parsed_nodes x
//! MIN_RESPONSE_NODE_PEAK_BYTES)`.
//!
//! Both figures are FLOORS, not fixed charges:
//! `ResolvedIsolateLimits::response_parse_factor` and
//! `response_node_peak_bytes` are operator-settable and
//! `validate_isolate_limits` refuses a row under either minimum. These
//! guards measure against the MINIMA, because a floor is only sound if
//! the measurement fits inside it — an operator may charge more, never
//! less.
//!
//! Every guard asserts a funded bound, the catch-all shape included.
//! `response_charge` computes the per-call reservation at the floors.
//! The integration guard checks the node delta in BOTH derivation and
//! validation, independently of how much the byte floor funds, so
//! deleting the production node term cannot hide behind the 256x floor.
//! Request-derived bookkeeping is charged separately, even without calls;
//! its node delta must not mask a missing per-call parse node term.
//! `the_catch_all_peak_is_node_driven_not_byte_driven` survives as the
//! DERIVATION of `MIN_RESPONSE_NODE_PEAK_BYTES` rather than as a report of
//! a gap: it is what fails if the per-node cost grows past the shipped
//! figure.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track;
    use super::alloc_track::live as live_on_this_thread;
    use crate::fake_bidder::FakeBidder;
    use auctioneer_bidder::types::{BidderName, TypedBid};
    use auctioneer_bidder::{
        Bidder as _, BidderHttpResponse, META_BYTES_CAPACITY, OutboundRequestMetaBuilder,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_exchange::scrub::construction_costs;
    use auctioneer_exchange::{
        BidderResultBundle, ResponseInput, ResponseTimeStat, build_response,
    };
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::{BidResponse, SeatBid};
    use auctioneer_runtime::bounded_json::NODE_RETAINED_BYTES;
    use auctioneer_runtime::budget::{
        ASSEMBLY_FACTOR, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, RESPONSE_ASSEMBLY_FACTOR,
        RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits, TargetPlatformConstants,
        derive_isolate_budget, validate_isolate_limits,
    };
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};
    use std::thread;

    const _: () = assert!(RESPONSE_PARSE_FACTOR == 256);
    const _: () = assert!(MIN_RESPONSE_PARSE_FACTOR == RESPONSE_PARSE_FACTOR);
    const _: () = assert!(RESPONSE_ASSEMBLY_FACTOR == 128);

    fn body(bids: usize, padding: usize) -> Vec<u8> {
        let rows: Vec<_> = (0..bids)
            .map(|n| {
                json!({
                    "id":format!("b{n}"), "impid":format!("i{n}"), "price":1.5_f64,
                    "adm":"x".repeat(padding), "crid":format!("c{n}")
                })
            })
            .collect();
        serde_json::to_vec(&json!({"id":"r","cur":"USD","seatbid":[{"seat":"s","bid":rows}]}))
            .unwrap()
    }

    #[test]
    fn dense_and_padded_response_retention_is_actually_funded() {
        for (bid_count, padding) in [(200, 0), (1000, 0), (4000, 0), (200, 1024), (1000, 1024)] {
            let body = body(bid_count, padding);
            let bytes = u64::try_from(body.len()).unwrap();
            let decode_start = alloc_track::reset_peak();
            let parsed: BidResponse = serde_json::from_slice(&body).unwrap();
            let decoded = u64::try_from(alloc_track::live().saturating_sub(decode_start)).unwrap();
            assert!(alloc_track::peak_since(decode_start) >= decoded);
            assert!(
                decoded.checked_mul(6).unwrap().div_ceil(5)
                    <= bytes.checked_mul(RESPONSE_PARSE_FACTOR).unwrap()
            );
            let bids = parsed
                .seatbid
                .into_iter()
                .flat_map(|seat| seat.bid)
                .map(|bid| TypedBid {
                    bid,
                    media_type: MediaType::Banner,
                })
                .collect();
            let input = ResponseInput {
                construction_diagnostics: BTreeMap::new(),
                bidder_visible_tmax_ms: 100,
                auction_timestamp_ms: 0,
                return_all_bid_status: false,
                seatnonbid: BTreeMap::new(),
                prebid_errors: vec![],
                prebid_warnings: vec![],
                bundles: BTreeMap::from([(
                    BidderName::new("fake_bb"),
                    BidderResultBundle {
                        bids,
                        errors: vec![],
                        warnings: vec![],
                        timing: ResponseTimeStat::default(),
                        dispatched: true,
                        browser_auctions: None,
                    },
                )]),
            };
            let before = alloc_track::reset_peak();
            let response = build_response(input).unwrap();
            let serialized = serde_json::to_vec(&response).unwrap();
            let retained = u64::try_from(alloc_track::live().saturating_sub(before)).unwrap();
            assert!(alloc_track::peak_since(before) >= retained);
            assert!(
                retained.checked_mul(6).unwrap().div_ceil(5)
                    <= bytes.checked_mul(RESPONSE_ASSEMBLY_FACTOR).unwrap()
            );
            drop(serialized);
            drop(response);
        }
    }

    /// A bid-dense bidder response body: the highest structure-to-byte
    /// ratio a real response reaches, which is where both factors are
    /// most exposed.
    ///
    /// Raw-string `format!`, not `serde_json::json!`, and the other
    /// builders below are the same: every figure in this file is a
    /// measurement of these exact bytes. `parse_cost` reads the body's
    /// own `len()` as the decoded size and weighs retained bytes
    /// against it, and `OBSERVED_PARSE_BYTES_PER_BID` /
    /// `OBSERVED_ASSEMBLY_BYTES_PER_BID` are derived from the result.
    /// `json!` renders its own spacing, so a conversion would change
    /// the denominator of every ratio while leaving the guards green —
    /// they would simply be funding a different fixture.
    fn bid_dense_body(bids: usize) -> String {
        let rows: Vec<String> = (0_usize..bids)
            .map(|n| {
                format!(r#"{{"id":"b{n}","impid":"i{n}","price":1.5,"adm":"<a/>","crid":"c{n}"}}"#)
            })
            .collect();
        format!(
            r#"{{"id":"r","cur":"USD","seatbid":[{{"seat":"s","bid":[{}]}}]}}"#,
            rows.join(",")
        )
    }

    /// Bytes the typed parse of `body` retained, and the body's own size.
    fn parse_cost(body: &str) -> (u64, u64) {
        let owned = body.to_owned();
        thread::spawn(move || {
            let decoded = u64::try_from(owned.len()).unwrap_or(u64::MAX);
            let before = live_on_this_thread();
            let parsed: BidResponse =
                serde_json::from_str(&owned).expect("the fixture is a valid bid response");
            let after = live_on_this_thread();
            let retained = u64::try_from(after.saturating_sub(before)).unwrap_or(0);
            // Count before the tree drops, so the reading and the value
            // describe the same thing.
            let seats = parsed.seatbid.len();
            drop(parsed);
            assert!(seats > 0, "the fixture produced at least one seatbid");
            (retained, decoded)
        })
        .join()
        .expect("the measuring thread does not panic")
    }

    /// Bytes the response assembly retained, and the decoded body size
    /// the bids came from.
    fn assembly_cost(body: &str) -> (u64, u64) {
        let owned = body.to_owned();
        thread::spawn(move || {
            let decoded = u64::try_from(owned.len()).unwrap_or(u64::MAX);
            let parsed: BidResponse =
                serde_json::from_str(&owned).expect("the fixture is a valid bid response");
            let bids: Vec<TypedBid> = parsed
                .seatbid
                .into_iter()
                .flat_map(|seat: SeatBid| seat.bid)
                .map(|bid| TypedBid {
                    bid,
                    media_type: MediaType::Banner,
                })
                .collect();
            let mut bundles = BTreeMap::new();
            bundles.insert(
                BidderName::new("fake_bb"),
                BidderResultBundle {
                    bids,
                    errors: vec![],
                    warnings: vec![],
                    timing: ResponseTimeStat::default(),
                    dispatched: true,
                    browser_auctions: None,
                },
            );
            let input = ResponseInput {
                construction_diagnostics: BTreeMap::new(),
                bidder_visible_tmax_ms: 100,
                auction_timestamp_ms: 0,
                return_all_bid_status: false,
                seatnonbid: BTreeMap::new(),
                bundles,
                prebid_errors: vec![],
                prebid_warnings: vec![],
            };
            // The window opens AFTER the bids exist: the parse's own cost
            // is `MIN_RESPONSE_PARSE_FACTOR`'s, and charging it twice would
            // make this guard pass for the wrong reason.
            let before = live_on_this_thread();
            let built = build_response(input).expect("the well-typed input assembles");
            let serialized = serde_json::to_vec(&built).expect("the assembled response serializes");
            let after = live_on_this_thread();
            let retained = u64::try_from(after.saturating_sub(before)).unwrap_or(0);
            drop(built);
            drop(serialized);
            (retained, decoded)
        })
        .join()
        .expect("the measuring thread does not panic")
    }

    /// What the budget charges ONE bidder-response parse at the
    /// FLOORS: the per-byte term plus the per-node term.
    ///
    /// The minima rather than a configured row, because a floor is only
    /// sound if the measurement fits inside it. A deployment that raises
    /// either field is charged more and is funded by this bound too.
    ///
    /// Written as the same sum `auctioneer_runtime::budget`'s
    /// `response_parse` computes per call. The integration guard below
    /// separately proves that production carries the node half, even
    /// when the byte half alone covers the measured peak.
    /// The node half is charged at
    /// the shipped `max_parsed_nodes` cap and NOT at the fixture's own
    /// node count, because that is what the budget reserves: a fixture
    /// under the cap is funded by the whole reservation.
    fn response_charge(decoded: u64) -> u64 {
        let bytes = decoded
            .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
            .expect("the per-byte half does not overflow");
        let nodes = u64::from(SHIPPED_RESPONSE_NODE_CAP)
            .checked_mul(MIN_RESPONSE_NODE_PEAK_BYTES)
            .expect("the per-node half does not overflow");
        bytes
            .checked_add(nodes)
            .expect("the per-call response charge does not overflow")
    }

    /// Integer thousandths, so a diagnostic carries the real ratio
    /// without a float conversion.
    fn milli_ratio(retained: u64, decoded: u64) -> u64 {
        retained
            .saturating_mul(1_000)
            .checked_div(decoded)
            .unwrap_or(0)
    }

    /// Bid counts the factor guards measure at.
    ///
    /// Retained bytes per bid is a SAWTOOTH in the bid count, because the
    /// seatbid `Vec` doubles its capacity: just past a doubling boundary
    /// it carries up to 2x spare capacity and retains ~698 B per bid,
    /// and just before the next boundary ~427 B. The per-byte ratio
    /// therefore peaks just ABOVE a boundary, and a guard that samples
    /// only round counts (200, 1 000, 4 000) lands in the troughs and
    /// reads ~6.1x where the real peak is ~10.6x.
    ///
    /// So this list straddles boundaries deliberately: 150, 300 and 600
    /// sit just past 128, 256 and 512 and are the peaks; 200, 400, 800
    /// and 4 000 are troughs. Dropping the peaks would make these guards
    /// pass at a factor that does not fund the worst case, which is the
    /// exact failure this module's earlier revision had.
    const BINDING_BID_COUNTS: [usize; 8] = [150, 200, 300, 400, 600, 800, 1_000, 4_000];

    /// Bytes one parsed bid retains, as an upper bound with headroom for
    /// allocator differences between platforms and for the capacity
    /// slack described above.
    ///
    /// This is a claim about the TYPE, not a substitute for the funded
    /// bound: the factors are what fund the isolate, and
    /// `response_parse_factor_guard` asserts those. This pins the figure
    /// the factors were DERIVED from, so a `Bid` that grows a field
    /// announces itself here rather than quietly eating the headroom.
    const OBSERVED_PARSE_BYTES_PER_BID: u64 = 760;

    /// The same figure for the assembly and serialization phase, at the
    /// moved-bid cost rather than the cloned one.
    const OBSERVED_ASSEMBLY_BYTES_PER_BID: u64 = 200;

    /// A bid whose creative padding makes it `bytes` long. Used to show
    /// the ratio is NOT worst here, which is why the factor guards below
    /// bind on the bid-dense fixture instead.
    fn padded_bid_body(bids: usize, bytes_per_bid: usize) -> String {
        let padding = "x".repeat(bytes_per_bid);
        let rows: Vec<String> = (0_usize..bids)
            .map(|n| {
                format!(
                    r#"{{"id":"b{n}","impid":"i{n}","price":1.5,"adm":"{padding}","crid":"c{n}"}}"#
                )
            })
            .collect();
        format!(
            r#"{{"id":"r","cur":"USD","seatbid":[{{"seat":"s","bid":[{}]}}]}}"#,
            rows.join(",")
        )
    }

    /// The typed parse alone, against the same funded bound, on every
    /// shape.
    ///
    /// Kept as a SUBSET claim now that `adapter_path_factor_guard` owns
    /// the funded bound: the parse is one phase of the path, so it must
    /// also fit, and a regression isolated to the parse is reported
    /// here with a narrower diagnosis. It no longer stands alone —
    /// standing alone is what let a short factor pass.
    ///
    /// It covers all three shapes rather than only the bid-dense one.
    /// At one bid per seatbid the parse alone reaches ~19.6x, over the
    /// old factor of 12 by itself, so the old single-shape fixture
    /// would not have caught even a parse-only overrun.
    #[test]
    fn response_parse_factor_guard() {
        for (shape, build) in SHAPES {
            for bids in BINDING_BID_COUNTS {
                let body = build(bids);
                let (retained, decoded) = parse_cost(&body);
                let charged = decoded
                    .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
                    .expect("the charge does not overflow");
                assert!(
                    retained <= charged,
                    "the {shape} shape at {bids} bids ({decoded} B) retained \
                     {retained} B in the parse alone, over the {charged} B the budget \
                     charges at MIN_RESPONSE_PARSE_FACTOR = {MIN_RESPONSE_PARSE_FACTOR} \
                     ({} milli-B per body byte observed)",
                    milli_ratio(retained, decoded),
                );
            }
        }
    }

    /// `RESPONSE_ASSEMBLY_FACTOR` funds what assembly plus serialization actually
    /// retains, on the same binding fixture and against the same kind of
    /// bound.
    ///
    /// This is the guard whose previous revision asserted an observed
    /// constant while the shipped factor was short by 8x. It now asserts
    /// `retained <= decoded x RESPONSE_ASSEMBLY_FACTOR`.
    ///
    /// The factor it asserts against is the one the MOVING assembly
    /// needs. A return to cloning is not reliably caught here — measured,
    /// the cloning version sits at ~1.7x on this bid-dense fixture,
    /// inside the factor — so what holds the move is `build_seatbids`
    /// taking its bundles by value, not this test.
    #[test]
    fn assembly_factor_guard() {
        for bids in BINDING_BID_COUNTS {
            let body = bid_dense_body(bids);
            let (retained, decoded) = assembly_cost(&body);
            let charged = decoded
                .checked_mul(RESPONSE_ASSEMBLY_FACTOR)
                .expect("the charge does not overflow");
            assert!(
                retained <= charged,
                "assembling a {bids}-bid response from {decoded} B of body retained \
                 {retained} B, over the {charged} B the budget charges at \
                 RESPONSE_ASSEMBLY_FACTOR = {RESPONSE_ASSEMBLY_FACTOR} ({} milli-B per body byte \
                 observed)",
                milli_ratio(retained, decoded),
            );
        }
    }

    /// The padded regime, kept to prove the bid-dense fixture above is
    /// the binding one: with bids an order of magnitude larger the same
    /// factors hold with room to spare, so a guard that measured only
    /// here would pass while the bid-dense case was unfunded. That is
    /// precisely how the earlier revision stayed green.
    #[test]
    fn large_bids_are_not_the_binding_case() {
        for bids in [200_usize, 1_000_usize] {
            let body = padded_bid_body(bids, 1_024);
            let (parse_retained, decoded) = parse_cost(&body);
            let (asm_retained, _same_decoded) = assembly_cost(&body);
            let parse_charged = decoded
                .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
                .expect("the charge does not overflow");
            let asm_charged = decoded
                .checked_mul(RESPONSE_ASSEMBLY_FACTOR)
                .expect("the charge does not overflow");
            assert!(
                parse_retained <= parse_charged,
                "{bids} padded bids: parse retained {parse_retained} B over \
                 {parse_charged} B",
            );
            assert!(
                asm_retained <= asm_charged,
                "{bids} padded bids: assembly retained {asm_retained} B over \
                 {asm_charged} B",
            );
            // The binding claim: the padded ratio is strictly slacker
            // than the bid-dense one the factors were set from.
            assert!(
                milli_ratio(parse_retained, decoded) < 2_000,
                "padded bids are supposed to be the slack case for parse",
            );
        }
    }

    /// Peak bytes the COMPLETE adapter path reached, and the body size
    /// it came from.
    ///
    /// "Complete" is the correction this measurement exists to make.
    /// `parse_cost` above stops at `serde_json::from_str::<BidResponse>`
    /// and reads what the parse retained; production then walks that
    /// tree and builds a `TypedBid` per bid, and while those accumulate
    /// the tree is still alive. So the isolate has to survive the parsed
    /// tree AND a full typed copy at once, and the parse-only reading
    /// misses that entirely.
    ///
    /// Driven through the synthetic `Bidder::make_bids` rather than a
    /// reimplementation of it, so the synthetic test exercises the live boundary API.
    fn adapter_path_peak(body: &str) -> (u64, u64) {
        let owned = body.to_owned();
        thread::spawn(move || {
            let decoded = u64::try_from(owned.len()).unwrap_or(u64::MAX);
            // Budgeted so wide it cannot bind. This row measures the
            // BYTES the decode retains per decoded byte, which is what
            // `MIN_RESPONSE_PARSE_FACTOR` funds; a budget that refused the
            // fixture would measure a refusal instead. The node bound
            // is proved where it is enforced, in the adapter's own
            // refusal test.
            let request = request_for_response(&owned);
            let response = BidderHttpResponse::new(200, vec![], owned.into_bytes())
                .within_node_budget(u32::MAX);
            // M4 retains metadata and each capped, scrubbed request across
            // fanout. Build that context outside the response-allocation
            // window; the per-bidder projection charge funds its lifetime.
            let sent = sent_meta();
            // The window opens after the FIXTURE exists, so the body's
            // own bytes are not charged to the path that reads them.
            let baseline = alloc_track::reset_peak();
            let decoded_bids = FakeBidder::default()
                .make_bids(&request, &sent, &response)
                .expect("the fixture is a valid bid response");
            let peak = alloc_track::peak_since(baseline);
            assert!(
                !decoded_bids.bids.is_empty(),
                "the fixture produced at least one bid",
            );
            drop(decoded_bids);
            (peak, decoded)
        })
        .join()
        .expect("the measuring thread does not panic")
    }

    /// One bid per seatbid, which multiplies the per-SEAT structure
    /// rather than the per-bid structure.
    fn one_bid_per_seat_body(seats: usize) -> String {
        let rows: Vec<String> = (0_usize..seats)
            .map(|n| {
                format!(
                    r#"{{"seat":"s{n}","bid":[{{"id":"b{n}","impid":"i{n}","price":1.5,"adm":"<a/>","crid":"c{n}"}}]}}"#
                )
            })
            .collect();
        format!(r#"{{"id":"r","cur":"USD","seatbid":[{}]}}"#, rows.join(","))
    }

    /// A minimal bid: the smallest wire form, so the per-byte ratio is
    /// at its worst.
    fn minimal_bid_body(bids: usize) -> String {
        let rows: Vec<String> = (0_usize..bids)
            .map(|n| format!(r#"{{"id":"{n}","impid":"{n}","price":1}}"#))
            .collect();
        format!(
            r#"{{"id":"r","cur":"USD","seatbid":[{{"seat":"s","bid":[{}]}}]}}"#,
            rows.join(",")
        )
    }

    /// Every response SHAPE the factor has to fund, with the fixture
    /// builder for each.
    ///
    /// Three shapes rather than one, because they stress different
    /// structure and the worst is not the one the earlier guard used.
    /// `minimal` is the smallest wire form per bid, so the per-byte
    /// ratio is at its worst; `bid-dense` is the previous fixture;
    /// `one-per-seat` multiplies the per-SEATBID structure instead of
    /// the per-bid structure, which no earlier fixture reached at all.
    /// A named response shape: its label and the fixture builder that
    /// produces one of `n` bids.
    type Shape = (&'static str, fn(usize) -> String);

    const SHAPES: [Shape; 3] = [
        ("minimal", minimal_bid_body),
        ("bid-dense", bid_dense_body),
        ("one-per-seat", one_bid_per_seat_body),
    ];

    /// `MIN_RESPONSE_PARSE_FACTOR` funds what the COMPLETE adapter path
    /// peaks at, on every shape, measured through the synthetic bidder.
    ///
    /// This is the guard the previous revision should have been. That
    /// one called `serde_json::from_str::<BidResponse>` and read what
    /// the parse RETAINED, which is a narrower phase than the synthetic bidder
    /// runs and a lower reading than the isolate has to survive:
    /// `make_bids` then builds a `TypedBid` per bid while the parsed
    /// tree is still alive, so the peak holds both. Measured, the
    /// parse-only reading is ~54% of the full path's peak — which is
    /// how a factor of 12 stayed green while being short at every shape
    /// tested, the guard's own bid-dense fixture included.
    ///
    /// Two corrections, therefore: drive `Bidder::make_bids` rather
    /// than a parse, and read the PEAK rather than the retained figure.
    #[test]
    fn adapter_path_factor_guard() {
        for (shape, build) in SHAPES {
            for bids in BINDING_BID_COUNTS {
                let body = build(bids);
                let (peak, decoded) = adapter_path_peak(&body);
                // The per-BYTE half alone, so a regression on these
                // shapes is diagnosed as a factor problem rather than
                // being absorbed by the node reservation. These shapes
                // are byte-driven: `adapter_path_node_term_guard` owns
                // the node-driven one.
                let charged = decoded
                    .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
                    .expect("the charge does not overflow");
                assert!(
                    peak <= charged,
                    "the {shape} shape at {bids} bids ({decoded} B decoded) peaked at \
                     {peak} B across the whole adapter path, over the {charged} B the \
                     per-byte half of the budget's response charge funds at \
                     MIN_RESPONSE_PARSE_FACTOR = {MIN_RESPONSE_PARSE_FACTOR} \
                     ({} milli-B per body byte observed)",
                    milli_ratio(peak, decoded),
                );
                // And against the WHOLE charge, which is what the
                // isolate reserves. Redundant on these shapes by
                // construction and asserted anyway: if the per-byte
                // half is ever the one that moves, the row that fails
                // should be the funded bound and not only its half.
                let whole = response_charge(decoded);
                assert!(
                    peak <= whole,
                    "the {shape} shape at {bids} bids ({decoded} B decoded) peaked at \
                     {peak} B, over the {whole} B the budget charges per call",
                );
            }
        }
    }

    #[test]
    fn mixed_bid_classification_and_errors_fit_the_funded_response_charge() {
        for bids in BINDING_BID_COUNTS {
            let mut response: serde_json::Value =
                serde_json::from_str(&minimal_bid_body(bids)).unwrap();
            for (index, bid) in response["seatbid"][0]["bid"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .enumerate()
            {
                bid["mtype"] = serde_json::json!(u32::from(index.is_multiple_of(2)));
            }
            let body = serde_json::to_string(&response).unwrap();
            let (peak, decoded) = adapter_path_peak(&body);
            let charged = decoded.checked_mul(MIN_RESPONSE_PARSE_FACTOR).unwrap();
            assert!(
                peak <= charged,
                "mixed {bids} bids: classification/error peak {peak} exceeds byte charge {charged}"
            );
            assert!(peak <= response_charge(decoded));
        }
    }

    /// The per-bid floor the parse factor was derived from.
    ///
    /// Separate from the factor guard on purpose: the factor is the
    /// funded bound and that guard asserts it, while this one says WHY
    /// the factor is the size it is. A `Bid` that grows fails here with
    /// a type-shaped message before it shows up as an unexplained
    /// factor increase.
    #[test]
    fn response_parse_retains_a_stable_per_bid_floor() {
        for bids in BINDING_BID_COUNTS {
            let body = bid_dense_body(bids);
            let (retained, decoded) = parse_cost(&body);
            let count = u64::try_from(bids).unwrap_or(u64::MAX);
            let per_bid = retained.checked_div(count).unwrap_or(u64::MAX);
            assert!(
                per_bid <= OBSERVED_PARSE_BYTES_PER_BID,
                "a {bids}-bid body of {decoded} B retained {retained} B, \
                 {per_bid} B per bid, over the measured \
                 {OBSERVED_PARSE_BYTES_PER_BID} B floor — the typed bid grew, so \
                 MIN_RESPONSE_PARSE_FACTOR needs re-deriving",
            );
        }
    }

    /// The bidder-response node budget every shipped adapter declares,
    /// which is what admits or refuses a fixture on this path.
    ///
    /// `run_auction` hands it to `BidderHttpResponse::within_node_budget`
    /// from `ResolvedIsolateLimits::max_parsed_nodes`, and each adapter
    /// envelope sets that to `1 << 16`. Stated here rather than imported
    /// because the adapter constants are private to their crates; each
    /// adapter's own envelope tests pin the row.
    const SHIPPED_RESPONSE_NODE_CAP: u32 = 1 << 16;

    /// `max_decoded_bytes` every shipped adapter declares — the figure
    /// `MIN_RESPONSE_PARSE_FACTOR` is charged against, once per call.
    const SHIPPED_DECODED_CAP: u64 = 256 * 1_024;

    /// Peak bytes the adapter path holds per SYNTACTIC NODE on the
    /// node-driven shape, rounded up from the measurement in
    /// `the_catch_all_peak_is_node_driven_not_byte_driven`.
    ///
    /// This is the figure that makes the cost structural: it barely
    /// moves as the wire size varies, so the peak is a function of the
    /// node count and not of the bytes the per-byte factor charges.
    ///
    /// 320 against a worst MEASURED 312.2, so the headroom is about 2.5%,
    /// enough for allocator differences between platforms, not enough
    /// to hide a shape change. `the_node_term_that_would_fund_the_response_parse`
    /// is what keeps it honest: it fails if the measured
    /// peak passes `max_parsed_nodes x` this figure, which is how the
    /// first attempt at 312 announced that it rounded the measurement
    /// the wrong way.
    const OBSERVED_PEAK_BYTES_PER_NODE: u64 = 320;

    /// The budget may not ship a per-node charge BELOW what this module
    /// measures.
    ///
    /// Two independently written constants in two crates — this one is
    /// the measurement, `MIN_RESPONSE_NODE_PEAK_BYTES` is the charge — so
    /// this fails if either moves without the other. It is not the
    /// shipped figure compared against its own definition: nothing
    /// derives one from the other, which is what gives it teeth.
    const _: () = assert!(
        MIN_RESPONSE_NODE_PEAK_BYTES >= OBSERVED_PEAK_BYTES_PER_NODE,
        "the budget's MIN_RESPONSE_NODE_PEAK_BYTES is below the per-node peak this \
         module measures, so the isolate reserves less per node than the adapter \
         path peaks at"
    );

    /// The ceiling this module brackets the measured response-parse
    /// ratio at, in milli-bytes of peak per decoded byte.
    ///
    /// The worst admissible catch-all shape measures about 102 000. The
    /// bracket exists so the measured cost stays independently bounded:
    /// a measurement past it means the per-node cost grew and the
    /// funding term in
    /// `the_node_term_that_would_fund_the_response_parse` is stale, not
    /// that the bracket should move.
    const RATIO_BRACKET_MILLI: u64 = 110_000;

    /// A schema-VALID bid response whose every bid hangs a nesting chain
    /// off `Bid`'s `#[serde(flatten)]` catch-all.
    ///
    /// Valid on purpose, and the catch-all is why: no schema narrows it,
    /// so `{"a":{"a":...}}` under `bid[]` retains one heap node per level
    /// while costing seven wire bytes per two nodes. This is the shape
    /// `response_node_budget.rs` refuses when it exceeds the budget; the
    /// fixtures here stay INSIDE the budget, which is the point — what
    /// the node cap admits is what the byte factor has to fund.
    fn catch_all_nested_body(bids: usize, depth: usize) -> String {
        let chain = format!("{}1{}", r#"{"a":"#.repeat(depth), "}".repeat(depth));
        let rows: Vec<String> = (0_usize..bids)
            .map(|n| format!(r#"{{"id":"b{n}","impid":"i{n}","price":1,"x":{chain}}}"#))
            .collect();
        format!(
            r#"{{"id":"r","cur":"USD","seatbid":[{{"seat":"s","bid":[{}]}}]}}"#,
            rows.join(",")
        )
    }

    /// Syntactic nodes in `raw`, under the same accounting
    /// `auctioneer_runtime::bounded_json` charges: every JSON value is a
    /// node and every object KEY is a node of its own.
    ///
    /// Reimplemented over a parsed tree rather than imported because the
    /// counting deserializer aborts on the first node past the budget
    /// and so cannot report a total. `a_fixture_at_the_cap_is_admitted`
    /// cross-checks this count against the real parse's verdict, so a
    /// drift between the two accounts fails rather than silently
    /// mis-sizing every fixture here.
    fn syntactic_nodes(raw: &str) -> u64 {
        fn walk(value: &serde_json::Value) -> u64 {
            match value {
                serde_json::Value::Object(map) => map.iter().fold(1, |acc, (_key, inner)| {
                    acc.saturating_add(1).saturating_add(walk(inner))
                }),
                serde_json::Value::Array(items) => items
                    .iter()
                    .fold(1, |acc, inner| acc.saturating_add(walk(inner))),
                serde_json::Value::Bool(_)
                | serde_json::Value::Null
                | serde_json::Value::Number(_)
                | serde_json::Value::String(_) => 1,
            }
        }
        let value: serde_json::Value = serde_json::from_str(raw).expect("the fixture parses");
        walk(&value)
    }

    /// Whether the shipped node budget ADMITS this body, answered by the
    /// real budgeted parse rather than by the node count above.
    fn admitted_by_the_shipped_node_cap(body: &str) -> bool {
        let response = BidderHttpResponse::new(200, vec![], body.as_bytes().to_vec())
            .within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
        FakeBidder::default()
            .make_bids(&request_for_response(body), &sent_meta(), &response)
            .is_ok()
    }

    // Build correlation context outside allocation windows. A classification
    // refusal must not stand in for the parser admission being measured.
    fn request_for_response(body: &str) -> BidRequest {
        let response: serde_json::Value = serde_json::from_str(body).unwrap();
        let ids: BTreeSet<&str> = response["seatbid"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|seat| seat["bid"].as_array().unwrap())
            .map(|bid| bid["impid"].as_str().unwrap())
            .collect();
        BidRequest {
            id: "request".into(),
            imp: ids
                .into_iter()
                .map(|id| Imp {
                    id: id.into(),
                    banner: Some(Banner::default()),
                    ..Imp::default()
                })
                .collect(),
            ..BidRequest::default()
        }
    }

    /// The retained outbound meta `make_bids` reads, built the way the
    /// sink builds it.
    fn sent_meta() -> auctioneer_bidder::OutboundRequestMeta {
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 100);
        builder.set_method("POST").expect("fits");
        builder
            .set_url("https://demo.example.test/openrtb")
            .expect("fits");
        builder.finish()
    }

    /// Catch-all nesting depths the funding guards sweep, with the bid count
    /// that lands each one just under [`SHIPPED_RESPONSE_NODE_CAP`].
    ///
    /// A bid of nesting `d` costs about `2d + 8` nodes — one object and
    /// one key per level, plus the bid's own fixed fields — so the bid
    /// count is derived rather than written down. Several depths rather
    /// than one because the whole claim is that the peak tracks NODES
    /// and not bytes: these differ by 23% in wire size at the same node
    /// count, and a single depth could not show that.
    ///
    /// 120 is the deepest that fits: `serde_json` refuses past 128
    /// levels and the response envelope spends four of them before a
    /// catch-all chain starts.
    const CATCH_ALL_DEPTHS: [usize; 5] = [10, 20, 50, 100, 120];

    /// The most bids of `depth` nesting that still fit the shipped node
    /// cap.
    ///
    /// Derived by MEASURING the fixture's node count and stepping down
    /// until it fits, not by an arithmetic estimate of the per-bid node
    /// cost. The estimate is what an earlier revision used, and it was
    /// 212 nodes optimistic at depth 10 — enough to put every fixture
    /// over the cap, where the guards would have been measuring bodies
    /// the adapter refuses outright. The real count comes from
    /// [`syntactic_nodes`], which
    /// `the_node_count_agrees_with_the_budgeted_parse` holds to the
    /// budgeted parse's own verdict.
    fn bids_at_depth(depth: usize) -> usize {
        let cap = u64::from(SHIPPED_RESPONSE_NODE_CAP);
        let per_bid = u64::try_from(depth.saturating_mul(2).saturating_add(8))
            .expect("the per-bid node estimate fits u64");
        let mut bids = usize::try_from(
            cap.checked_div(per_bid)
                .expect("the per-bid node cost is non-zero"),
        )
        .expect("the bid count fits usize");
        while bids > 1 && syntactic_nodes(&catch_all_nested_body(bids, depth)) > cap {
            // One bid at a time: the overshoot is a few hundred nodes
            // out of 65 536, so this converges in a handful of steps and
            // lands on the LARGEST admissible count rather than
            // somewhere safely below it. The node count is what sets the
            // peak, so leaving slack here would understate the cost
            // these guards measure.
            bids = bids.saturating_sub(1);
        }
        bids
    }

    /// The catch-all depth sweep fits the shipped M4 parse charge, and
    /// remains bounded by the independently measured ratio bracket.
    #[test]
    fn the_parse_factor_funds_the_catch_all_nesting_shape() {
        let mut worst_ratio = 0_u64;
        let mut worst = String::new();
        for depth in CATCH_ALL_DEPTHS {
            let bids = bids_at_depth(depth);
            let body = catch_all_nested_body(bids, depth);
            assert!(
                admitted_by_the_shipped_node_cap(&body),
                "the depth-{depth} fixture ({bids} bids) must pass the shipped \
                 {SHIPPED_RESPONSE_NODE_CAP}-node budget, or it measures a body the \
                 adapter refuses before parsing and proves nothing about the charge",
            );
            let (peak, decoded) = adapter_path_peak(&body);
            assert!(
                decoded <= SHIPPED_DECODED_CAP,
                "the depth-{depth} fixture is {decoded} B, past the \
                 {SHIPPED_DECODED_CAP} B max_decoded_bytes the per-byte half is \
                 charged against — so its overrun would be the byte cap's doing, not \
                 the charge's",
            );
            let charged = decoded
                .checked_mul(RESPONSE_PARSE_FACTOR)
                .expect("the charge does not overflow");
            let ratio = milli_ratio(peak, decoded);
            assert!(
                peak.checked_mul(6)
                    .expect("peak plus margin fits")
                    .div_ceil(5)
                    <= charged,
                "the catch-all nesting shape at depth {depth} ({bids} bids, \
                 {decoded} B decoded) peaked at {peak} B, exceeding the \
                 {charged} B funded at RESPONSE_PARSE_FACTOR = {RESPONSE_PARSE_FACTOR} \
                 with the required 20% margin",
            );
            if ratio > worst_ratio {
                worst_ratio = ratio;
                worst = format!("depth {depth} ({bids} bids, {decoded} B)");
            }
        }

        // The independently measured ratio remains bounded: the
        // node cap puts an absolute ceiling on the peak, so the ratio
        // cannot run away. A measured ratio past this bracket means the
        // per-node cost grew — re-derive OBSERVED_PEAK_BYTES_PER_NODE
        // and the funding term with it.
        assert!(
            worst_ratio <= RATIO_BRACKET_MILLI,
            "the worst catch-all shape ({worst}) peaked at {worst_ratio} milli-B per \
             decoded byte, past the {RATIO_BRACKET_MILLI} measured bracket. \
             The per-node cost grew: re-measure \
             OBSERVED_PEAK_BYTES_PER_NODE = {OBSERVED_PEAK_BYTES_PER_NODE} and the \
             funding term in \
             the_node_term_that_would_fund_the_response_parse",
        );
        assert!(
            worst_ratio <= RESPONSE_PARSE_FACTOR.saturating_mul(1_000),
            "the worst catch-all shape ({worst}) came in at {worst_ratio} milli-B per \
             decoded byte, above RESPONSE_PARSE_FACTOR = {RESPONSE_PARSE_FACTOR}; the \
             catch-all sweep exceeds its funded bound",
        );
    }

    /// The full per-call charge funds the catch-all shape at every
    /// admitted depth. M4's conservative byte half also funds it alone.
    /// The production node term is proved independently by the delta
    /// integration guard, not by assuming the byte half must be short.
    #[test]
    fn adapter_path_node_term_guard() {
        let mut worst_ratio = 0_u64;
        for depth in CATCH_ALL_DEPTHS {
            let bids = bids_at_depth(depth);
            let body = catch_all_nested_body(bids, depth);
            assert!(
                admitted_by_the_shipped_node_cap(&body),
                "the depth-{depth} fixture ({bids} bids) must pass the shipped \
                 {SHIPPED_RESPONSE_NODE_CAP}-node budget",
            );
            let (peak, decoded) = adapter_path_peak(&body);
            assert!(decoded <= SHIPPED_DECODED_CAP);
            let charged = response_charge(decoded);
            let ratio = milli_ratio(peak, decoded);
            assert!(
                peak <= charged,
                "the catch-all nesting shape at depth {depth} ({bids} bids, \
                 {decoded} B decoded) peaked at {peak} B, over the {charged} B \
                 per-call charge at MIN_RESPONSE_PARSE_FACTOR = \
                 {MIN_RESPONSE_PARSE_FACTOR} and MIN_RESPONSE_NODE_PEAK_BYTES = \
                 {MIN_RESPONSE_NODE_PEAK_BYTES} ({ratio} milli-B per decoded byte)",
            );
            let byte_half = decoded
                .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
                .expect("the per-byte charge does not overflow");
            assert!(
                peak.checked_mul(6)
                    .expect("peak plus margin fits")
                    .div_ceil(5)
                    <= byte_half,
                "the depth-{depth} catch-all peak of {peak} B exceeds the \
                 conservative {byte_half} B byte charge with the required 20% margin",
            );
            worst_ratio = worst_ratio.max(ratio);
        }
        assert!(
            worst_ratio <= RATIO_BRACKET_MILLI,
            "the catch-all sweep reached {worst_ratio} milli-B per decoded byte, \
             past the measured {RATIO_BRACKET_MILLI} bracket; re-measure the node cost",
        );
        assert!(
            worst_ratio <= MIN_RESPONSE_PARSE_FACTOR.saturating_mul(1_000),
            "the catch-all sweep exceeds MIN_RESPONSE_PARSE_FACTOR = \
             {MIN_RESPONSE_PARSE_FACTOR}",
        );
    }

    /// Catch-all nesting costs track admitted nodes rather than wire bytes.
    ///
    /// This is the DERIVATION of `MIN_RESPONSE_NODE_PEAK_BYTES`, not a
    /// report of a gap. It survived the gap closing because it is what
    /// fails if the per-node cost grows past the shipped figure.
    ///
    /// Across [`CATCH_ALL_DEPTHS`] the node count is held near the cap
    /// while the wire size varies by about a quarter. If the cost were
    /// per-byte, bytes-per-byte would hold and bytes-per-node would
    /// move. Preserve that distinction even though M4's conservative
    /// shipped byte factor now funds the entire admitted sweep.
    #[test]
    fn the_catch_all_peak_is_node_driven_not_byte_driven() {
        let mut per_node_seen: Vec<u64> = Vec::new();
        let mut ratios: Vec<u64> = Vec::new();
        let mut sizes: Vec<u64> = Vec::new();
        for depth in CATCH_ALL_DEPTHS {
            let bids = bids_at_depth(depth);
            let body = catch_all_nested_body(bids, depth);
            let nodes = syntactic_nodes(&body);
            let (peak, decoded) = adapter_path_peak(&body);
            let per_node = peak
                .checked_div(nodes)
                .expect("a parsed response has at least one node");
            assert!(
                per_node <= OBSERVED_PEAK_BYTES_PER_NODE,
                "at depth {depth} the adapter path peaked at {peak} B over {nodes} \
                 syntactic nodes, {per_node} B per node, past the measured \
                 {OBSERVED_PEAK_BYTES_PER_NODE} B the shipped \
                 MIN_RESPONSE_NODE_PEAK_BYTES = {MIN_RESPONSE_NODE_PEAK_BYTES} is derived \
                 from — re-derive both constants",
            );
            per_node_seen.push(per_node);
            ratios.push(milli_ratio(peak, decoded));
            sizes.push(decoded);
        }

        let per_node_spread = per_node_seen
            .iter()
            .copied()
            .max()
            .expect("the sweep is non-empty")
            .saturating_sub(per_node_seen.iter().copied().min().unwrap_or(0));
        let ratio_spread = ratios
            .iter()
            .copied()
            .max()
            .expect("the sweep is non-empty")
            .saturating_sub(ratios.iter().copied().min().unwrap_or(0));
        let widest = sizes.iter().copied().max().unwrap_or(0);
        let narrowest = sizes.iter().copied().min().unwrap_or(u64::MAX);
        assert!(
            widest > narrowest,
            "the sweep must vary the wire size, or it cannot tell a per-node cost \
             from a per-byte one; every fixture came in at {widest} B",
        );
        // Spreads in comparable units: bytes per node against
        // milli-bytes per byte scaled to the same per-node base.
        let ratio_spread_per_node = ratio_spread
            .saturating_mul(narrowest)
            .checked_div(1_000)
            .unwrap_or(0)
            .checked_div(u64::from(SHIPPED_RESPONSE_NODE_CAP))
            .unwrap_or(0);
        assert!(
            per_node_spread < ratio_spread_per_node.max(1).saturating_mul(4),
            "bytes per NODE moved by {per_node_spread} B across the sweep while \
             bytes per decoded BYTE moved by the equivalent of \
             {ratio_spread_per_node} B per node ({narrowest}..{widest} B of wire at a \
             held node count). The per-node figure is supposed to be the stable one — \
             if the per-BYTE figure is now the stable one, the cost became byte-driven \
             and a per-byte factor can bound it after all, which changes the finding \
             in the_parse_factor_funds_the_catch_all_nesting_shape",
        );
    }

    /// Keep M3b's independently derived node-term ceiling: it covers every
    /// measured catch-all peak, and M4's shipped byte charge funds it too.
    #[test]
    fn the_node_term_that_would_fund_the_response_parse() {
        let node_term_per_call = u64::from(SHIPPED_RESPONSE_NODE_CAP)
            .checked_mul(OBSERVED_PEAK_BYTES_PER_NODE)
            .expect("the node term does not overflow");
        let byte_term_per_call = SHIPPED_DECODED_CAP
            .checked_mul(RESPONSE_PARSE_FACTOR)
            .expect("the byte term does not overflow");
        for depth in CATCH_ALL_DEPTHS {
            let bids = bids_at_depth(depth);
            let body = catch_all_nested_body(bids, depth);
            let (peak, decoded) = adapter_path_peak(&body);
            assert!(
                peak <= node_term_per_call,
                "the catch-all shape at depth {depth} ({bids} bids, {decoded} B) \
                 peaked at {peak} B, past the {node_term_per_call} B independently \
                 derived from max_parsed_nodes = {SHIPPED_RESPONSE_NODE_CAP} x \
                 OBSERVED_PEAK_BYTES_PER_NODE = {OBSERVED_PEAK_BYTES_PER_NODE}",
            );
        }
        assert!(
            node_term_per_call <= byte_term_per_call,
            "the node-derived {node_term_per_call} B ceiling exceeds M4's \
             funded parse charge of {byte_term_per_call} B per call",
        );
    }

    /// The shipped node term alone covers every measured catch-all peak.
    /// M4's byte floor is conservatively larger at the shipped caps;
    /// node accounting is still mandatory in the production equation.
    #[test]
    fn the_shipped_node_term_funds_the_response_parse() {
        let node_term_per_call = u64::from(SHIPPED_RESPONSE_NODE_CAP)
            .checked_mul(MIN_RESPONSE_NODE_PEAK_BYTES)
            .expect("the node term does not overflow");
        let byte_term_per_call = SHIPPED_DECODED_CAP
            .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
            .expect("the byte term does not overflow");
        for depth in CATCH_ALL_DEPTHS {
            let bids = bids_at_depth(depth);
            let body = catch_all_nested_body(bids, depth);
            let (peak, decoded) = adapter_path_peak(&body);
            assert!(
                peak <= node_term_per_call,
                "the catch-all shape at depth {depth} ({bids} bids, {decoded} B) \
                 peaked at {peak} B, past the {node_term_per_call} B the shipped node \
                 term funds — max_parsed_nodes = {SHIPPED_RESPONSE_NODE_CAP} x \
                 MIN_RESPONSE_NODE_PEAK_BYTES = {MIN_RESPONSE_NODE_PEAK_BYTES}. The node term \
                 does not cover the measured peak on its own, so re-derive it",
            );
        }
        assert!(
            node_term_per_call <= byte_term_per_call,
            "the node-derived {node_term_per_call} B ceiling exceeds M4's \
             funded parse charge of {byte_term_per_call} B per call",
        );
    }

    /// A limits row wide enough that `validate_isolate_limits` returns
    /// its terms instead of a ceiling refusal, so the term under test
    /// is readable.
    ///
    /// Never installed and never shipped. Only the fields the
    /// response-parse term reads carry meaningful figures — the call
    /// count, the decoded cap and the node budget, at the shipped
    /// `max_outbound_requests = 64`, 256 KiB and 65 536. Everything
    /// else is whatever satisfies the validator's own floors, because
    /// the guard differences two rows and every untouched term cancels.
    fn roomy_limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: u64::MAX,
            baseline_bytes: 0,
            inbound_request_bytes: SHIPPED_DECODED_CAP,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: SHIPPED_DECODED_CAP,
            max_outbound_raw_bytes: SHIPPED_DECODED_CAP,
            max_transport_bytes: SHIPPED_DECODED_CAP,
            max_decoded_bytes: SHIPPED_DECODED_CAP,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            // The assembly-scratch floor the validator enforces:
            // `calls x decoded x ASSEMBLY_FACTOR`.
            final_response_bytes: 64 * SHIPPED_DECODED_CAP * ASSEMBLY_FACTOR,
            egress_permits: 1,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 1,
            aux_route_peak_bytes: 768 * 1_024 * 1_024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 1,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 50,
            max_outbound_requests: 64,
            max_concurrent_auctions: 1,
            max_active_body_streams: 1,
            max_runtime_config_bytes: 1 << 20,
            max_runtime_config_blob_bytes: 1 << 20,
            max_parsed_nodes: SHIPPED_RESPONSE_NODE_CAP,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 1 << 14,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            // `DEFAULT_CONFIG_JSON_NODES`, inlined: this crate carries
            // no `edgezero-core` dependency.
            max_config_json_nodes: 8_192,
            terminal_response_permits: 2,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        }
    }

    /// Platform constants for [`roomy_limits`]. The header-knob and
    /// entry-count relations the validator checks are arithmetic on
    /// these, so they are written to satisfy them rather than copied
    /// from a target: `knob = ceiling + 1` and
    /// `entries = ceiling / 32`.
    fn roomy_target() -> TargetPlatformConstants {
        TargetPlatformConstants {
            platform_concurrent_ceiling: 1,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: 1 << 14,
            egress_handoff_factor: 2,
        }
    }

    /// The per-call outbound-metadata charge. Any value works: the
    /// guard differences two rows that share it, so it cancels.
    const OUTBOUND_META_BYTES: u32 = 4_384;

    /// The budget's `response_parse` term ACTUALLY carries the node
    /// charge, read out of BOTH `derive_isolate_budget` and
    /// `validate_isolate_limits`, not just the constants.
    ///
    /// Every other guard in this module reads
    /// `MIN_RESPONSE_NODE_PEAK_BYTES` and multiplies it itself, which is a
    /// claim about the constant and not about the equation. Deleting
    /// the node term from `response_parse` leaves all of them green:
    /// measured, it does. So this one differences two derived/validated rows
    /// that differ ONLY in `max_parsed_nodes` and asserts the
    /// call-side reservation moved by the node charge per call, and the
    /// response reservation additionally carries independent bookkeeping
    /// and the fixed debug allowance, even at zero calls.
    /// It covers the shipped floors and raised operator fields/call counts,
    /// so a hard-coded minimum or a once-per-auction charge also fails.
    ///
    /// The row is deliberately roomy so the validator returns `Derived`
    /// rather than a ceiling refusal — the term is what is under test,
    /// not whether a shipped row fits.
    #[test]
    fn the_budget_charges_the_node_term_per_call() {
        for (calls, byte_factor, node_peak_bytes) in [
            (0, MIN_RESPONSE_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES),
            (64, MIN_RESPONSE_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES),
            (
                128,
                MIN_RESPONSE_PARSE_FACTOR * 2,
                MIN_RESPONSE_NODE_PEAK_BYTES * 2,
            ),
        ] {
            let mut wide = roomy_limits();
            wide.max_outbound_requests = calls;
            wide.response_parse_factor = byte_factor;
            wide.response_node_peak_bytes = node_peak_bytes;
            wide.final_response_bytes = u64::from(calls)
                .checked_mul(wide.max_decoded_bytes)
                .and_then(|bytes| bytes.checked_mul(ASSEMBLY_FACTOR))
                .expect("the assembly floor does not overflow");
            // A comparator only: the measured fixtures still use the full cap.
            let mut narrow = wide;
            narrow.max_parsed_nodes = 0;
            let target = roomy_target();
            let costs = construction_costs();
            let catalog = auctioneer_runtime::CatalogResourceCosts::UNCHANGED;
            let with = derive_isolate_budget(&wide, &target, OUTBOUND_META_BYTES, costs, catalog)
                .expect("the roomy row derives");
            let without =
                derive_isolate_budget(&narrow, &target, OUTBOUND_META_BYTES, costs, catalog)
                    .expect("the zero-node row derives");
            assert_eq!(
                validate_isolate_limits(&wide, &target, OUTBOUND_META_BYTES, costs, catalog)
                    .expect("the roomy row validates"),
                with,
            );
            assert_eq!(
                validate_isolate_limits(&narrow, &target, OUTBOUND_META_BYTES, costs, catalog)
                    .expect("the zero-node row validates"),
                without,
            );
            let expected_call_nodes = u64::from(wide.max_outbound_requests)
                .checked_mul(u64::from(wide.max_parsed_nodes))
                .and_then(|nodes| nodes.checked_mul(wide.response_node_peak_bytes))
                .expect("the node charge does not overflow");
            assert_eq!(
                with.call_side_term.checked_sub(without.call_side_term),
                Some(expected_call_nodes),
                "zeroing max_parsed_nodes must remove {expected_call_nodes} B from the CALL-side \
                 term: {calls} calls x {} nodes x {node_peak_bytes} B. The node term \
                 must remain per-call even with a conservative byte factor of \
                 {byte_factor}",
                wide.max_parsed_nodes,
            );
            let expected_bookkeeping_nodes = u64::from(wide.max_parsed_nodes)
                .checked_mul(NODE_RETAINED_BYTES)
                .and_then(|bytes| bytes.checked_mul(4))
                .expect("the bookkeeping node charge does not overflow");
            assert_eq!(
                with.bookkeeping_term.checked_sub(without.bookkeeping_term),
                Some(expected_bookkeeping_nodes),
                "bookkeeping must fund its own node population independently of {calls} calls",
            );
            assert_eq!(
                with.auction_charges
                    .response
                    .checked_sub(without.auction_charges.response),
                Some(
                    expected_call_nodes
                        .checked_add(expected_bookkeeping_nodes)
                        .expect("the combined node charge does not overflow"),
                ),
                "the response reservation must carry per-call parse nodes plus bookkeeping nodes",
            );
            if calls == 0 {
                let expected_bookkeeping = wide
                    .max_outbound_raw_bytes
                    .checked_mul(4)
                    .and_then(|bytes| bytes.checked_add(expected_bookkeeping_nodes))
                    .expect("the full bookkeeping charge does not overflow");
                assert_eq!(with.bookkeeping_term, expected_bookkeeping);
                // Independently pinned zero-row debug allowance: narrowing
                // response parse nodes must not borrow or remove debug funding.
                let fixed_debug = 8_916_128;
                assert_eq!(with.debug_term, fixed_debug);
                assert_eq!(without.debug_term, fixed_debug);
                assert_eq!(
                    with.auction_charges.response,
                    expected_bookkeeping + fixed_debug
                );
                assert_eq!(
                    without.auction_charges.response,
                    without.bookkeeping_term + fixed_debug,
                );
            }
        }
    }

    /// The node-count accounting `syntactic_nodes` uses agrees with the
    /// budgeted parse's own verdict at the boundary.
    ///
    /// Without this the fixtures above could be sized against a node
    /// count the real parser does not share, and "admissible at the
    /// shipped cap" would be a claim about this file rather than about
    /// production.
    #[test]
    fn the_node_count_agrees_with_the_budgeted_parse() {
        let depth = 120_usize;
        let bids = bids_at_depth(depth);
        let body = catch_all_nested_body(bids, depth);
        let counted = syntactic_nodes(&body);
        assert!(
            admitted_by_the_shipped_node_cap(&body),
            "the fixture at the derived bid count must be admitted at \
             {SHIPPED_RESPONSE_NODE_CAP} nodes; this file counted {counted}",
        );
        let request = request_for_response(&body);
        let response = BidderHttpResponse::new(200, vec![], body.into_bytes())
            .within_node_budget(u32::try_from(counted.saturating_sub(1)).unwrap_or(u32::MAX));
        let errors = FakeBidder::default()
            .make_bids(&request, &sent_meta(), &response)
            .expect_err("one node below the fixture's own count must refuse parsing");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(
            errors[0].message, "bidder response exceeded the JSON node budget",
            "classification refusal must not mask a disagreement in node accounting",
        );
    }

    /// And the same for the assembly phase.
    #[test]
    fn assembly_retains_a_stable_per_bid_floor() {
        for bids in BINDING_BID_COUNTS {
            let body = bid_dense_body(bids);
            let (retained, decoded) = assembly_cost(&body);
            let count = u64::try_from(bids).unwrap_or(u64::MAX);
            let per_bid = retained.checked_div(count).unwrap_or(u64::MAX);
            assert!(
                per_bid <= OBSERVED_ASSEMBLY_BYTES_PER_BID,
                "assembling {bids} bids from {decoded} B of body retained \
                 {retained} B, {per_bid} B per bid, over the measured \
                 {OBSERVED_ASSEMBLY_BYTES_PER_BID} B floor",
            );
        }
    }

    fn debug_mime_peak(value: String) -> (bool, u64) {
        thread::spawn(move || {
            let baseline = alloc_track::reset_peak();
            let accepted = auctioneer_bidder::is_debug_json_content_type(&value);
            let peak = alloc_track::peak_since(baseline);
            (accepted, peak)
        })
        .join()
        .unwrap()
    }

    #[test]
    fn debug_mime_source_and_parameter_scratch_are_independently_funded() {
        use auctioneer_runtime::debug_budget::{
            DEBUG_MIME_FIXED_BYTES, DEBUG_MIME_PARAMETER_PEAK_BYTES, DEBUG_MIME_SOURCE_FACTOR,
        };
        use std::io::{self, Write as _};

        let mut peaks = Vec::new();
        for width in [8 * 1024_usize, 32 * 1024, 64 * 1024] {
            let prefix = "application/json;a=";
            let sparse = format!("{prefix}{}", "x".repeat(width - prefix.len()));
            let dense_params = (width - "application/json".len())
                .checked_div(";a=b".len())
                .unwrap();
            let dense = format!("application/json{}", ";a=b".repeat(dense_params));
            let mut row = Vec::new();
            for (value, parameters) in [(sparse, 1_usize), (dense, dense_params)] {
                let bytes = u64::try_from(value.len()).unwrap();
                let count = u64::try_from(parameters).unwrap();
                let (accepted, peak) = debug_mime_peak(value);
                assert!(
                    accepted,
                    "the measurement must reach the actual MIME parser"
                );
                assert!(
                    peak > 0,
                    "a vacuous zero-allocation measurement proves nothing"
                );
                let source_funding = bytes.checked_mul(DEBUG_MIME_SOURCE_FACTOR).unwrap();
                let parameter_funding = count.checked_mul(DEBUG_MIME_PARAMETER_PEAK_BYTES).unwrap();
                let funded = source_funding
                    .checked_add(parameter_funding)
                    .and_then(|subtotal| subtotal.checked_add(DEBUG_MIME_FIXED_BYTES))
                    .unwrap();
                let with_margin = peak.checked_mul(6).unwrap().div_ceil(5);
                writeln!(io::stderr().lock(),
                    "debug MIME: bytes={bytes} parameters={count} peak={peak} margin={with_margin} funded={funded}"
                ).unwrap();
                assert!(
                    with_margin <= funded,
                    "bytes={bytes} parameters={count} peak={peak} margin={with_margin} funded={funded}"
                );
                row.push(peak);
            }
            assert!(
                row[1] > row[0],
                "parameter density must change the measured peak"
            );
            peaks.push(row);
        }
        for column in [0, 1] {
            assert!(
                peaks[2][column] > peaks[0][column] * 2,
                "varying source width must exercise real source/parameter storage"
            );
        }
    }

    #[test]
    fn over_cap_debug_mime_and_original_header_evidence_allocate_nothing() {
        use auctioneer_bidder::DebugBodyEligibility;

        let prefix = "application/json;a=";
        let oversized = format!("{prefix}{}", "x".repeat(64 * 1024 + 1 - prefix.len()));
        assert_eq!(oversized.len(), 64 * 1024 + 1);
        assert_eq!(debug_mime_peak(oversized), (false, 0));
        thread::spawn(|| {
            for (headers, expected) in [
                (vec![], DebugBodyEligibility::Ineligible),
                (
                    vec![("content-type", Some("application/json"))],
                    DebugBodyEligibility::SingleTextValue,
                ),
                (
                    vec![
                        ("content-type", Some("application/json")),
                        ("Content-Type", None),
                    ],
                    DebugBodyEligibility::Ineligible,
                ),
                (
                    vec![("content-type", Some("text/html"))],
                    DebugBodyEligibility::SingleTextValue,
                ),
            ] {
                let baseline = alloc_track::reset_peak();
                let result = DebugBodyEligibility::from_complete_headers(headers);
                let peak = alloc_track::peak_since(baseline);
                assert_eq!(result, expected);
                assert_eq!(peak, 0);
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn debug_permission_inspection_allocates_nothing_for_any_gate_outcome() {
        use auctioneer_bidder::{BidderName, BidderRegistry};
        use auctioneer_config::types::{AccountConfig, BidderInfo};
        use auctioneer_exchange::{AliasMap, debug::DebugPermission};

        thread::spawn(|| {
            let mut registry = BidderRegistry::new();
            for (name, allow) in [("fake_allowed", true), ("fake_denied", false)] {
                registry
                    .register(
                        Box::leak(Box::new(BidderInfo {
                            name: name.into(),
                            debug_allow: allow,
                            ..BidderInfo::default()
                        })),
                        Box::new(FakeBidder::default()),
                    )
                    .expect("unique test bidder");
            }
            let mut aliases = AliasMap::default();
            for (alias, base) in [
                ("fake_alias_allowed", "fake_allowed"),
                ("fake_alias_denied", "fake_denied"),
                ("fake_alias_missing", "fake_absent"),
            ] {
                aliases
                    .bases
                    .insert(BidderName::new(alias), BidderName::new(base));
            }
            let names = [
                "fake_allowed",
                "fake_denied",
                "fake_alias_allowed",
                "fake_alias_denied",
                "fake_alias_missing",
                "fake_absent",
            ]
            .map(BidderName::new);
            for test in [None, Some(1_i32)] {
                let request = BidRequest {
                    test,
                    ..BidRequest::default()
                };
                for caller in [false, true] {
                    for account_allow in [false, true] {
                        let account = AccountConfig {
                            debug_allow: account_allow,
                            ..AccountConfig::default()
                        };
                        for (name, base_allow) in
                            names.iter().zip([true, false, true, false, false, false])
                        {
                            let expected =
                                account_allow && (caller || test == Some(1_i32)) && base_allow;
                            let baseline = alloc_track::reset_peak();
                            let permission = DebugPermission::resolve(&request, caller, &account);
                            let actual = permission.bidder_allowed(name, &aliases, &registry);
                            let peak = alloc_track::peak_since(baseline);
                            assert_eq!(actual, expected, "{name}, {permission:?}");
                            assert_eq!(peak, 0, "{name}, {permission:?}");
                        }
                    }
                }
            }
            // The zero peaks above need a live counter, not a disabled allocator.
            let baseline = alloc_track::reset_peak();
            let control = BidderName::new("fake_allocated_name__");
            assert!(alloc_track::peak_since(baseline) > 0);
            assert_eq!(control.as_str(), "fake_allocated_name__");
        })
        .join()
        .unwrap();
    }
}
