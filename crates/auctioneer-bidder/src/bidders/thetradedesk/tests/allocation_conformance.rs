//! Counting-allocator conformance for borrowed settings and request construction.

mod response_allocation_conformance;

use crate::alloc_track::{self, AllowAllEndpoints, construction_budget, size_growth_is_bounded};
use auctioneer_bidder::{
    BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundBuildError, OutboundRequestSink,
    SinkLimits, outbound_meta_bytes,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::json;
use std::{hint::black_box, thread};

#[test]
fn typed_settings_binding_and_field_resolution_allocate_nothing() {
    use auctioneer_bidder::{BidderSettings, bidder_settings::thetradedesk};
    for width in [1, 64, 4_096] {
        thread::spawn(move || {
            let base: BidderSettings = serde_json::from_value(json!({
                "implementation":"thetradedesk", "values":{"supply_id":"x".repeat(width)}
            }))
            .unwrap();
            let alias: BidderSettings = serde_json::from_value(json!({
                "implementation":"thetradedesk", "values":{}
            }))
            .unwrap();
            let original = base
                .as_thetradedesk()
                .unwrap()
                .supply_id
                .as_ref()
                .unwrap()
                .as_ptr();
            let mut context = BidderRequestInfo::new(
                BidderName::new("ttd"),
                BidderName::new("thetradedesk"),
                "acct",
                None,
                "https://bidder.example.test/bid/{{.SupplyId}}",
                &["SupplyId"],
                None,
                None,
                EntryPoint::Auction,
                &[],
            );
            alloc_track::start();
            let bound = context.set_settings(Some("thetradedesk"), Some(&alias), Some(&base));
            let resolved = thetradedesk::resolve(
                context.settings().and_then(BidderSettings::as_thetradedesk),
                context
                    .base_settings()
                    .and_then(BidderSettings::as_thetradedesk),
            );
            let value = black_box(resolved.supply_id());
            let measured = alloc_track::finish();
            bound.unwrap();
            assert_eq!(value.unwrap().as_ptr(), original);
            assert_eq!(value.unwrap().len(), width);
            assert_eq!(measured.allocated(), 0);
            assert_eq!(measured.peak(), 0);
            alloc_track::start();
            let clone = black_box(base.clone());
            let clone_measurement = alloc_track::finish();
            assert!(clone_measurement.allocated() >= width);
            assert_eq!(
                clone
                    .as_thetradedesk()
                    .unwrap()
                    .supply_id
                    .as_ref()
                    .unwrap()
                    .len(),
                width
            );
        })
        .join()
        .unwrap();
    }
}

fn ttd_allocation_request(
    count: usize,
    root_width: usize,
    imp_width: usize,
    supply_from_settings: bool,
) -> BidRequest {
    serde_json::from_value(json!({
        "id":"allocation",
        "payload":"r".repeat(root_width),
        "site":{"publisher":{"id":"old-publisher"}},
        "imp":(0..count).map(|index| json!({
            "id":format!("i-{index}"),
            "banner":{"w":300_i32,"h":250_i32},
            "payload":"i".repeat(imp_width),
            "ext":{"bidder":{
                "publisherId":"p",
                "supplySourceId":if supply_from_settings { "" } else { "s" },
            }},
        })).collect::<Vec<_>>(),
    }))
    .unwrap()
}

fn ttd_allocation_construction_budget(cap: usize, count: usize) -> usize {
    // Only adjust the existing one-call bound for the actual imp-id table.
    // No new control allowance or allocator-derived expectation.
    let extra_metadata = outbound_meta_bytes(u16::try_from(count).unwrap())
        .checked_sub(outbound_meta_bytes(1))
        .unwrap();
    construction_budget(cap)
        .checked_add(usize::try_from(extra_metadata).unwrap())
        .unwrap()
}

fn ttd_construction_probe(
    bidder: &dyn auctioneer_bidder::Bidder,
    context: &BidderRequestInfo<'_>,
    request: &BidRequest,
    cap: usize,
    slots: u32,
) -> alloc_track::Measurement {
    let count = request.imp.len();
    let before = serde_json::to_value(request).unwrap();
    let mut expected = before.clone();
    expected["site"]["publisher"]["id"] = json!("p");
    let expected_bytes = serde_json::to_vec(&expected).unwrap().len();
    let funded = ttd_allocation_construction_budget(cap, count);
    let budget = CallBudget::new(slots);
    let mut sink = OutboundRequestSink::new(
        SinkLimits::new(cap, u16::try_from(count).unwrap()),
        &budget,
        &AllowAllEndpoints,
    );
    alloc_track::start();
    let semantic = bidder.make_requests(request, context, &mut sink);
    let outcome = sink.into_outcome();
    let measured = alloc_track::finish();

    semantic.unwrap();
    assert!(
        measured.peak() <= funded && measured.allocated() <= funded && measured.largest() <= funded,
        "count={count} slots={slots} cap={cap}: {measured:?}"
    );
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(outcome.suppressed_error_count, 0);
    if slots == 0 {
        assert_eq!(measured.peak(), 0);
        assert_eq!(measured.allocated(), 0);
        assert_eq!(measured.largest(), 0);
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        );
        assert_eq!(outcome.rejected_call_count, 1);
        assert_eq!(budget.remaining(), 0);
    } else if expected_bytes > cap {
        assert!(measured.peak() > 0);
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::OversizeRaw {
                limit: u64::try_from(cap).unwrap(),
            }]
        );
        assert_eq!(outcome.rejected_call_count, 0);
        assert_eq!(budget.remaining(), 1, "oversize must refund its slot");
    } else {
        assert!(measured.peak() > 0);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.rejected_call_count, 0);
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(budget.remaining(), 0);
        assert_ttd_emitted_call(
            &outcome.accepted[0],
            request,
            &expected,
            expected_bytes,
            cap,
        );
    }
    assert_eq!(serde_json::to_value(request).unwrap(), before);
    measured
}

fn assert_ttd_emitted_call(
    call: &auctioneer_bidder::BidderHttpRequest,
    request: &BidRequest,
    expected: &serde_json::Value,
    expected_bytes: usize,
    cap: usize,
) {
    assert_eq!(call.method(), "POST");
    assert_eq!(call.url(), "https://bidder.example.test/bid/s");
    assert_eq!(call.body().len(), expected_bytes);
    assert!(call.body().len() <= cap);
    assert_eq!(
        call.covered_imp_ids().collect::<Vec<_>>(),
        request
            .imp
            .iter()
            .map(|imp| imp.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &serde_json::from_slice::<serde_json::Value>(call.body()).unwrap(),
        expected
    );
}

fn assert_ttd_payload_grid(
    bidder: &dyn auctioneer_bidder::Bidder,
    context: &BidderRequestInfo<'_>,
    count: usize,
    slots: u32,
    cap: usize,
    supply_from_settings: bool,
) {
    let mut observations = Vec::with_capacity(4);
    for root_width in [64_usize, 0x0004_0000] {
        for imp_width in [64_usize, 0x0004_0000] {
            let request =
                ttd_allocation_request(count, root_width, imp_width, supply_from_settings);
            for imp in &request.imp {
                assert_eq!(
                    imp.ext.as_ref().unwrap().bidder.as_ref().unwrap()["supplySourceId"],
                    if supply_from_settings { "" } else { "s" }
                );
            }
            observations.push(ttd_construction_probe(
                bidder, context, &request, cap, slots,
            ));
        }
    }
    // Row-major [small/small, small/large, large/small, large/large].
    for (small, large) in [(0, 1), (2, 3), (0, 2), (1, 3)] {
        assert!(
            size_growth_is_bounded(&observations[small], &observations[large]),
            "edge {small}->{large}, count={count} slots={slots} cap={cap} settings={supply_from_settings}: {observations:?}"
        );
    }
}

#[test]
fn thetradedesk_construction_is_bounded_across_independent_payload_axes() {
    use auctioneer_bidder::BidderSettings;
    use auctioneer_bidder::compiled_bidders;

    thread::spawn(|| {
        let bidder = (compiled_bidders()
            .iter()
            .find(|entry| entry.key == "thetradedesk")
            .unwrap()
            .factory)();
        for supply_from_settings in [false, true] {
            let settings: BidderSettings = serde_json::from_value(json!({
                "implementation":"thetradedesk",
                "values":{"supply_id":if supply_from_settings { "s" } else { "ignored" }},
            }))
            .unwrap();
            let mut context = BidderRequestInfo::new(
                BidderName::new("ttd"),
                BidderName::new("thetradedesk"),
                "account",
                None,
                "https://bidder.example.test/bid/{{.SupplyId}}",
                &["SupplyId"],
                None,
                None,
                EntryPoint::Auction,
                &[],
            );
            context
                .set_settings(Some("thetradedesk"), Some(&settings), None)
                .unwrap();
            for count in [1_usize, 8] {
                for slots in [0_u32, 1] {
                    for cap in [32_usize, 0x4000] {
                        assert_ttd_payload_grid(
                            bidder.as_ref(),
                            &context,
                            count,
                            slots,
                            cap,
                            supply_from_settings,
                        );
                    }
                }
            }
        }
    })
    .join()
    .unwrap();
}

#[test]
fn thetradedesk_payload_differentials_detect_intentional_caller_clones() {
    thread::spawn(|| {
        for count in [1_usize, 8] {
            for widen_imp in [false, true] {
                let mut observations = Vec::with_capacity(2);
                for width in [64_usize, 0x0004_0000] {
                    let request = ttd_allocation_request(
                        count,
                        if widen_imp { 64 } else { width },
                        if widen_imp { width } else { 64 },
                        false,
                    );
                    alloc_track::start();
                    let cloned = black_box(request.clone());
                    let measured = alloc_track::finish();
                    drop(black_box(cloned));
                    observations.push(measured);
                }
                assert!(
                    !size_growth_is_bounded(&observations[0], &observations[1]),
                    "clone control must fail the same differential: count={count} \
                     imp={widen_imp}: {observations:?}"
                );
                for cap in [32_usize, 0x4000] {
                    let funded = ttd_allocation_construction_budget(cap, count);
                    assert!(observations[1].peak() > funded);
                    assert!(observations[1].allocated() > funded);
                    assert!(observations[1].largest() > funded);
                }
            }
        }
    })
    .join()
    .unwrap();
}
