//! What the auction CONSTRUCTS, measured against the budgets that
//! reject the work.
//!
//! Both bounds here are already provable from the auction's OUTPUT, and
//! that is exactly why an output assertion proves nothing. The response
//! names the bidders that dispatched and carries exactly as many
//! outbound calls as `max_outbound_requests` permits, whatever the
//! implementation did to get there. A pipeline that builds a full
//! per-bidder request for every recognised bidder, hands every one to
//! its adapter, collects every body and only then truncates the vector
//! returns the same response as one that builds nothing it will not
//! send.
//!
//! So the quantity under test is the number of CONSTRUCTIONS, measured
//! two ways. For the per-bidder request body, by an adapter that tallies
//! its own entries. For the per-bidder `BidRequest` the split
//! materializes, by the allocator: no hook exists inside the split, and
//! a counting global allocator can only exist in a test binary, which is
//! why that measurement lives here rather than beside the function.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

/// Test-scoped counting allocator (per-thread, peak bytes). Delegates
/// every operation to `System`.
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the per-bidder construction bound"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<usize> = const { Cell::new(0) };
        static PEAK: Cell<usize> = const { Cell::new(0) };
    }

    /// Reset this thread's counters, so a measurement starts from zero
    /// regardless of what the harness allocated before it.
    pub fn reset_on_this_thread() {
        let _live = LIVE.try_with(|cell| cell.set(0));
        let _peak = PEAK.try_with(|cell| cell.set(0));
    }

    /// The HIGHEST live-byte total this thread reached since the last
    /// reset.
    ///
    /// Peak rather than live, because the quantity under test is a
    /// transient: a build-then-truncate implementation frees the
    /// rejected clones before returning, so a live-bytes reading taken
    /// afterwards shows nothing. The peak is what the isolate budget
    /// has to survive.
    pub fn peak_on_this_thread() -> usize {
        PEAK.try_with(Cell::get).unwrap_or(0)
    }

    fn add(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only
        // relaxes the measurement, never breaks the allocator.
        let _outcome = LIVE.try_with(|cell| {
            let now = cell.get().saturating_add(bytes);
            cell.set(now);
            let _peak = PEAK.try_with(|peak| {
                if now > peak.get() {
                    peak.set(now);
                }
            });
        });
    }

    fn sub(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(cell.get().saturating_sub(bytes));
        });
    }

    pub struct CountingAllocator;

    // SAFETY: every operation delegates verbatim to `System`; the
    // counting side effect touches only thread-local counters and
    // never the returned memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            add(new_size);
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc;
            // new_size is the caller's requested size.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }
}

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

mod support;

#[cfg(test)]
mod tests {
    use super::alloc_track::{peak_on_this_thread, reset_on_this_thread};
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRegistry, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::construction_costs;
    use auctioneer_exchange::context::RequestConstructionLimits;
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::split::group_by_bidder;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionError, AuctionPlan, BidderCall,
        BidderCallResult, BidderFanout, FanoutInternalError, ResolvedAccount, ResolvedAliases,
        run_auction,
    };
    use auctioneer_openrtb::ext::imp::{ExtImpPrebid, ImpExt};
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::device::Device;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{
        AuctionBudget, ChargeKind, Clock as _, FakeClock, ResourceError, SeededRng,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };

    /// Every registered bidder names a distinct endpoint, so a count of
    /// adapter entries is a count of distinct per-bidder constructions
    /// rather than of one bidder's repeated calls.
    const BIDDER_COUNT: usize = 24;

    /// The max-bidders cap under test. Deliberately far below
    /// `BIDDER_COUNT`: the gap between them is the work a
    /// build-then-truncate pipeline performs and throws away.
    const BIDDER_CAP: usize = 2;

    /// Bytes of shared extension data on the request, repeated into
    /// `site.keywords`. Large enough that one whole-request clone
    /// dominates every other allocation the auction makes, so the
    /// measurement separates `BIDDER_CAP` clones from `BIDDER_COUNT`
    /// clones far outside any noise.
    const SHARED_EXT_BYTES: usize = 64 * 1024;

    struct NoBidFanout;

    #[async_trait(?Send)]
    impl BidderFanout for NoBidFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], vec![])),
                    elapsed: Duration::from_millis(1),
                })
                .collect())
        }
    }

    #[derive(Default)]
    struct ProjectionCountingFanout(AtomicUsize);

    #[async_trait(?Send)]
    impl BidderFanout for ProjectionCountingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.0.fetch_add(calls.len(), Ordering::Relaxed);
            NoBidFanout.send_all(calls, deadline).await
        }
    }

    /// Tallies every outbound request it is asked to build, then
    /// delegates.
    ///
    /// The count is the whole point. No assertion on the response can
    /// tell "this body was never built" from "this body was built and
    /// then truncated away" — both leave the call absent from the
    /// dispatch list.
    struct CountingBidder {
        inner: FakeBidder<'static>,
        constructions: Arc<AtomicUsize>,
        ipv6_constructions: Option<Arc<AtomicUsize>>,
    }

    impl Bidder for CountingBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            self.constructions.fetch_add(1, Ordering::Relaxed);
            if let Some(counter) = &self.ipv6_constructions
                && request
                    .device
                    .as_ref()
                    .is_some_and(|device| device.ipv6.is_some())
            {
                counter.fetch_add(1, Ordering::Relaxed);
            }
            self.inner.make_requests(request, info, sink)
        }

        fn make_bids(
            &self,
            request: &BidRequest,
            sent: &OutboundRequestMeta,
            response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            self.inner.make_bids(request, sent, response)
        }
    }

    /// Pushes `attempts` outbound requests, counting the bodies it is
    /// actually allowed to WRITE.
    ///
    /// The counter sits inside the body closure, not beside the push.
    /// That placement is what makes the measurement meaningful: a sink
    /// that reserves a call slot before allocating never runs the
    /// closure for a refused request, while one that builds the body
    /// and checks the budget afterwards runs it every time. Both report
    /// the same number of accepted requests.
    struct FanoutBidder {
        attempts: usize,
        bodies_written: Arc<AtomicUsize>,
    }

    impl Bidder for FanoutBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            _request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            for attempt in 0..self.attempts {
                // The advisory `Result` is deliberately ignored: the
                // sink has already recorded every rejection, and an
                // adapter that keeps pushing is the shape that
                // distinguishes a construction-time budget from a
                // post-hoc truncation.
                let _outcome = sink.push_request(
                    info,
                    |meta| {
                        meta.set_method("POST")?;
                        meta.set_url(info.endpoint_template)?;
                        Ok(())
                    },
                    |body| {
                        self.bodies_written.fetch_add(1, Ordering::Relaxed);
                        body.write_all(
                            serde_json::json!({"attempt": attempt})
                                .to_string()
                                .as_bytes(),
                        )
                    },
                );
            }
            Ok(())
        }

        fn make_bids(
            &self,
            _request: &BidRequest,
            _sent: &OutboundRequestMeta,
            _response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            Ok(BidderResponse {
                bids: vec![],
                currency: "USD".into(),
                errors: vec![],
                auction_configs: Vec::new(),
            })
        }
    }

    fn leak_info(name: String, endpoint: String) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name,
            endpoint: Some(endpoint),
            uses_https: true,
            ..BidderInfo::default()
        }))
    }

    /// Zero-padded so `BidderName`'s `Ord` ordering — which is the
    /// registry order the cap truncates in — matches the numeric order
    /// these tests reason about.
    fn bidder_names() -> Vec<String> {
        (0..BIDDER_COUNT).map(|i| format!("fake_{i:03}")).collect()
    }

    /// `BIDDER_COUNT` bidders, every one a counting adapter over the
    /// SAME tally, so the total is "how many per-bidder bodies did this
    /// auction construct".
    fn counting_registry(constructions: &Arc<AtomicUsize>) -> BidderRegistry {
        counting_registry_with_ipv6(constructions, None)
    }

    fn counting_registry_with_ipv6(
        constructions: &Arc<AtomicUsize>,
        ipv6_constructions: Option<&Arc<AtomicUsize>>,
    ) -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        for name in bidder_names() {
            registry
                .register(
                    leak_info(
                        name.clone(),
                        format!("https://demo.example.test/{name}/openrtb"),
                    ),
                    Box::new(CountingBidder {
                        inner: FakeBidder::default(),
                        constructions: Arc::clone(constructions),
                        ipv6_constructions: ipv6_constructions.cloned(),
                    }),
                )
                .expect("unique test bidder");
        }
        registry
    }

    fn host_with_caps(max_bidders: usize, max_outbound: usize) -> HostConfig {
        HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: 750,
                max_tmax_ms: 1500,
                max_bidder_response_bytes: 0x0004_0000,
                max_bidders_per_auction: max_bidders,
                max_outbound_requests: max_outbound,
                max_outbound_raw_bytes: 0x0004_0000,
                max_outbound_request_bytes: 0x0004_0000,
                max_transport_bytes: 0x0004_0000,
                max_decoded_bytes: 0x0004_0000,
                max_parsed_nodes: 0x0001_0000,
            },
            ..HostConfig::default()
        }
    }

    /// One imp naming `count` bidders, carrying `SHARED_EXT_BYTES` of
    /// shared request-level data so a per-bidder whole-request clone is
    /// the dominant allocation rather than a rounding error.
    fn request_naming(count: usize) -> BidRequest {
        use auctioneer_openrtb::types::inventory::Site;
        let mut bidders = serde_json::Map::new();
        for name in bidder_names().iter().take(count) {
            bidders.insert(name.clone(), serde_json::json!({"placementId": 1_i64}));
        }
        BidRequest {
            id: "auc-construction-bound".into(),
            site: Some(Site {
                id: Some("stub".into()),
                name: Some("k".repeat(SHARED_EXT_BYTES)),
                ..Site::default()
            }),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner {
                    w: Some(300),
                    h: Some(250),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            cur: vec!["USD".into()],
            ..BidRequest::default()
        }
    }

    /// Bytes of per-IMP data, carried inside the imp itself.
    ///
    /// Distinct from `SHARED_EXT_BYTES`, which sits at request level.
    /// The two bound different clones: the request-level bytes are paid
    /// by `materialize_per_bidder`'s whole-request clone, and these are
    /// paid by the per-imp scoped copy. A fixture carrying only shared
    /// bytes cannot tell a deferred scoped clone from an eager one,
    /// because the scoped copy of a small imp costs almost nothing.
    const PER_IMP_BYTES: usize = 32 * 1024;

    /// One imp naming `count` bidders, with `PER_IMP_BYTES` of data
    /// INSIDE the imp and nothing at request level.
    ///
    /// The per-bidder scoped imp is a copy of this imp, so a split that
    /// builds one per recognized bidder pays `count x PER_IMP_BYTES`
    /// while one that builds them for survivors only pays
    /// `cap x PER_IMP_BYTES`.
    fn request_with_heavy_imps(count: usize) -> BidRequest {
        let mut bidders = serde_json::Map::new();
        for name in bidder_names().iter().take(count) {
            bidders.insert(name.clone(), serde_json::json!({"placementId": 1_i64}));
        }
        BidRequest {
            id: "auc-scoped-imp-bound".into(),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner {
                    w: Some(300),
                    h: Some(250),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                    // Passthrough data on the imp, carried into every
                    // scoped copy of it.
                    other: [(
                        "heavy".to_owned(),
                        serde_json::Value::String("k".repeat(PER_IMP_BYTES)),
                    )]
                    .into_iter()
                    .collect(),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            cur: vec!["USD".into()],
            ..BidRequest::default()
        }
    }

    /// Drive one auction over `host` with heavy imps, naming `offered`
    /// bidders.
    fn drive_heavy(host: &HostConfig, offered: usize, registry: &BidderRegistry) -> usize {
        drive_request(host, request_with_heavy_imps(offered), registry)
    }

    /// Drive one auction over `host`, naming `offered` bidders, and
    /// report how many bidders landed in `responsetimemillis`.
    fn drive(host: &HostConfig, offered: usize, registry: &BidderRegistry) -> usize {
        drive_request(host, request_naming(offered), registry)
    }

    /// The shared driving code: one auction over `req`, reporting how
    /// many bidders landed in `responsetimemillis`.
    fn drive_request(host: &HostConfig, req: BidRequest, registry: &BidderRegistry) -> usize {
        block_on(async {
            let normalized =
                prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let fanout = NoBidFanout;
            let ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                host,
                &resolved,
                registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                response_budgets(registry, host),
                BTreeMap::new(),
            );
            let mut audit = DisclosureAudit::default();
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("run_auction succeeds")
                .into_bid_response()
                .unwrap();
            resp.ext
                .as_ref()
                .and_then(|ext| ext.get("responsetimemillis"))
                .and_then(serde_json::Value::as_object)
                .map_or(0, serde_json::Map::len)
        })
    }

    fn drive_with_projection_headroom(
        request: BidRequest,
        headroom: u64,
        construction_limits: Option<RequestConstructionLimits>,
    ) -> (Result<BidResponse, AuctionError>, usize) {
        // Scratch-only tests independently fund the retained IPv6 destinations.
        let funded_headroom = headroom.checked_add(2 * 39).unwrap();
        drive_with_projection_headroom_for_scopes(request, funded_headroom, construction_limits, 1)
    }

    fn drive_with_projection_headroom_for_scopes(
        request: BidRequest,
        headroom: u64,
        construction_limits: Option<RequestConstructionLimits>,
        scopes: usize,
    ) -> (Result<BidResponse, AuctionError>, usize) {
        let requires_ipv6 = request
            .device
            .as_ref()
            .is_some_and(|device| device.ipv6.is_some());
        let host = host_with_caps(scopes, scopes);
        let constructions = Arc::new(AtomicUsize::new(0));
        let ipv6_constructions = Arc::new(AtomicUsize::new(0));
        let registry = counting_registry_with_ipv6(&constructions, Some(&ipv6_constructions));
        let normalized =
            prepare(request, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
        if requires_ipv6 {
            assert_eq!(
                normalized.device.as_ref().unwrap().ipv6.as_deref(),
                Some("2001:4860:4860::8888"),
                "the projection fixture retains its public IPv6 through prepare"
            );
        }
        let sampling = if scopes == 0 {
            BTreeMap::from([(auctioneer_bidder::BidderName::new("fake_000"), 0.0_f64)])
        } else {
            BTreeMap::new()
        };
        let plan = AuctionPlan::new(
            ResolvedAliases::empty(),
            BTreeSet::new(),
            response_budgets(&registry, &host),
            sampling,
        );
        let grouped = group_by_bidder(
            &normalized,
            &plan.aliases.map,
            &registry,
            &plan.disabled_bidders,
            &plan.aliases.dropped_alias_names,
        )
        .unwrap();
        assert_eq!(grouped.bidder_imps().len(), scopes.max(1));
        let survivors: Vec<_> = grouped.bidder_imps().keys().take(scopes).cloned().collect();
        assert_eq!(survivors.len(), scopes);
        let projection_charge = grouped.projection_charge(&survivors).unwrap();
        // The existing auction context charge is separate from the retained
        // growth/scratch boundary under test. Pin its independent root/name
        // terms so a changed production charge cannot move both sides together.
        let context = if scopes == 0 {
            0
        } else {
            4096_u64
                .checked_add(u64::try_from(scopes).unwrap().checked_mul(98_304).unwrap())
                .unwrap()
        };
        let capacity = projection_charge
            .checked_add(context)
            .unwrap()
            .checked_add(headroom)
            .unwrap();
        let budget = AuctionBudget::new(capacity);
        let receipt = budget
            .try_reserve(ChargeKind::Projection, capacity)
            .unwrap();
        receipt
            .ensure_capacity(ChargeKind::Projection, projection_charge)
            .expect("projection alone fits; retained growth and scratch must fit separately");
        drop(grouped);
        let account = AccountConfig::default();
        let resolved = ResolvedAccount {
            id: "unknown".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let fanout = ProjectionCountingFanout::default();
        let mut ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
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
        ctx.projection_reservation = Some(&receipt);
        ctx.construction_limits = construction_limits;
        let mut audit = DisclosureAudit::default();
        let result = block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
            .and_then(auctioneer_exchange::AuctionOutcome::into_bid_response);
        assert_projection_dispatch(
            &result,
            &audit,
            scopes,
            requires_ipv6,
            &ipv6_constructions,
            &fanout,
        );
        (result, constructions.load(Ordering::Relaxed))
    }

    fn assert_projection_dispatch(
        result: &Result<BidResponse, AuctionError>,
        audit: &DisclosureAudit,
        scopes: usize,
        requires_ipv6: bool,
        ipv6_constructions: &AtomicUsize,
        fanout: &ProjectionCountingFanout,
    ) {
        if result.is_err() {
            assert_eq!(audit.attempted_scopes, 0, "refusal precedes scrub scopes");
            assert_eq!(
                ipv6_constructions.load(Ordering::Relaxed),
                0,
                "refusal never constructs an IPv6-bearing adapter request"
            );
            assert_eq!(
                fanout.0.load(Ordering::Relaxed),
                0,
                "refusal never dispatches"
            );
        } else {
            assert_eq!(
                audit.attempted_scopes,
                u64::try_from(scopes).unwrap(),
                "the funded survivors reach scrub"
            );
            assert_eq!(
                fanout.0.load(Ordering::Relaxed),
                scopes,
                "the funded calls dispatch"
            );
            if requires_ipv6 {
                assert_eq!(
                    ipv6_constructions.load(Ordering::Relaxed),
                    scopes,
                    "the admitted adapter inputs retain the IPv6 field"
                );
            }
        }
    }

    fn request_with_ipv6(scopes: usize) -> BidRequest {
        let mut request = request_naming(scopes);
        request.device = Some(Device {
            ipv6: Some("2001:4860:4860::8888".to_owned()),
            ..Device::default()
        });
        assert_eq!(
            request.device.as_ref().unwrap().ipv6.as_deref(),
            Some("2001:4860:4860::8888")
        );
        request
    }

    #[test]
    fn projection_receipt_retained_growth_exact_boundary_admits_all_scopes() {
        // Independent of the descriptor: typed and flattened IPv6 destinations.
        const RETAINED_PER_SCOPE: u64 = 2 * 39;
        let scratch = construction_costs().reusable_scratch_bytes;
        for scopes in [1, 2] {
            let growth = u64::try_from(scopes)
                .unwrap()
                .checked_mul(RETAINED_PER_SCOPE)
                .unwrap();
            let headroom = growth.checked_add(scratch).unwrap();
            let (result, constructions) = drive_with_projection_headroom_for_scopes(
                request_with_ipv6(scopes),
                headroom,
                None,
                scopes,
            );
            result.expect("exact projection plus per-scope destination growth plus scratch fits");
            assert_eq!(constructions, scopes);
        }
    }

    #[test]
    fn projection_receipt_retained_growth_one_below_refuses_before_construction() {
        const RETAINED_PER_SCOPE: u64 = 2 * 39;
        let scratch = construction_costs().reusable_scratch_bytes;
        for scopes in [1, 2] {
            let growth = u64::try_from(scopes)
                .unwrap()
                .checked_mul(RETAINED_PER_SCOPE)
                .unwrap();
            let headroom = growth.checked_add(scratch).unwrap().checked_sub(1).unwrap();
            let (result, constructions) = drive_with_projection_headroom_for_scopes(
                request_with_ipv6(scopes),
                headroom,
                None,
                scopes,
            );
            assert!(
                matches!(
                    result,
                    Err(AuctionError::Resource(ResourceError::Exhausted {
                        kind: ChargeKind::Projection
                    }))
                ),
                "{scopes} scopes must not consume retained destination growth from scratch"
            );
            assert_eq!(constructions, 0);
        }
    }

    #[test]
    fn projection_receipt_underdeclared_retained_growth_is_floored() {
        const RETAINED_PER_SCOPE: u64 = 2 * 39;
        let costs = construction_costs();
        assert_eq!(costs.retained_per_scope_bytes, RETAINED_PER_SCOPE);
        for declared_growth in [0, 1] {
            let limits = RequestConstructionLimits {
                max_nodes: 0x0001_0000,
                max_serialized_bytes: 0x0004_0000,
                reusable_scratch_bytes: costs.reusable_scratch_bytes,
                retained_per_scope_bytes: declared_growth,
            };
            for scopes in [1, 2] {
                let exact = u64::try_from(scopes)
                    .unwrap()
                    .checked_mul(RETAINED_PER_SCOPE)
                    .unwrap()
                    .checked_add(costs.reusable_scratch_bytes)
                    .unwrap();
                let declared = u64::try_from(scopes)
                    .unwrap()
                    .checked_mul(declared_growth)
                    .unwrap()
                    .checked_add(costs.reusable_scratch_bytes)
                    .unwrap();
                for headroom in [declared, exact.checked_sub(1).unwrap(), exact] {
                    let (result, constructions) = drive_with_projection_headroom_for_scopes(
                        request_with_ipv6(scopes),
                        headroom,
                        Some(limits),
                        scopes,
                    );
                    if headroom == exact {
                        result.expect("the actual retained growth floor fits exactly");
                        assert_eq!(constructions, scopes);
                    } else {
                        assert!(matches!(
                            result,
                            Err(AuctionError::Resource(ResourceError::Exhausted {
                                kind: ChargeKind::Projection
                            }))
                        ));
                        assert_eq!(constructions, 0);
                    }
                }
            }
        }
    }

    #[test]
    fn projection_receipt_uses_exact_installed_retained_growth_allowance() {
        let costs = construction_costs();
        let installed_growth = costs.retained_per_scope_bytes.checked_add(123).unwrap();
        let limits = RequestConstructionLimits {
            max_nodes: 0x0001_0000,
            max_serialized_bytes: 0x0004_0000,
            reusable_scratch_bytes: costs.reusable_scratch_bytes,
            retained_per_scope_bytes: installed_growth,
        };
        for scopes in [1, 2] {
            let exact = u64::try_from(scopes)
                .unwrap()
                .checked_mul(installed_growth)
                .unwrap()
                .checked_add(costs.reusable_scratch_bytes)
                .unwrap();
            for headroom in [exact.checked_sub(1).unwrap(), exact] {
                let (result, constructions) = drive_with_projection_headroom_for_scopes(
                    request_with_ipv6(scopes),
                    headroom,
                    Some(limits),
                    scopes,
                );
                if headroom == exact {
                    result.expect("installed retained growth is charged per scope exactly");
                    assert_eq!(constructions, scopes);
                } else {
                    assert!(matches!(
                        result,
                        Err(AuctionError::Resource(ResourceError::Exhausted {
                            kind: ChargeKind::Projection
                        }))
                    ));
                    assert_eq!(constructions, 0);
                }
            }
        }
    }

    #[test]
    fn projection_retained_growth_addition_and_multiplication_refuse_overflow() {
        let limits = RequestConstructionLimits {
            max_nodes: 0x0001_0000,
            max_serialized_bytes: 0x0004_0000,
            reusable_scratch_bytes: construction_costs().reusable_scratch_bytes,
            retained_per_scope_bytes: u64::MAX,
        };
        for scopes in [1, 2] {
            let (result, constructions) = drive_with_projection_headroom_for_scopes(
                request_with_ipv6(scopes),
                0,
                Some(limits),
                scopes,
            );
            assert!(matches!(
                result,
                Err(AuctionError::Resource(ResourceError::CapacityOverflow))
            ));
            assert_eq!(constructions, 0);
        }
    }

    #[test]
    fn projection_zero_survivors_charge_scratch_without_retained_growth() {
        let scratch = construction_costs().reusable_scratch_bytes;
        let limits = RequestConstructionLimits {
            max_nodes: 0x0001_0000,
            max_serialized_bytes: 0x0004_0000,
            reusable_scratch_bytes: scratch,
            retained_per_scope_bytes: u64::MAX,
        };
        for headroom in [scratch.checked_sub(1).unwrap(), scratch] {
            let (result, constructions) = drive_with_projection_headroom_for_scopes(
                request_naming(1),
                headroom,
                Some(limits),
                0,
            );
            if headroom == scratch {
                result.expect("zero scopes multiply even maximal retained growth to zero");
            } else {
                assert!(matches!(
                    result,
                    Err(AuctionError::Resource(ResourceError::Exhausted {
                        kind: ChargeKind::Projection
                    }))
                ));
            }
            assert_eq!(constructions, 0);
        }
    }

    #[test]
    fn projection_receipt_without_limits_preserves_actual_engine_scratch() {
        let scratch = construction_costs().reusable_scratch_bytes;
        assert!(scratch > 0);
        let (refused_result, refused_constructions) = drive_with_projection_headroom(
            request_naming(1),
            scratch.checked_sub(1).unwrap(),
            None,
        );
        assert!(matches!(
            refused_result,
            Err(AuctionError::Resource(ResourceError::Exhausted {
                kind: ChargeKind::Projection
            }))
        ));
        assert_eq!(refused_constructions, 0);

        let (admitted_result, admitted_constructions) =
            drive_with_projection_headroom(request_naming(1), scratch, None);
        admitted_result.expect("the exact projection plus engine scratch boundary must fit");
        assert_eq!(admitted_constructions, 1);
    }

    #[test]
    fn projection_receipt_uses_exact_installed_scratch_allowance() {
        let engine_scratch = construction_costs().reusable_scratch_bytes;
        let installed_scratch = engine_scratch.checked_add(123).unwrap();
        let limits = RequestConstructionLimits {
            max_nodes: 0x0001_0000,
            max_serialized_bytes: 0x0004_0000,
            reusable_scratch_bytes: installed_scratch,
            retained_per_scope_bytes: 2 * 39,
        };
        for headroom in [
            engine_scratch,
            installed_scratch.checked_sub(1).unwrap(),
            installed_scratch,
        ] {
            let (result, constructions) =
                drive_with_projection_headroom(request_naming(1), headroom, Some(limits));
            if headroom == installed_scratch {
                result.expect("the exact installed scratch boundary must fit");
                assert_eq!(constructions, 1);
            } else {
                assert!(matches!(
                    result,
                    Err(AuctionError::Resource(ResourceError::Exhausted {
                        kind: ChargeKind::Projection
                    }))
                ));
                assert_eq!(constructions, 0);
            }
        }
    }

    fn assert_underdeclared_scratch_floor(declared_scratch: u64) {
        let engine_scratch = construction_costs().reusable_scratch_bytes;
        assert!(declared_scratch < engine_scratch);
        let limits = RequestConstructionLimits {
            max_nodes: 0x0001_0000,
            max_serialized_bytes: 0x0004_0000,
            reusable_scratch_bytes: declared_scratch,
            retained_per_scope_bytes: 2 * 39,
        };
        for headroom in [declared_scratch, engine_scratch.checked_sub(1).unwrap()] {
            let (result, constructions) =
                drive_with_projection_headroom(request_naming(1), headroom, Some(limits));
            assert!(
                matches!(
                    result,
                    Err(AuctionError::Resource(ResourceError::Exhausted {
                        kind: ChargeKind::Projection
                    }))
                ),
                "declared scratch {declared_scratch} cannot admit headroom {headroom} below actual scratch {engine_scratch}"
            );
            assert_eq!(constructions, 0);
        }

        let (result, constructions) =
            drive_with_projection_headroom(request_naming(1), engine_scratch, Some(limits));
        result.expect("the exact projection plus actual scratch boundary must fit");
        assert_eq!(constructions, 1);
    }

    #[test]
    fn projection_receipt_underdeclared_scratch_zero_is_floored() {
        assert_underdeclared_scratch_floor(0);
    }

    #[test]
    fn projection_receipt_underdeclared_scratch_small_is_floored() {
        assert_underdeclared_scratch_floor(1);
    }

    #[test]
    fn projection_and_installed_scratch_addition_refuses_overflow() {
        let limits = RequestConstructionLimits {
            max_nodes: 0x0001_0000,
            max_serialized_bytes: 0x0004_0000,
            reusable_scratch_bytes: u64::MAX,
            retained_per_scope_bytes: 2 * 39,
        };
        let (result, constructions) =
            drive_with_projection_headroom(request_naming(1), 0, Some(limits));
        assert!(matches!(
            result,
            Err(AuctionError::Resource(ResourceError::CapacityOverflow))
        ));
        assert_eq!(constructions, 0);
    }

    #[test]
    fn scratch_refusal_precedes_bidder_local_fpd_materialization() {
        let mut request = request_naming(1);
        request.imp[0].ext.as_mut().unwrap().prebid = JsonField::Value(ExtImpPrebid {
            imp: [("fake_000".into(), serde_json::json!({"banner": null}))].into(),
            ..Default::default()
        });
        let scratch = construction_costs().reusable_scratch_bytes;
        let (refused_result, refused_constructions) =
            drive_with_projection_headroom(request.clone(), scratch.checked_sub(1).unwrap(), None);
        assert!(matches!(
            refused_result,
            Err(AuctionError::Resource(ResourceError::Exhausted {
                kind: ChargeKind::Projection
            }))
        ));
        assert_eq!(refused_constructions, 0);

        let (invalid_result, invalid_constructions) =
            drive_with_projection_headroom(request, scratch, None);
        assert!(matches!(
            invalid_result,
            Err(AuctionError::InvalidImpFirstPartyData(_))
        ));
        assert_eq!(invalid_constructions, 0);
    }

    /// Peak bytes `call` reached, measured on a THREAD OF ITS OWN.
    ///
    /// The counter is thread-local and a fresh thread starts at zero,
    /// so the reading covers this call and nothing the harness did
    /// around it.
    fn peak_bytes_of<F>(call: F) -> usize
    where
        F: FnOnce() + Send + 'static,
    {
        thread::spawn(move || {
            reset_on_this_thread();
            call();
            peak_on_this_thread()
        })
        .join()
        .expect("measurement thread completed")
    }

    /// The max-bidders cap is a CONSTRUCTION-TIME bound: a bidder it
    /// rejects never has a per-bidder `BidRequest` materialized for it.
    ///
    /// Measured against a BASELINE auction that offers exactly
    /// `BIDDER_CAP` bidders and so rejects nobody. Both auctions
    /// dispatch the same `BIDDER_CAP` bidders and return
    /// indistinguishable responses; the only thing that separates them
    /// is what the capped one allocated for the bidders it discarded.
    #[test]
    fn bidders_the_cap_rejects_never_materialize_a_request() {
        let host = host_with_caps(BIDDER_CAP, 100);

        // BOTH runs register all `BIDDER_COUNT` bidders, and differ only
        // in how many the REQUEST names. That is what isolates the
        // quantity under test. Registering 2 against 24 would have let
        // the capped run's extra registry entries — a leaked
        // `BidderInfo` and a boxed adapter per bidder — land in its
        // peak, and those have nothing to do with materializing a
        // request. Holding the registry identical makes the difference
        // between the two peaks attributable to per-bidder request
        // construction and nothing else.
        let baseline_host = host.clone();
        let baseline_peak = peak_bytes_of(move || {
            let constructions = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(&constructions);
            let dispatched = drive(&baseline_host, BIDDER_CAP, &registry);
            assert_eq!(dispatched, BIDDER_CAP, "baseline offers exactly the cap");
        });

        let capped_host = host.clone();
        let capped_peak = peak_bytes_of(move || {
            let constructions = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(&constructions);
            let dispatched = drive(&capped_host, BIDDER_COUNT, &registry);
            // The fixture genuinely reaches the cap: without this the
            // measurement below could be comparing two auctions that
            // both rejected nobody.
            assert_eq!(
                dispatched, BIDDER_CAP,
                "the over-cap auction must still dispatch exactly the cap",
            );
            // And the per-bidder BODIES are bounded too, which is the
            // same claim one level down.
            assert_eq!(
                constructions.load(Ordering::Relaxed),
                BIDDER_CAP,
                "a bidder the max-bidders cap rejects must never construct \
                 its outbound body",
            );
        });

        // A bounded implementation materializes `BIDDER_CAP` requests in
        // both runs, so the over-cap auction costs the baseline plus the
        // scoped-imp bookkeeping for the names it discarded — far less
        // than one whole-request clone. A build-then-truncate
        // implementation materializes `BIDDER_COUNT` of them, paying
        // `BIDDER_COUNT - BIDDER_CAP` extra whole-request clones of
        // `SHARED_EXT_BYTES` each.
        //
        // The budget is the baseline plus ONE spare clone's worth, which
        // is generous for the bounded shape and unreachable for the
        // other: the gap it has to fit inside is 22 clones wide.
        let budget = baseline_peak + SHARED_EXT_BYTES;
        let discarded = BIDDER_COUNT - BIDDER_CAP;
        let build_then_truncate_floor = discarded * SHARED_EXT_BYTES;
        let overhead = capped_peak.saturating_sub(baseline_peak);
        assert!(
            capped_peak <= budget,
            "offering {BIDDER_COUNT} bidders under a cap of {BIDDER_CAP} peaked at \
             {capped_peak} bytes against a {budget}-byte budget (baseline \
             {baseline_peak}, one shared-ext clone {SHARED_EXT_BYTES}): per-bidder \
             requests are being materialized for bidders the cap then discards",
        );

        // The SEPARATION, not just the budget. A budget alone can pass
        // because it happened to be loose; requiring the overhead to
        // stay far below what the defect would have cost means a future
        // regression that creeps partway back still fails here.
        //
        // Stated as a multiplication rather than `floor / 2` because the
        // workspace denies integer division, and doubling the measured
        // side is the same comparison without the rounding question.
        assert!(
            overhead.saturating_mul(2) < build_then_truncate_floor,
            "cap-bound headroom: baseline {baseline_peak} B, capped {capped_peak} B, \
             overhead {overhead} B over baseline for {discarded} discarded bidders \
             (budget {budget} B); that overhead is not clearly separated from the \
             {build_then_truncate_floor} B a build-then-truncate implementation \
             would add, so the bound may be slipping back",
        );
    }

    /// The scoped IMP is built after the cap too, not just the
    /// per-bidder request.
    ///
    /// A separate claim from the row above, and it needs its own
    /// fixture. `bidders_the_cap_rejects_never_materialize_a_request`
    /// puts its bytes at REQUEST level, where the only clone that can
    /// pay for them is `materialize_per_bidder`'s — so it passes whether
    /// the per-imp scoped copy is deferred or not. The scoped copy is a
    /// copy of the IMP, so proving it deferred needs the bytes inside
    /// the imp.
    ///
    /// That gap was real: `build_scoped_imp` ran `imp_value.clone()`
    /// once per (imp x recognized bidder) during the split, before the
    /// privacy filter, the sampler and the cap had run. A request
    /// naming thousands of aliases therefore retained a copy of its
    /// whole imp array per alias — the reviewer measured a 230,531-byte
    /// request with 3,500 aliases retaining 469,342,437 bytes, past
    /// even this target's entire declared per-auction peak.
    ///
    /// Measured against a baseline that offers exactly the cap, so both
    /// runs dispatch the same bidders and return indistinguishable
    /// responses. Only what the capped run allocated for the names it
    /// discarded separates them.
    #[test]
    fn imps_scoped_for_bidders_the_cap_rejects_are_never_built() {
        let host = host_with_caps(BIDDER_CAP, 100);

        // Both runs register every bidder and differ only in how many
        // the REQUEST names, for the reason the row above gives: a
        // differing registry would put leaked `BidderInfo`s and boxed
        // adapters in one peak and not the other.
        let baseline_host = host.clone();
        let baseline_peak = peak_bytes_of(move || {
            let constructions = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(&constructions);
            let dispatched = drive_heavy(&baseline_host, BIDDER_CAP, &registry);
            assert_eq!(dispatched, BIDDER_CAP, "baseline offers exactly the cap");
        });

        let capped_host = host.clone();
        let capped_peak = peak_bytes_of(move || {
            let constructions = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(&constructions);
            let dispatched = drive_heavy(&capped_host, BIDDER_COUNT, &registry);
            // The fixture genuinely reaches the cap. Without this the
            // comparison could be between two auctions that both
            // rejected nobody — which is the shape of a test that
            // names a scenario its values never enter.
            assert_eq!(
                dispatched, BIDDER_CAP,
                "the over-cap auction must still dispatch exactly the cap",
            );
        });

        // The two runs do not have the same INPUT: naming 24 bidders in
        // `imp.ext.bidder` is a bigger request than naming 2, and the
        // auction legitimately pays for the request it was given — the
        // typed fixture, its `serde_json::Value` form, and the
        // participation scan over it. That cost is proportional to the
        // input, not to the bidders the cap discarded, so it is
        // measured and allowed for rather than left to inflate the
        // budget.
        //
        // What must NOT scale with the discarded bidders is the scoped
        // imp. A deferred scoped clone adds, per discarded bidder, a
        // name and its params — bytes, not kilobytes. An eager one adds
        // a whole `PER_IMP_BYTES`-sized copy each.
        let baseline_input = peak_bytes_of(|| {
            let _request = request_with_heavy_imps(BIDDER_CAP);
        });
        let capped_input = peak_bytes_of(|| {
            let _request = request_with_heavy_imps(BIDDER_COUNT);
        });
        let input_growth = capped_input.saturating_sub(baseline_input);

        let discarded = BIDDER_COUNT - BIDDER_CAP;
        let eager_floor = discarded * PER_IMP_BYTES;
        // The input's own growth, charged through the typed value, its
        // JSON form and the scan's map of it. Three passes over the
        // same growth is generous and still an order of magnitude
        // under one eager scoped clone of this imp.
        let input_allowance = input_growth.saturating_mul(4);
        let budget = baseline_peak + input_allowance + PER_IMP_BYTES;
        let overhead = capped_peak.saturating_sub(baseline_peak);
        assert!(
            capped_peak <= budget,
            "offering {BIDDER_COUNT} bidders under a cap of {BIDDER_CAP} peaked at \
             {capped_peak} bytes against a {budget}-byte budget (baseline \
             {baseline_peak}, input growth {input_growth} B allowed at \
             {input_allowance} B, one scoped imp {PER_IMP_BYTES} B): scoped imps are \
             being built for bidders the cap then discards",
        );

        // The SEPARATION as well as the budget, so a regression that
        // creeps partway back still fails here rather than fitting
        // inside a budget that happened to be loose. This is the
        // assertion that would fail on the eager shape: 22 discarded
        // bidders at 32 KiB each is 704 KiB, against an overhead that
        // has to stay under half of it.
        assert!(
            overhead.saturating_mul(2) < eager_floor,
            "scoped-imp headroom: baseline {baseline_peak} B, capped {capped_peak} B, \
             overhead {overhead} B for {discarded} discarded bidders (of which \
             {input_growth} B is the larger input itself); that is not clearly \
             separated from the {eager_floor} B an eager scoped clone would add, so \
             the bound may be slipping back",
        );
    }

    /// Imps the supplement-amplification fixture carries.
    ///
    /// The request-level supplement is merged per (imp x bidder), so the
    /// amplification needs BOTH dimensions: one imp would make it a
    /// per-bidder cost the bidder cap already bounds, and one bidder
    /// would make it a per-imp cost the imp cap bounds. 20 is enough
    /// that `imps x bidders` is two orders of magnitude over
    /// `imps x cap` while every run stays inside
    /// `IMP_CAP_NOT_UNDER_TEST`.
    const SUPPLEMENT_IMPS: usize = 20;

    /// Bytes of request-level `ext.prebid.bidderparams` data per bidder.
    ///
    /// Carried at REQUEST level, declared once, and legitimately needed
    /// only by the bidders that survive selection. An eager merge copies
    /// it into every participating imp for every recognized bidder,
    /// which is `SUPPLEMENT_IMPS x BIDDER_COUNT` copies of this; the
    /// funded shape is `SUPPLEMENT_IMPS x BIDDER_CAP`.
    ///
    /// Sized so the whole supplement BLOCK — all `BIDDER_COUNT` of them,
    /// which `materialize_per_bidder` carries into each survivor's outbound
    /// body because `ext.prebid` is request-level and is not scoped per
    /// bidder — stays under the 256 KiB outbound raw cap. At 8 KiB it
    /// did not: both survivors failed with code 5 `OversizeRaw` and the
    /// over-cap run dispatched nobody, so the comparison would have been
    /// between a working auction and a broken one.
    const SUPPLEMENT_BYTES: usize = 2 * 1_024;

    /// `SUPPLEMENT_IMPS` imps naming `count` bidders, carrying
    /// `count x SUPPLEMENT_BYTES` of per-bidder data.
    ///
    /// `at_request_level` decides WHERE those bytes sit, and that is the
    /// whole experiment:
    ///
    /// * `true` puts them in `ext.prebid.bidderparams`, one entry per
    ///   bidder — the shape an eager per-imp merge copies once per
    ///   (imp x recognized bidder);
    /// * `false` puts the same volume in `site.name`, where it is
    ///   request-level data nothing copies per pair. The supplement map
    ///   is left empty.
    ///
    /// Everything else is held identical: the same imps, the same
    /// `imp.ext.bidder` participation map, the same total byte count.
    /// The imps themselves are small and each bidder's own per-imp
    /// params are a single integer, so the only large thing in either
    /// request is that block — which is what makes the difference
    /// between the two peaks attributable to the merge.
    fn request_with_request_level_supplements(count: usize, at_request_level: bool) -> BidRequest {
        use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
        use auctioneer_openrtb::types::bid_request::BidRequestExt;
        use auctioneer_openrtb::types::inventory::Site;

        let names = bidder_names();
        let participating = names.iter().take(count);
        let mut supplements: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        if at_request_level {
            for name in participating.clone() {
                supplements.insert(
                    name.clone(),
                    serde_json::json!({"shared": "s".repeat(SUPPLEMENT_BYTES)}),
                );
            }
        }
        let mut bidders = serde_json::Map::new();
        for name in participating {
            bidders.insert(name.clone(), serde_json::json!({"placementId": 1_i64}));
        }
        let imps: Vec<Imp> = (0..SUPPLEMENT_IMPS)
            .map(|index| Imp {
                id: format!("imp-{index}"),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders.clone())),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            })
            .collect();
        // The control's ballast: the same volume of request-level bytes
        // the supplement block holds, somewhere the split does not scope
        // per bidder.
        let ballast = if at_request_level {
            None
        } else {
            Some(Site {
                id: Some("stub".into()),
                name: Some("s".repeat(SUPPLEMENT_BYTES.saturating_mul(count))),
                ..Site::default()
            })
        };
        BidRequest {
            id: "auc-supplement-bound".into(),
            imp: imps,
            cur: vec!["USD".into()],
            site: ballast,
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    bidderparams: supplements,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        }
    }

    /// Request-level `ext.prebid.bidderparams` is a SELECTION-TIME
    /// cost: the supplement is materialized for the bidder/imp pairs
    /// that survive, not for every pair the participation scan
    /// recognized.
    ///
    /// The supplement is declared once per bidder at request level and
    /// applies to every imp that bidder landed on. Merging it during the
    /// per-imp scan copies it `imps x recognized bidders` times, before
    /// the privacy filter, the sampler or the max-bidders cap have had
    /// their say — so a request naming many bidders across many imps
    /// retains a copy per pair and then discards almost all of them.
    /// Applying it in `build_scoped_imp` instead makes it
    /// `imps x survivors`.
    ///
    /// # Why the comparison holds the bidder count fixed
    ///
    /// `imps_scoped_for_bidders_the_cap_rejects_are_never_built`
    /// compares 2 offered bidders against 24 and allows for the larger
    /// input. That construction does not work here, and measuring showed
    /// why: the supplement block is `BIDDER_COUNT` entries of request
    /// data, so going from 2 to 24 grows the INPUT by 394 KiB, and the
    /// auction legitimately pays for that input several times over —
    /// through the typed fixture, its `serde_json::Value` form and the
    /// participation scan. That legitimate growth is larger than the
    /// amplification being measured, so an offered-count comparison
    /// cannot separate them whatever allowance it picks.
    ///
    /// So both runs offer `BIDDER_COUNT` bidders and both reject the
    /// same 22. They differ only in WHERE the supplement bytes sit: one
    /// request carries them at request level, where the eager merge
    /// copies them per pair, and the control carries the same bytes in
    /// `site.name`, where nothing copies them per pair at all. The two
    /// inputs are within a few hundred bytes of each other, which the
    /// assertion checks, so the difference between the peaks is the
    /// merge and nothing else.
    #[test]
    fn request_level_supplements_are_not_copied_for_bidders_the_cap_rejects() {
        let host = host_with_caps(BIDDER_CAP, 1_000);

        let supplement_host = host.clone();
        let supplement_peak = peak_bytes_of(move || {
            let constructions = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(&constructions);
            let dispatched = drive_request(
                &supplement_host,
                request_with_request_level_supplements(BIDDER_COUNT, true),
                &registry,
            );
            // Both runs must genuinely reach the cap, or the comparison
            // is between two auctions that rejected nobody — the shape
            // of a test that names a scenario its values never enter.
            assert_eq!(
                dispatched, BIDDER_CAP,
                "the supplemented auction must dispatch exactly the cap",
            );
        });

        let control_host = host.clone();
        let control_peak = peak_bytes_of(move || {
            let constructions = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(&constructions);
            let dispatched = drive_request(
                &control_host,
                request_with_request_level_supplements(BIDDER_COUNT, false),
                &registry,
            );
            assert_eq!(
                dispatched, BIDDER_CAP,
                "the control auction must dispatch exactly the cap",
            );
        });

        // The two inputs must be the same size, or the comparison is
        // back to measuring input growth.
        let supplement_input = peak_bytes_of(|| {
            let _request = request_with_request_level_supplements(BIDDER_COUNT, true);
        });
        let control_input = peak_bytes_of(|| {
            let _request = request_with_request_level_supplements(BIDDER_COUNT, false);
        });
        let input_gap = supplement_input.abs_diff(control_input);
        // The two fixtures hold the same VOLUME but not the same SHAPE:
        // `BIDDER_COUNT` map entries against one long `String`, so the
        // supplement side pays for 24 `BTreeMap` entries, 24 one-key
        // `serde_json::Map`s and 24 separately-rounded allocations.
        // Measured, that is 18 KiB on 464 KiB — under 5%, and an order
        // of magnitude below one imp-row of eager merging. The gap is
        // allowed for explicitly and then CHARGED into
        // `funded_ceiling`, so it cannot pay for the amplification.
        let shape_allowance = supplement_input
            .max(control_input)
            .saturating_div(10)
            .max(SUPPLEMENT_BYTES);
        assert!(
            input_gap <= shape_allowance,
            "the two fixtures must be within a shape difference of each other for the \
             difference between their peaks to be the merge: supplemented input \
             {supplement_input} B against control {control_input} B, a gap of \
             {input_gap} B over the {shape_allowance} B allowed. A larger gap means \
             the two requests are no longer the same experiment",
        );

        let discarded = BIDDER_COUNT - BIDDER_CAP;
        // What an eager per-pair merge adds: one supplement copy per
        // discarded bidder per imp.
        let eager_floor = discarded
            .saturating_mul(SUPPLEMENT_IMPS)
            .saturating_mul(SUPPLEMENT_BYTES);
        // What the funded shape adds, per surviving bidder:
        //
        // * its scoped imps — one supplement copy per imp, which
        //   `build_scoped_imp` merges into a copy it is making anyway;
        // * its whole-request clone, which carries `ext.prebid` and so
        //   carries the WHOLE supplement block, all `BIDDER_COUNT`
        //   entries of it. `ext.prebid` is request-level and is not
        //   scoped per bidder, so that is the request the bidder is
        //   genuinely being sent.
        //
        // Doubled for the typed round-trip the materializer runs over
        // the same bytes, and the fixtures' own shape gap added on top.
        // The result is 181 KiB against the 901 KiB an eager per-pair
        // merge costs, so the ceiling separates the two by 5x without
        // being tuned to the measurement.
        let per_survivor = SUPPLEMENT_IMPS
            .saturating_mul(SUPPLEMENT_BYTES)
            .saturating_add(BIDDER_COUNT.saturating_mul(SUPPLEMENT_BYTES));
        let funded_ceiling = BIDDER_CAP
            .saturating_mul(per_survivor)
            .saturating_mul(2)
            .saturating_add(input_gap);
        let merge_cost = supplement_peak.saturating_sub(control_peak);
        assert!(
            merge_cost <= funded_ceiling,
            "moving {SUPPLEMENT_BYTES} B of per-bidder data from site.name to \
             ext.prebid.bidderparams, with {BIDDER_COUNT} bidders across \
             {SUPPLEMENT_IMPS} imps under a cap of {BIDDER_CAP}, raised the peak by \
             {merge_cost} B (supplemented {supplement_peak}, control \
             {control_peak}) — past the {funded_ceiling} B the surviving \
             {BIDDER_CAP} bidders' own scoped copies account for. The supplement is \
             being merged for the {discarded} bidders the cap discards; an eager \
             per-pair merge costs {eager_floor} B",
        );

        // The SEPARATION as well as the ceiling, so a regression that
        // creeps partway back still fails rather than fitting inside a
        // ceiling that happened to be loose.
        assert!(
            merge_cost.saturating_mul(2) < eager_floor,
            "request-level supplement headroom: supplemented {supplement_peak} B, \
             control {control_peak} B, so the merge cost {merge_cost} B for \
             {discarded} discarded bidders across {SUPPLEMENT_IMPS} imps. That is \
             not clearly separated from the {eager_floor} B an eager per-pair merge \
             would add, so the bound may be slipping back",
        );
    }

    /// `max_outbound_requests` is a CONSTRUCTION-TIME bound: the body of
    /// a call it refuses is never written.
    ///
    /// One bidder attempts `attempts` pushes against a budget of
    /// `OUTBOUND_CAP`. A pipeline that collects every body and
    /// truncates the vector afterwards writes all `attempts`; a sink
    /// that reserves its call slot before allocating writes
    /// `OUTBOUND_CAP`. Both dispatch `OUTBOUND_CAP` calls.
    #[test]
    fn calls_the_outbound_budget_refuses_never_write_a_body() {
        const ATTEMPTS: usize = 20;
        const OUTBOUND_CAP: usize = 3;

        let bodies_written = Arc::new(AtomicUsize::new(0));
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak_info(
                    "fake_000".to_owned(),
                    "https://demo.example.test/fake_000/openrtb".to_owned(),
                ),
                Box::new(FanoutBidder {
                    attempts: ATTEMPTS,
                    bodies_written: Arc::clone(&bodies_written),
                }),
            )
            .expect("unique test bidder");

        let host = host_with_caps(50, OUTBOUND_CAP);
        let dispatched = drive(&host, 1, &registry);
        assert_eq!(dispatched, 1, "the one bidder must dispatch");

        assert_eq!(
            bodies_written.load(Ordering::Relaxed),
            OUTBOUND_CAP,
            "an adapter attempting {ATTEMPTS} requests against an outbound budget \
             of {OUTBOUND_CAP} must be refused DURING construction: only \
             {OUTBOUND_CAP} bodies may ever be written",
        );
    }

    /// The outbound budget is ONE allowance for the whole auction, not
    /// one per bidder.
    ///
    /// Each of `BIDDERS` adapters attempts `PER_BIDDER` pushes against
    /// an auction cap of `OUTBOUND_CAP`, and `BIDDERS * PER_BIDDER` is
    /// far over it. A budget constructed once and shared — which is
    /// what the auction does — writes `OUTBOUND_CAP` bodies in total
    /// and refuses the rest, including every push by the bidders that
    /// ran after the allowance was spent. A budget constructed per
    /// bidder instead writes `min(PER_BIDDER, OUTBOUND_CAP)` for EACH
    /// of them, which is `bidders x cap` calls out of an allowance of
    /// `cap`.
    ///
    /// A single-bidder fixture cannot tell those apart: with one
    /// budget-holder the per-bidder and auction-global allowances are
    /// the same number. This row needs several bidders for that reason
    /// alone.
    #[test]
    fn the_outbound_budget_is_one_allowance_across_every_bidder() {
        const BIDDERS: usize = 4;
        // Below the cap, so no single bidder can exhaust the allowance
        // by itself: the refusals have to come from what the EARLIER
        // bidders already spent.
        const PER_BIDDER: usize = 2;
        const OUTBOUND_CAP: usize = 3;

        let bodies_written = Arc::new(AtomicUsize::new(0));
        let mut registry = BidderRegistry::new();
        for name in bidder_names().into_iter().take(BIDDERS) {
            registry
                .register(
                    leak_info(
                        name.clone(),
                        format!("https://demo.example.test/{name}/openrtb"),
                    ),
                    Box::new(FanoutBidder {
                        attempts: PER_BIDDER,
                        bodies_written: Arc::clone(&bodies_written),
                    }),
                )
                .expect("unique test bidder");
        }

        // The bidder cap is well clear of `BIDDERS`, so the only thing
        // bounding this auction is the outbound budget.
        let host = host_with_caps(50, OUTBOUND_CAP);
        // `drive` reports the bidders that ended up with a dispatched
        // call. Two is what the shared allowance produces here — the
        // first bidder takes two slots and the second takes the last
        // one — and the assertion below is what distinguishes the two
        // implementations. This one only rules out a fixture where the
        // bidder cap, not the outbound budget, did the bounding.
        let dispatched = drive(&host, BIDDERS, &registry);
        assert!(
            dispatched > 0 && dispatched < BIDDERS,
            "the fixture must be bounded by the outbound budget: {dispatched} of \
             {BIDDERS} bidders dispatched, so either nothing was refused or the \
             bidder cap refused everyone",
        );

        let written = bodies_written.load(Ordering::Relaxed);
        assert_eq!(
            written,
            OUTBOUND_CAP,
            "{BIDDERS} bidders attempting {PER_BIDDER} pushes each share ONE allowance \
             of {OUTBOUND_CAP}: a budget built per bidder would have written up to \
             {} bodies, one cap's worth for each of them",
            BIDDERS * OUTBOUND_CAP.min(PER_BIDDER),
        );
    }
}
