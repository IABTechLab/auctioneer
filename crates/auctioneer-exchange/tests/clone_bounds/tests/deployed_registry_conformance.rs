// Deployed-registry metadata conformance at the unchanged inbound-byte ceiling.
// Retains the baked roster, host overlay, independent allocation windows and caps.
// The shared grouping control uses a separate synthetic roster.

use super::*;

fn alias_base() -> &'static str {
    auctioneer_config_data::bidders()
        .iter()
        .min_by_key(|info| (info.name.len(), info.name.as_str()))
        .unwrap()
        .name
        .as_str()
}

fn dense_fixture(participants: usize, host: &BTreeMap<String, String>) -> Value {
    let reserved: BTreeSet<&str> = host
        .keys()
        .map(String::as_str)
        .chain(
            auctioneer_config_data::bidders()
                .iter()
                .map(|info| info.name.as_str()),
        )
        .collect();
    let aliases: BTreeMap<String, &str> = (0..)
        .map(shortest_wire_alias)
        .filter(|name| !reserved.contains(name.as_str()))
        .take(participants)
        .map(|name| (name, alias_base()))
        .collect();
    assert_eq!(aliases.len(), participants, "distinct minimal identities");
    let mut bidders: BTreeMap<String, Value> = aliases
        .keys()
        .map(|name| (name.clone(), json!(0_i32)))
        .collect();
    for name in host.keys() {
        bidders.insert(name.clone(), json!(0_i32));
    }
    for bidder in auctioneer_config_data::bidders() {
        bidders.insert(bidder.name.clone(), json!(0_i32));
    }
    json!({
        "id": "r", "site": {"id": "s"}, "padding": "",
        "imp": [{"id": "i", "banner": {"w": 1_i32, "h": 1_i32}, "ext": {"bidder": bidders}}],
        "ext": {"prebid": {"aliases": aliases}}
    })
}

#[test]
fn dense_fixture_does_not_redeclare_reserved_names() {
    let host = BTreeMap::from([(shortest_wire_alias(0), alias_base().to_owned())]);
    let fixture = dense_fixture(100, &host);
    let aliases = fixture["ext"]["prebid"]["aliases"].as_object().unwrap();
    assert_eq!(aliases.len(), 100);
    for name in host.keys().map(String::as_str).chain(
        auctioneer_config_data::bidders()
            .iter()
            .map(|info| info.name.as_str()),
    ) {
        assert!(!aliases.contains_key(name), "reserved name {name}");
    }
}

/// Pack distinct short aliases until the next declaration/membership pair no
/// longer fits, then pad to the exact input ceiling. Host aliases and the full
/// baked registry also participate; no request-alias count cap is introduced.
fn maximum_cardinality_fixture(host: &BTreeMap<String, String>) -> (BidRequest, usize) {
    let mut low = 0_usize;
    let mut high = INBOUND_BYTES;
    while low.checked_add(1).unwrap() < high {
        let mid = low.midpoint(high);
        if serde_json::to_vec(&dense_fixture(mid, host)).unwrap().len() <= INBOUND_BYTES {
            low = mid;
        } else {
            high = mid;
        }
    }
    let mut value = dense_fixture(low, host);
    let used = serde_json::to_vec(&value).unwrap().len();
    value["padding"] = json!("x".repeat(INBOUND_BYTES.checked_sub(used).unwrap()));
    assert_eq!(serde_json::to_vec(&value).unwrap().len(), INBOUND_BYTES);
    assert!(
        serde_json::to_vec(&dense_fixture(low.checked_add(1).unwrap(), host))
            .unwrap()
            .len()
            > INBOUND_BYTES
    );
    (serde_json::from_value(value).unwrap(), low)
}

#[test]
fn grouping_factor_guard_at_input_derived_maximum_cardinality() {
    thread::spawn(|| {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = BidderRegistry::new();
    for info in auctioneer_config_data::bidders() {
        registry.register(info, Box::new(NoRequests(Arc::clone(&calls)))).expect("unique test bidder");
    }
    let host: BTreeMap<String, String> = (0..HOST_ALIASES)
        .map(|n| (format!("fake_{n}"), alias_base().into())).collect();
    let (req, participants) = maximum_cardinality_fixture(&host);
    assert!(participants > 10_000, "input-derived, not a small fixed roster");
    let disabled = BTreeSet::new();

    // Independently measure the ONE Value tree, charged to inbound parsing,
    // then subtract only that known retained allocation from grouping peak.
    let value_baseline = alloc_track::live();
    let value = serde_json::to_value(&req).unwrap();
    let value_bytes = u64::try_from(alloc_track::live() - value_baseline).unwrap();
    drop(value);
    let baseline = alloc_track::reset_peak();
    let aliases = resolve_aliases(&req, &registry, &disabled, &host).unwrap();
    let grouped = group_by_bidder(&req, &aliases.map, &registry, &disabled,
        &aliases.dropped_alias_names).unwrap();
    let group_peak = alloc_track::peak_since(baseline);
    let bookkeeping_peak = group_peak.checked_sub(value_bytes).unwrap();
    let count = grouped.bidder_imps().len();
    assert_eq!(count, participants + HOST_ALIASES + auctioneer_config_data::bidders().len());
    assert!(grouped.bidder_imps().values().all(|indices| indices == &[0]));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let group_allowance = u64::try_from(INBOUND_BYTES).unwrap() * GROUPING_FACTOR;
    assert!(bookkeeping_peak <= group_allowance,
        "{count} identities: grouping/aliases {bookkeeping_peak} > {group_allowance}");

    let mut one_clone = 0;
    for cap in [0_usize, 1, 8] {
        let survivors: Vec<_> = grouped.bidder_imps().keys().take(cap).cloned().collect();
        let clone_baseline = alloc_track::reset_peak();
        let requests = materialize_per_bidder(&grouped, &survivors, &aliases.map,
            &registry, &aliases.dropped_alias_names).unwrap();
        let peak = alloc_track::peak_since(clone_baseline);
        let retained = u64::try_from(alloc_track::live() - clone_baseline).unwrap();
        assert_eq!(requests.len(), cap);
        if cap == 0 {
            assert_eq!(retained, 0, "grouping has no eager typed requests");
            assert_eq!(peak, 0, "no projection scratch for an empty survivor set");
        } else if cap == 1 {
            one_clone = retained;
        } else {
            let cap_u64 = u64::try_from(cap).unwrap();
            assert!(retained.abs_diff(one_clone * cap_u64) < 4096,
                "retention must scale with survivors, not {count} participants");
            assert!(peak < retained + one_clone * 3,
                "projection scratch is bounded independently of participant count");
        }
        let funded = u64::try_from(cap * INBOUND_BYTES).unwrap() * TYPED_CLONE_FACTOR;
        writeln!(io::stderr(), "{INBOUND_BYTES} input bytes, {count} identities: grouping={bookkeeping_peak}/{group_allowance}, cap={cap}, typed retained={retained}, peak={peak}, typed-clone allowance (full peak guarded separately)={funded}").unwrap();
    }
}).join().unwrap();
}
