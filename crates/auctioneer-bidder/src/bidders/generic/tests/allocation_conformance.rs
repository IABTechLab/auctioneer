//! Original shipping-decoder allocation guards, independent of exchange's synthetic bidder.

use crate::alloc_track::response as alloc_track;
use crate::alloc_track::response::live as live_on_this_thread;
use auctioneer_bidder::{
    Bidder, BidderHttpResponse, META_BYTES_CAPACITY, OutboundRequestMetaBuilder,
};
use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
use auctioneer_openrtb::types::bid_response::BidResponse;
use auctioneer_runtime::budget::{
    MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, RESPONSE_PARSE_FACTOR,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{self, Write as _};
use std::thread;

fn bidder() -> Box<dyn Bidder> {
    (auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|entry| entry.key == "generic")
        .unwrap()
        .factory)()
}

fn response(bid: &Value, count: usize) -> Vec<u8> {
    serde_json::to_vec(
        &json!({"id":"r", "cur":"USD", "seatbid":[{"seat":"", "bid":vec![bid; count]}]}),
    )
    .unwrap()
}

#[test]
fn decode_peak_with_margin_fits_its_distinct_charge() {
    let minimal = json!({"id":"", "impid":"", "price":0_i32});
    let mut nested = minimal.clone();
    nested["ext"] = json!({"unknown":vec![json!({"":[]}); 2000]});
    let mut creative = minimal.clone();
    creative["adm"] = json!("\"\\\n".repeat(1024));
    let mut full_nested = nested.clone();
    full_nested["ext"]["unknown"] = json!(vec![json!({"":[]}); 20_000]);
    let fixtures = [
        ("minimal", response(&minimal, 1)),
        ("dense-bids", response(&minimal, 8000)),
        ("nested-maps", response(&nested, 1)),
        ("dense-nested-maps", response(&full_nested, 1)),
        ("escaped-creative", response(&creative, 20)),
    ];
    let mut failures = Vec::new();
    for (name, raw) in fixtures {
        assert!(raw.len() <= 256 * 1024, "{name} exceeds decoded cap");
        let bytes = u64::try_from(raw.len()).unwrap();
        let decode = thread::spawn(move || {
            let bidder = bidder();
            let mut pool = Vec::with_capacity(3584);
            let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
            builder.set_method("POST").unwrap();
            builder.set_url("https://bid.example/openrtb").unwrap();
            let sent = builder.finish();
            let response =
                BidderHttpResponse::new(200, vec![], raw).within_node_budget(0x0001_0000);
            // Bidder-visible context is held before decoding and has its
            // own projection charge. Match the fixture's empty impid;
            // classification refusal must not stand in for measured bids.
            let req = BidRequest {
                imp: vec![Imp {
                    banner: Some(Banner::default()),
                    ..Imp::default()
                }],
                ..BidRequest::default()
            };
            let decode_start = alloc_track::reset_peak();
            let parsed = bidder.make_bids(&req, &sent, &response).unwrap();
            assert!(!parsed.bids.is_empty());
            alloc_track::peak_since(decode_start)
        })
        .join()
        .unwrap();
        let margined = decode.checked_mul(6).unwrap().div_ceil(5);
        let funded = bytes.checked_mul(RESPONSE_PARSE_FACTOR).unwrap();
        writeln!(
            io::stderr(),
            "{name}/decode: raw={bytes} peak={decode} margin={margined} funded={funded}"
        )
        .unwrap();
        if margined > funded {
            failures.push(format!("{name}/decode: {margined} > {funded}"));
        }
    }
    assert!(
        failures.is_empty(),
        "underfunded response phases: {failures:?}"
    );
}

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

/// Per-call parse funding, with independent byte and node terms.
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
const BINDING_BID_COUNTS: [usize; 8] = [150, 200, 300, 400, 600, 800, 1_000, 4_000];
fn adapter_path_peak(body: &str) -> (u64, u64) {
    let owned = body.to_owned();
    thread::spawn(move || {
        let bidder = bidder();
        let decoded = u64::try_from(owned.len()).unwrap_or(u64::MAX);
        // Budgeted so wide it cannot bind. This row measures the
        // BYTES the decode retains per decoded byte, which is what
        // `MIN_RESPONSE_PARSE_FACTOR` funds; a budget that refused the
        // fixture would measure a refusal instead. The node bound
        // is proved where it is enforced, in the adapter's own
        // refusal test.
        let request = request_for_response(&owned);
        let response =
            BidderHttpResponse::new(200, vec![], owned.into_bytes()).within_node_budget(u32::MAX);
        // M4 retains metadata and each capped, scrubbed request across
        // fanout. Build that context outside the response-allocation
        // window; the per-bidder projection charge funds its lifetime.
        let sent = sent_meta();
        // The window opens after the FIXTURE exists, so the body's
        // own bytes are not charged to the path that reads them.
        let baseline = alloc_track::reset_peak();
        let decoded_bids = bidder
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
/// peaks at, on every shape, measured through the shipped adapter.
///
/// This is the guard the previous revision should have been. That
/// one called `serde_json::from_str::<BidResponse>` and read what
/// the parse RETAINED, which is a narrower phase than production
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

/// The adapter path MOVES each bid out of the parsed tree rather
/// than cloning it.
///
/// Asserted as a separation, not an absolute: the cloning version
/// holds the parsed tree and two typed copies at the peak, so it
/// costs a full `Vec<TypedBid>` more than the moving one. At 4 000
/// minimal bids that is about 900 KB, so a bound set a comfortable
/// margin below the cloning cost cannot be met by a clone and is
/// met with room by a move.
///
/// Needed because the factor guard alone would not catch a return
/// to cloning once the factor funds the moving path with headroom —
/// which is exactly how the previous revision's headroom hid the
/// defect.
#[test]
fn the_adapter_path_moves_bids_rather_than_cloning_them() {
    // The widest fixture, where the per-bid term dominates and the
    // two implementations are furthest apart.
    const BIDS: usize = 4_000;
    let body = minimal_bid_body(BIDS);
    let (peak, decoded) = adapter_path_peak(&body);
    let (parse_retained, _same_decoded) = parse_cost(&body);
    // A clone holds the parsed tree plus TWO typed copies' worth at
    // the peak; a move holds the tree plus one. So the overhead a
    // move adds over the parse alone must stay under the parse's own
    // retained size — the cloning version measured ~2.5x that.
    let overhead = peak.saturating_sub(parse_retained);
    assert!(
        overhead < parse_retained,
        "the adapter path peaked at {peak} B over a parse retaining \
         {parse_retained} B ({decoded} B decoded), so its typed assembly added \
         {overhead} B — more than the parsed tree itself, which is the signature \
         of cloning each bid out of the tree instead of moving it",
    );
}

/// Response node cap in every shipped transport envelope.
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
    bidder()
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

/// Independent syntactic accounting agrees with the sealed parser's boundary.
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
    let errors = bidder()
        .make_bids(&request, &sent_meta(), &response)
        .expect_err("one node below the fixture's own count must refuse parsing");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, 4);
    assert_eq!(
        errors[0].message, "bidder response exceeded the JSON node budget",
        "classification refusal must not mask a disagreement in node accounting",
    );
}
