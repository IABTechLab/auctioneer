//! Measures the real auction path before adapter allocations can hide clone retention.
//!
//! The dense guard certifies grouping bookkeeping and survivor population, not
//! the typed byte factor. Historical fixture measurements cover retained clones,
//! not the full projection peak.
//! The newer factor is independently guarded by `projection_charge.rs`.

use auctioneer_exchange::test_support as support;

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    mod deployed_registry_conformance;
    use super::alloc_track;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRegistry, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta,
    };
    use auctioneer_config_data::types::{AccountConfig, BidderInfo};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::split::{group_by_bidder, materialize_per_bidder};
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, resolve_aliases, run_auction,
    };
    use auctioneer_openrtb::{
        PrepareLimits, RequestHeaders, prepare, types::bid_request::BidRequest,
    };
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::budget::{GROUPING_FACTOR, TYPED_CLONE_FACTOR};
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::{self, Write as _};
    use std::sync::{
        Arc, LazyLock,
        atomic::{AtomicUsize, Ordering},
    };
    use std::thread;

    struct NoRequests(Arc<AtomicUsize>);

    impl Bidder for NoRequests {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            _: &BidRequest,
            _: &BidderRequestInfo<'_>,
            _sink: &mut auctioneer_bidder::OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn make_bids(
            &self,
            _: &BidRequest,
            _: &OutboundRequestMeta,
            _: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            Err(vec![BidderError::failed_to_request_bids(
                "unexpected response",
            )])
        }
    }

    struct NoFanout;

    #[async_trait(?Send)]
    impl BidderFanout for NoFanout {
        async fn send_all(
            &self,
            _: Vec<BidderCall>,
            _: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Err(FanoutInternalError::Internal(
                "unexpected outbound requests".into(),
            ))
        }
    }

    fn fixture(participants: usize, padding: usize) -> BidRequest {
        // Each renamed key occurs in both membership and alias declarations.
        let renamed_key_bytes = participants.checked_mul(2 * (5 - 1)).unwrap();
        let payload_padding = padding.checked_sub(renamed_key_bytes).unwrap();
        let aliases: BTreeMap<String, &str> = (0..participants)
            .map(|n| (format!("fake_{n:05}"), "fake_bb"))
            .collect();
        let bidders: BTreeMap<&str, Value> = aliases
            .keys()
            .map(|name| (name.as_str(), json!({})))
            .collect();
        serde_json::from_value(json!({
        "id": "r", "site": {"id": "s"}, "payload": "x".repeat(payload_padding),
        "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32}, "ext": {"bidder": bidders}}],
        "ext": {"prebid": {"aliases": aliases}}
    }))
    .unwrap()
    }

    fn registry(calls: &Arc<AtomicUsize>) -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: "fake_bb".into(),
                    endpoint: "https://bidder.example.test/".into(),
                    ..BidderInfo::default()
                })),
                Box::new(NoRequests(Arc::clone(calls))),
            )
            .expect("unique test bidder");
        registry
    }

    fn live_peak(participants: usize, cap: usize) -> u64 {
        thread::spawn(move || {
            let calls = Arc::new(AtomicUsize::new(0));
            let registry = registry(&calls);
            let req = prepare(
                fixture(participants, 128 * 1024),
                &RequestHeaders::default(),
                &PrepareLimits {
                    max_impressions: 100_u32,
                    ..PrepareLimits::default()
                },
            )
            .unwrap();
            let disabled = BTreeSet::new();
            let aliases = resolve_aliases(&req, &registry, &disabled, &BTreeMap::new()).unwrap();
            let mut host = auctioneer_config_data::host_default().clone();
            host.resource_limits.max_bidders_per_auction = cap;
            host.resource_limits.default_tmax_ms = 1000;
            host.resource_limits.max_tmax_ms = 5000;
            let plan = AuctionPlan::new(
                aliases,
                disabled,
                response_budgets(&registry, &host),
                BTreeMap::new(),
            );
            let config = AccountConfig::default();
            let account = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &config,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                &host,
                &account,
                &registry,
                &NoFanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let baseline = alloc_track::reset_peak();
            let mut audit = DisclosureAudit::default();
            let response = block_on(run_auction(&req, &ctx, &plan, &mut audit)).unwrap();
            let peak = alloc_track::peak_since(baseline);
            assert_eq!(calls.load(Ordering::SeqCst), cap);
            drop(response);
            peak
        })
        .join()
        .unwrap()
    }

    #[test]
    fn live_clone_peak_scales_with_cap_not_participants() {
        let small = live_peak(16, 1);
        let large = live_peak(128, 1);
        let capped = live_peak(128, 8);
        writeln!(
            io::stderr(),
            "live peaks: 16/cap1={small}, 128/cap1={large}, 128/cap8={capped}"
        )
        .unwrap();
        assert!(
            large < small + 512 * 1024,
            "112 additional names must not retain 112 whole requests: {small} -> {large}"
        );
        assert!(
            capped > large,
            "survivor requests, not just names, are measured"
        );
        assert!(
            capped < large + 2 * 1024 * 1024,
            "only seven additional clones: {large} -> {capped}"
        );
    }

    const INBOUND_BYTES: usize = 256 * 1024;
    // This baseline has no max_aliases setting. Exercise a 64-entry host overlay
    // without claiming it is an enforced maximum; request cardinality is input-derived.
    const HOST_ALIASES: usize = 64;

    // Adversarial minimum-length wire keys maximize cardinality at a fixed
    // byte ceiling. These are not provider names or ordinary fixture bidders.
    fn shortest_wire_alias(mut n: usize) -> String {
        let mut digits = Vec::new();
        loop {
            digits.push(char::from(
                b'!'.checked_add(u8::try_from(n.checked_rem(90).unwrap()).unwrap())
                    .unwrap(),
            ));
            n = n.checked_div(90).unwrap();
            if n == 0 {
                break;
            }
        }
        // Skip JSON escape characters; aliases still deliberately include / and ~.
        digits
            .into_iter()
            .map(|c| match c {
                '"' => '~',
                '\\' => '{',
                other => other,
            })
            .collect()
    }

    fn synthetic_infos() -> &'static [BidderInfo] {
        static INFOS: LazyLock<Vec<BidderInfo>> = LazyLock::new(|| {
            ["fake_aaa", "fake_bb", "fake_ccc"]
                .into_iter()
                .map(|name| BidderInfo {
                    name: name.into(),
                    ..BidderInfo::default()
                })
                .collect()
        });
        &INFOS
    }

    fn dense_fixture(participants: usize, host: &BTreeMap<String, String>) -> Value {
        let reserved: BTreeSet<&str> = host
            .keys()
            .map(String::as_str)
            .chain(synthetic_infos().iter().map(|info| info.name.as_str()))
            .collect();
        let aliases: BTreeMap<String, &str> = (0..)
            .map(shortest_wire_alias)
            .filter(|name| !reserved.contains(name.as_str()))
            .take(participants)
            .map(|name| (name, "fake_bb"))
            .collect();
        assert_eq!(aliases.len(), participants, "distinct minimal identities");
        let mut bidders: BTreeMap<String, Value> = aliases
            .keys()
            .map(|name| (name.clone(), json!(0_i32)))
            .collect();
        for name in host.keys() {
            bidders.insert(name.clone(), json!(0_i32));
        }
        for bidder in synthetic_infos() {
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
        let host = BTreeMap::from([(shortest_wire_alias(0), "fake_bb".to_owned())]);
        let fixture = dense_fixture(100, &host);
        let aliases = fixture["ext"]["prebid"]["aliases"].as_object().unwrap();
        assert_eq!(aliases.len(), 100);
        for name in host
            .keys()
            .map(String::as_str)
            .chain(synthetic_infos().iter().map(|info| info.name.as_str()))
        {
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
        for info in synthetic_infos() {
            registry.register(info, Box::new(NoRequests(Arc::clone(&calls)))).expect("unique test bidder");
        }
        let host: BTreeMap<String, String> = (0..HOST_ALIASES)
            .map(|n| (format!("fake_{n}"), "fake_bb".into())).collect();
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
        assert_eq!(count, participants + HOST_ALIASES + synthetic_infos().len());
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
}
