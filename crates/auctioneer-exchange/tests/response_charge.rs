//! Peak allocation of synthetic bidder decoding and live exchange response assembly.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track;
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use auctioneer_bidder::{
        Bidder as _, BidderHttpResponse, BidderName, BidderRegistry, OutboundRequestMetaBuilder,
    };
    use auctioneer_config_data::types::MAX_BIDDER_NAME_BYTES;
    use auctioneer_exchange::construction_costs;
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::targeting::Winner;
    use auctioneer_exchange::{
        AuctionDeadline, AuctionError, BidderCall, BidderCallResult, BidderFanout,
        BidderResultBundle, FanoutInternalError, ResponseInput, build_response,
    };
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::{NormalizedBidRequest, PrepareLimits, RequestHeaders, prepare};
    use auctioneer_runtime::budget::{
        ASSEMBLY_FACTOR, Derived, HEADER_MAP_CHARGE, MAX_GVL_PARSED_BYTES,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        RESPONSE_ASSEMBLY_FACTOR, RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits,
        TargetPlatformConstants, derive_isolate_budget,
    };
    use auctioneer_runtime::count_json_nodes;
    use auctioneer_runtime::debug_budget::debug_peak_bytes;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::io::{self, Write as _};
    use std::iter;
    use std::mem::take;
    use std::slice::from_ref;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    struct MeasuredFanout {
        body: Vec<u8>,
        baseline: AtomicI64,
    }

    struct NoDispatch;

    #[test]
    fn zero_call_diagnostic_owners_and_wire_rows_fit_their_separate_context_charge() {
        use auctioneer_bidder::{ConstructionDiagnostic, ConstructionDiagnostics};
        use auctioneer_exchange::construction_diagnostics::construction_diagnostic_costs;

        for names in [1_u32, 16, 50] {
            for rows in [0_usize, 1, 32, 33, 1_000] {
                thread::spawn(move || {
                    let baseline = alloc_track::reset_peak();
                    let mut diagnostics = BTreeMap::new();
                    for index in 0..names {
                        let mut owner = ConstructionDiagnostics::default();
                        let before_recording = alloc_track::live();
                        for _ in 0..rows {
                            owner.push(ConstructionDiagnostic::InvalidImpressionParameters);
                        }
                        assert_eq!(alloc_track::live(), before_recording);
                        assert_eq!(owner.len(), rows.min(32));
                        let name = format!("fake_{index:059}");
                        assert_eq!(name.len(), MAX_BIDDER_NAME_BYTES);
                        diagnostics.insert(BidderName::new(name), owner);
                    }
                    let response = build_response(ResponseInput {
                        bidder_visible_tmax_ms: 100,
                        auction_timestamp_ms: 0,
                        return_all_bid_status: false,
                        seatnonbid: BTreeMap::new(),
                        bundles: BTreeMap::new(),
                        construction_diagnostics: diagnostics,
                        prebid_errors: vec![],
                        prebid_warnings: vec![],
                    })
                    .unwrap();
                    let ext = response.ext.as_ref().unwrap();
                    if rows == 0 {
                        assert!(ext.get("errors").is_none());
                    } else {
                        let wire = ext["errors"].as_object().unwrap();
                        assert_eq!(wire.len(), usize::try_from(names).unwrap());
                        assert!(
                            wire.values()
                                .all(|entries| entries.as_array().unwrap().len() == rows.min(32))
                        );
                    }
                    serde_json::to_writer(io::sink(), &response).unwrap();
                    let peak = alloc_track::peak_since(baseline);
                    let reserve = construction_diagnostic_costs(names).context_per_auction_bytes;
                    // The literal equation is independent of the production constants.
                    assert_eq!(reserve, 4_096 + u64::from(names) * 98_304);
                    assert!(peak > 0, "the allocator has a real positive control");
                    assert!(
                        peak <= reserve,
                        "names={names} rows={rows} peak={peak} reserve={reserve}"
                    );
                })
                .join()
                .unwrap();
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl auctioneer_exchange::BidderFanout for NoDispatch {
        async fn send_all(
            &self,
            _: Vec<auctioneer_exchange::BidderCall>,
            _: auctioneer_exchange::AuctionDeadline,
        ) -> Result<
            Vec<auctioneer_exchange::BidderCallResult>,
            auctioneer_exchange::FanoutInternalError,
        > {
            Err(auctioneer_exchange::FanoutInternalError::Internal(
                "unexpected dispatch of privacy-denied bidders".into(),
            ))
        }
    }

    #[async_trait::async_trait(?Send)]
    impl auctioneer_exchange::BidderFanout for MeasuredFanout {
        async fn send_all(
            &self,
            calls: Vec<auctioneer_exchange::BidderCall>,
            _: auctioneer_exchange::AuctionDeadline,
        ) -> Result<
            Vec<auctioneer_exchange::BidderCallResult>,
            auctioneer_exchange::FanoutInternalError,
        > {
            assert_eq!(calls.len(), 1);
            drop(calls);
            let results = vec![auctioneer_exchange::BidderCallResult {
                result: Ok(BidderHttpResponse::new(200, vec![], self.body.clone())),
                elapsed: Duration::ZERO,
            }];
            self.baseline
                .store(alloc_track::reset_peak(), Ordering::SeqCst);
            Ok(results)
        }
    }

    #[test]
    fn actual_auction_targeting_and_assembly_fit_the_held_response_charge() {
        use auctioneer_config_data::types::{AccountConfig, BidderInfo};
        use auctioneer_exchange::{
            AccountSource, AuctionContext, AuctionPlan, ResolvedAccount, resolve_aliases,
            run_auction,
        };
        use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
        use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
        use auctioneer_runtime::{
            AuctionBudget, ChargeKind, Clock as _, FakeClock, SeededRng, serialized_json_bytes,
        };
        use futures::executor::block_on;
        use std::collections::BTreeSet;

        let mut nested = json!({"id":"b", "impid":"i", "price":1_i32});
        nested["ext"] = json!({"unknown":vec![json!({"":[]});20_000]});
        for (name, count, prefix, bidder, bid) in [
            (
                "ordinary",
                1,
                "hb".into(),
                "fake_bb".into(),
                json!({"id":"b","impid":"i","price":1_i32}),
            ),
            (
                "dense",
                4000,
                "hb".into(),
                "fake_bb".into(),
                json!({"id":"b","impid":"i","price":1_i32}),
            ),
            ("nested", 1, "hb".into(), "fake_bb".into(), nested),
            (
                "prefix",
                128,
                "p".repeat(1024),
                "fake_bb".into(),
                json!({"id":"b","impid":"i","price":1_i32}),
            ),
            (
                "alias",
                512,
                "hb".into(),
                "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                json!({"id":"b","impid":"i","price":1_i32}),
            ),
        ] {
            let (peak, margin, funded) = thread::spawn(move || {
                assert!(bidder.len() <= MAX_BIDDER_NAME_BYTES);
                let raw = response(&bid, count);
                assert!(raw.len() <= 256 * 1024);
                let mut input = json!({"id":"r", "site":{"id":"s"}, "imp":[{
                    "id":"i", "banner":{"w":300_i32,"h":250_i32}, "ext":{"bidder":{bidder.clone():{}}}
                }], "ext":{"prebid":{"targeting":{"prefix":prefix}}}});
                if bidder != "fake_bb" { input["ext"]["prebid"]["aliases"] = json!({bidder.clone():"fake_bb"}); }
                let req = prepare(serde_json::from_value(input).unwrap(), &RequestHeaders::default(), &PrepareLimits {max_impressions:100, ..PrepareLimits::default()}).unwrap();
                let mut registry = auctioneer_bidder::BidderRegistry::new();
                registry.register(Box::leak(Box::new(BidderInfo {name:"fake_bb".into(), endpoint:"https://example.test/bid".into(), ..BidderInfo::default()})), Box::new(FakeBidder::default())).expect("unique test bidder");
                let mut host = auctioneer_config_data::host_default().clone();
                host.gdpr.enabled = false;
                let plan = AuctionPlan::new(
                    resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap(),
                    BTreeSet::new(),
                    response_budgets(&registry, &host),
                    BTreeMap::new(),
                );
                let account_config = AccountConfig::default();
                let account = ResolvedAccount {id:"a".into(), source:AccountSource::DefaultFallback, config:&account_config};
                let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
                let rng = SeededRng::new(42);
                let decoded = u64::try_from(raw.len()).unwrap() * (1 + ASSEMBLY_FACTOR + RESPONSE_PARSE_FACTOR + RESPONSE_ASSEMBLY_FACTOR) + HEADER_MAP_CHARGE;
                let bidder_bytes = serialized_json_bytes(&bidder).unwrap();
                // Preserve the original allocation-margin oracle separately
                // from the exact synthetic receipt's new identity preflight.
                let metadata_bytes = u64::try_from(req.id.len() + 128 + 2 * bidder.len() + 128).unwrap();
                let bookkeeping = 4 * (metadata_bytes + 32 * 384);
                assert_eq!(size_of::<String>() + size_of::<Option<Winner>>(), 64);
                let retained_identity = 64 + 1 + u64::try_from(bidder.len()).unwrap();
                let targeting = u64::try_from(count).unwrap() * (serialized_json_bytes(&prefix).unwrap() + bidder_bytes + 64) * RESPONSE_ASSEMBLY_FACTOR;
                let ordinary_funded = bookkeeping + decoded + targeting;
                let debug = debug_peak_bytes(u32::try_from(host.resource_limits.max_outbound_requests).unwrap()).unwrap();
                let funded = ordinary_funded + retained_identity + debug;
                let budget = AuctionBudget::new(funded);
                let receipt = budget.try_reserve(ChargeKind::Response, funded).unwrap();
                let fanout = MeasuredFanout {body:raw, baseline:AtomicI64::new(0)};
                let mut ctx = AuctionContext::with_request_start(clock.now_monotonic(), &host, &account, &registry, &fanout, &clock, &rng, GvlStore::baked(), Gcl::baked(), clock.now(), false);
                ctx.response_reservation = Some(&receipt);
                let mut audit = DisclosureAudit::default();
                let output = block_on(run_auction(&req, &ctx, &plan, &mut audit)).unwrap()
                    .into_bid_response().unwrap();
                assert_eq!(output.seatbid.iter().map(|seat| seat.bid.len()).sum::<usize>(), count);
                assert!(output.seatbid[0].bid[0].ext.as_ref().unwrap().prebid.as_ref().unwrap().targeting.contains_key("hb_bidder"));
                serde_json::to_writer(io::sink(), &output).unwrap();
                let peak = alloc_track::peak_since(fanout.baseline.load(Ordering::SeqCst));
                let margin = peak.checked_mul(6).unwrap().div_ceil(5);
                // Prepayment includes debug even when unrequested; the measured
                // ordinary path must still fit its original disjoint allowance.
                assert!(margin <= ordinary_funded, "{name}: margin={margin} > ordinary_funded={ordinary_funded}");
                (peak, margin, ordinary_funded)
            }).join().unwrap();
            writeln!(
                io::stderr(),
                "auction/{name}: peak={peak} margin={margin} funded={funded}"
            )
            .unwrap();
        }
    }

    fn denial_request(visible: bool) -> NormalizedBidRequest {
        let aliases: BTreeMap<String, &str> = (0_u32..128_u32)
            .map(|n| (format!("fake_a{n}"), "fake_bb"))
            .collect();
        assert!(
            aliases
                .keys()
                .all(|name| name.len() <= MAX_BIDDER_NAME_BYTES)
        );
        let bidders: BTreeMap<&str, Value> = aliases
            .keys()
            .map(|name| (name.as_str(), json!({})))
            .collect();
        prepare(
            serde_json::from_value(json!({
                "id":"r", "site":{"id":"s"}, "imp":[{
                    "id":"i".repeat(16 * 1024),
                    "banner":{"w":300_i32,"h":250_i32},
                    "ext":{"bidder":bidders}
                }], "ext":{"prebid":{"aliases":aliases,"returnallbidstatus":visible}}
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 100,
                ..PrepareLimits::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn bookkeeping_refuses_visible_rows_before_copying_large_imp_ids() {
        use auctioneer_config_data::types::{AccountConfig, BidderInfo};
        use auctioneer_exchange::{
            AccountSource, AuctionContext, AuctionError, AuctionPlan, ResolvedAccount,
            resolve_aliases, run_auction,
        };
        use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
        use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
        use futures::executor::block_on;
        use std::collections::BTreeSet;

        for visible in [false, true] {
            thread::spawn(move || {
                let req = denial_request(visible);
                let mut registry = auctioneer_bidder::BidderRegistry::new();
                registry
                    .register(
                        Box::leak(Box::new(BidderInfo {
                            name: "fake_bb".into(),
                            endpoint: "https://example.test/bid".into(),
                            ..BidderInfo::default()
                        })),
                        Box::new(FakeBidder::default()),
                    )
                    .expect("unique test bidder");
                let mut host = auctioneer_config_data::host_default().clone();
                host.gdpr.enabled = false;
                host.resource_limits.max_bidders_per_auction = 1;
                let plan = AuctionPlan::new(
                    resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap(),
                    BTreeSet::new(),
                    response_budgets(&registry, &host),
                    BTreeMap::new(),
                );
                let mut config = AccountConfig::default();
                config.privacy.allowactivities =
                    serde_json::from_value(json!({"fetchBids":{"default":false}})).unwrap();
                let account = ResolvedAccount {
                    id: "a".into(),
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
                    &NoDispatch,
                    &clock,
                    &rng,
                    GvlStore::baked(),
                    Gcl::baked(),
                    clock.now(),
                    false,
                );
                let before = alloc_track::reset_peak();
                let mut audit = DisclosureAudit::default();
                let result = block_on(run_auction(&req, &ctx, &plan, &mut audit))
                    .and_then(auctioneer_exchange::AuctionOutcome::into_bid_response);
                let peak = alloc_track::peak_since(before);
                assert!(
                    peak < 4 * host.resource_limits.max_outbound_raw_bytes,
                    "visible={visible}: peak={peak}; repeated imp-id copies must not be retained"
                );
                if visible {
                    let error = result.unwrap_err();
                    assert_eq!(error.code(), 2);
                    assert!(matches!(
                        &error,
                        AuctionError::BadInput(message)
                            if message == "response bookkeeping exceeds the materialization budget"
                    ));
                } else {
                    let output = result.unwrap();
                    assert!(output.seatbid.is_empty());
                    let ext = output.ext.unwrap();
                    assert_eq!(ext["warnings"].as_object().unwrap().len(), 128);
                    assert!(ext["prebid"].get("seatnonbid").is_none());
                }
            })
            .join()
            .unwrap();
        }
    }

    fn response(bid: &Value, count: usize) -> Vec<u8> {
        serde_json::to_vec(
            &json!({"id":"r", "cur":"USD", "seatbid":[{"seat":"", "bid":vec![bid; count]}]}),
        )
        .unwrap()
    }

    struct BrowserFanout {
        responses: Mutex<Vec<BidderCallResult>>,
        coverage: Vec<Vec<&'static str>>,
        retained: u64,
        dispatched: AtomicUsize,
    }

    impl BrowserFanout {
        fn new(bodies: &[(u16, Vec<u8>)], coverage: Vec<Vec<&'static str>>) -> Self {
            let before = alloc_track::live();
            let responses = bodies
                .iter()
                .map(|(status, body)| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(*status, vec![], body.clone())),
                    elapsed: Duration::ZERO,
                })
                .collect();
            let fanout = Self {
                responses: Mutex::new(responses),
                coverage,
                retained: 0,
                dispatched: AtomicUsize::new(0),
            };
            Self {
                retained: u64::try_from(alloc_track::live().checked_sub(before).unwrap()).unwrap(),
                ..fanout
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for BrowserFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), self.coverage.len());
            for (call, expected) in calls.iter().zip(&self.coverage) {
                assert_eq!(
                    call.request.covered_imp_ids().collect::<Vec<_>>(),
                    *expected
                );
            }
            self.dispatched.store(calls.len(), Ordering::SeqCst);
            drop(calls);
            Ok(take(&mut *self.responses.lock().unwrap()))
        }
    }

    struct BrowserObserved {
        result: Result<BidResponse, AuctionError>,
        peak: u64,
        allocated: u64,
        derived: Derived,
        dispatched: usize,
    }

    fn browser_request(names: &[String]) -> Value {
        let bidders: BTreeMap<_, _> = names
            .iter()
            .map(|name| {
                (
                    name.as_str(),
                    json!({"slotid": 123_456_i32, "accountid": 78_910_i32}),
                )
            })
            .collect();
        let aliases: BTreeMap<_, _> = names
            .iter()
            .filter(|name| name.as_str() != "fake_bb____")
            .map(|name| (name.as_str(), "fake_bb____"))
            .collect();
        json!({"id": "r", "tmax": 1000_i32, "site": {"id": "s", "page": "https://fakeb.test"},
            "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"ae": 1_i32, "bidder": bidders}}],
            "ext": {"prebid": {"aliases": aliases}}})
    }

    fn browser_config(imp: &str, config: &Value) -> Value {
        json!({"impid": imp, "config": config})
    }

    fn browser_response(entries: &[Value], currency: &str, bids: usize) -> Vec<u8> {
        let bid = json!({"id": "b", "impid": "i", "price": 1_i32,
            "w": 300_i32, "h": 250_i32, "adm": "creative",
            "ext": {"prebid": {"type": "banner"}}});
        serde_json::to_vec(&json!({"id": "r", "cur": currency,
            "seatbid": [{"bid": vec![bid; bids]}], "ext": {"auctionconfigs": entries}}))
        .unwrap()
    }

    fn browser_registry(coverage: Option<Vec<Vec<&'static str>>>) -> BidderRegistry {
        use auctioneer_config_data::types::BidderInfo;

        // Keep identity and endpoint byte widths stable for the resource controls.
        let mut registry = BidderRegistry::new();
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: "fake_bb____".into(),
                    endpoint:
                        "https://bidder.example.test/openrtb?fixture=browser_coverage_controls"
                            .into(),
                    ..BidderInfo::default()
                })),
                Box::new(FakeBidder {
                    request_coverage: coverage,
                    ..FakeBidder::default()
                }),
            )
            .expect("unique test bidder");
        registry
    }

    fn browser_receipt_case(
        input: Value,
        bodies: &[(u16, Vec<u8>)],
        coverage: Option<Vec<Vec<&'static str>>>,
        held_response: Option<u64>,
    ) -> BrowserObserved {
        browser_receipt_case_inspecting(input, bodies, coverage, held_response, |_| {})
    }

    fn browser_receipt_case_inspecting(
        input: Value,
        bodies: &[(u16, Vec<u8>)],
        coverage: Option<Vec<Vec<&'static str>>>,
        held_response: Option<u64>,
        inspect: impl FnOnce(&auctioneer_exchange::AuctionOutcome),
    ) -> BrowserObserved {
        use auctioneer_config_data::types::AccountConfig;
        use auctioneer_exchange::{
            AccountSource, AuctionContext, AuctionPlan, ResolvedAccount, resolve_aliases,
            run_auction,
        };
        use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
        use auctioneer_runtime::{AuctionBudget, ChargeKind, Clock as _, FakeClock, SeededRng};
        use futures::executor::block_on;
        use std::collections::BTreeSet;

        let request = prepare(
            serde_json::from_value(input).unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 8,
                ..PrepareLimits::default()
            },
        )
        .unwrap();
        let expected = coverage
            .clone()
            .unwrap_or_else(|| vec![vec!["i"]; bodies.len()]);
        let registry = browser_registry(coverage);
        let calls = u32::try_from(bodies.len()).unwrap();
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        host.resource_limits.max_bidders_per_auction = 16;
        host.resource_limits.max_outbound_requests = usize::try_from(calls).unwrap();
        host.resource_limits.max_outbound_raw_bytes = 32 * 1024;
        host.resource_limits.max_outbound_request_bytes = 32 * 1024;
        host.resource_limits.max_parsed_nodes = 512;
        host.resource_limits.max_decoded_bytes = 32 * 1024;
        host.resource_limits.max_transport_bytes = 16 * 1024;
        let plan = AuctionPlan::new(
            resolve_aliases(&request, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap(),
            BTreeSet::new(),
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let config = AccountConfig::default();
        let account = ResolvedAccount {
            id: "account".into(),
            source: AccountSource::DefaultFallback,
            config: &config,
        };
        let fanout = BrowserFanout::new(bodies, expected);
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        // The allocation controls use the unchanged declared runtime profile,
        // never the output-name estimator. Only the refusal test shortens it.
        let derived = runtime_funding(calls, 32 * 1024);
        let held = held_response.unwrap_or(derived.auction_charges.response);
        let budget = AuctionBudget::new(held);
        let receipt = budget.try_reserve(ChargeKind::Response, held).unwrap();
        let mut ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host,
            &account,
            &registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        ctx.response_reservation = Some(&receipt);
        let mut audit = DisclosureAudit::default();
        let before = alloc_track::reset_peak();
        let allocated_before = alloc_track::allocated();
        let result = block_on(run_auction(&request, &ctx, &plan, &mut audit)).and_then(|outcome| {
            inspect(&outcome);
            outcome.into_bid_response()
        });
        // Snapshot before serialization so acceptance/refusal timing compares
        // the same engine work, even when an earlier parse peak is larger.
        let allocated = alloc_track::allocated()
            .checked_sub(allocated_before)
            .unwrap();
        if let Ok(output) = &result {
            serde_json::to_writer(io::sink(), output).unwrap();
        }
        // Include the already-live HTTP bodies and result vector alongside
        // decode, retained canonical names, vector growth and wire wrappers.
        let peak = alloc_track::peak_since(before)
            .checked_add(fanout.retained)
            .unwrap();
        assert_eq!(budget.in_use(), held);
        drop(receipt);
        assert_eq!(budget.in_use(), 0);
        BrowserObserved {
            result,
            peak,
            allocated,
            derived,
            dispatched: fanout.dispatched.load(Ordering::SeqCst),
        }
    }

    fn browser_wire(observed: &BrowserObserved) -> Value {
        let output = observed.result.as_ref().unwrap();
        let serialized = serde_json::to_vec(output).unwrap();
        serde_json::from_slice(&serialized).unwrap()
    }

    #[test]
    fn config_only_aliases_keep_exchange_identity_and_opaque_payload() {
        let names = vec!["fake_z".into(), "fake_a".into()];
        let payload = json!({"bidder": "fake_forged", "adapter": "fake_forged", "nested": [1_i32, {"k": true}]});
        let row = browser_config("i", &payload);
        let body = browser_response(&[row], "USD", 0);
        let observed = browser_receipt_case_inspecting(
            browser_request(&names),
            &[(200, body.clone()), (200, body)],
            None,
            None,
            |outcome| {
                assert_eq!(outcome.impression_ids(), ["i"]);
                assert_eq!(outcome.winners().len(), 1);
                assert!(outcome.winners()[0].is_none());
                assert!(outcome.selected_bid(0).is_none());
                assert_eq!(outcome.bidders().len(), 2);
                for name in ["fake_a", "fake_z"] {
                    let bundle = &outcome.bidders()[&BidderName::new(name)];
                    assert!(bundle.bids.is_empty());
                    assert!(bundle.errors.is_empty());
                    assert!(bundle.dispatched);
                    let browser = bundle.browser_auctions.as_ref().unwrap();
                    assert_eq!(browser.adapter.as_str(), "fake_bb____");
                    assert_eq!(browser.configs.len(), 1);
                    assert_eq!(browser.configs[0].imp_id, "i");
                    assert_eq!(browser.configs[0].config, payload);
                }
                assert!(outcome.response_material().prebid_errors.is_empty());
            },
        );
        assert_eq!(observed.dispatched, 2);
        let wire = browser_wire(&observed);
        assert_eq!(
            wire["ext"]["prebid"]["fledge"]["auctionconfigs"],
            json!([
                {"impid": "i", "bidder": "fake_a", "adapter": "fake_bb____", "config": payload},
                {"impid": "i", "bidder": "fake_z", "adapter": "fake_bb____", "config": payload}
            ])
        );
        assert!(observed.result.as_ref().unwrap().seatbid.is_empty());
        assert!(observed.result.as_ref().unwrap().cur.is_empty());
        assert!(wire["ext"]["prebid"].get("targeting").is_none());
        assert!(wire["ext"].get("errors").is_none());
    }

    #[test]
    fn configs_survive_non_usd_bid_refusal_independently_of_bid_count() {
        for currency in ["USD", "EUR", ""] {
            for bids in [0_usize, 1, 3] {
                let observed = browser_receipt_case(
                    browser_request(&["fake_bb____".into()]),
                    &[(
                        200,
                        browser_response(
                            &[browser_config("i", &json!({"ok": true}))],
                            currency,
                            bids,
                        ),
                    )],
                    None,
                    None,
                );
                let wire = browser_wire(&observed);
                assert_eq!(
                    wire["ext"]["prebid"]["fledge"]["auctionconfigs"],
                    json!([
                        {"impid": "i", "bidder": "fake_bb____", "adapter": "fake_bb____", "config": {"ok": true}}
                    ])
                );
                let returned = observed
                    .result
                    .as_ref()
                    .unwrap()
                    .seatbid
                    .iter()
                    .map(|seat| seat.bid.len())
                    .sum::<usize>();
                assert_eq!(returned, if currency == "EUR" { 0 } else { bids });
                if currency == "EUR" {
                    assert_eq!(
                        wire["ext"]["warnings"]["fake_bb____"],
                        json!([
                            {"code": 10004_u32, "message": "non-USD currency; bids dropped"}
                        ])
                    );
                }
            }
        }
    }

    #[test]
    fn browser_wire_correlation_requires_scoped_request_and_each_calls_sent_ids() {
        let mut input = browser_request(&["fake_a".into()]);
        let mut second = input["imp"][0].clone();
        second["id"] = json!("j");
        input["imp"].as_array_mut().unwrap().push(second);
        let entries = vec![
            browser_config("i", &json!({"ordinal": 1_i32})),
            browser_config("i", &json!({"ordinal": 2_i32})),
            browser_config("j", &json!({"ordinal": 3_i32})),
            browser_config("forged", &json!({"secret": "outside-scope"})),
            browser_config("missing", &json!({"secret": "not-sent"})),
        ];
        let body = browser_response(&entries, "USD", 0);
        let observed = browser_receipt_case(
            input,
            &[(200, body.clone()), (200, body)],
            Some(vec![vec!["i", "forged"], vec!["j"]]),
            None,
        );
        assert_eq!(observed.dispatched, 2);
        let wire = browser_wire(&observed);
        assert_eq!(
            wire["ext"]["prebid"]["fledge"]["auctionconfigs"],
            json!([
                {"impid": "i", "bidder": "fake_a", "adapter": "fake_bb____", "config": {"ordinal": 1_i32}},
                {"impid": "i", "bidder": "fake_a", "adapter": "fake_bb____", "config": {"ordinal": 2_i32}},
                {"impid": "j", "bidder": "fake_a", "adapter": "fake_bb____", "config": {"ordinal": 3_i32}}
            ])
        );
        let diagnostic = json!({"code": 4_u32, "message":
            "browser configuration contains an impression not requested by this call; uncorrelated configurations dropped"});
        assert_eq!(
            wire["ext"]["errors"]["fake_a"],
            json!([diagnostic, diagnostic])
        );
    }

    #[test]
    fn empty_sent_coverage_and_failed_responses_omit_browser_output() {
        let config = browser_response(
            &[browser_config("i", &json!({"secret": "not-authorized"}))],
            "USD",
            0,
        );
        let empty = browser_receipt_case(
            browser_request(&["fake_a".into()]),
            &[(200, config)],
            Some(vec![vec![]]),
            None,
        );
        let wire = browser_wire(&empty);
        assert!(wire["ext"]["prebid"].get("fledge").is_none());
        assert_eq!(wire["ext"]["errors"]["fake_a"].as_array().unwrap().len(), 1);
        assert_eq!(wire["ext"]["errors"]["fake_a"][0]["code"], 4_u32);
        assert!(!wire["ext"]["errors"].to_string().contains("not-authorized"));
        for (status, body) in [
            (200, browser_response(&[], "USD", 0)),
            (
                500,
                browser_response(
                    &[browser_config("i", &json!({"not": "accepted"}))],
                    "USD",
                    0,
                ),
            ),
            (200, b"not-json".to_vec()),
        ] {
            let observed = browser_receipt_case(
                browser_request(&["fake_a".into()]),
                &[(status, body)],
                None,
                None,
            );
            assert!(
                browser_wire(&observed)["ext"]["prebid"]
                    .get("fledge")
                    .is_none()
            );
        }
    }

    #[test]
    fn config_only_count_payload_nodes_aliases_and_calls_fit_the_runtime_receipt_with_margin() {
        for calls in [1_u32, 4, 16] {
            for alias_bytes in ["fake_short".len(), MAX_BIDDER_NAME_BYTES] {
                for count in [0_usize, 1, 8, 24] {
                    for payload_bytes in [0_usize, 32, 512] {
                        for nodes in [0_usize, 4] {
                            thread::spawn(move || {
                                let names: Vec<String> = (0..calls).map(|index| {
                                    let mut name = format!("fake_{index:02}");
                                    name.push_str(&"a".repeat(alias_bytes.checked_sub(name.len()).unwrap()));
                                    name
                                }).collect();
                                let config = json!({"payload": "x".repeat(payload_bytes), "nodes": vec![0_u32; nodes]});
                                let entries: Vec<_> = iter::repeat_with(|| browser_config("i", &config)).take(count).collect();
                                let body = browser_response(&entries, "USD", 0);
                                assert!(body.len() <= 32 * 1024);
                                assert!(count_json_nodes(&body, 512).unwrap() <= 512);
                                let bodies: Vec<_> = iter::repeat_with(|| (200, body.clone()))
                                    .take(usize::try_from(calls).unwrap()).collect();
                                let observed = browser_receipt_case(browser_request(&names), &bodies, None, None);
                                assert_eq!(observed.dispatched, usize::try_from(calls).unwrap());
                                let wire = browser_wire(&observed);
                                assert!(observed.result.as_ref().unwrap().seatbid.is_empty());
                                if count == 0 {
                                    assert!(wire["ext"]["prebid"].get("fledge").is_none());
                                } else {
                                    let rows = wire["ext"]["prebid"]["fledge"]["auctionconfigs"].as_array().unwrap();
                                    assert_eq!(rows.len(), count * names.len());
                                    for (name, chunk) in names.iter().zip(rows.chunks_exact(count)) {
                                        for row in chunk {
                                            assert_eq!(row, &json!({"impid": "i", "bidder": name, "adapter": "fake_bb____", "config": config}));
                                        }
                                    }
                                }
                                let ordinary = observed.derived.auction_charges.response
                                    .checked_sub(observed.derived.debug_term).unwrap();
                                let margin = observed.peak.checked_mul(6).unwrap().div_ceil(5);
                                assert!(observed.peak > 0);
                                assert!(margin <= ordinary,
                                    "calls={calls} alias={alias_bytes} count={count} payload={payload_bytes} nodes={nodes}: peak={} margin={margin} ordinary={ordinary}", observed.peak);
                                if count == 0 {
                                    assert!(margin <= observed.derived.bookkeeping_term,
                                        "empty fixed bundle owners must fit bookkeeping without decode or debug funding");
                                }
                            }).join().unwrap();
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn multiple_calls_for_one_alias_fit_runtime_funding_and_preserve_append_order() {
        for calls in [1_u32, 4, 16] {
            for alias_bytes in ["fake_short".len(), MAX_BIDDER_NAME_BYTES] {
                for count in [0_usize, 1, 8, 24] {
                    thread::spawn(move || {
                        let mut alias = String::with_capacity(alias_bytes);
                        alias.push_str("fake_short");
                        alias.extend(iter::repeat_n('a', alias_bytes.checked_sub(alias.len()).unwrap()));
                        let bodies: Vec<_> = (0..calls).map(|call| {
                            let entries: Vec<_> = (0..count).map(|record| {
                                browser_config("i", &json!({"call": call, "record": record, "payload": "x".repeat(32)}))
                            }).collect();
                            let body = browser_response(&entries, "USD", 0);
                            assert!(body.len() <= 32 * 1024);
                            assert!(count_json_nodes(&body, 512).unwrap() <= 512);
                            (200, body)
                        }).collect();
                        let observed = browser_receipt_case(
                            browser_request(from_ref(&alias)), &bodies,
                            Some(vec![vec!["i"]; usize::try_from(calls).unwrap()]), None,
                        );
                        assert_eq!(observed.dispatched, usize::try_from(calls).unwrap());
                        let wire = browser_wire(&observed);
                        assert!(observed.result.as_ref().unwrap().seatbid.is_empty());
                        if count == 0 {
                            assert!(wire["ext"]["prebid"].get("fledge").is_none());
                        } else {
                            let expected: Vec<_> = (0..calls).flat_map(|call| {
                                let requested_alias = &alias;
                                (0..count).map(move |record| json!({"impid": "i", "bidder": requested_alias,
                                    "adapter": "fake_bb____", "config": {"call": call, "record": record, "payload": "x".repeat(32)}}))
                            }).collect();
                            assert_eq!(wire["ext"]["prebid"]["fledge"]["auctionconfigs"], json!(expected));
                        }
                        let ordinary = observed.derived.auction_charges.response
                            .checked_sub(observed.derived.debug_term).unwrap();
                        let margin = observed.peak.checked_mul(6).unwrap().div_ceil(5);
                        assert!(observed.peak > 0);
                        assert!(margin <= ordinary,
                            "aliases=1 calls={calls} alias={alias_bytes} count={count}: peak={} margin={margin} ordinary={ordinary}", observed.peak);
                        if count == 0 {
                            assert!(margin <= observed.derived.bookkeeping_term);
                        }
                    }).join().unwrap();
                }
            }
        }
    }

    #[test]
    fn fixed_denied_and_browser_bundle_owners_fit_independent_request_bookkeeping() {
        use auctioneer_exchange::{BrowserAuctionResult, ErrorEntry};

        for names in [1_usize, 16] {
            for browser in [false, true] {
                thread::spawn(move || {
                    let derived = runtime_funding(0, 32 * 1024);
                    assert_eq!(
                        derived.auction_charges.response - derived.debug_term,
                        derived.bookkeeping_term
                    );
                    let before = alloc_track::reset_peak();
                    let bundles = (0..names)
                        .map(|index| {
                            let name = format!("fake_{index:059}");
                            (
                                BidderName::new(name),
                                BidderResultBundle {
                                    dispatched: browser,
                                    warnings: vec![ErrorEntry {
                                        code: 10017,
                                        message: "bidder disallowed by privacy".into(),
                                    }],
                                    browser_auctions: browser.then(|| BrowserAuctionResult {
                                        adapter: BidderName::new("fake_c"),
                                        configs: Vec::new(),
                                    }),
                                    ..BidderResultBundle::default()
                                },
                            )
                        })
                        .collect();
                    let output = build_response(ResponseInput {
                        bundles,
                        bidder_visible_tmax_ms: 100,
                        auction_timestamp_ms: 0,
                        return_all_bid_status: false,
                        seatnonbid: BTreeMap::new(),
                        construction_diagnostics: BTreeMap::new(),
                        prebid_errors: vec![],
                        prebid_warnings: vec![],
                    })
                    .unwrap();
                    serde_json::to_writer(io::sink(), &output).unwrap();
                    let peak = alloc_track::peak_since(before);
                    let margin = peak.checked_mul(6).unwrap().div_ceil(5);
                    // Independent literal bookkeeping allowance: response globals,
                    // one public identity and sixteen fixed nodes per bundle.
                    let fixed = 4
                        * (129
                            + 16 * 384
                            + u64::try_from(names).unwrap() * (2 * 64 + 128 + 16 * 384));
                    assert!(peak > 0);
                    assert!(
                        margin <= fixed,
                        "names={names} browser={browser}: margin={margin} fixed={fixed}"
                    );
                    assert!(fixed <= derived.bookkeeping_term);
                    let wire = serde_json::to_value(output).unwrap();
                    assert_eq!(wire["ext"]["warnings"].as_object().unwrap().len(), names);
                    assert!(wire["ext"]["prebid"].get("fledge").is_none());
                })
                .join()
                .unwrap();
            }
        }
    }

    #[test]
    fn browser_name_expansion_refuses_one_byte_below_the_pre_wrapper_receipt() {
        use auctioneer_exchange::AuctionError;
        use auctioneer_runtime::{ChargeKind, ResourceError, serialized_json_bytes};

        let alias = "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        let count = 32_usize;
        let entries: Vec<_> = iter::repeat_with(|| browser_config("i", &json!({})))
            .take(count)
            .collect();
        let body = browser_response(&entries, "USD", 0);
        assert!(body.len() <= 32 * 1024);
        assert!(count_json_nodes(&body, 512).unwrap() <= 512);
        assert_eq!(size_of::<String>() + size_of::<Option<Winner>>(), 64);
        let name_bytes = u64::try_from(alias.len()).unwrap();
        let bookkeeping = 4 * (1 + 128 + 2 * name_bytes + 128 + 32 * 384) + 64 + 1 + name_bytes;
        let decoded = u64::try_from(body.len()).unwrap()
            * (1 + ASSEMBLY_FACTOR + RESPONSE_PARSE_FACTOR + RESPONSE_ASSEMBLY_FACTOR)
            + HEADER_MAP_CHARGE;
        let expanded = u64::try_from(count).unwrap()
            * (serialized_json_bytes(&alias).unwrap()
                + serialized_json_bytes("fake_bb____").unwrap()
                + 64)
            * RESPONSE_ASSEMBLY_FACTOR;
        // Deliberately shortened receipts test the preflight boundary only;
        // they are not the funding source for the allocation-margin controls.
        let exact = bookkeeping + decoded + expanded + runtime_funding(1, 32 * 1024).debug_term;
        let accepted = browser_receipt_case(
            browser_request(from_ref(&alias)),
            &[(200, body.clone())],
            None,
            Some(exact),
        );
        let refused = browser_receipt_case(
            browser_request(from_ref(&alias)),
            &[(200, body)],
            None,
            Some(exact - 1),
        );
        assert_eq!(accepted.dispatched, 1);
        assert_eq!(
            refused.dispatched, 1,
            "bookkeeping and decode preflights must already pass"
        );
        assert!(matches!(
            refused.result,
            Err(AuctionError::Resource(ResourceError::Exhausted {
                kind: ChargeKind::Response,
            }))
        ));
        assert_eq!(
            browser_wire(&accepted)["ext"]["prebid"]["fledge"]["auctionconfigs"]
                .as_array()
                .unwrap()
                .len(),
            count
        );
        // These are only the repeated identity strings, not wrapper maps or
        // vector capacity. A check delayed until after assembly saves neither.
        let identity_bytes = u64::try_from(count * (alias.len() + "fake_bb____".len())).unwrap();
        let saved = accepted.allocated.checked_sub(refused.allocated).unwrap();
        assert!(
            saved >= identity_bytes,
            "refusal must precede wrapper allocation: accepted={} refused={} saved={saved} identities={identity_bytes}",
            accepted.allocated,
            refused.allocated
        );
    }

    #[test]
    fn decode_and_assembly_peak_with_margin_fit_their_distinct_charges() {
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
            let (decode, assembly) = thread::spawn(move || {
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
                let parsed = FakeBidder::default()
                    .make_bids(&req, &sent, &response)
                    .unwrap();
                assert!(!parsed.bids.is_empty());
                let decode = alloc_track::peak_since(decode_start);
                let input = ResponseInput {
                    construction_diagnostics: BTreeMap::new(),
                    bidder_visible_tmax_ms: 100,
                    auction_timestamp_ms: 0,
                    return_all_bid_status: false,
                    seatnonbid: BTreeMap::new(),
                    bundles: BTreeMap::from([(
                        BidderName::new("fake_bb"),
                        BidderResultBundle {
                            bids: parsed.bids,
                            dispatched: true,
                            ..Default::default()
                        },
                    )]),
                    prebid_errors: vec![],
                    prebid_warnings: vec![],
                };
                let before = alloc_track::reset_peak();
                let built = build_response(input).unwrap();
                // The egress buffer is separately charged. Writing to a sink
                // includes serialization scratch without a second body buffer.
                serde_json::to_writer(io::sink(), &built).unwrap();
                let assembly = alloc_track::peak_since(before);
                drop(built);
                (decode, assembly)
            })
            .join()
            .unwrap();
            for (stage, measured, factor) in [
                ("decode", decode, RESPONSE_PARSE_FACTOR),
                ("assembly", assembly, RESPONSE_ASSEMBLY_FACTOR),
            ] {
                let margined = measured.checked_mul(6).unwrap().div_ceil(5);
                let funded = bytes.checked_mul(factor).unwrap();
                writeln!(
                    io::stderr(),
                    "{name}/{stage}: raw={bytes} peak={measured} margin={margined} funded={funded}"
                )
                .unwrap();
                if margined > funded {
                    failures.push(format!("{name}/{stage}: {margined} > {funded}"));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "underfunded response phases: {failures:?}"
        );
    }

    // Synthetic platform inputs, not a copied native envelope. The binding
    // response dimensions stay at 512 nodes even when native caps change.
    fn runtime_funding(calls: u32, metadata_bytes: u64) -> Derived {
        let limits = ResolvedIsolateLimits {
            isolate_total_bytes: u64::MAX,
            baseline_bytes: 0,
            inbound_request_bytes: 32 * 1024,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: 32 * 1024,
            max_outbound_raw_bytes: metadata_bytes,
            max_transport_bytes: 16 * 1024,
            max_decoded_bytes: 32 * 1024,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: u64::from(calls.max(1))
                .checked_mul(32 * 1024)
                .and_then(|bytes| bytes.checked_mul(ASSEMBLY_FACTOR))
                .unwrap(),
            egress_permits: 1,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 1,
            aux_route_peak_bytes: 768 * 1024 * 1024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 1,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 16,
            max_outbound_requests: calls,
            max_concurrent_auctions: 1,
            max_active_body_streams: 1,
            max_runtime_config_bytes: 1 << 20,
            max_runtime_config_blob_bytes: 1 << 20,
            max_parsed_nodes: 512,
            max_copied_header_bytes: 4096,
            max_admitted_header_bytes: 1 << 14,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 8,
            max_config_json_nodes: 8192,
            terminal_response_permits: 2,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4096,
        };
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 1,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1024,
            parser_transient_bytes: 1 << 14,
            egress_handoff_factor: 2,
        };
        derive_isolate_budget(
            &limits,
            &target,
            auctioneer_bidder::outbound_meta_bytes(8),
            construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap()
    }

    fn full_fpd_request(visible: bool) -> Value {
        let aliases: BTreeMap<_, _> = (0..16_u32)
            .map(|index| (format!("fake_a{index:02}"), "fake_bb"))
            .collect();
        let bidders: BTreeMap<_, _> = aliases
            .keys()
            .map(|name| (name.clone(), json!({})))
            .collect();
        let fpd: BTreeMap<_, _> = aliases
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    json!({"banner": {"ext": {"receipt_fpd": true, "padding": ""}}}),
                )
            })
            .collect();
        let mut value = json!({"id": "r", "tmax": 1000_i32, "site": {"id": "s"},
            "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32}, "ext": {"bidder": bidders, "prebid": {"imp": fpd}}}],
            "ext": {"prebid": {"aliases": aliases, "returnallbidstatus": visible}, "nodes": []}
        });
        let nodes = auctioneer_runtime::count_json_nodes(&serde_json::to_vec(&value).unwrap(), 512)
            .unwrap();
        value["ext"]["nodes"] = json!(vec![
            0_u32;
            usize::try_from(512_u32.checked_sub(nodes).unwrap())
                .unwrap()
        ]);
        let bytes = serde_json::to_vec(&value).unwrap().len();
        let padding = (32 * 1024_usize).checked_sub(bytes).unwrap();
        let per_scope = padding.checked_div(16).unwrap();
        let extra_scopes = padding.checked_rem(16).unwrap();
        for (index, patch) in value["imp"][0]["ext"]["prebid"]["imp"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .enumerate()
        {
            let scope_bytes = per_scope
                .checked_add(usize::from(index < extra_scopes))
                .unwrap();
            patch["banner"]["ext"]["padding"] = json!("x".repeat(scope_bytes));
        }
        assert_eq!(
            auctioneer_runtime::count_json_nodes(&serde_json::to_vec(&value).unwrap(), 512)
                .unwrap(),
            512
        );
        assert_eq!(serde_json::to_vec(&value).unwrap().len(), 32 * 1024);
        value
    }

    struct FullResponseFanout {
        responses: Mutex<Vec<auctioneer_exchange::BidderCallResult>>,
        retained: u64,
        dispatched: AtomicUsize,
        fpd: AtomicUsize,
    }

    impl FullResponseFanout {
        fn new(calls: u32) -> Self {
            let mut bid = json!({"id": "b", "impid": "i", "price": 1_i32, "mtype": 1_i32, "w": 300_i32, "h": 250_i32, "adm": ""});
            let used = response(&bid, 1).len();
            bid["adm"] = json!("c".repeat((32 * 1024_usize).checked_sub(used).unwrap()));
            let raw = response(&bid, 1);
            assert_eq!(raw.len(), 32 * 1024);
            let before = alloc_track::live();
            let results = iter::repeat_with(|| auctioneer_exchange::BidderCallResult {
                result: Ok(BidderHttpResponse::new(200, vec![], raw.clone())),
                elapsed: Duration::ZERO,
            })
            .take(usize::try_from(calls).unwrap())
            .collect();
            let fanout = Self {
                responses: Mutex::new(results),
                retained: 0,
                dispatched: AtomicUsize::new(0),
                fpd: AtomicUsize::new(0),
            };
            drop(fanout.responses.lock().unwrap());
            Self {
                retained: u64::try_from(alloc_track::live().checked_sub(before).unwrap()).unwrap(),
                ..fanout
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl auctioneer_exchange::BidderFanout for FullResponseFanout {
        async fn send_all(
            &self,
            calls: Vec<auctioneer_exchange::BidderCall>,
            _: auctioneer_exchange::AuctionDeadline,
        ) -> Result<
            Vec<auctioneer_exchange::BidderCallResult>,
            auctioneer_exchange::FanoutInternalError,
        > {
            self.dispatched.store(calls.len(), Ordering::SeqCst);
            let fpd = calls
                .iter()
                .filter(|call| {
                    let body: Value = serde_json::from_slice(call.request.body()).unwrap();
                    let padding = body
                        .pointer("/imp/0/banner/ext/padding")
                        .and_then(Value::as_str)
                        .unwrap();
                    assert!(padding.len() >= 1024);
                    assert!(padding.bytes().all(|byte| byte == b'x'));
                    body.pointer("/imp/0/banner/ext/receipt_fpd") == Some(&json!(true))
                })
                .count();
            self.fpd.store(fpd, Ordering::SeqCst);
            drop(calls);
            Ok(take(&mut *self.responses.lock().unwrap()))
        }
    }

    struct ReceiptObserved {
        result: Result<Value, auctioneer_exchange::AuctionError>,
        peak: u64,
        derived: Derived,
        dispatched: usize,
        fpd: usize,
    }

    fn runtime_receipt_case(
        input: Value,
        calls: u32,
        deny: bool,
        metadata_bytes: u64,
    ) -> ReceiptObserved {
        use auctioneer_config_data::types::{AccountConfig, BidderInfo};
        use auctioneer_exchange::{
            AccountSource, AuctionContext, AuctionPlan, ResolvedAccount, resolve_aliases,
            run_auction,
        };
        use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
        use auctioneer_runtime::{AuctionBudget, ChargeKind, Clock as _, FakeClock, SeededRng};
        use futures::executor::block_on;
        use std::collections::BTreeSet;

        let request = prepare(
            serde_json::from_value(input).unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 8,
                ..PrepareLimits::default()
            },
        )
        .unwrap();
        let mut registry = auctioneer_bidder::BidderRegistry::new();
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: "fake_bb".into(),
                    endpoint: "https://example.test/bid".into(),
                    ..BidderInfo::default()
                })),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        host.resource_limits.max_bidders_per_auction = 16;
        host.resource_limits.max_outbound_requests = usize::try_from(calls).unwrap();
        host.resource_limits.max_outbound_raw_bytes = metadata_bytes;
        host.resource_limits.max_outbound_request_bytes = 32 * 1024;
        host.resource_limits.max_parsed_nodes = 512;
        host.resource_limits.max_decoded_bytes = 32 * 1024;
        host.resource_limits.max_transport_bytes = 16 * 1024;
        let plan = AuctionPlan::new(
            resolve_aliases(&request, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap(),
            BTreeSet::new(),
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let mut config = AccountConfig::default();
        if deny {
            config.privacy.allowactivities =
                serde_json::from_value(json!({"fetchBids": {"default": false}})).unwrap();
        }
        let account = ResolvedAccount {
            id: "account".into(),
            source: AccountSource::DefaultFallback,
            config: &config,
        };
        let fanout = FullResponseFanout::new(calls);
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let derived = runtime_funding(calls, metadata_bytes);
        let budget = AuctionBudget::new(derived.auction_charges.response);
        let receipt = budget
            .try_reserve(ChargeKind::Response, derived.auction_charges.response)
            .unwrap();
        let mut ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host,
            &account,
            &registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        ctx.response_reservation = Some(&receipt);
        let mut audit = DisclosureAudit::default();
        // Fixtures and the input request are prepared outside the window.
        // Add the already-live response buffers/metadata to the engine peak;
        // retaining them while decoding/assembling is not allocation-free.
        let before = alloc_track::reset_peak();
        let result = block_on(run_auction(&request, &ctx, &plan, &mut audit))
            .and_then(auctioneer_exchange::AuctionOutcome::into_bid_response)
            .map(|output| {
                serde_json::to_writer(io::sink(), &output).unwrap();
                serde_json::to_value(output).unwrap()
            });
        let peak = alloc_track::peak_since(before)
            .checked_add(fanout.retained)
            .unwrap();
        assert_eq!(budget.in_use(), derived.auction_charges.response);
        drop(receipt);
        assert_eq!(budget.in_use(), 0);
        ReceiptObserved {
            result,
            peak,
            derived,
            dispatched: fanout.dispatched.load(Ordering::SeqCst),
            fpd: fanout.fpd.load(Ordering::SeqCst),
        }
    }

    #[test]
    fn full_fpd_and_sixteen_full_responses_fit_the_runtime_derived_512_node_receipt() {
        for visible in [false, true] {
            thread::spawn(move || {
            let observed = runtime_receipt_case(full_fpd_request(visible), 16, false, 32 * 1024);
            assert_eq!(observed.derived.auction_charges.response, 216_469_664);
            assert_eq!(observed.derived.bookkeeping_term, 917_504);
            assert_eq!(observed.derived.debug_term, 9_310_368);
            assert_eq!(observed.dispatched, 16);
            assert_eq!(observed.fpd, 16);
            let output = observed.result.unwrap();
            assert_eq!(output["seatbid"].as_array().unwrap().len(), 16);
            for seat in output["seatbid"].as_array().unwrap() {
                assert_eq!(seat["bid"].as_array().unwrap().len(), 1);
                assert!(seat["bid"][0]["adm"].as_str().unwrap().len() > 32 * 1024 - 256);
                assert_eq!(seat["bid"][0]["w"], 300_i32);
                assert_eq!(seat["bid"][0]["h"], 250_i32);
            }
            let ordinary = observed.derived.auction_charges.response - observed.derived.debug_term;
            let peak = observed.peak;
            let margin = peak.checked_mul(6).unwrap().div_ceil(5);
            assert!(margin <= ordinary, "ordinary peak must not borrow debug funding");
            writeln!(io::stderr(), "full-fpd/512-nodes/status={visible}: calls=16 peak={peak} margin={margin} ordinary={ordinary}").unwrap();
        }).join().unwrap();
        }
    }

    #[test]
    fn zero_call_privacy_denials_fit_metadata_without_decoded_or_debug_funding() {
        for visible in [false, true] {
            thread::spawn(move || {
                let observed = runtime_receipt_case(full_fpd_request(visible), 0, true, 32 * 1024);
                assert_eq!(observed.dispatched, 0);
                assert_eq!(observed.fpd, 0);
                let ordinary = observed.derived.auction_charges.response - observed.derived.debug_term;
                assert_eq!(ordinary, observed.derived.bookkeeping_term);
                assert_eq!(ordinary, 917_504);
                let output = observed.result.unwrap();
                assert_eq!(output["id"], "r");
                assert_eq!(output["ext"]["warnings"].as_object().unwrap().len(), 16);
                for warnings in output["ext"]["warnings"].as_object().unwrap().values() {
                    assert_eq!(warnings.as_array().unwrap().len(), 1);
                    assert_eq!(warnings[0]["code"], 10017_u32);
                }
                let nonbids = &output["ext"]["prebid"]["seatnonbid"];
                if visible {
                    assert_eq!(nonbids.as_array().unwrap().len(), 16);
                    for row in nonbids.as_array().unwrap() {
                        assert_eq!(row["nonbid"], json!([{"impid": "i", "statuscode": 300_u32}]));
                    }
                } else { assert!(nonbids.is_null()); }
                let peak = observed.peak;
                let margin = peak.checked_mul(6).unwrap().div_ceil(5);
                assert!(margin <= ordinary, "metadata cannot borrow nonexistent call charges or prepaid debug");
                writeln!(io::stderr(), "zero-call-denials/status={visible}: peak={peak} margin={margin} ordinary={ordinary}").unwrap();
            }).join().unwrap();
        }
    }

    #[test]
    fn escaped_response_and_nonbid_ids_observe_exact_and_over_metadata_bytes() {
        for visible in [false, true] {
            for over in [false, true] {
                thread::spawn(move || {
                    let imp_id = "\0\n\"\\";
                    // Original wire-metadata byte boundary; the one-copy typed
                    // identity must fit the same funded retained ceiling.
                    assert_eq!(size_of::<String>() + size_of::<Option<Winner>>(), 64);
                    let id_bytes = if visible { 206_usize } else { 242 } + usize::from(over);
                    let id = "\0".repeat(id_bytes);
                    let input = json!({"id": id, "site": {"id": "s"},
                        "imp": [{"id": imp_id, "banner": {"w": 300_i32, "h": 250_i32}, "ext": {"bidder": {"fake_bb": {}}}}],
                        "ext": {"prebid": {"returnallbidstatus": visible}}});
                    let observed = runtime_receipt_case(input, 0, true, 512);
                    assert_eq!(observed.dispatched, 0);
                    if over {
                        let error = observed.result.unwrap_err();
                        assert_eq!(error.code(), 2);
                        assert_eq!(error.to_string(), "BadInput: response bookkeeping exceeds the materialization budget");
                    } else {
                        let output = observed.result.unwrap();
                        assert_eq!(output["id"], id);
                        assert_eq!(output["ext"]["warnings"]["fake_bb"][0]["code"], 10017_u32);
                        if visible {
                            assert_eq!(output["ext"]["prebid"]["seatnonbid"][0]["nonbid"], json!([{"impid": imp_id, "statuscode": 300_u32}]));
                        } else { assert!(output["ext"]["prebid"]["seatnonbid"].is_null()); }
                        let margin = observed.peak.checked_mul(6).unwrap().div_ceil(5);
                        assert!(margin <= observed.derived.bookkeeping_term);
                        writeln!(io::stderr(), "escaped-ids/status={visible}: peak={} margin={margin} ordinary={}", observed.peak, observed.derived.bookkeeping_term).unwrap();
                    }
                }).join().unwrap();
            }
        }
    }

    #[test]
    fn zero_call_global_warning_and_response_id_share_the_metadata_cap() {
        for over in [false, true] {
            thread::spawn(move || {
                // Original response/global/bidder/warning byte boundary; typed
                // identities do not consume the wire-metadata byte quota.
                assert_eq!(size_of::<String>() + size_of::<Option<Winner>>(), 64);
                let id = "\0".repeat(184 + usize::from(over));
                let input = json!({"id": id, "test": 1_i32, "site": {"id": "s"},
                    "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32}, "ext": {"bidder": {"fake_bb": {}}}}]});
                let observed = runtime_receipt_case(input, 0, true, 512);
                assert_eq!(observed.dispatched, 0);
                assert_eq!(observed.derived.auction_charges.response - observed.derived.debug_term, observed.derived.bookkeeping_term);
                if over {
                    let error = observed.result.unwrap_err();
                    assert_eq!(error.code(), 2);
                    assert_eq!(error.to_string(), "BadInput: response bookkeeping exceeds the materialization budget");
                } else {
                    let output = observed.result.unwrap();
                    assert_eq!(output["id"], id);
                    assert_eq!(output["ext"]["warnings"]["fake_bb"][0]["code"], 10017_u32);
                    assert_eq!(output["ext"]["warnings"]["prebid"], json!([{"code": 10002_u32, "message": "debug disabled for account"}]));
                    assert!(output["ext"]["debug"].is_null());
                    let margin = observed.peak.checked_mul(6).unwrap().div_ceil(5);
                    assert!(margin <= observed.derived.bookkeeping_term);
                    writeln!(io::stderr(), "zero-call-global-warning: peak={} margin={margin} ordinary={}", observed.peak, observed.derived.bookkeeping_term).unwrap();
                }
            }).join().unwrap();
        }
    }
}
