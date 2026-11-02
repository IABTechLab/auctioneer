// Published borrowed-request transform and advisory-header allocation evidence.
// Owner fixture differences identify actual transforms, never synthetic stand-ins.
// Original sparse shapes and independent funded bounds remain unchanged.

use super::*;
use auctioneer_bidder::test_support::serving_catalog::{
    ServingBidder, default_active_publications,
};
use serde_json::{Value, json};

fn changed_root_keys(bidder: &ServingBidder) -> BTreeSet<&str> {
    let before = bidder.owner_request["ext"].as_object().unwrap();
    let after = bidder
        .owner_expected_request_body
        .as_ref()
        .expect("allocation evidence requires an owner outbound expectation")["ext"]
        .as_object()
        .unwrap();
    before
        .keys()
        .chain(after.keys())
        .map(String::as_str)
        .filter(|key| before.get(*key) != after.get(*key))
        .collect()
}

fn transforming_bidders() -> Vec<ServingBidder> {
    let bidders: Vec<_> = default_active_publications()
        .into_iter()
        .filter(|bidder| !changed_root_keys(bidder).is_empty())
        .collect();
    assert!(
        !bidders.is_empty(),
        "default-active allocation evidence must include an owner-specified root transform"
    );
    bidders
}

fn sparse_body(bidder: &ServingBidder, impressions: u16, supplement: bool) -> String {
    let name = bidder.name();
    let mut chain = json!(0_i32);
    for _ in 0_u32..120_u32 {
        chain = Value::Object(serde_json::Map::from_iter([(String::new(), chain)]));
    }
    let chains = vec![chain; 270];
    let imps: Vec<_> = (0..impressions)
        .map(|i| {
            json!({
                "id": format!("i{i}"), "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"prebid": {"bidder": {name: bidder.params}}}
            })
        })
        .collect();
    let ext = if supplement {
        json!({"prebid": {"bidderparams": {name: {"x": chains}}}})
    } else {
        json!({"x": chains})
    };
    serde_json::to_string(&json!({
        "id": "r", "site": {"id": "s"}, "imp": imps, "ext": ext
    }))
    .unwrap()
}

fn registry(bidder: &ServingBidder) -> BidderRegistry {
    let mut registry = BidderRegistry::new();
    registry
        .register(bidder.info, (bidder.factory)())
        .expect("unique test bidder");
    registry
}

fn endpoint_macro_names(bidder: &ServingBidder) -> Vec<&str> {
    bidder
        .info
        .endpoint_macros
        .iter()
        .map(String::as_str)
        .collect()
}

fn scoped_provider(
    bidder: &ServingBidder,
    grouped: &GroupedBidders,
    registry: &BidderRegistry,
    max_nodes: u32,
    max_bytes: usize,
) -> Result<PerBidderRequest, AuctionError> {
    let survivors = [BidderName::new(bidder.name())];
    grouped.ensure_supplement_capacity(&survivors, max_nodes, max_bytes)?;
    Ok(materialize_per_bidder(
        grouped,
        &survivors,
        &AliasMap::default(),
        registry,
        &BTreeSet::new(),
    )?
    .pop()
    .unwrap())
}

// Compare only owner-specified changes: the sparse input intentionally omits
// unchanged owner fields and adds its independently sized extension tree.
fn assert_changed_fields(
    before: Option<&Value>,
    expected: Option<&Value>,
    actual: Option<&Value>,
) -> usize {
    if before == expected {
        return 0;
    }
    if let (Some(Value::Object(before_fields)), Some(Value::Object(expected_fields))) =
        (before, expected)
    {
        let keys: BTreeSet<_> = before_fields.keys().chain(expected_fields.keys()).collect();
        return keys
            .into_iter()
            .map(|key| {
                assert_changed_fields(
                    before_fields.get(key),
                    expected_fields.get(key),
                    actual.and_then(|value| value.get(key)),
                )
            })
            .sum();
    }
    assert_eq!(
        actual, expected,
        "sparse outbound must perform the owner transform"
    );
    1
}

fn assert_root_transform(bidder: &ServingBidder, body: &[u8]) {
    let actual: Value = serde_json::from_slice(body).unwrap();
    assert!(
        assert_changed_fields(
            bidder.owner_request.get("ext"),
            bidder
                .owner_expected_request_body
                .as_ref()
                .unwrap()
                .get("ext"),
            actual.get("ext"),
        ) > 0,
        "the measured factory must perform an independently expected transform"
    );
}

#[test]
fn copied_advisory_headers_on_borrowed_published_transform_fit_the_funded_charge() {
    fn peak_with_headers(bidder: ServingBidder, copy_cap: u32) -> u64 {
        let body = sparse_body(&bidder, 1, false);
        thread::spawn(move || {
            let ua = "u".repeat(0x0001_0000);
            let referer = format!("https://example.test/{}", "p".repeat(0x0001_0000));
            let registry = registry(&bidder);
            let adapter = (bidder.factory)();
            let endpoint_macros = endpoint_macro_names(&bidder);
            let host_allowlist = bidder
                .info
                .host_allowlist
                .as_ref()
                .map(|hosts| hosts.iter().map(String::as_str).collect::<Vec<_>>());
            let before = alloc_track::live();
            alloc_track::reset_peak();
            let request: BidRequest =
                bounded_json::from_slice_within(body.as_bytes(), NODE_CAP).unwrap();
            let normalized = auctioneer_openrtb::prepare(
                request,
                &auctioneer_openrtb::RequestHeaders {
                    user_agent: Some(&ua),
                    referer: Some(&referer),
                    ..Default::default()
                },
                &auctioneer_openrtb::PrepareLimits {
                    max_copied_header_bytes: copy_cap,
                    ..Default::default()
                },
            )
            .unwrap();
            let underlying: &BidRequest = &normalized;
            let grouped = group_by_bidder(
                underlying,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
            let scoped = scoped_provider(
                &bidder,
                &grouped,
                &registry,
                NODE_CAP,
                usize::try_from(BODY_CAP).unwrap(),
            )
            .unwrap();
            let info = BidderRequestInfo::new(
                BidderName::new(bidder.name()),
                BidderName::new(bidder.name()),
                "acct",
                None,
                &bidder.info.endpoint,
                &endpoint_macros,
                host_allowlist.as_deref(),
                None,
                EntryPoint::Auction,
                &[],
            );
            let budget = CallBudget::new(1);
            let validator = ExchangeEndpointValidator::new(bidder.info);
            let mut sink = OutboundRequestSink::new(
                SinkLimits::new(usize::try_from(BODY_CAP).unwrap(), 100),
                &budget,
                &validator,
            );
            adapter
                .make_requests(&scoped.request, &info, &mut sink)
                .unwrap();
            let outcome = sink.into_outcome();
            assert_eq!(outcome.accepted.len(), 1);
            black_box((&normalized, underlying, &grouped, &scoped, &outcome));
            let measured = alloc_track::peak_since(before);
            assert_root_transform(&bidder, outcome.accepted[0].body());
            measured
        })
        .join()
        .unwrap()
    }
    for bidder in transforming_bidders() {
        let copied = 0x0001_0000;
        let baseline = peak_with_headers(bidder.clone(), 0);
        let with_headers = peak_with_headers(bidder, copied);
        let extra = with_headers
            .checked_sub(baseline)
            .expect("copied headers must not reduce the request peak");
        // Normalized, grouped Value and scoped request retain the headers;
        // the transform borrows the scope. The sink buffer exists in both runs.
        let three_copies = 3 * u64::from(copied);
        assert!(
            (three_copies..=three_copies + 4_096).contains(&extra),
            "borrowed default-active transform must retain three header copies plus field/container \
             overhead: baseline {baseline}, with headers {with_headers}, extra {extra}"
        );
        // Production still funds five copies; this optimization does not lower it.
        assert!(
            extra <= 5 * u64::from(copied) + 4_096,
            "advisory peak {extra} must fit the funded five-copy bound plus overhead"
        );
    }
}

#[test]
fn published_transform_copies_a_sparse_extension_at_most_once() {
    for bidder in transforming_bidders() {
        let body = sparse_body(&bidder, 1, false);
        thread::spawn(move || {
            let request: BidRequest = bounded_json::from_slice_within(body.as_bytes(), NODE_CAP).unwrap();
            let registry = registry(&bidder);
            let adapter = (bidder.factory)();
            let endpoint_macros = endpoint_macro_names(&bidder);
            let host_allowlist = bidder.info.host_allowlist.as_ref().map(|hosts| {
                hosts.iter().map(String::as_str).collect::<Vec<_>>()
            });
            let grouped = group_by_bidder(
                &request, &AliasMap::default(), &registry, &BTreeSet::new(), &BTreeSet::new(),
            ).unwrap();
            let scoped = scoped_provider(&bidder, &grouped, &registry, NODE_CAP, usize::try_from(BODY_CAP).unwrap()).unwrap();
            let info = BidderRequestInfo::new(
                BidderName::new(bidder.name()), BidderName::new(bidder.name()), "acct", None,
                &bidder.info.endpoint, &endpoint_macros, host_allowlist.as_deref(),
                None, EntryPoint::Auction, &[],
            );
            let budget = CallBudget::new(1);
            let validator = ExchangeEndpointValidator::new(bidder.info);
            let before = alloc_track::live();
            alloc_track::reset_peak();
            let mut sink = OutboundRequestSink::new(SinkLimits::new(usize::try_from(BODY_CAP).unwrap(), 100), &budget, &validator);
            adapter.make_requests(&scoped.request, &info, &mut sink).unwrap();
            let outcome = sink.into_outcome();
            let measured = alloc_track::peak_since(before);
            let one_tree_and_sink = u64::from(NODE_CAP) * NODE_RETAINED_BYTES + 4 * BODY_CAP + 0x4000;
            assert_eq!(outcome.accepted.len(), 1);
            assert!(measured <= one_tree_and_sink,
                "default-active transform peak {measured} exceeds one sparse tree plus sink {one_tree_and_sink}");
            for namespace in changed_root_keys(&bidder) {
                assert!(!scoped.request.ext.as_ref().unwrap().other.contains_key(namespace));
            }
            assert_root_transform(&bidder, outcome.accepted[0].body());
        }).join().unwrap();
    }
}
