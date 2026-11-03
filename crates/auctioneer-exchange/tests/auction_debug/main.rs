//! Debug through the engine, without an HTTP envelope or a second response parse.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;
#[cfg(test)]
#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

use auctioneer_exchange::test_support as support;
#[cfg(test)]
use auctioneer_exchange::test_support::privacy_fixture;

#[cfg(test)]
mod tests {
    // Published provider captures require the native host filesystem. The fake
    // projection and privacy controls stay portable below.
    #[cfg(not(target_arch = "wasm32"))]
    mod provider_conformance;
    use super::{alloc_track, privacy_fixture, support};
    use crate::fake_bidder::{FakeBidder, create_bidder};
    use async_trait::async_trait;
    use auctioneer_bidder::{
        Bidder, BidderError, BidderErrorKind, BidderHttpResponse, BidderName, BidderRegistry,
        BidderRequestInfo, BidderResponse, DebugBodyEligibility, DebugBodySchema,
        OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::{
        AccountConfig, BidderInfo, Compression, HostConfig, ResourceLimits,
    };
    use auctioneer_exchange::fanout::{TransportError, TransportErrorKind};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionError, AuctionPlan, BidderCall,
        BidderCallResult, BidderFanout, FanoutError, FanoutInternalError, OversizeOrigin,
        OversizeRequestStage, ResolvedAccount, TimeoutCause, UndecodableEncoding, resolve_aliases,
        run_auction,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::debug_budget::debug_peak_bytes;
    use auctioneer_runtime::{
        AuctionBudget, ChargeKind, Clock, FakeClock, MonotonicInstant, SeededRng,
    };
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::iter::repeat_with;
    use std::mem::take;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    };
    use std::time::Duration;

    #[derive(Clone, Copy, Default)]
    enum AdapterErrors {
        #[default]
        Ordinary,
        SemanticErr,
        SemanticOk,
        MixedErr,
        MixedOk,
    }

    struct Adapter {
        schema: DebugBodySchema,
        schema_reads: Arc<AtomicUsize>,
        errors: AdapterErrors,
        oversized_first: bool,
        expire_after_build_reads: usize,
        clock: Arc<TestClock>,
    }

    impl Bidder for Adapter {
        fn debug_body_schema(&self) -> DebugBodySchema {
            self.schema_reads.fetch_add(1, Ordering::SeqCst);
            self.schema
        }

        fn make_requests(
            &self,
            request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            for (index, imp) in request.imp.iter().enumerate() {
                let mut scoped = request.clone();
                scoped.imp = vec![imp.clone()];
                if self.oversized_first && index == 0 {
                    scoped.other.insert(
                        "padding".into(),
                        json!((0_u32..8192_u32).collect::<Vec<_>>()),
                    );
                }
                let _outcome = sink.push_request(
                    info,
                    |meta| {
                        meta.set_method("POST")?;
                        meta.set_url(info.endpoint_template)?;
                        meta.header("Content-Type", "application/json; charset=utf-8")?;
                        meta.header("Authorization", "OUTBOUND_SECRET")?;
                        meta.imp_id(&imp.id)?;
                        Ok(())
                    },
                    |writer| writer.write_json(&scoped),
                );
            }
            self.clock
                .expire_in
                .store(self.expire_after_build_reads, Ordering::SeqCst);
            Ok(())
        }

        fn make_bids(
            &self,
            request: &BidRequest,
            sent: &OutboundRequestMeta,
            response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            let semantic = BidderError::bad_server_response("semantic failure");
            let other_unmarshal = BidderError::failed_to_unmarshal("response_parse");
            let mut parse = BidderError::failed_to_unmarshal("typed parse failure");
            parse.kind = BidderErrorKind::ResponseParse;
            match self.errors {
                AdapterErrors::SemanticErr => Err(vec![semantic, other_unmarshal]),
                AdapterErrors::MixedErr => Err(vec![semantic, other_unmarshal, parse]),
                AdapterErrors::Ordinary | AdapterErrors::SemanticOk | AdapterErrors::MixedOk => {
                    let mut result = FakeBidder::default().make_bids(request, sent, response)?;
                    if matches!(
                        self.errors,
                        AdapterErrors::SemanticOk | AdapterErrors::MixedOk
                    ) {
                        result.errors.push(semantic);
                        result.errors.push(other_unmarshal);
                    }
                    if matches!(self.errors, AdapterErrors::MixedOk) {
                        result.errors.push(parse);
                    }
                    Ok(result)
                }
            }
        }
    }

    struct TestClock {
        inner: FakeClock,
        tick_ms: AtomicUsize,
        expire_in: AtomicUsize,
        expiry_allocation_baseline: AtomicI64,
    }

    impl TestClock {
        fn new() -> Self {
            Self {
                inner: FakeClock::new(time::OffsetDateTime::UNIX_EPOCH),
                tick_ms: AtomicUsize::new(0),
                expire_in: AtomicUsize::new(0),
                expiry_allocation_baseline: AtomicI64::new(i64::MIN),
            }
        }

        fn expiry_allocation_peak(&self) -> Option<u64> {
            let baseline = self.expiry_allocation_baseline.load(Ordering::SeqCst);
            (baseline != i64::MIN).then(|| alloc_track::peak_since(baseline))
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> time::OffsetDateTime {
            self.inner.now()
        }

        fn now_monotonic(&self) -> MonotonicInstant {
            let remaining = self.expire_in.load(Ordering::SeqCst);
            if remaining > 0 && self.expire_in.fetch_sub(1, Ordering::SeqCst) == 1 {
                self.inner.advance(Duration::from_secs(2));
                self.expiry_allocation_baseline
                    .store(alloc_track::reset_peak(), Ordering::SeqCst);
            }
            self.inner.advance(Duration::from_millis(
                u64::try_from(self.tick_ms.load(Ordering::SeqCst)).unwrap(),
            ));
            self.inner.now_monotonic()
        }
    }

    enum Outcome {
        Body(Value),
        Uncertified(Value),
        Syntax,
        Empty,
        Error(FanoutError),
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Sent {
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    struct Fanout<'a> {
        outcomes: Mutex<Vec<(Outcome, Duration)>>,
        sent: Mutex<Vec<Sent>>,
        clock: &'a TestClock,
        tick_after_send: usize,
        advance_after_send: Duration,
        wrong_batch: bool,
    }

    #[async_trait(?Send)]
    impl BidderFanout for Fanout<'_> {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut sent = self.sent.lock().unwrap();
            let timeouts = calls.iter().map(|call| call.timeout).collect::<Vec<_>>();
            for call in calls {
                sent.push(Sent {
                    method: call.request.method().into(),
                    url: call.request.url().into(),
                    headers: call
                        .request
                        .headers()
                        .map(|(k, v)| (k.into(), v.into()))
                        .collect(),
                    body: call.request.body().to_vec(),
                });
            }
            self.clock
                .tick_ms
                .store(self.tick_after_send, Ordering::SeqCst);
            self.clock.inner.advance(self.advance_after_send);
            if self.wrong_batch {
                return Ok(Vec::new());
            }
            let outcomes = take(&mut *self.outcomes.lock().unwrap());
            assert_eq!(outcomes.len(), sent.len(), "fixed positional fixture");
            for ((outcome, elapsed), timeout) in outcomes.iter().zip(timeouts) {
                if matches!(
                    outcome,
                    Outcome::Error(FanoutError::Timeout(TimeoutCause::Bidder))
                ) {
                    assert!(*elapsed >= timeout, "fixture expires the per-call deadline");
                    assert!(
                        self.clock.now_monotonic() < deadline.deadline_at,
                        "capture retains auction budget after per-call timeout"
                    );
                }
            }
            Ok(outcomes
                .into_iter()
                .map(|(outcome, elapsed)| BidderCallResult {
                    result: match outcome {
                        Outcome::Body(body) => Ok(BidderHttpResponse::from_transport(
                            200,
                            vec![
                                ("Content-Type".into(), "application/json".into()),
                                ("Content-Encoding".into(), "gzip".into()),
                                ("Set-Cookie".into(), "RESPONSE_SECRET".into()),
                            ],
                            serde_json::to_vec(&body).unwrap(),
                            DebugBodyEligibility::SingleTextValue,
                        )),
                        Outcome::Syntax => Ok(BidderHttpResponse::from_transport(
                            200,
                            vec![("Content-Type".into(), "application/json".into())],
                            b"{".to_vec(),
                            DebugBodyEligibility::SingleTextValue,
                        )),
                        Outcome::Uncertified(body) => Ok(BidderHttpResponse::new(
                            200,
                            vec![("Content-Type".into(), "application/json".into())],
                            serde_json::to_vec(&body).unwrap(),
                        )),
                        Outcome::Empty => Ok(BidderHttpResponse::new(204, vec![], vec![])),
                        Outcome::Error(error) => Err(error),
                    },
                    elapsed,
                })
                .collect())
        }
    }

    struct Fixture {
        host: HostConfig,
        account: AccountConfig,
        caller_debug: bool,
        bidders: Vec<(&'static str, bool)>,
        bidder_factory: Option<fn() -> Box<dyn Bidder>>,
        disabled: BTreeSet<BidderName>,
        samples: BTreeMap<BidderName, f64>,
        schema: DebugBodySchema,
        errors: AdapterErrors,
        oversized_first: bool,
        gzip: bool,
        expired: bool,
        expire_after_build_reads: usize,
        tick_after_send: usize,
        advance_after_send: Duration,
        response_bytes: Option<u64>,
        wrong_batch: bool,
    }

    impl Default for Fixture {
        fn default() -> Self {
            Self {
                host: HostConfig {
                    resource_limits: ResourceLimits {
                        default_tmax_ms: 1000,
                        max_tmax_ms: 2000,
                        max_bidders_per_auction: 50,
                        max_outbound_requests: 100,
                        max_outbound_raw_bytes: 256 * 1024,
                        max_outbound_request_bytes: 256 * 1024,
                        max_bidder_response_bytes: 256 * 1024,
                        max_transport_bytes: 256 * 1024,
                        max_decoded_bytes: 256 * 1024,
                        max_parsed_nodes: 0x0001_0000,
                    },
                    ..HostConfig::default()
                },
                account: AccountConfig {
                    debug_allow: true,
                    ..AccountConfig::default()
                },
                caller_debug: true,
                bidders: vec![("fake_bb", true)],
                bidder_factory: None,
                disabled: BTreeSet::new(),
                samples: BTreeMap::new(),
                schema: DebugBodySchema::OpenRtb,
                errors: AdapterErrors::Ordinary,
                oversized_first: false,
                gzip: false,
                expired: false,
                expire_after_build_reads: 0,
                tick_after_send: 0,
                advance_after_send: Duration::ZERO,
                response_bytes: None,
                wrong_batch: false,
            }
        }
    }

    struct Observed {
        response: Result<Value, AuctionError>,
        sent: Vec<Sent>,
        schema_reads: usize,
        audit: DisclosureAudit,
        expiry_allocation_peak: Option<u64>,
    }

    impl Fixture {
        fn run(&self, value: Value, outcomes: Vec<(Outcome, Duration)>) -> Observed {
            let request = prepare(
                serde_json::from_value(value).unwrap(),
                &RequestHeaders::default(),
                &PrepareLimits {
                    max_impressions: 100,
                    ..PrepareLimits::default()
                },
            )
            .unwrap();
            let original = serde_json::to_value(&*request).unwrap();
            let clock = Arc::new(TestClock::new());
            let schema_reads = Arc::new(AtomicUsize::new(0));
            let mut registry = BidderRegistry::new();
            for &(name, debug_allow) in &self.bidders {
                let adapter: Box<dyn Bidder> = if let Some(factory) = self.bidder_factory {
                    factory()
                } else {
                    Box::new(Adapter {
                        schema: self.schema,
                        schema_reads: Arc::clone(&schema_reads),
                        errors: self.errors,
                        oversized_first: self.oversized_first,
                        clock: Arc::clone(&clock),
                        expire_after_build_reads: self.expire_after_build_reads,
                    })
                };
                registry
                    .register(
                        Box::leak(Box::new(BidderInfo {
                            name: name.into(),
                            endpoint: "https://bidder.example.test/private?token=SECRET".into(),
                            uses_https: true,
                            debug_allow,
                            bidder_timeout_ms: Some(10),
                            endpoint_compression: self.gzip.then_some(Compression::Gzip),
                            ..BidderInfo::default()
                        })),
                        adapter,
                    )
                    .expect("unique test bidder");
            }
            let account = ResolvedAccount {
                id: "publisher".into(),
                source: AccountSource::DefaultFallback,
                config: &self.account,
            };
            let plan = AuctionPlan::new(
                resolve_aliases(&request, &registry, &self.disabled, &self.host.aliases).unwrap(),
                self.disabled.clone(),
                support::response_budgets(&registry, &self.host),
                self.samples.clone(),
            );
            let fanout = Fanout {
                outcomes: Mutex::new(outcomes),
                sent: Mutex::new(Vec::new()),
                clock: &clock,
                tick_after_send: self.tick_after_send,
                advance_after_send: self.advance_after_send,
                wrong_batch: self.wrong_batch,
            };
            let rng = SeededRng::new(42);
            let start = clock.now_monotonic();
            if self.expired {
                clock.inner.advance(Duration::from_secs(2));
            }
            let mut ctx = AuctionContext::with_request_start(
                start,
                &self.host,
                &account,
                &registry,
                &fanout,
                &*clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                self.caller_debug,
            );
            let budget = AuctionBudget::new(self.response_bytes.unwrap_or(0));
            let receipt = self
                .response_bytes
                .map(|bytes| budget.try_reserve(ChargeKind::Response, bytes).unwrap());
            ctx.response_reservation = receipt.as_ref();
            let mut audit = DisclosureAudit::default();
            let result = block_on(run_auction(&request, &ctx, &plan, &mut audit));
            let expiry_allocation_peak = clock.expiry_allocation_peak();
            let response = result.map(|response| serde_json::to_value(response).unwrap());
            assert_eq!(serde_json::to_value(&*request).unwrap(), original);
            Observed {
                response,
                sent: fanout.sent.into_inner().unwrap(),
                schema_reads: schema_reads.load(Ordering::SeqCst),
                audit,
                expiry_allocation_peak,
            }
        }
    }

    fn request(names: &[&str], count: usize) -> Value {
        let mut request =
            serde_json::to_value(privacy_fixture::request().expect("loaded fixture deserializes"))
                .unwrap();
        request.as_object_mut().unwrap().remove("app");
        request["user"]["ext"]
            .as_object_mut()
            .unwrap()
            .remove("consent");
        request["regs"] = json!({"coppa": 1_i32, "ext": {"gpc": "1"}});
        request["ext"] =
            json!({"prebid": {"returnallbidstatus": true, "targeting": {"includeformat": true}}});
        request["imp"] = json!(
            (0..count)
                .map(|index| json!({
                "id": format!("imp-{index}"), "banner": {"w": 300_i32, "h": 250_i32},
                        "ext": {"bidder": names.iter().map(|name| ((*name).to_owned(), json!({})))
                            .collect::<serde_json::Map<String, Value>>()}
                    }))
                .collect::<Vec<_>>()
        );
        request
    }

    fn bid(imp: usize) -> Value {
        json!({"id": "RESPONSE_ID", "cur": "USD", "seatbid": [{"seat": "fake_PRIVATE_SEAT", "bid": [{
            "id": "BID_SECRET", "impid": format!("imp-{imp}"), "price": 1.5_f64,
            "w": 300_i32, "h": 250_i32, "mtype": 1_i32, "adm": "CREATIVE_SECRET"
        }]}]})
    }

    fn body(body: Value, elapsed_ms: u64) -> (Outcome, Duration) {
        (Outcome::Body(body), Duration::from_millis(elapsed_ms))
    }

    fn empty_debug() -> Value {
        json!({"httpcalls": {}, "truncated": false, "dropped_calls": 0_u32})
    }

    fn warning_count(response: &Value, bidder: &str, code: u32) -> usize {
        response["ext"]["warnings"][bidder]
            .as_array()
            .map_or(0, |warnings| {
                warnings
                    .iter()
                    .filter(|warning| warning["code"] == code)
                    .count()
            })
    }

    fn assert_debug_capture_preserves_ordinary_output(
        name: &'static str,
        params: Value,
        factory: fn() -> Box<dyn Bidder>,
    ) {
        let mut fixture = Fixture {
            bidders: vec![(name, true)],
            bidder_factory: Some(factory),
            caller_debug: false,
            ..Fixture::default()
        };
        let mut value = request(&[name], 1);
        value["imp"][0]["ext"]["bidder"][name] = params;
        let off = fixture.run(value.clone(), vec![body(bid(0), 75)]);
        fixture.caller_debug = true;
        let on = fixture.run(value, vec![body(bid(0), 75)]);
        assert_eq!(off.sent, on.sent, "{name}");
        let mut response = on.response.unwrap();
        let debug = response["ext"]
            .as_object_mut()
            .unwrap()
            .remove("debug")
            .unwrap();
        assert_eq!(response, off.response.unwrap(), "{name}");
        let row = &debug["httpcalls"][name][0];
        assert_eq!(row["request"]["body_status"], "captured", "{name}");
        assert_eq!(row["response"]["body_status"], "captured", "{name}");
        assert_eq!(
            response["seatbid"][0]["bid"].as_array().unwrap().len(),
            1,
            "{name}"
        );
    }

    #[test]
    fn canonical_fake_projects_both_sides_without_changing_ordinary_output() {
        assert_debug_capture_preserves_ordinary_output("fake_bb", json!({}), create_bidder);
    }

    #[test]
    fn account_denial_survives_normal_no_bid_and_expired_returns() {
        for (no_bid, expired) in [(false, false), (true, false), (false, true)] {
            let mut fixture = Fixture {
                account: AccountConfig::default(),
                expired,
                ..Fixture::default()
            };
            if no_bid {
                fixture.samples.insert(BidderName::new("fake_bb"), 0.0_f64);
            }
            let outcomes = if !no_bid && !expired {
                vec![body(bid(0), 5)]
            } else {
                vec![]
            };
            let observed = fixture.run(request(&["fake_bb"], 1), outcomes);
            let response = observed.response.unwrap();
            assert_eq!(warning_count(&response, "prebid", 10002), 1);
            assert_eq!(warning_count(&response, "fake_bb", 10003), 0);
            assert!(response["ext"].get("debug").is_none());
            assert_eq!(observed.schema_reads, 0);
        }
    }

    #[test]
    fn body_signals_are_ored_with_the_caller_flag_without_forcing_permission() {
        for signal in [
            json!({"test": 1_i32}),
            json!({"debug": true}),
            json!({"debug": 1_i32}),
        ] {
            let mut value = request(&["fake_bb"], 1);
            if let Some(test) = signal.get("test") {
                value["test"] = test.clone();
            }
            if let Some(debug) = signal.get("debug") {
                value["ext"]["prebid"]["debug"] = debug.clone();
            }
            for allow in [false, true] {
                let fixture = Fixture {
                    caller_debug: false,
                    account: AccountConfig {
                        debug_allow: allow,
                        ..AccountConfig::default()
                    },
                    ..Fixture::default()
                };
                let response = fixture
                    .run(value.clone(), vec![body(bid(0), 5)])
                    .response
                    .unwrap();
                assert_eq!(response["ext"].get("debug").is_some(), allow);
                assert_eq!(
                    warning_count(&response, "prebid", 10002),
                    usize::from(!allow)
                );
            }
        }
    }

    #[test]
    fn unrequested_debug_has_no_capture_or_denial_warnings() {
        let fixture = Fixture {
            caller_debug: false,
            account: AccountConfig::default(),
            ..Fixture::default()
        };
        let observed = fixture.run(request(&["fake_bb"], 1), vec![body(bid(0), 5)]);
        let response = observed.response.unwrap();
        assert!(response["ext"].get("debug").is_none());
        assert_eq!(warning_count(&response, "prebid", 10002), 0);
        assert_eq!(warning_count(&response, "fake_bb", 10003), 0);
        assert_eq!(observed.schema_reads, 0);
    }

    #[test]
    fn denied_base_alias_warns_once_per_observable_name_despite_multiple_calls() {
        let fixture = Fixture {
            bidders: vec![("fake_bb", false), ("fake_shadow", true)],
            ..Fixture::default()
        };
        let mut value = request(&["fake_bb", "fake_shadow"], 2);
        value["ext"]["prebid"]["aliases"] = json!({"fake_shadow": "fake_bb"});
        let observed = fixture.run(
            value,
            vec![
                body(bid(0), 1),
                body(bid(1), 2),
                body(bid(0), 3),
                body(bid(1), 4),
            ],
        );
        let response = observed.response.unwrap();
        for name in ["fake_bb", "fake_shadow"] {
            assert_eq!(warning_count(&response, name, 10003), 1);
        }
        assert_eq!(response["ext"]["debug"], empty_debug());
        assert_eq!(
            observed.schema_reads, 0,
            "denied calls cannot project bodies"
        );
    }

    #[test]
    fn permitted_empty_call_set_has_the_exact_empty_shape() {
        let mut fixture = Fixture::default();
        fixture.samples.insert(BidderName::new("fake_bb"), 0.0_f64);
        let response = fixture
            .run(request(&["fake_bb"], 1), vec![])
            .response
            .unwrap();
        assert_eq!(response["ext"]["debug"], empty_debug());
    }

    #[test]
    fn same_drain_timeouts_forward_equal_floored_elapsed_instead_of_resampling() {
        let fixture = Fixture {
            tick_after_send: 1,
            advance_after_send: Duration::from_micros(17_345),
            ..Fixture::default()
        };
        let response = fixture
            .run(
                request(&["fake_bb"], 2),
                repeat_with(|| {
                    (
                        Outcome::Error(FanoutError::Timeout(TimeoutCause::Bidder)),
                        Duration::from_micros(17_345),
                    )
                })
                .take(2)
                .collect(),
            )
            .response
            .unwrap();
        let rows = response["ext"]["debug"]["httpcalls"]["fake_bb"]
            .as_array()
            .unwrap();
        assert_eq!(rows.len(), 2);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row["call_index"], index);
            assert_eq!(row["elapsed_ms"], 17_u32);
            assert_eq!(row["error"], "timeout");
            assert!(row.get("response").is_none());
        }
        assert_eq!(
            response["ext"]["errors"]["fake_bb"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(warning_count(&response, "prebid", 10002), 0);
    }

    #[test]
    fn positional_rows_preserve_completion_and_observation_times_without_sorting_by_elapsed() {
        for elapsed in [[70, 5], [70, 75]] {
            let response = Fixture::default()
                .run(
                    request(&["fake_bb"], 2),
                    vec![body(bid(0), elapsed[0]), body(bid(1), elapsed[1])],
                )
                .response
                .unwrap();
            let rows = response["ext"]["debug"]["httpcalls"]["fake_bb"]
                .as_array()
                .unwrap();
            assert_eq!(rows[0]["elapsed_ms"], elapsed[0]);
            assert_eq!(rows[1]["elapsed_ms"], elapsed[1]);
            assert_eq!(rows[0]["call_index"], 0_u32);
            assert_eq!(rows[1]["call_index"], 1_u32);
        }
    }

    #[test]
    fn typed_parse_provenance_in_both_adapter_channels_preserves_sibling_bids_once() {
        for errors in [
            AdapterErrors::SemanticErr,
            AdapterErrors::MixedErr,
            AdapterErrors::SemanticOk,
            AdapterErrors::MixedOk,
        ] {
            let fixture = Fixture {
                errors,
                ..Fixture::default()
            };
            let response = fixture
                .run(request(&["fake_bb"], 1), vec![body(bid(0), 5)])
                .response
                .unwrap();
            let mixed = matches!(errors, AdapterErrors::MixedErr | AdapterErrors::MixedOk);
            let ok = matches!(errors, AdapterErrors::SemanticOk | AdapterErrors::MixedOk);
            let row = &response["ext"]["debug"]["httpcalls"]["fake_bb"][0];
            assert_eq!(row.get("error"), mixed.then_some(&json!("response_parse")));
            assert_eq!(row["response"]["body_status"], "captured");
            assert_eq!(
                response["ext"]["errors"]["fake_bb"]
                    .as_array()
                    .unwrap()
                    .len(),
                if mixed { 3 } else { 2 }
            );
            assert_eq!(
                response["seatbid"].as_array().map_or(0, Vec::len),
                usize::from(ok)
            );
            if ok {
                let bid = &response["seatbid"][0]["bid"][0];
                assert_eq!(bid["price"], 1.5_f64);
                assert_eq!(bid["ext"]["prebid"]["type"], "banner");
                assert_eq!(bid["ext"]["prebid"]["targeting"]["hb_pb"], "1.50");
                assert_eq!(response["seatbid"][0]["bid"].as_array().unwrap().len(), 1);
            }
        }
    }

    #[test]
    fn syntax_and_installed_node_budget_failures_are_typed_response_parse() {
        for syntax in [true, false] {
            let mut fixture = Fixture::default();
            fixture.host.resource_limits.max_parsed_nodes = 48;
            let mut oversized = bid(0);
            oversized["unknown"] = json!((0_i32..80_i32).collect::<Vec<_>>());
            let outcome = if syntax {
                (Outcome::Syntax, Duration::from_millis(5))
            } else {
                body(oversized, 5)
            };
            let response = fixture
                .run(request(&["fake_bb"], 1), vec![outcome])
                .response
                .unwrap();
            let row = &response["ext"]["debug"]["httpcalls"]["fake_bb"][0];
            assert_eq!(row["error"], "response_parse");
            assert_eq!(
                response["ext"]["errors"]["fake_bb"][0]["code"],
                if syntax { 14_u32 } else { 4_u32 }
            );
            assert!(row["response"].get("body").is_none());
        }
    }

    #[test]
    fn finalization_rollback_does_not_consume_a_call_index_or_misassociate_results() {
        for gzip in [false, true] {
            let mut fixture = Fixture {
                oversized_first: true,
                gzip,
                ..Fixture::default()
            };
            fixture.host.resource_limits.max_outbound_request_bytes = 4096;
            let observed = fixture.run(request(&["fake_bb"], 2), vec![body(bid(1), 75)]);
            assert_eq!(observed.sent.len(), 1);
            let response = observed.response.unwrap();
            let debug = &response["ext"]["debug"];
            let rows = debug["httpcalls"]["fake_bb"].as_array().unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["call_index"], 0_u32);
            assert_eq!(rows[0]["elapsed_ms"], 75_u32);
            assert!(rows[0].get("error").is_none());
            assert_eq!(debug["dropped_calls"], 0_u32);
            assert_eq!(
                response["ext"]["errors"]["fake_bb"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(response["seatbid"][0]["bid"][0]["impid"], "imp-1");
        }
    }

    #[test]
    fn expiry_during_request_projection_stops_before_gzip_reserves_its_encoded_buffer() {
        // After make_requests: timing, pre-build, bidder, call, capture entry,
        // origin, then the post-projection deadline observation expires.
        let fixture = Fixture {
            gzip: true,
            expire_after_build_reads: 7,
            bidders: vec![("fake_aa", true), ("fake_bb", false)],
            ..Fixture::default()
        };
        let observed = fixture.run(request(&["fake_aa", "fake_bb"], 1), vec![]);
        let peak = observed.expiry_allocation_peak.unwrap();
        let encoded_cap = fixture.host.resource_limits.max_outbound_request_bytes;
        assert_eq!(
            peak, 0,
            "expiry during projection must precede gzip's {encoded_cap}-byte encoded reservation; post-expiry peak was {peak}"
        );
        assert_eq!(
            observed.schema_reads, 1,
            "permitted request reached projection"
        );
        assert!(
            observed.sent.is_empty(),
            "the expired partial batch cannot reach fanout"
        );
        let response = observed.response.unwrap();
        assert!(
            response["ext"].get("debug").is_none(),
            "expired capture emits no indexed call rows"
        );
        assert_eq!(warning_count(&response, "fake_aa", 10003), 0);
        assert_eq!(warning_count(&response, "fake_bb", 10003), 0);
        assert_eq!(response["ext"]["errors"]["prebid"][0]["code"], 12_u32);
    }

    #[test]
    fn gzip_projects_precompression_body_and_final_headers_without_changing_transport_or_bids() {
        let mut fixture = Fixture {
            gzip: true,
            caller_debug: false,
            ..Fixture::default()
        };
        let value = request(&["fake_bb"], 1);
        let off = fixture.run(value.clone(), vec![body(bid(0), 70)]);
        fixture.caller_debug = true;
        let on = fixture.run(value, vec![body(bid(0), 70)]);
        assert_eq!(off.sent, on.sent);
        assert_eq!(on.sent[0].body[..2], [0x1f, 0x8b]);
        assert_eq!(off.audit, on.audit);
        let mut response = on.response.unwrap();
        let debug = response["ext"]
            .as_object_mut()
            .unwrap()
            .remove("debug")
            .unwrap();
        assert_eq!(response, off.response.unwrap());
        let row = &debug["httpcalls"]["fake_bb"][0];
        assert_eq!(
            row["request"]["endpoint_origin"],
            "https://bidder.example.test"
        );
        assert_eq!(
            row["request"]["headers"]["content-encoding"],
            json!(["gzip"])
        );
        assert_eq!(row["request"]["headers"]["sec-gpc"], json!(["1"]));
        assert_eq!(row["request"]["body_status"], "captured");
        let projected: Value =
            serde_json::from_str(row["request"]["body"].as_str().unwrap()).unwrap();
        assert_eq!(projected["imp"][0]["banner"]["w"], 300_u32);
        assert!(
            projected["device"].get("ip").is_none(),
            "COPPA scrub precedes debug projection"
        );
        assert_eq!(projected["device"]["dnt"], 1_u32);
        assert_eq!(row["response"]["body_status"], "captured");
        let encoded = serde_json::to_string(&debug).unwrap();
        assert!(encoded.len() <= 0x0001_0000);
        for secret in [
            "OUTBOUND_SECRET",
            "RESPONSE_SECRET",
            "CREATIVE_SECRET",
            "BID_SECRET",
            "RESPONSE_ID",
            "fake_PRIVATE_SEAT",
            "token=",
        ] {
            assert!(!encoded.contains(secret), "debug leaked {secret}");
        }
    }

    #[test]
    fn pre_fanout_exclusions_never_produce_bidder_denial_warnings() {
        for exclusion in [
            "privacy",
            "disabled",
            "sampled",
            "cap",
            "outbound-cap",
            "expired",
            "built-expired",
            "finalized-expired",
            "finalization",
        ] {
            let mut fixture = Fixture {
                bidders: vec![("fake_bb", false)],
                ..Fixture::default()
            };
            let mut value = request(&["fake_bb"], 1);
            let mut outcomes = vec![];
            match exclusion {
                "privacy" => {
                    fixture.account.privacy.allowactivities =
                        serde_json::from_value(json!({"fetchBids": {"default": false}})).unwrap();
                }
                "disabled" => {
                    fixture.disabled.insert(BidderName::new("fake_bb"));
                    fixture.bidders.insert(0, ("fake_aa", true));
                    value = request(&["fake_aa", "fake_bb"], 1);
                    outcomes.push(body(bid(0), 5));
                }
                "sampled" => {
                    fixture.samples.insert(BidderName::new("fake_bb"), 0.0_f64);
                }
                "cap" => {
                    fixture.bidders.insert(0, ("fake_aa", true));
                    fixture.host.resource_limits.max_bidders_per_auction = 1;
                    value = request(&["fake_aa", "fake_bb"], 1);
                    outcomes.push(body(bid(0), 5));
                }
                "outbound-cap" => {
                    fixture.bidders.insert(0, ("fake_aa", true));
                    fixture.host.resource_limits.max_outbound_requests = 1;
                    value = request(&["fake_aa", "fake_bb"], 1);
                    outcomes.push(body(bid(0), 5));
                }
                "expired" => fixture.expired = true,
                "built-expired" => fixture.expire_after_build_reads = 2,
                // Include the post-projection check before the final outer
                // read, so this row expires after slot finalization.
                "finalized-expired" => fixture.expire_after_build_reads = 6,
                "finalization" => fixture.host.resource_limits.max_outbound_request_bytes = 1,
                _ => panic!("unknown exclusion fixture"),
            }
            let observed = fixture.run(value, outcomes);
            let response = observed.response.unwrap();
            assert_eq!(warning_count(&response, "fake_bb", 10003), 0, "{exclusion}");
            assert_eq!(
                observed.schema_reads,
                usize::from(matches!(exclusion, "cap" | "disabled" | "outbound-cap"))
            );
            if !matches!(exclusion, "cap" | "disabled" | "outbound-cap") {
                assert!(observed.sent.is_empty(), "{exclusion}");
            }
        }
    }

    #[test]
    fn permission_gaps_keep_original_finalized_call_indices_and_do_not_count_as_dropped() {
        let fixture = Fixture {
            bidders: vec![("fake_aa", false), ("fake_bb", true)],
            ..Fixture::default()
        };
        let response = fixture
            .run(
                request(&["fake_aa", "fake_bb"], 1),
                vec![body(bid(0), 70), body(bid(0), 75)],
            )
            .response
            .unwrap();
        let debug = &response["ext"]["debug"];
        assert_eq!(debug["httpcalls"]["fake_bb"][0]["call_index"], 1_u32);
        assert!(debug["httpcalls"].get("fake_aa").is_none());
        assert_eq!(debug["dropped_calls"], 0_u32);
        assert_eq!(warning_count(&response, "fake_aa", 10003), 1);
    }

    #[test]
    fn empty_response_and_unsupported_schema_do_not_invent_parse_errors() {
        for schema in [DebugBodySchema::OpenRtb, DebugBodySchema::Unsupported] {
            let fixture = Fixture {
                schema,
                ..Fixture::default()
            };
            let response = fixture
                .run(
                    request(&["fake_bb"], 1),
                    vec![(Outcome::Empty, Duration::from_millis(5))],
                )
                .response
                .unwrap();
            let row = &response["ext"]["debug"]["httpcalls"]["fake_bb"][0];
            assert_eq!(row["response"]["body_status"], "empty");
            assert!(row.get("error").is_none());
        }
    }

    #[test]
    fn response_prepaid_debug_capacity_cannot_be_borrowed_for_ordinary_bookkeeping() {
        let bytes = debug_peak_bytes(100).unwrap();
        let fixture = Fixture {
            response_bytes: Some(bytes),
            ..Fixture::default()
        };
        let observed = fixture.run(request(&["fake_bb"], 1), vec![body(bid(0), 5)]);
        assert!(matches!(observed.response, Err(AuctionError::Resource(_))));
        assert!(
            observed.sent.is_empty(),
            "capacity is checked before capture or dispatch"
        );
    }

    #[test]
    fn batch_contract_breaches_remain_fatal_instead_of_manufacturing_debug() {
        let fixture = Fixture {
            wrong_batch: true,
            ..Fixture::default()
        };
        let observed = fixture.run(request(&["fake_bb"], 1), vec![]);
        assert!(
            matches!(observed.response, Err(AuctionError::Internal(message)) if message == "outbound batch contract violation")
        );
    }

    #[test]
    fn allowed_base_alias_uses_observable_keys_even_when_the_shadowed_bidder_denies_debug() {
        let fixture = Fixture {
            bidders: vec![("fake_bb", true), ("fake_shadow", false)],
            ..Fixture::default()
        };
        let mut value = request(&["fake_bb", "fake_shadow"], 1);
        value["ext"]["prebid"]["aliases"] = json!({"fake_shadow": "fake_bb"});
        let response = fixture
            .run(value, vec![body(bid(0), 70), body(bid(0), 75)])
            .response
            .unwrap();
        let calls = response["ext"]["debug"]["httpcalls"].as_object().unwrap();
        assert_eq!(
            calls.keys().map(String::as_str).collect::<Vec<_>>(),
            ["fake_bb", "fake_shadow"]
        );
        for (index, name) in ["fake_bb", "fake_shadow"].into_iter().enumerate() {
            assert_eq!(calls[name][0]["call_index"], index);
            assert_eq!(calls[name][0]["response"]["body_status"], "captured");
            assert_eq!(warning_count(&response, name, 10003), 0);
        }
    }

    #[test]
    fn transport_failures_keep_fixed_metadata_and_never_parse_or_invent_response_bodies() {
        let errors = [
            (FanoutError::Preflight, "preflight", 4_u32),
            (
                FanoutError::OversizeRequest {
                    stage: OversizeRequestStage::Raw,
                },
                "oversize_request",
                5,
            ),
            (
                FanoutError::OversizeResponse(OversizeOrigin::RequestHeaders),
                "oversize_request",
                4,
            ),
            (
                FanoutError::OversizeResponse(OversizeOrigin::DecodedBody),
                "oversize_response",
                4,
            ),
            (
                FanoutError::OversizeResponse(OversizeOrigin::Unclassified),
                "oversize_response",
                4,
            ),
            (
                FanoutError::UndecodableResponse(UndecodableEncoding::Unsupported(
                    "CODEC_SECRET".into(),
                )),
                "decode",
                4,
            ),
            (FanoutError::BadServerResponse, "transport", 4),
            (
                FanoutError::Transport(TransportError {
                    kind: TransportErrorKind::Initial,
                }),
                "transport",
                4,
            ),
            (FanoutError::Unreachable, "transport", 999),
            (
                FanoutError::Other("TRANSPORT_SECRET".into()),
                "transport",
                999,
            ),
        ];
        for (error, category, code) in errors {
            let response = Fixture::default()
                .run(
                    request(&["fake_bb"], 1),
                    vec![(Outcome::Error(error), Duration::from_millis(75))],
                )
                .response
                .unwrap();
            let row = &response["ext"]["debug"]["httpcalls"]["fake_bb"][0];
            assert_eq!(row["error"], category);
            assert_eq!(row["elapsed_ms"], 75_u32);
            assert!(row.get("response").is_none());
            assert_eq!(response["ext"]["errors"]["fake_bb"][0]["code"], code);
            assert_eq!(
                response["ext"]["errors"]["fake_bb"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            let serialized = serde_json::to_string(&response).unwrap();
            assert!(!serialized.contains("TRANSPORT_SECRET"));
            assert!(!serialized.contains("CODEC_SECRET"));
        }
    }

    #[test]
    fn mixed_transport_parse_and_valid_results_retain_their_positions_and_exactly_one_valid_bid() {
        let response = Fixture::default()
            .run(
                request(&["fake_bb"], 3),
                vec![
                    (
                        Outcome::Error(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
                        Duration::from_micros(17_345),
                    ),
                    (Outcome::Syntax, Duration::from_millis(70)),
                    body(bid(2), 75),
                ],
            )
            .response
            .unwrap();
        let rows = response["ext"]["debug"]["httpcalls"]["fake_bb"]
            .as_array()
            .unwrap();
        assert_eq!(
            rows.iter()
                .map(|row| row["call_index"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(rows[0]["error"], "timeout");
        assert_eq!(rows[0]["elapsed_ms"], 17_u32);
        assert_eq!(rows[1]["error"], "response_parse");
        assert_eq!(rows[2]["elapsed_ms"], 75_u32);
        assert!(rows[2].get("error").is_none());
        assert_eq!(
            response["ext"]["errors"]["fake_bb"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(
            response["ext"]["errors"].get("prebid").is_none(),
            "slot cause is not auction-clock expiry"
        );
        assert_eq!(response["seatbid"][0]["bid"].as_array().unwrap().len(), 1);
        assert_eq!(response["seatbid"][0]["bid"][0]["impid"], "imp-2");
    }

    #[test]
    fn queued_never_dispatched_timeouts_keep_the_shared_fanout_anchor() {
        let response = Fixture::default()
            .run(
                request(&["fake_bb"], 2),
                repeat_with(|| {
                    (
                        Outcome::Error(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
                        Duration::from_micros(75_999),
                    )
                })
                .take(2)
                .collect(),
            )
            .response
            .unwrap();
        let rows = response["ext"]["debug"]["httpcalls"]["fake_bb"]
            .as_array()
            .unwrap();
        assert_eq!(rows[0]["elapsed_ms"], 75_u32);
        assert_eq!(rows[1]["elapsed_ms"], 75_u32);
        assert!(rows.iter().all(|row| row.get("response").is_none()));
    }

    #[test]
    fn elapsed_saturates_only_the_supplied_positional_value() {
        let response = Fixture::default()
            .run(
                request(&["fake_bb"], 1),
                vec![(
                    Outcome::Error(FanoutError::Unreachable),
                    Duration::from_secs(u64::from(u32::MAX)),
                )],
            )
            .response
            .unwrap();
        assert_eq!(
            response["ext"]["debug"]["httpcalls"]["fake_bb"][0]["elapsed_ms"],
            u32::MAX
        );
    }

    #[test]
    fn projection_omission_does_not_turn_valid_bids_into_parse_errors() {
        for (schema, certified) in [
            (DebugBodySchema::OpenRtb, false),
            (DebugBodySchema::Unsupported, true),
        ] {
            let fixture = Fixture {
                schema,
                ..Fixture::default()
            };
            let outcome = if certified {
                Outcome::Body(bid(0))
            } else {
                Outcome::Uncertified(bid(0))
            };
            let response = fixture
                .run(
                    request(&["fake_bb"], 1),
                    vec![(outcome, Duration::from_millis(5))],
                )
                .response
                .unwrap();
            let row = &response["ext"]["debug"]["httpcalls"]["fake_bb"][0];
            assert_eq!(row["response"]["body_status"], "omitted_type");
            assert!(row.get("error").is_none());
            assert!(row["response"].get("body").is_none());
            assert_eq!(response["seatbid"][0]["bid"].as_array().unwrap().len(), 1);
            assert_eq!(response["ext"]["debug"]["truncated"], true);
            assert_eq!(response["ext"]["debug"]["dropped_calls"], 0_u32);
        }
    }

    #[test]
    fn valid_json_semantic_failure_keeps_the_valid_sibling_without_a_parse_category() {
        let mut body = bid(0);
        body["seatbid"][0]["bid"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "bad-bid", "impid": "unknown", "price": 2.0_f64,
            }));
        let response = Fixture::default()
            .run(request(&["fake_bb"], 1), vec![self::body(body, 5)])
            .response
            .unwrap();
        assert!(
            response["ext"]["debug"]["httpcalls"]["fake_bb"][0]
                .get("error")
                .is_none()
        );
        assert_eq!(
            response["ext"]["errors"]["fake_bb"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(response["seatbid"][0]["bid"].as_array().unwrap().len(), 1);
        assert_eq!(response["seatbid"][0]["bid"][0]["impid"], "imp-0");
    }

    #[test]
    fn capture_caps_omit_late_rows_without_changing_outbound_bytes_or_ordinary_output() {
        let mut fixture = Fixture {
            caller_debug: false,
            ..Fixture::default()
        };
        let value = request(&["fake_bb"], 70);
        let off = fixture.run(
            value.clone(),
            (0..70_usize).map(|index| body(bid(index), 5)).collect(),
        );
        fixture.caller_debug = true;
        let on = fixture.run(
            value,
            (0..70_usize).map(|index| body(bid(index), 5)).collect(),
        );
        assert_eq!(off.sent, on.sent);
        let mut response = on.response.unwrap();
        let debug = response["ext"]
            .as_object_mut()
            .unwrap()
            .remove("debug")
            .unwrap();
        assert_eq!(response, off.response.unwrap());
        let rows = debug["httpcalls"]["fake_bb"].as_array().unwrap();
        assert!(rows.len() <= 64);
        assert_eq!(debug["dropped_calls"], 70 - rows.len());
        assert_eq!(debug["truncated"], true);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row["call_index"], index);
        }
        assert!(serde_json::to_vec(&debug).unwrap().len() <= 0x0001_0000);
        assert_eq!(response["seatbid"][0]["bid"].as_array().unwrap().len(), 70);
    }

    #[test]
    fn capture_deadline_exhaustion_omits_debug_wholly_without_losing_a_valid_bid() {
        let fixture = Fixture {
            advance_after_send: Duration::from_secs(2),
            ..Fixture::default()
        };
        let response = fixture
            .run(request(&["fake_bb"], 1), vec![body(bid(0), 70)])
            .response
            .unwrap();
        assert!(response["ext"].get("debug").is_none());
        assert_eq!(response["seatbid"][0]["bid"].as_array().unwrap().len(), 1);
        assert_eq!(response["ext"]["errors"]["prebid"][0]["code"], 12_u32);
    }

    #[test]
    fn all_finalizations_rejected_leave_a_successfully_empty_capture_without_dropped_calls() {
        let mut fixture = Fixture::default();
        fixture.host.resource_limits.max_outbound_request_bytes = 1;
        let observed = fixture.run(request(&["fake_bb"], 2), vec![]);
        let response = observed.response.unwrap();
        assert!(observed.sent.is_empty());
        assert_eq!(response["ext"]["debug"], empty_debug());
        assert_eq!(warning_count(&response, "fake_bb", 10003), 0);
        assert_eq!(
            response["ext"]["errors"]["fake_bb"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn funded_response_receipt_supports_capture_coexisting_with_bids_and_targeting() {
        let fixture = Fixture {
            response_bytes: Some(debug_peak_bytes(100).unwrap() + 16 * 1024 * 1024),
            ..Fixture::default()
        };
        let response = fixture
            .run(request(&["fake_bb"], 1), vec![body(bid(0), 5)])
            .response
            .unwrap();
        assert_eq!(
            response["ext"]["debug"]["httpcalls"]["fake_bb"][0]["response"]["body_status"],
            "captured"
        );
        assert_eq!(
            response["seatbid"][0]["bid"][0]["ext"]["prebid"]["targeting"]["hb_pb"],
            "1.50"
        );
    }
}
