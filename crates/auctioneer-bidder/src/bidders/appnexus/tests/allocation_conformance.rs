//! Counting-allocator conformance for the bidder-owned request transform.
#![expect(
    clippy::print_stderr,
    reason = "allocation evidence belongs in the retained Linux verification log"
)]

use crate::alloc_track::{
    self, AllowAllEndpoints, assert_bounded, construction_budget, size_growth_is_bounded,
};
use auctioneer_bidder::{
    Bidder, BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundBuildError,
    OutboundRequestSink, SinkLimits,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::{hint::black_box, thread};

const SUCCESS_CAP: usize = 2 * 1024 * 1024;

fn bidder() -> Box<dyn Bidder> {
    (auctioneer_bidder::compiled_bidders()
        .iter()
        .find(|entry| entry.key == "appnexus")
        .unwrap()
        .factory)()
}

fn nested_unknown_tree(nodes: usize) -> serde_json::Value {
    serde_json::json!({
        "branches": (0..nodes).map(|index| serde_json::json!({
            "index": index,
            "nested": [{"payload": "x".repeat(256), "flags": [true, null, false]}]
        })).collect::<Vec<_>>()
    })
}

fn request_with_unknown_trees(nodes: usize) -> BidRequest {
    serde_json::from_value(serde_json::json!({
        "id": "allocation-regression",
        "imp": [{"id": "i", "ext": {"bidder": {"placementId": 11_u64}}}],
        "user": {
            "unknown_user": nested_unknown_tree(nodes),
            "ext": {"unknown_ext": nested_unknown_tree(nodes)}
        },
        "ext": {
            "prebid": {"debug": true},
            "unknown_ext": nested_unknown_tree(nodes),
            "appnexus": {
                "keep": nested_unknown_tree(nodes),
                "placement_ids": nested_unknown_tree(nodes)
            }
        },
        "unknown_request": nested_unknown_tree(nodes)
    }))
    .unwrap()
}

fn info() -> BidderRequestInfo<'static> {
    BidderRequestInfo::new(
        BidderName::new("appnexus"),
        BidderName::new("appnexus"),
        "acct",
        None,
        "https://ib.adnxs.com/openrtb2",
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    )
}

fn tiny_cap_probe(nodes: usize) -> alloc_track::Measurement {
    thread::spawn(move || {
        let bidder = bidder();
        let request = request_with_unknown_trees(nodes);
        let info = info();
        let budget = CallBudget::new(1);
        let mut sink =
            OutboundRequestSink::new(SinkLimits::new(32, 1), &budget, &AllowAllEndpoints);

        alloc_track::start();
        let semantic = bidder.make_requests(&request, &info, &mut sink);
        let outcome = sink.into_outcome();
        let measurement = alloc_track::finish();

        eprintln!(
            "tiny cap, {nodes} nodes per tree: peak={} B, allocated={} B, largest={} B",
            measurement.peak(),
            measurement.allocated(),
            measurement.largest()
        );
        assert_bounded(&measurement, 32);
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            vec![OutboundBuildError::OversizeRaw { limit: 32 }]
        );
        assert_eq!(budget.remaining(), 1);
        measurement
    })
    .join()
    .unwrap()
}

#[test]
fn tiny_body_caps_reject_large_unknown_trees_without_copying_them() {
    for nodes in [64, 2048] {
        let _measurement = tiny_cap_probe(nodes);
    }
}

#[test]
fn tiny_body_cap_allocation_does_not_scale_with_unknown_tree_size() {
    let small = tiny_cap_probe(64);
    let large = tiny_cap_probe(2048);
    assert!(
        size_growth_is_bounded(&small, &large),
        "32x more caller nodes must not scale rejected construction: small={small:?}, large={large:?}",
    );
}

#[test]
fn exhausted_call_budget_does_not_copy_unknown_request_trees() {
    thread::spawn(|| {
        let bidder = bidder();
        let request = request_with_unknown_trees(2048);
        let info = info();
        let budget = CallBudget::new(0);
        let mut sink =
            OutboundRequestSink::new(SinkLimits::new(32, 1), &budget, &AllowAllEndpoints);

        alloc_track::start();
        let semantic = bidder.make_requests(&request, &info, &mut sink);
        let outcome = sink.into_outcome();
        let measurement = alloc_track::finish();

        eprintln!(
            "exhausted call budget: peak={} B, allocated={} B, largest={} B",
            measurement.peak(),
            measurement.allocated(),
            measurement.largest()
        );
        assert_eq!(measurement.peak(), 0, "no slot means no construction");
        assert_eq!(measurement.allocated(), 0, "not even transient copies");
        assert_eq!(measurement.largest(), 0);
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            vec![OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        );
        assert_eq!(outcome.rejected_call_count, 1);
    })
    .join()
    .unwrap();
}

fn success_probe(nodes: usize) -> alloc_track::Measurement {
    thread::spawn(move || {
        let bidder = bidder();
        let request = request_with_unknown_trees(nodes);
        let info = info();
        let budget = CallBudget::new(1);
        let mut sink =
            OutboundRequestSink::new(SinkLimits::new(SUCCESS_CAP, 1), &budget, &AllowAllEndpoints);

        alloc_track::start();
        let semantic = bidder.make_requests(&request, &info, &mut sink);
        let outcome = sink.into_outcome();
        let measurement = alloc_track::finish();

        eprintln!(
            "accepted body, {nodes} nodes per tree: peak={} B, allocated={} B, largest={} B",
            measurement.peak(),
            measurement.allocated(),
            measurement.largest()
        );
        assert_bounded(&measurement, SUCCESS_CAP);
        semantic.unwrap();
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(budget.remaining(), 0);
        let body: serde_json::Value = serde_json::from_slice(outcome.accepted[0].body()).unwrap();
        assert_eq!(
            body["ext"]["appnexus"]["placement_ids"],
            serde_json::json!([11_u64])
        );
        assert_eq!(
            body["ext"]["appnexus"]["keep"]["branches"]
                .as_array()
                .unwrap()
                .len(),
            nodes
        );
        assert!(request.ext.as_ref().unwrap().other["appnexus"]["placement_ids"].is_object());
        measurement
    })
    .join()
    .unwrap()
}

#[test]
fn successful_transformation_allocates_one_capped_body_without_copying_trees() {
    let _measurement = success_probe(256);
}

#[test]
fn successful_body_allocation_does_not_scale_with_unknown_tree_size() {
    let small = success_probe(16);
    let large = success_probe(256);
    assert!(
        size_growth_is_bounded(&small, &large),
        "16x more caller nodes at a fixed body cap must not scale construction: \
         small={small:?}, large={large:?}",
    );
}

fn caller_clone_probe(nodes: usize) -> alloc_track::Measurement {
    thread::spawn(move || {
        let request = request_with_unknown_trees(nodes);
        alloc_track::start();
        let copied = black_box(request.clone());
        let measurement = alloc_track::finish();
        drop(black_box(copied));
        measurement
    })
    .join()
    .unwrap()
}

#[test]
fn allocation_guards_detect_a_caller_tree_clone() {
    // Negative control: deliberately perform the regression in isolation.
    // Both the absolute tiny-cap guard and the differential must detect it.
    let small = caller_clone_probe(64);
    let large = caller_clone_probe(2048);
    assert!(large.peak() > construction_budget(32));
    assert!(large.allocated() > construction_budget(32));
    assert!(
        !size_growth_is_bounded(&small, &large),
        "the differential must reject an intentional caller clone: small={small:?}, large={large:?}",
    );
}
