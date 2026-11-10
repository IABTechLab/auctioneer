//! Counting-allocator conformance for chunked requests, settings and SDK decoding.

mod response_allocation_conformance;

use crate::alloc_track::{
    self, AllowAllEndpoints, CONSTRUCTION_CONTROL_BYTES, construction_budget,
    size_growth_is_bounded,
};
use auctioneer_bidder::{
    BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundBuildError, OutboundRequestSink,
    SinkLimits, outbound_meta_bytes,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;
use std::{hint::black_box, thread};

struct MsftAllocationChunk {
    wire: serde_json::Value,
    body_len: usize,
    covered: Vec<String>,
}

struct MsftAllocationFixture {
    request: BidRequest,
    before: serde_json::Value,
    chunks: Vec<MsftAllocationChunk>,
}

struct MsftAllocationOutcome {
    accepted: Vec<usize>,
    covered: Vec<String>,
    errors: Vec<OutboundBuildError>,
    rejected: u32,
    remaining: u32,
}

fn msft_allocation_request(count: usize, root_width: usize, imp_width: usize) -> BidRequest {
    serde_json::from_value(json!({
        "id":"allocation", "tmax":81_i32,
        "payload":{"nested":[{"text":"r".repeat(root_width)}],"flags":[true,null,false]},
        "app":{"id":"application","ext":{"prebid":{"source":"mobile","version":"1"}}},
        "imp":(0..count).map(|index| json!({
            "id":format!("i-{index}"), "tagid":"old",
            "banner":{"format":[{"w":300_i32,"h":250_i32}]},
            "payload":{"nested":[{"text":"i".repeat(imp_width)}]},
            "ext":{"gpid":"g","discard":true,"bidder":{
                "placement_id":7_i64,"member":42_i64,"inv_code":"inventory",
                "allow_smaller_sizes":false,"use_pmt_rule":false,"keywords":"k=v",
                "traffic_source_code":"source","pubclick":"https://click.test/",
                "ext_inv_code":"external","ext_imp_id":"auction","banner_frameworks":[1_i32,2_i32],
            }},
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn msft_allocation_literal_imp(index: usize, imp_width: usize) -> serde_json::Value {
    // Literal source-protocol oracle, not production serialization or relocation.
    json!({
        "id":format!("i-{index}"),"tagid":"inventory","displaymanagerver":"mobile-1",
        "banner":{"format":[{"w":300_i32,"h":250_i32}],"w":300_i32,"h":250_i32,"api":[1,2]},
        "payload":{"nested":[{"text":"i".repeat(imp_width)}]},
        "ext":{"gpid":"g","appnexus":{
            "placement_id":7_i64,"allow_smaller_sizes":false,"use_pmt_rule":false,
            "keywords":"k=v","traffic_source_code":"source","pub_click":"https://click.test/",
            "ext_inv_code":"external","ext_imp_id":"auction",
        }},
    })
}

fn msft_allocation_fixture(
    count: usize,
    root_width: usize,
    imp_width: usize,
    hb_source: i64,
    amp: bool,
) -> MsftAllocationFixture {
    let request = msft_allocation_request(count, root_width, imp_width);
    let before = serde_json::to_value(&request).unwrap();
    assert!(before.get("ext").is_none());
    let namespace = if amp {
        json!({"hb_source":hb_source,"is_amp":1_i64})
    } else {
        json!({"hb_source":hb_source})
    };
    let chunks = (0..count)
        .step_by(10)
        .map(|start| {
            let end = start.checked_add(10).unwrap().min(count);
            let wire = json!({
                "id":"allocation","tmax":81_i32,
                "payload":{"nested":[{"text":"r".repeat(root_width)}],"flags":[true,null,false]},
                "app":{"id":"application","ext":{"prebid":{"source":"mobile","version":"1"}}},
                "ext":{"appnexus":namespace},
                "imp":(start..end).map(|index| msft_allocation_literal_imp(index, imp_width))
                    .collect::<Vec<_>>(),
            });
            MsftAllocationChunk {
                body_len: serde_json::to_vec(&wire).unwrap().len(),
                covered: (start..end).map(|index| format!("i-{index}")).collect(),
                wire,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(chunks.len(), count.div_ceil(10));
    MsftAllocationFixture {
        request,
        before,
        chunks,
    }
}

fn msft_allocation_context<'a>(entry: EntryPoint) -> BidderRequestInfo<'a> {
    use auctioneer_runtime::bounded_json::JsonFieldShape;

    let mut context = BidderRequestInfo::new(
        BidderName::new("msft-alias"),
        BidderName::new("msft"),
        "account",
        None,
        "https://bidder.example.test/openrtb2/{{.AccountID}}?member_id=old&x=a%2Fb&m%65mber_id=other&member_id&x=a+b&x=a%20b&flag&blank=&Member_id=case&member%255fid=twice",
        &["AccountID"],
        None,
        None,
        entry,
        &[],
    );
    context.original_request_ext_shape = JsonFieldShape::Absent;
    context
}

fn msft_allocation_per_attempt(cap: usize, count: usize) -> usize {
    // SinkLimits uses the actual auction imp count for EVERY offset table,
    // including the final one-imp chunk. Only that real metadata delta is added.
    let table_delta = outbound_meta_bytes(u16::try_from(count).unwrap())
        .checked_sub(outbound_meta_bytes(1))
        .unwrap();
    construction_budget(cap)
        .checked_add(usize::try_from(table_delta).unwrap())
        .unwrap()
}

fn msft_allocation_expected_outcome(
    chunks: &[MsftAllocationChunk],
    cap: usize,
    slots: u32,
) -> MsftAllocationOutcome {
    let mut expected = MsftAllocationOutcome {
        accepted: Vec::new(),
        covered: Vec::new(),
        errors: Vec::new(),
        rejected: 0,
        remaining: slots,
    };
    for (index, chunk) in chunks.iter().enumerate() {
        if expected.remaining == 0 {
            if expected.rejected == 0 {
                expected
                    .errors
                    .push(OutboundBuildError::CallBudgetExhausted { max_calls: slots });
            }
            expected.rejected = expected.rejected.checked_add(1).unwrap();
        } else if chunk.body_len > cap {
            expected.errors.push(OutboundBuildError::OversizeRaw {
                limit: u64::try_from(cap).unwrap(),
            });
        } else {
            expected.accepted.push(index);
            expected.covered.extend(chunk.covered.iter().cloned());
            expected.remaining = expected.remaining.checked_sub(1).unwrap();
        }
    }
    expected
}

fn msft_assert_allocation_bounds(
    measured: &alloc_track::Measurement,
    cap: usize,
    count: usize,
    chunks: usize,
    slots: u32,
    sdk_raw_bytes: usize,
) {
    let per_attempt = msft_allocation_per_attempt(cap, count);
    // Escaped keys/strings share Serde scratch (capacity < 2R), while
    // decoded source + version retain at most R. These live 3R bytes reuse
    // released materialization wire storage, not a new budget coefficient.
    // Cumulative allocation also counts freed scratch reallocations:
    // their geometric series is < 4R, plus retained strings <= R.
    let allocated_bound = per_attempt
        .checked_mul(chunks)
        .unwrap()
        .checked_add(sdk_raw_bytes.checked_mul(5).unwrap())
        .unwrap();
    let peak_bound = per_attempt
        .checked_mul(chunks)
        .unwrap()
        .checked_add(sdk_raw_bytes.checked_mul(3).unwrap())
        .unwrap();
    let largest_bound = per_attempt.max(sdk_raw_bytes.checked_mul(2).unwrap());
    assert!(
        measured.allocated() <= allocated_bound
            && measured.peak() <= peak_bound
            && measured.largest() <= largest_bound,
        "count={count} chunks={chunks} cap={cap} slots={slots} per_attempt={per_attempt}: {measured:?}",
    );
    if slots == 0 {
        assert_eq!(measured.allocated(), 0);
        assert_eq!(measured.peak(), 0);
        assert_eq!(measured.largest(), 0);
    } else {
        assert!(measured.allocated() > 0);
        assert!(measured.peak() > 0);
        assert!(measured.largest() > 0);
    }
}

fn msft_assert_allocation_call(
    call: &auctioneer_bidder::BidderHttpRequest,
    expected: &MsftAllocationChunk,
    cap: usize,
) {
    assert_eq!(call.method(), "POST");
    assert_eq!(
        call.url(),
        "https://bidder.example.test/openrtb2/account?x=a%2Fb&x=a+b&x=a%20b&flag&blank=&Member_id=case&member%255fid=twice&member_id=42"
    );
    assert_eq!(
        call.headers().collect::<Vec<_>>(),
        [
            ("Content-Type", "application/json;charset=utf-8"),
            ("Accept", "application/json"),
            ("x-openrtb-version", "2.6"),
        ]
    );
    assert_eq!(call.body().len(), expected.body_len);
    assert!(call.body().len() <= cap);
    assert_eq!(
        call.covered_imp_ids().collect::<Vec<_>>(),
        expected
            .covered
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(call.body()).unwrap(),
        expected.wire
    );
}

fn msft_allocation_probe(
    bidder: &dyn auctioneer_bidder::Bidder,
    context: &BidderRequestInfo<'_>,
    fixture: &MsftAllocationFixture,
    cap: usize,
    slots: u32,
) -> alloc_track::Measurement {
    msft_allocation_probe_with_sdk(bidder, context, fixture, cap, slots, 0)
}

fn msft_allocation_probe_with_sdk(
    bidder: &dyn auctioneer_bidder::Bidder,
    context: &BidderRequestInfo<'_>,
    fixture: &MsftAllocationFixture,
    cap: usize,
    slots: u32,
    sdk_raw_bytes: usize,
) -> alloc_track::Measurement {
    use auctioneer_bidder::BidderHttpRequest;

    // Oracle construction, JSON sizing and coverage planning precede the window.
    let expected = msft_allocation_expected_outcome(&fixture.chunks, cap, slots);
    let count = fixture.request.imp.len();
    let budget = CallBudget::new(slots);
    let mut sink = OutboundRequestSink::new(
        SinkLimits::new(cap, u16::try_from(count).unwrap()),
        &budget,
        &AllowAllEndpoints,
    );
    alloc_track::start();
    let semantic = bidder.make_requests(&fixture.request, context, &mut sink);
    let outcome = sink.into_outcome();
    let measured = alloc_track::finish();

    semantic.unwrap();
    msft_assert_allocation_bounds(
        &measured,
        cap,
        count,
        fixture.chunks.len(),
        slots,
        sdk_raw_bytes,
    );
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(outcome.suppressed_error_count, 0);
    assert_eq!(outcome.rejected_call_count, expected.rejected);
    assert_eq!(outcome.errors, expected.errors);
    assert_eq!(outcome.accepted.len(), expected.accepted.len());
    assert_eq!(budget.remaining(), expected.remaining);
    for (call, index) in outcome.accepted.iter().zip(&expected.accepted) {
        msft_assert_allocation_call(call, &fixture.chunks[*index], cap);
    }
    assert_eq!(
        outcome
            .accepted
            .iter()
            .flat_map(BidderHttpRequest::covered_imp_ids)
            .collect::<Vec<_>>(),
        expected
            .covered
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    if fixture.chunks.iter().all(|chunk| chunk.body_len <= cap) {
        assert_eq!(
            outcome.accepted.len(),
            usize::try_from(slots).unwrap().min(fixture.chunks.len())
        );
    }
    if slots != 0 && fixture.chunks.iter().all(|chunk| chunk.body_len > cap) {
        // Three failed chunks must all be attempted even with only one slot.
        assert_eq!(outcome.errors.len(), fixture.chunks.len());
        assert_eq!(budget.remaining(), slots, "every oversize attempt refunds");
    }
    assert_eq!(
        serde_json::to_value(&fixture.request).unwrap(),
        fixture.before
    );
    measured
}

fn msft_allocation_payload_grid(
    bidder: &dyn auctioneer_bidder::Bidder,
    context: &BidderRequestInfo<'_>,
    count: usize,
    slots: u32,
    cap: usize,
    hb_source: i64,
) {
    let mut observations = Vec::with_capacity(4);
    let mut body_attempts = Vec::with_capacity(4);
    for root_width in [64_usize, 0x0004_0000] {
        for imp_width in [64_usize, 0x0004_0000] {
            let fixture = msft_allocation_fixture(
                count,
                root_width,
                imp_width,
                hb_source,
                context.entry_point == EntryPoint::Amp,
            );
            let expected = msft_allocation_expected_outcome(&fixture.chunks, cap, slots);
            body_attempts.push(
                expected
                    .accepted
                    .len()
                    .checked_add(
                        expected
                            .errors
                            .iter()
                            .filter(|error| matches!(error, OutboundBuildError::OversizeRaw { .. }))
                            .count(),
                    )
                    .unwrap(),
            );
            observations.push(msft_allocation_probe(bidder, context, &fixture, cap, slots));
        }
    }
    // Row-major edges independently widen root and retained per-imp payloads.
    for (small, large) in [(0, 1), (2, 3), (0, 2), (1, 3)] {
        // Failed bodies refund their slot, so a larger input may reach
        // more capped writes than a fitting input that exhausted slots.
        // Allow only that independently planned extra body/meta work.
        let extra_attempts = body_attempts[large].saturating_sub(body_attempts[small]);
        let extra_work = msft_allocation_per_attempt(cap, count)
            .checked_mul(extra_attempts)
            .unwrap();
        assert!(
            observations[large].peak()
                <= observations[small]
                    .peak()
                    .saturating_add(CONSTRUCTION_CONTROL_BYTES)
                && observations[large].largest()
                    <= observations[small]
                        .largest()
                        .saturating_add(CONSTRUCTION_CONTROL_BYTES)
                && observations[large].allocated()
                    <= observations[small]
                        .allocated()
                        .saturating_add(extra_work)
                        .saturating_add(CONSTRUCTION_CONTROL_BYTES),
            "edge={small}->{large} count={count} slots={slots} cap={cap}: {observations:?}"
        );
    }
}

#[test]
fn msft_chunk_construction_is_funded_across_payload_slots_caps_and_settings() {
    use auctioneer_bidder::BidderSettings;

    thread::spawn(|| {
        let bidder = (auctioneer_bidder::compiled_bidders()
            .iter()
            .find(|entry| entry.key == "msft")
            .unwrap()
            .factory)();
        let base: BidderSettings = serde_json::from_value(json!({
            "implementation":"msft","settings":{"hb_source":50_i64,"hb_source_video":60_i64},
        }))
        .unwrap();
        let auction_zero: BidderSettings = serde_json::from_value(json!({
            "implementation":"msft","settings":{"hb_source":0_i64},
        }))
        .unwrap();
        let video_zero: BidderSettings = serde_json::from_value(json!({
            "implementation":"msft","settings":{"hb_source_video":0_i64},
        }))
        .unwrap();
        // Literal fieldwise/default oracles: an explicit zero blocks base
        // inheritance, then selects the entrypoint default (5 or 6).
        for (entry, requested, inherited, hb_source) in [
            (EntryPoint::Auction, None, None, 5_i64),
            (EntryPoint::Amp, None, None, 5),
            (EntryPoint::Video, None, None, 6),
            (EntryPoint::Auction, None, Some(&base), 50),
            (EntryPoint::Video, None, Some(&base), 60),
            (EntryPoint::Auction, Some(&auction_zero), Some(&base), 5),
            (EntryPoint::Amp, Some(&auction_zero), Some(&base), 5),
            (EntryPoint::Video, Some(&auction_zero), Some(&base), 60),
            (EntryPoint::Auction, Some(&video_zero), Some(&base), 50),
            (EntryPoint::Video, Some(&video_zero), Some(&base), 6),
        ] {
            let mut context = msft_allocation_context(entry);
            context
                .set_settings(Some("msft"), requested, inherited)
                .unwrap();
            for count in [1_usize, 11, 21] {
                for slots in [0_u32, 1, 3] {
                    for cap in [32_usize, 0x4000] {
                        msft_allocation_payload_grid(
                            bidder.as_ref(),
                            &context,
                            count,
                            slots,
                            cap,
                            hb_source,
                        );
                    }
                }
            }
        }
    })
    .join()
    .unwrap();
}

fn msft_allocation_clone_probe(request: &BidRequest) -> alloc_track::Measurement {
    alloc_track::start();
    let cloned = black_box(request.clone());
    let measured = alloc_track::finish();
    drop(black_box(cloned));
    measured
}

#[test]
fn msft_real_caller_clones_break_absolute_and_cross_size_bounds() {
    thread::spawn(|| {
        for count in [1_usize, 11, 21] {
            for widen_imp in [false, true] {
                let small = msft_allocation_request(count, 64, 64);
                let large = msft_allocation_request(
                    count,
                    if widen_imp { 64 } else { 0x0004_0000 },
                    if widen_imp { 0x0004_0000 } else { 64 },
                );
                let small_clone = msft_allocation_clone_probe(&small);
                let large_clone = msft_allocation_clone_probe(&large);
                assert!(!size_growth_is_bounded(&small_clone, &large_clone));
                assert!(large_clone.peak() > small_clone.peak() + CONSTRUCTION_CONTROL_BYTES);
                assert!(
                    large_clone.allocated() > small_clone.allocated() + CONSTRUCTION_CONTROL_BYTES
                );
                assert!(large_clone.largest() > small_clone.largest() + CONSTRUCTION_CONTROL_BYTES);
                for cap in [32_usize, 0x4000] {
                    let funded = msft_allocation_per_attempt(cap, count)
                        .checked_mul(count.div_ceil(10))
                        .unwrap();
                    assert!(large_clone.peak() > funded);
                    assert!(large_clone.allocated() > funded);
                    assert!(large_clone.largest() > funded);
                }
            }
        }
    })
    .join()
    .unwrap();
}

#[test]
fn msft_no_slot_zero_reading_has_a_live_allocator_positive_control() {
    thread::spawn(|| {
        let bidder = (auctioneer_bidder::compiled_bidders()
            .iter()
            .find(|entry| entry.key == "msft")
            .unwrap()
            .factory)();
        let fixture = msft_allocation_fixture(21, 64, 64, 5, false);
        let context = msft_allocation_context(EntryPoint::Auction);
        let measured = msft_allocation_probe(bidder.as_ref(), &context, &fixture, 32, 0);
        assert_eq!(measured.allocated(), 0);
        let control = msft_allocation_clone_probe(&fixture.request);
        assert!(control.allocated() > 0 && control.peak() > 0 && control.largest() > 0);
    })
    .join()
    .unwrap();
}

fn msft_allocation_sdk_fixture(
    width: usize,
    widen_source: bool,
    widen_version: bool,
    escaped_source: bool,
    explicit_version: bool,
) -> MsftAllocationFixture {
    // The unknown-root and unknown-imp axes stay fixed at 64. The SDK is
    // independently widened; normal source/version strings contain NO escapes.
    let source = if widen_source {
        (if escaped_source { "\n" } else { "s" }).repeat(width)
    } else {
        "mobile".to_owned()
    };
    let version = if widen_version {
        "v".repeat(width)
    } else {
        "1".to_owned()
    };
    let sdk = json!({"source":source,"version":version});
    let display = if explicit_version {
        "explicit".to_owned()
    } else {
        format!("{source}-{version}")
    };
    let mut fixture = msft_allocation_fixture(21, 64, 64, 5, false);
    let mut input = fixture.before;
    input["app"]["ext"]["prebid"] = sdk.clone();
    if explicit_version {
        for imp in input["imp"].as_array_mut().unwrap() {
            imp["displaymanagerver"] = json!("explicit");
        }
    }
    fixture.request = serde_json::from_value(input).unwrap();
    fixture.before = serde_json::to_value(&fixture.request).unwrap();
    for chunk in &mut fixture.chunks {
        chunk.wire["app"]["ext"]["prebid"] = sdk.clone();
        for imp in chunk.wire["imp"].as_array_mut().unwrap() {
            imp["displaymanagerver"] = json!(display);
        }
        chunk.body_len = serde_json::to_vec(&chunk.wire).unwrap().len();
    }
    fixture
}

fn msft_allocation_sdk_axis(
    bidder: &dyn auctioneer_bidder::Bidder,
    context: &BidderRequestInfo<'_>,
    widen_source: bool,
    widen_version: bool,
    escaped_source: bool,
    explicit_version: bool,
) {
    let small = msft_allocation_sdk_fixture(
        64,
        widen_source,
        widen_version,
        escaped_source,
        explicit_version,
    );
    let large = msft_allocation_sdk_fixture(
        0x0004_0000,
        widen_source,
        widen_version,
        escaped_source,
        explicit_version,
    );
    for slots in [0_u32, 1, 3] {
        let small_work = msft_allocation_probe(bidder, context, &small, 32, slots);
        let large_work = msft_allocation_probe(bidder, context, &large, 32, slots);
        assert!(
            size_growth_is_bounded(&small_work, &large_work),
            "SDK source={widen_source} version={widen_version} escaped={escaped_source} \
             explicit={explicit_version} slots={slots}: small={small_work:?} large={large_work:?}"
        );
    }
}

fn msft_allocation_sdk_case(source: bool, version: bool, escaped: bool, explicit: bool) {
    // Explicit displaymanagerver needs no SDK decode. Oversized containing
    // Apps must refuse before decoding; these bounds stay input-independent.
    thread::spawn(move || {
        let bidder = (auctioneer_bidder::compiled_bidders()
            .iter()
            .find(|entry| entry.key == "msft")
            .unwrap()
            .factory)();
        let context = msft_allocation_context(EntryPoint::Auction);
        msft_allocation_sdk_axis(
            bidder.as_ref(),
            &context,
            source,
            version,
            escaped,
            explicit,
        );
    })
    .join()
    .unwrap();
}

#[test]
fn msft_unescaped_sdk_source_must_not_be_copied_before_the_first_capped_byte() {
    msft_allocation_sdk_case(true, false, false, false);
}

#[test]
fn msft_unescaped_sdk_version_must_not_be_copied_before_the_first_capped_byte() {
    msft_allocation_sdk_case(false, true, false, false);
}

#[test]
fn msft_unescaped_sdk_source_and_version_must_not_be_copied_before_the_first_capped_byte() {
    msft_allocation_sdk_case(true, true, false, false);
}

#[test]
fn msft_escaped_sdk_source_must_fit_the_same_fixed_construction_bounds() {
    msft_allocation_sdk_case(true, false, true, false);
}

fn msft_late_sdk_fixture(
    source_width: usize,
    version_width: usize,
    unknown_key_width: usize,
    repeated_escapes: bool,
    oversized_last_imp: bool,
) -> MsftAllocationFixture {
    let source = if repeated_escapes {
        "\n".repeat(source_width)
    } else {
        format!(
            "{}\n{}",
            "s".repeat(source_width >> 1),
            "s".repeat(source_width >> 1)
        )
    };
    let version = if repeated_escapes {
        "\t".repeat(version_width)
    } else {
        format!(
            "{}\t{}",
            "v".repeat(version_width >> 1),
            "v".repeat(version_width >> 1)
        )
    };
    let sdk = json!({
        "source":source,"version":version,
        (format!("{}\nignored", "k".repeat(unknown_key_width))):null,
    });
    let mut fixture = msft_allocation_fixture(21, 64, 64, 5, false);
    let mut input = fixture.before;
    input["app"]["ext"]["prebid"] = sdk.clone();
    for (index, imp) in input["imp"].as_array_mut().unwrap().iter_mut().enumerate() {
        if index != 20 {
            imp["displaymanagerver"] = json!("explicit");
        }
    }
    if oversized_last_imp {
        input["imp"][20]["payload"]["nested"][0]["text"] = json!("i".repeat(0x0004_0000));
    }
    fixture.request = serde_json::from_value(input).unwrap();
    fixture.before = serde_json::to_value(&fixture.request).unwrap();
    for (index, chunk) in fixture.chunks.iter_mut().enumerate() {
        chunk.wire["app"]["ext"]["prebid"] = sdk.clone();
        for imp in chunk.wire["imp"].as_array_mut().unwrap() {
            imp["displaymanagerver"] = if index == 2 {
                json!(format!("{source}-{version}"))
            } else {
                json!("explicit")
            };
        }
        if index == 2 && oversized_last_imp {
            chunk.wire["imp"][0]["payload"]["nested"][0]["text"] = json!("i".repeat(0x0004_0000));
        }
        chunk.body_len = serde_json::to_vec(&chunk.wire).unwrap().len();
    }
    fixture
}

#[test]
fn msft_late_escaped_sdk_decode_is_funded_with_two_retained_calls() {
    thread::spawn(|| {
        let bidder = (auctioneer_bidder::compiled_bidders()
            .iter()
            .find(|entry| entry.key == "msft")
            .unwrap()
            .factory)();
        let context = msft_allocation_context(EntryPoint::Auction);
        let cap = 0x8000;
        for source_width in [128, 4096] {
            for version_width in [64, 2048] {
                for unknown_key_width in [0, 2048] {
                    for repeated_escapes in [false, true] {
                        for oversized_last_imp in [false, true] {
                            let fixture = msft_late_sdk_fixture(
                                source_width,
                                version_width,
                                unknown_key_width,
                                repeated_escapes,
                                oversized_last_imp,
                            );
                            assert!(
                                fixture.chunks[..2]
                                    .iter()
                                    .all(|chunk| chunk.body_len <= cap)
                            );
                            assert_eq!(fixture.chunks[2].body_len > cap, oversized_last_imp);
                            let raw_bytes = fixture
                                .request
                                .app
                                .as_ref()
                                .unwrap()
                                .ext
                                .as_ref()
                                .unwrap()
                                .get()
                                .len();
                            // App must fit before decoding. The existing two
                            // scoped-supplement working copies fund 2 * (2 * cap)
                            // wire bytes; those temporaries are already gone.
                            assert!(raw_bytes <= cap);
                            assert!(raw_bytes * 3 <= cap * 4);
                            msft_allocation_probe_with_sdk(
                                bidder.as_ref(),
                                &context,
                                &fixture,
                                cap,
                                3,
                                raw_bytes,
                            );
                            if oversized_last_imp {
                                let cloned = msft_allocation_clone_probe(&fixture.request);
                                let fixed = msft_allocation_per_attempt(cap, 21) * 3;
                                assert!(cloned.peak() > fixed + raw_bytes * 3);
                                assert!(cloned.allocated() > fixed + raw_bytes * 5);
                                assert!(
                                    cloned.largest()
                                        > msft_allocation_per_attempt(cap, 21).max(raw_bytes * 2)
                                );
                            }
                        }
                    }
                }
            }
        }
    })
    .join()
    .unwrap();
}

#[test]
fn msft_explicit_display_version_must_not_decode_unescaped_sdk_source() {
    msft_allocation_sdk_case(true, false, false, true);
}

#[test]
fn msft_explicit_display_version_must_not_decode_unescaped_sdk_version() {
    msft_allocation_sdk_case(false, true, false, true);
}

#[test]
fn msft_explicit_display_version_must_not_decode_unescaped_sdk_source_and_version() {
    msft_allocation_sdk_case(true, true, false, true);
}

#[test]
fn msft_explicit_display_version_must_not_decode_escaped_sdk_source() {
    msft_allocation_sdk_case(true, false, true, true);
}
