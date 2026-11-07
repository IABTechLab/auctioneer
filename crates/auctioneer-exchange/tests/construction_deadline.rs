//! Late adapter construction spends the original auction budget, without fanout.

mod support;

mod body_allocations {
    #![expect(unsafe_code, reason = "test-only allocator delegates to System")]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    // Distinct capacities identify raw sink bodies and gzip finalization output.
    pub const CAP: usize = 0x0001_0001;
    pub const ENCODED_CAP: usize = 0x0001_0003;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
        static COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
        static ENCODED_COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
        static ENCODED_ALLOCATED: Cell<bool> = const { Cell::new(false) };
    }

    pub struct Tracking;

    impl Tracking {
        pub fn start() -> Self {
            COUNTS.with(|counts| counts.set((0, 0)));
            ENCODED_COUNTS.with(|counts| counts.set((0, 0)));
            ENCODED_ALLOCATED.with(|allocated| allocated.set(false));
            ACTIVE.with(|active| {
                assert!(
                    !active.replace(true),
                    "body-allocation tracking must not nest"
                );
            });
            Self
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Tracking {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(false));
        }
    }

    pub fn counts() -> (usize, usize) {
        COUNTS.with(Cell::get)
    }

    pub fn encoded_counts() -> (usize, usize) {
        ENCODED_COUNTS.with(Cell::get)
    }

    pub fn encoded_allocated() -> bool {
        ENCODED_ALLOCATED.with(Cell::get)
    }

    fn record(size: usize, allocated: bool) {
        let counter = match size {
            CAP => &COUNTS,
            ENCODED_CAP => &ENCODED_COUNTS,
            _ => return,
        };
        let _result = ACTIVE.try_with(|active| {
            if active.get() {
                let _result = counter.try_with(|counts| {
                    let (allocs, frees) = counts.get();
                    counts.set(if allocated {
                        (allocs.saturating_add(1), frees)
                    } else {
                        (allocs, frees.saturating_add(1))
                    });
                });
                if allocated && size == ENCODED_CAP {
                    let _encoded_result = ENCODED_ALLOCATED.try_with(|seen| seen.set(true));
                }
            }
        });
    }

    pub struct Allocator;

    // SAFETY: pointer and layout operations delegate unchanged to System.
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller supplied the allocation layout.
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                record(layout.size(), true);
            }
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller supplied the allocation layout.
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                record(layout.size(), true);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            record(layout.size(), false);
            // SAFETY: the pointer and layout came from a matching allocation.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: the caller supplied the original layout and new size.
            let next = unsafe { System.realloc(ptr, layout, new_size) };
            if !next.is_null() {
                record(layout.size(), false);
                record(new_size, true);
            }
            next
        }
    }
}

#[global_allocator]
static ALLOCATOR: body_allocations::Allocator = body_allocations::Allocator;

#[cfg(test)]
mod tests {
    use super::body_allocations;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderName, BidderRegistry, BidderRequestInfo,
        BidderResponse, DebugBodySchema, OutboundBuildError, OutboundRequestMeta,
        OutboundRequestSink,
    };
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, Compression};
    use auctioneer_exchange::construction_costs;
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, BootEnvelope, FanoutInternalError, ResolvedAccount, resolve_aliases,
        run_auction,
    };
    use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, WarningCode, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{Clock, FakeClock, MonotonicInstant, SeededRng};
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, LazyLock, Mutex};
    use std::time::Duration;

    static INFOS: LazyLock<Vec<BidderInfo>> = LazyLock::new(|| {
        [
            "fake_a0",
            "fake_b000",
            "fake_c000",
            "fake_d00",
            "fake_e0000",
        ]
        .into_iter()
        .map(|name| BidderInfo {
            name: name.into(),
            endpoint: "https://example.test/bid".into(),
            uses_https: true,
            ..BidderInfo::default()
        })
        .collect()
    });

    static GZIP_INFO: LazyLock<BidderInfo> = LazyLock::new(|| BidderInfo {
        name: "fake_gzip".into(),
        endpoint: "https://example.test/bid".into(),
        uses_https: true,
        endpoint_compression: Some(Compression::Gzip),
        ..BidderInfo::default()
    });

    static EMPTY_INFO: LazyLock<BidderInfo> = LazyLock::new(|| BidderInfo {
        name: "fake_z0".into(),
        endpoint: "https://example.test/bid".into(),
        uses_https: true,
        ..BidderInfo::default()
    });

    struct FinalizationClock {
        clock: Arc<FakeClock>,
        deadline: MonotonicInstant,
        expire_after_encoding: bool,
    }

    impl Clock for FinalizationClock {
        fn now(&self) -> time::OffsetDateTime {
            self.clock.now()
        }

        fn now_monotonic(&self) -> MonotonicInstant {
            if self.expire_after_encoding && body_allocations::encoded_allocated() {
                // Latch expiry on the actual gzip allocation, not clock-call order.
                self.clock.advance_to(self.deadline);
            }
            self.clock.now_monotonic()
        }
    }

    #[derive(Clone, Debug, Default)]
    struct Probe {
        makes: usize,
        metadata: usize,
        accepted: usize,
        rejected: usize,
        response_hooks: usize,
        buffers_at_return: (usize, usize),
        encoded_at_return: (usize, usize),
    }

    struct Scripted {
        clock: Arc<FakeClock>,
        probe: Arc<Mutex<Probe>>,
        spend: Duration,
        attempts: u8,
        accepted_attempts: u8,
        semantic_error: bool,
        partial_diagnostics: bool,
    }

    impl Bidder for Scripted {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        #[expect(
            clippy::panic_in_result_fn,
            clippy::unwrap_in_result,
            reason = "test adapter asserts construction outcomes and recording state"
        )]
        fn make_requests(
            &self,
            request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            if self.partial_diagnostics {
                sink.diagnostic(
                    auctioneer_bidder::ConstructionDiagnostic::InvalidImpressionParameters,
                );
            }
            let mut probe = self.probe.lock().unwrap();
            probe.makes = probe.makes.saturating_add(1);
            assert_eq!(
                request.tmax,
                Some(250),
                "adapter-visible tmax is not the host cap"
            );
            for attempt in 0..self.attempts {
                let outcome = sink.push_request(
                    info,
                    |meta| {
                        probe.metadata = probe.metadata.saturating_add(1);
                        meta.set_method("POST")?;
                        meta.set_url(info.endpoint_template)?;
                        meta.imp_id("imp-1")?;
                        Ok(())
                    },
                    |writer| writer.write_all(b"{}"),
                );
                if attempt < self.accepted_attempts {
                    outcome.expect("first request must fit, including after a sibling's refund");
                } else {
                    assert!(matches!(
                        outcome,
                        Err(OutboundBuildError::CallBudgetExhausted { .. })
                    ));
                    probe.rejected = probe.rejected.saturating_add(1);
                }
            }
            probe.accepted = sink.accepted_count();
            probe.buffers_at_return = body_allocations::counts();
            probe.encoded_at_return = body_allocations::encoded_counts();
            self.clock.advance(self.spend);
            if self.semantic_error {
                Err(vec![BidderError::bad_input("prior params")])
            } else {
                Ok(())
            }
        }

        #[expect(clippy::unwrap_in_result, reason = "test response-hook recording lock")]
        fn make_bids(
            &self,
            _: &BidRequest,
            _: &OutboundRequestMeta,
            _: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            let mut probe = self.probe.lock().unwrap();
            probe.response_hooks = probe.response_hooks.saturating_add(1);
            Ok(BidderResponse {
                bids: vec![],
                currency: "USD".into(),
                errors: vec![],
                auction_configs: Vec::new(),
            })
        }
    }

    #[derive(Default)]
    struct Dispatch {
        batches: usize,
        calls: usize,
        deadline: Option<MonotonicInstant>,
        gzip_calls: usize,
        raw_buffers: (usize, usize),
        encoded_buffers: (usize, usize),
    }

    #[derive(Default)]
    struct RecordingFanout(Mutex<Dispatch>);

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut sent = self.0.lock().unwrap();
            sent.batches = sent.batches.saturating_add(1);
            sent.calls = sent.calls.saturating_add(calls.len());
            sent.deadline = Some(deadline.deadline_at);
            sent.gzip_calls = sent.gzip_calls.saturating_add(
                calls
                    .iter()
                    .filter(|call| {
                        call.request.headers().any(|(name, value)| {
                            name.eq_ignore_ascii_case("Content-Encoding") && value == "gzip"
                        }) && call.request.body().starts_with(&[0x1f, 0x8b])
                    })
                    .count(),
            );
            sent.raw_buffers = body_allocations::counts();
            sent.encoded_buffers = body_allocations::encoded_counts();
            Ok(calls
                .into_iter()
                .map(|_| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], vec![])),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    struct Observed {
        response: BidResponse,
        probes: BTreeMap<String, Probe>,
        dispatch: Dispatch,
        buffers: (usize, usize),
        deadline: MonotonicInstant,
    }

    struct FinalizationObserved {
        auction: Observed,
        encoded_buffers: (usize, usize),
        finished_at: MonotonicInstant,
    }

    fn run(spend: Duration, diagnostics: bool) -> Observed {
        run_with_partials(spend, diagnostics, false)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one ordered fixture keeps the original deadline, adapter probes and allocation lifetime together"
    )]
    fn run_with_partials(
        spend: Duration,
        diagnostics: bool,
        partial_diagnostics: bool,
    ) -> Observed {
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        host.resource_limits.max_tmax_ms = 100;
        host.resource_limits.max_outbound_raw_bytes = u64::try_from(body_allocations::CAP).unwrap();
        host.resource_limits.max_outbound_requests = if diagnostics { 1 } else { 2 };
        host.resource_limits.max_bidders_per_auction = if diagnostics { 3 } else { 2 };
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let start = clock.now_monotonic();
        let deadline = start.checked_add(Duration::from_millis(100)).unwrap();
        clock.advance(Duration::from_millis(40));
        let mut registry = BidderRegistry::new();
        let mut probes = BTreeMap::new();
        let mut bidders = serde_json::Map::new();
        for info in INFOS
            .iter()
            .filter(|info| diagnostics || matches!(info.name.as_str(), "fake_b000" | "fake_c000"))
        {
            let probe = Arc::new(Mutex::new(Probe::default()));
            probes.insert(info.name.clone(), Arc::clone(&probe));
            bidders.insert(info.name.clone(), json!({}));
            registry
                .register(
                    info,
                    Box::new(Scripted {
                        clock: Arc::clone(&clock),
                        probe,
                        spend: if info.name == "fake_b000" {
                            spend
                        } else {
                            Duration::ZERO
                        },
                        attempts: if diagnostics && info.name == "fake_b000" {
                            2
                        } else {
                            1
                        },
                        accepted_attempts: 1,
                        semantic_error: info.name == "fake_a0",
                        partial_diagnostics,
                    }),
                )
                .expect("unique test bidder");
        }
        let normalized = prepare(
            serde_json::from_value(json!({
                "id": "construction-expiry", "tmax": 250_i64, "site": {"id": "site"},
                "regs": {"gdpr": 0_i32, "gpp_sid": []},
                "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"bidder": bidders}}],
                "ext": {"prebid": {"returnallbidstatus": true}}
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 100,
                ..PrepareLimits::default()
            },
        )
        .unwrap();
        let disabled = if diagnostics {
            BTreeSet::from([BidderName::new("fake_e0000")])
        } else {
            BTreeSet::new()
        };
        let plan = AuctionPlan::new(
            resolve_aliases(&normalized, &registry, &disabled, &host.aliases).unwrap(),
            disabled,
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let account = AccountConfig::default();
        let resolved = ResolvedAccount {
            id: "account".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let fanout = RecordingFanout::default();
        let rng = SeededRng::new(42);
        let ctx = AuctionContext::with_request_start(
            start,
            &host,
            &resolved,
            &registry,
            &fanout,
            clock.as_ref(),
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        let mut audit = DisclosureAudit::default();
        let tracking = body_allocations::Tracking::start();
        let response = block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
            .unwrap()
            .into_bid_response()
            .unwrap();
        let buffers = body_allocations::counts();
        drop(tracking);
        Observed {
            response,
            probes: probes
                .into_iter()
                .map(|(name, probe)| (name, probe.lock().unwrap().clone()))
                .collect(),
            dispatch: fanout.0.into_inner().unwrap(),
            buffers,
            deadline,
        }
    }

    fn gzip_request(participants: &[&str]) -> auctioneer_openrtb::NormalizedBidRequest {
        let bidders: serde_json::Map<String, Value> = participants
            .iter()
            .map(|name| ((*name).into(), json!({})))
            .collect();
        prepare(
            serde_json::from_value(json!({
                "id": "finalization-expiry", "tmax": 250_i64, "site": {"id": "site"},
                "regs": {"gdpr": 0_i32, "gpp_sid": []},
                "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"bidder": bidders}}],
                "ext": {"prebid": {"returnallbidstatus": true,
                    "aliases": {"fake_a": "fake_gzip", "fake_b": "fake_gzip"}}}
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits::default(),
        )
        .unwrap()
    }

    fn run_gzip_finalization(
        expire_after_encoding: bool,
        participants: &[&str],
        attempts_per_bidder: u8,
        accepted_calls_per_bidder: u8,
    ) -> FinalizationObserved {
        run_gzip_with_partials(
            expire_after_encoding,
            participants,
            attempts_per_bidder,
            accepted_calls_per_bidder,
            false,
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "gzip and empty-sibling fixtures share the original deadline and allocation tracking"
    )]
    fn run_gzip_with_partials(
        expire_after_encoding: bool,
        participants: &[&str],
        attempts_per_bidder: u8,
        accepted_calls_per_bidder: u8,
        partial_diagnostics: bool,
    ) -> FinalizationObserved {
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        host.resource_limits.max_tmax_ms = 100;
        host.resource_limits.max_outbound_raw_bytes = u64::try_from(body_allocations::CAP).unwrap();
        host.resource_limits.max_outbound_request_bytes =
            u64::try_from(body_allocations::ENCODED_CAP).unwrap();
        host.resource_limits.max_outbound_requests = participants
            .iter()
            .filter(|name| **name != EMPTY_INFO.name)
            .count()
            .checked_mul(usize::from(accepted_calls_per_bidder))
            .expect("fixture call population fits usize");
        host.resource_limits.max_bidders_per_auction = participants.len();
        let base_clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let start = base_clock.now_monotonic();
        let deadline = start.checked_add(Duration::from_millis(100)).unwrap();
        base_clock.advance(Duration::from_millis(99));
        let clock = FinalizationClock {
            clock: Arc::clone(&base_clock),
            deadline,
            expire_after_encoding,
        };
        let gzip_probe = Arc::new(Mutex::new(Probe::default()));
        let mut registry = BidderRegistry::new();
        registry
            .register(
                &GZIP_INFO,
                Box::new(Scripted {
                    clock: Arc::clone(&base_clock),
                    probe: Arc::clone(&gzip_probe),
                    spend: Duration::ZERO,
                    attempts: attempts_per_bidder,
                    accepted_attempts: accepted_calls_per_bidder,
                    semantic_error: false,
                    partial_diagnostics,
                }),
            )
            .expect("unique test bidder");
        let mut probes = BTreeMap::from([("fake_gzip".into(), gzip_probe)]);
        if participants.contains(&EMPTY_INFO.name.as_str()) {
            let empty_probe = Arc::new(Mutex::new(Probe::default()));
            registry
                .register(
                    &EMPTY_INFO,
                    Box::new(Scripted {
                        clock: base_clock,
                        probe: Arc::clone(&empty_probe),
                        spend: Duration::ZERO,
                        attempts: 0,
                        accepted_attempts: 0,
                        semantic_error: false,
                        partial_diagnostics,
                    }),
                )
                .expect("unique test bidder");
            probes.insert(EMPTY_INFO.name.clone(), empty_probe);
        }
        let normalized = gzip_request(participants);
        let disabled = BTreeSet::new();
        let mut plan = AuctionPlan::new(
            resolve_aliases(&normalized, &registry, &disabled, &host.aliases).unwrap(),
            disabled,
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        if let Some(budget) = plan
            .budgets
            .get_mut(&BidderName::new(EMPTY_INFO.name.as_str()))
        {
            // A live empty sibling emits a clamp warning; expiry must skip its lookup.
            let funded = *budget;
            budget.max_transport_bytes = funded.max_transport_bytes.checked_add(1).unwrap();
            plan = plan.with_boot_envelope(BootEnvelope {
                max_transport_bytes: funded.max_transport_bytes,
                max_decoded_bytes: funded.max_decoded_bytes,
                construction_costs: construction_costs(),
            });
        }
        let account = AccountConfig::default();
        let resolved = ResolvedAccount {
            id: "account".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let fanout = RecordingFanout::default();
        let rng = SeededRng::new(42);
        let mut ctx = AuctionContext::with_request_start(
            start,
            &host,
            &resolved,
            &registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        ctx.features.gzip_outbound = true;
        let mut audit = DisclosureAudit::default();
        let tracking = body_allocations::Tracking::start();
        let response = block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
            .unwrap()
            .into_bid_response()
            .unwrap();
        let buffers = body_allocations::counts();
        let encoded_buffers = body_allocations::encoded_counts();
        let finished_at = clock.now_monotonic();
        drop(tracking);
        FinalizationObserved {
            auction: Observed {
                response,
                probes: probes
                    .into_iter()
                    .map(|(name, probe)| (name, probe.lock().unwrap().clone()))
                    .collect(),
                dispatch: fanout.0.into_inner().unwrap(),
                buffers,
                deadline,
            },
            encoded_buffers,
            finished_at,
        }
    }

    fn assert_expired(observed: &Observed, survivors: &[&str]) {
        assert_eq!(
            observed.dispatch.batches, 0,
            "no partial batch may reach fanout"
        );
        assert_eq!(observed.dispatch.calls, 0);
        assert_eq!(observed.dispatch.deadline, None);
        assert!(
            observed
                .probes
                .values()
                .all(|probe| probe.response_hooks == 0)
        );
        assert!(observed.response.seatbid.is_empty());
        assert_eq!(observed.response.nbr, Some(7_i32));
        let ext = observed.response.ext.as_ref().unwrap();
        assert_eq!(
            ext["errors"]["prebid"],
            json!([{"code": 12_u32, "message": "tmax expired"}])
        );
        assert!(
            ext.get("responsetimemillis").is_none(),
            "construction is not dispatch"
        );
        let rows: Vec<Value> = survivors
            .iter()
            .map(|name| {
                json!({
                    "seat": name, "nonbid": [{"impid": "imp-1", "statuscode": 101_u32}]
                })
            })
            .collect();
        assert_eq!(ext["prebid"]["seatnonbid"], json!(rows));
        for name in survivors {
            assert_eq!(
                ext["errors"][name].as_array().unwrap().last().unwrap(),
                &json!({"code": 1_u32, "message": "timeout"})
            );
        }
        assert_eq!(
            ext["warnings"]["prebid"],
            json!([{
                "code": 10001_u32,
                "message": "regs.gdpr set together with regs.gpp_sid; gpp_sid presence disambiguates when non-empty, else regs.gdpr wins"
            }])
        );
    }

    #[test]
    fn first_adapter_spends_original_budget_and_next_sibling_never_constructs() {
        for millis in [60, 61] {
            let observed = run(Duration::from_millis(millis), false);
            assert_expired(&observed, &["fake_b000", "fake_c000"]);
            let first = &observed.probes["fake_b000"];
            assert_eq!((first.makes, first.metadata, first.accepted), (1, 1, 1));
            assert_eq!(
                first.buffers_at_return,
                (1, 0),
                "accepted body is live inside adapter"
            );
            assert_eq!(observed.probes["fake_c000"].makes, 0);
            assert_eq!(
                observed.buffers,
                (1, 1),
                "expiry must release the accepted body"
            );
        }
    }

    #[test]
    fn one_millisecond_remaining_constructs_sibling_and_dispatches_original_deadline() {
        let observed = run(Duration::from_millis(59), false);
        assert_eq!(observed.dispatch.batches, 1);
        assert_eq!(observed.dispatch.calls, 2);
        assert_eq!(observed.dispatch.deadline, Some(observed.deadline));
        for probe in observed.probes.values() {
            assert_eq!(
                (
                    probe.makes,
                    probe.metadata,
                    probe.accepted,
                    probe.response_hooks
                ),
                (1, 1, 1, 1)
            );
        }
        assert_eq!(observed.buffers, (2, 2));
        assert!(observed.response.ext.as_ref().unwrap()["errors"].is_null());
    }

    fn assert_queued_gzip_stops(participants: &[&str], calls_per_bidder: u8) {
        let observed =
            run_gzip_finalization(true, participants, calls_per_bidder, calls_per_bidder);
        let auction = &observed.auction;
        assert_expired(auction, participants);
        let probe = &auction.probes["fake_gzip"];
        assert_eq!(probe.makes, participants.len());
        assert_eq!(
            probe.metadata, 2,
            "both raw calls were constructed before expiry"
        );
        assert_eq!(probe.accepted, usize::from(calls_per_bidder));
        assert_eq!(probe.encoded_at_return, (0, 0));
        assert_eq!(auction.buffers, (2, 2), "release both accepted raw bodies");
        assert_eq!(observed.finished_at, auction.deadline);
        assert_eq!(
            observed.encoded_buffers,
            (1, 1),
            "only the first queued call may allocate an encoded body before expiry"
        );
    }

    #[test]
    fn partial_diagnostics_survive_both_post_construction_deadline_exits() {
        let construction = run_with_partials(Duration::from_millis(60), true, true);
        let finalized = run_gzip_with_partials(true, &["fake_a", "fake_b"], 1, 1, true).auction;
        for (observed, names) in [
            (construction, vec!["fake_a0", "fake_b000"]),
            (finalized, vec!["fake_a", "fake_b"]),
        ] {
            assert_eq!(observed.dispatch.calls, 0);
            let ext = observed.response.ext.unwrap();
            assert_eq!(
                ext["errors"]["prebid"],
                json!([{"code":12_u32,"message":"tmax expired"}])
            );
            for name in names {
                let rows = ext["errors"][name].as_array().unwrap();
                assert!(
                    rows.contains(&json!({"code":2_u32,"message":"invalid impression parameters"}))
                );
            }
        }
    }

    #[test]
    fn gzip_finalization_stops_queued_calls_at_the_original_deadline() {
        assert_queued_gzip_stops(&["fake_gzip"], 2);
    }

    #[test]
    fn gzip_finalization_stops_sibling_aliases_at_the_original_deadline() {
        assert_queued_gzip_stops(&["fake_a", "fake_b"], 1);
    }

    #[test]
    fn queued_gzip_calls_before_expiry_dispatch_with_the_original_deadline() {
        for (participants, calls_per_bidder) in [
            (&["fake_gzip"][..], 2_u8),
            (&["fake_a", "fake_b"][..], 1_u8),
        ] {
            let observed =
                run_gzip_finalization(false, participants, calls_per_bidder, calls_per_bidder);
            let auction = &observed.auction;
            assert_eq!(auction.dispatch.batches, 1);
            assert_eq!(auction.dispatch.calls, 2);
            assert_eq!(auction.dispatch.gzip_calls, 2);
            assert_eq!(auction.dispatch.deadline, Some(auction.deadline));
            assert_eq!(auction.probes["fake_gzip"].response_hooks, 2);
            assert_eq!(auction.dispatch.encoded_buffers, (2, 0));
            assert_eq!(auction.buffers, (2, 2));
            assert_eq!(observed.encoded_buffers, (2, 2));
            assert_eq!(
                auction.deadline.duration_since(observed.finished_at),
                Duration::from_millis(1)
            );
            assert!(auction.response.ext.as_ref().unwrap()["errors"].is_null());
        }
    }

    #[test]
    fn gzip_finalization_spends_original_budget_and_drops_both_bodies_without_fanout() {
        let observed = run_gzip_finalization(true, &["fake_gzip"], 1, 1);
        let auction = &observed.auction;
        assert_expired(auction, &["fake_gzip"]);
        let probe = &auction.probes["fake_gzip"];
        assert_eq!((probe.makes, probe.metadata, probe.accepted), (1, 1, 1));
        assert_eq!(probe.buffers_at_return, (1, 0));
        assert_eq!(
            probe.encoded_at_return,
            (0, 0),
            "adapter construction must finish before the allocation-triggered expiry"
        );
        assert_eq!(auction.buffers, (1, 1), "raw body is dropped by gzip");
        assert_eq!(
            observed.encoded_buffers,
            (1, 1),
            "finalization allocated an encoded body and expiry released it"
        );
        assert_eq!(observed.finished_at, auction.deadline);
        assert_eq!(
            auction.response.ext.as_ref().unwrap()["errors"],
            json!({
                "fake_gzip": [{"code": 1_u32, "message": "timeout"}],
                "prebid": [{"code": 12_u32, "message": "tmax expired"}]
            })
        );
    }

    #[test]
    fn gzip_finalization_before_expiry_dispatches_with_the_original_deadline() {
        let observed = run_gzip_finalization(false, &["fake_gzip"], 1, 1);
        let auction = &observed.auction;
        let probe = &auction.probes["fake_gzip"];
        assert_eq!(
            (
                probe.makes,
                probe.metadata,
                probe.accepted,
                probe.response_hooks
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(probe.buffers_at_return, (1, 0));
        assert_eq!(probe.encoded_at_return, (0, 0));
        assert_eq!(auction.dispatch.batches, 1);
        assert_eq!(auction.dispatch.calls, 1);
        assert_eq!(auction.dispatch.gzip_calls, 1, "real gzip header and body");
        assert_eq!(auction.dispatch.deadline, Some(auction.deadline));
        assert_eq!(auction.dispatch.raw_buffers, (1, 1));
        assert_eq!(
            auction.dispatch.encoded_buffers,
            (1, 0),
            "fanout owns the live encoded body"
        );
        assert_eq!(auction.buffers, (1, 1));
        assert_eq!(observed.encoded_buffers, (1, 1));
        assert_eq!(
            auction.deadline.duration_since(observed.finished_at),
            Duration::from_millis(1)
        );
        assert!(auction.response.ext.as_ref().unwrap()["errors"].is_null());
    }

    #[test]
    fn expired_gzip_skips_empty_sibling_budget_resolution() {
        for expire_after_encoding in [false, true] {
            let observed =
                run_gzip_finalization(expire_after_encoding, &["fake_gzip", "fake_z0"], 1, 1);
            let auction = &observed.auction;
            let empty = &auction.probes["fake_z0"];
            assert_eq!(
                (
                    empty.makes,
                    empty.metadata,
                    empty.accepted,
                    empty.response_hooks
                ),
                (1, 0, 0, 0),
                "the sibling returns an empty accepted list without a semantic error"
            );
            assert_eq!(auction.probes["fake_gzip"].accepted, 1);
            assert_eq!(auction.buffers, (1, 1));
            assert_eq!(observed.encoded_buffers, (1, 1));
            let ext = auction.response.ext.as_ref().unwrap();
            if expire_after_encoding {
                assert_expired(auction, &["fake_gzip", "fake_z0"]);
                assert_eq!(observed.finished_at, auction.deadline);
                assert!(
                    ext["warnings"]["fake_z0"].is_null(),
                    "expiry must stop before resolving the empty sibling's response budget"
                );
            } else {
                assert_eq!(auction.dispatch.batches, 1);
                assert_eq!(auction.dispatch.calls, 1);
                assert_eq!(auction.dispatch.gzip_calls, 1);
                assert_eq!(auction.dispatch.deadline, Some(auction.deadline));
                assert_eq!(auction.probes["fake_gzip"].response_hooks, 1);
                assert_eq!(
                    auction.deadline.duration_since(observed.finished_at),
                    Duration::from_millis(1)
                );
                assert_eq!(
                    ext["warnings"]["fake_z0"],
                    json!([{
                        "code": WarningCode::OutboundBudgetClamp.as_u32(),
                        "message": "per-bidder response budget exceeded the adapter's boot envelope and was clamped down"
                    }]),
                    "the live control must make the empty sibling's lookup observable"
                );
                assert!(ext["errors"].is_null());
            }
        }
    }

    #[test]
    fn capped_gzip_attempts_preserve_diagnostics_and_the_original_deadline() {
        for expire_after_encoding in [false, true] {
            let observed = run_gzip_finalization(expire_after_encoding, &["fake_gzip"], 3, 1);
            let auction = &observed.auction;
            let probe = &auction.probes["fake_gzip"];
            assert_eq!(
                (probe.makes, probe.metadata, probe.accepted, probe.rejected),
                (1, 1, 1, 2),
                "only the admitted attempt may construct metadata and a raw body"
            );
            assert_eq!(probe.buffers_at_return, (1, 0));
            assert_eq!(probe.encoded_at_return, (0, 0));
            assert_eq!(auction.buffers, (1, 1));
            assert_eq!(observed.encoded_buffers, (1, 1));
            let ext = auction.response.ext.as_ref().unwrap();
            assert_eq!(
                ext["warnings"]["fake_gzip"],
                json!([{
                    "code": 10999_u32,
                    "message": "2 outbound request attempt(s) rejected by max_outbound_requests"
                }])
            );
            if expire_after_encoding {
                assert_expired(auction, &["fake_gzip"]);
                assert_eq!(observed.finished_at, auction.deadline);
                assert_eq!(
                    ext["errors"],
                    json!({
                        "fake_gzip": [{"code": 1_u32, "message": "timeout"}],
                        "prebid": [{"code": 12_u32, "message": "tmax expired"}]
                    })
                );
            } else {
                assert_eq!(auction.dispatch.batches, 1);
                assert_eq!(auction.dispatch.calls, 1);
                assert_eq!(auction.dispatch.gzip_calls, 1);
                assert_eq!(auction.dispatch.deadline, Some(auction.deadline));
                assert_eq!(probe.response_hooks, 1);
                assert_eq!(auction.dispatch.raw_buffers, (1, 1));
                assert_eq!(auction.dispatch.encoded_buffers, (1, 0));
                assert_eq!(
                    auction.deadline.duration_since(observed.finished_at),
                    Duration::from_millis(1)
                );
                assert!(ext["errors"].is_null());
            }
        }
    }

    #[test]
    fn late_expiry_retains_prior_errors_and_caps_after_semantic_failure_refunds_slot() {
        let observed = run(Duration::from_millis(60), true);
        assert_expired(&observed, &["fake_a0", "fake_b000", "fake_c000"]);
        let prior = &observed.probes["fake_a0"];
        assert_eq!((prior.makes, prior.metadata, prior.accepted), (1, 1, 1));
        let spender = &observed.probes["fake_b000"];
        assert_eq!(
            (spender.makes, spender.metadata, spender.accepted),
            (1, 1, 1),
            "prior semantic failure refunds the only slot; cap rejection skips metadata"
        );
        assert_eq!(
            spender.buffers_at_return,
            (2, 1),
            "prior body already discarded, current body retained"
        );
        assert_eq!(observed.buffers, (2, 2));
        for name in ["fake_c000", "fake_d00", "fake_e0000"] {
            assert_eq!(observed.probes[name].makes, 0);
        }
        let ext = observed.response.ext.as_ref().unwrap();
        assert_eq!(
            ext["errors"],
            json!({
                "fake_a0": [{"code": 2_u32, "message": "prior params"}, {"code": 1_u32, "message": "timeout"}],
                "fake_b000": [{"code": 1_u32, "message": "timeout"}],
                "fake_c000": [{"code": 1_u32, "message": "timeout"}],
                "prebid": [{"code": 12_u32, "message": "tmax expired"}]
            })
        );
        assert_eq!(
            ext["warnings"]["fake_b000"],
            json!([{
                "code": 10999_u32, "message": "1 outbound request attempt(s) rejected by max_outbound_requests"
            }])
        );
        assert_eq!(
            ext["warnings"]["fake_d00"],
            json!([{
                "code": 10999_u32, "message": "bidder dropped: max_bidders_per_auction (3) exceeded"
            }])
        );
        assert_eq!(
            ext["warnings"]["fake_e0000"],
            json!([{
                "code": 6_u32, "message": "bidder fake_e0000 disabled by overlay (imp[0])"
            }])
        );
    }
}
