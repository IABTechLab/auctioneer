//! Fixtures are built before allocator windows; no raw-body access is added.
#![expect(
    unsafe_code,
    reason = "test allocator delegates to the existing TLS tracker"
)]

use crate::fake_bidder::FakeBidder;

use crate::alloc_track::{self, CountingAllocator};
use crate::support::response_budgets;
use auctioneer_bidder::{
    Bidder, BidderError, BidderHttpRequest, BidderHttpResponse, BidderName, BidderRegistry,
    BidderRequestInfo, BidderResponse, CallBudget, DebugBodySchema, EntryPoint,
    OutboundRequestMeta, OutboundRequestSink, SinkLimits,
};
use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig};
use auctioneer_exchange::debug::dto::DebugHttpCall;
use auctioneer_exchange::scrub::DisclosureAudit;
use auctioneer_exchange::{
    AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
    BidderFanout, ExchangeEndpointValidator, FanoutInternalError, ResolvedAccount, resolve_aliases,
    run_auction,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::{NormalizedBidRequest, PrepareLimits, RequestHeaders, prepare};
use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
use auctioneer_runtime::{Clock as _, FakeClock, MonotonicInstant, SeededRng};
use futures::{executor::block_on, task::noop_waker};
use serde_json::{Value, json};
use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::future::{Future as _, pending};
use std::mem::{size_of, take};
use std::ptr;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::task::Context;
use std::time::Duration;

pub struct Allocator;

pub const REQUEST_MIME_BYTES: usize = 1021;

thread_local! {
    static REFUSE: Cell<bool> = const { Cell::new(false) };
    static REFUSALS: Cell<usize> = const { Cell::new(0) };
    static WRITERS: Cell<usize> = const { Cell::new(0) };
    static MIME_SOURCES: Cell<usize> = const { Cell::new(0) };
    static REQUEST_MIME_SOURCES: Cell<usize> = const { Cell::new(0) };
    static CALL_STORAGE: Cell<usize> = const { Cell::new(0) };
    static CALL_STORAGE_BYTES: Cell<usize> = const { Cell::new(0) };
}

fn refuse(size: usize) -> bool {
    if REFUSE
        .try_with(|armed| armed.replace(false))
        .unwrap_or(false)
    {
        let _result = REFUSALS.try_with(|count| count.set(count.get().saturating_add(1)));
        return true;
    }
    if size == 16 * 1024 {
        let _result = WRITERS.try_with(|count| count.set(count.get().saturating_add(1)));
    }
    if size == 64 * 1024 {
        let _result = MIME_SOURCES.try_with(|count| count.set(count.get().saturating_add(1)));
    }
    if size == REQUEST_MIME_BYTES {
        let _result =
            REQUEST_MIME_SOURCES.try_with(|count| count.set(count.get().saturating_add(1)));
    }
    if CALL_STORAGE_BYTES
        .try_with(Cell::get)
        .is_ok_and(|bytes| bytes != 0 && size == bytes)
    {
        let _result = CALL_STORAGE.try_with(|count| count.set(count.get().saturating_add(1)));
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probes {
    pub writers: usize,
    pub mime_sources: usize,
    pub request_mime_sources: usize,
    pub call_storage: usize,
}

pub fn probes() -> Probes {
    Probes {
        writers: WRITERS.with(Cell::get),
        mime_sources: MIME_SOURCES.with(Cell::get),
        request_mime_sources: REQUEST_MIME_SOURCES.with(Cell::get),
        call_storage: CALL_STORAGE.with(Cell::get),
    }
}

impl Probes {
    fn since(self, start: Self) -> Self {
        Self {
            writers: self.writers.checked_sub(start.writers).unwrap(),
            mime_sources: self.mime_sources.checked_sub(start.mime_sources).unwrap(),
            request_mime_sources: self
                .request_mime_sources
                .checked_sub(start.request_mime_sources)
                .unwrap(),
            call_storage: self.call_storage.checked_sub(start.call_storage).unwrap(),
        }
    }
}

pub struct CallStorageProbe(usize);

impl CallStorageProbe {
    pub fn new(ceiling: u32) -> Self {
        // Row owns one bidder String and the public call DTO. A real collector
        // positive control pins this layout signature rather than assuming it.
        let bytes = usize::try_from(ceiling.min(64))
            .unwrap()
            .checked_mul(size_of::<(String, DebugHttpCall)>())
            .unwrap();
        Self(CALL_STORAGE_BYTES.with(|target| target.replace(bytes)))
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for CallStorageProbe {
    fn drop(&mut self) {
        CALL_STORAGE_BYTES.with(|target| target.set(self.0));
    }
}

// SAFETY: delegates the original pointer/layout to the existing System tracker.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if refuse(layout.size()) {
            return ptr::null_mut();
        }
        // SAFETY: the caller supplied a valid allocation layout.
        unsafe { CountingAllocator.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if refuse(layout.size()) {
            return ptr::null_mut();
        }
        // SAFETY: the caller supplied a valid allocation layout.
        unsafe { CountingAllocator.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwards the matching allocation unchanged.
        unsafe { CountingAllocator.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if refuse(new_size) {
            return ptr::null_mut();
        }
        // SAFETY: forwards the valid allocation and new size unchanged.
        unsafe { CountingAllocator.realloc(ptr, layout, new_size) }
    }
}

pub struct RefuseNext;

impl RefuseNext {
    pub fn arm() -> Self {
        REFUSALS.with(|count| count.set(0));
        REFUSE.with(|armed| assert!(!armed.replace(true)));
        Self
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for RefuseNext {
    fn drop(&mut self) {
        REFUSE.with(|armed| armed.set(false));
        assert_eq!(REFUSALS.with(Cell::get), 1);
    }
}

pub fn deadline(clock: &FakeClock) -> MonotonicInstant {
    clock.origin().checked_add(Duration::from_secs(10)).unwrap()
}

pub fn outbound(value: &Value, headers: bool) -> BidderHttpRequest {
    let endpoint = "https://bidder.example/PATH_CANARY?token=URL_CANARY#FRAGMENT_CANARY";
    let bidder = BidderInfo {
        endpoint: endpoint.into(),
        ..BidderInfo::default()
    };
    let info = BidderRequestInfo::new(
        BidderName::new("fake_bb"),
        BidderName::new("fake_bb"),
        "account",
        None,
        endpoint,
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    );
    let budget = CallBudget::new(1);
    let validator = ExchangeEndpointValidator::new(&bidder);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(64 * 1024, 1), &budget, &validator);
    sink.push_request(
        &info,
        |meta| {
            meta.set_method("POST")?;
            meta.set_url(endpoint)?;
            meta.header("Content-Type", "application/json; token=HEADER_CANARY")?;
            if headers {
                meta.header("Authorization", "AUTH_CANARY")?;
                meta.header("Proxy-Authorization", "PROXY_AUTH_CANARY")?;
                meta.header("Cookie", "COOKIE_CANARY")?;
                meta.header("X-Api-Key", "API_KEY_CANARY")?;
                meta.header("X-Forwarded-For", "FORWARDED_IP_CANARY")?;
                meta.header("CANARY-Header-Name", "CANARY_HEADER_VALUE")?;
            }
            Ok(())
        },
        |writer| writer.write_json(value),
    )
    .unwrap();
    sink.into_outcome().accepted.pop().unwrap()
}

struct Adapter {
    schema_reads: Arc<AtomicUsize>,
    request_mime: Option<String>,
}

impl Bidder for Adapter {
    fn debug_body_schema(&self) -> DebugBodySchema {
        self.schema_reads.fetch_add(1, Ordering::Relaxed);
        FakeBidder::default().debug_body_schema()
    }

    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>> {
        // Keep real request construction, with optional MIME for allocation probes.
        for imp in &request.imp {
            let scoped = BidRequest {
                id: request.id.clone(),
                imp: vec![imp.clone()],
                site: request.site.clone(),
                ..BidRequest::default()
            };
            if let Some(mime) = &self.request_mime {
                let _outcome = sink.push_request(
                    info,
                    |meta| {
                        meta.set_method("POST")?;
                        meta.set_url(info.endpoint_template)?;
                        meta.header("Content-Type", mime)?;
                        meta.imp_id(&imp.id)
                    },
                    |writer| writer.write_json(&scoped),
                );
            } else {
                FakeBidder::default().make_requests(&scoped, info, sink)?;
            }
        }
        Ok(())
    }

    fn make_bids(
        &self,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        FakeBidder::default().make_bids(request, sent, response)
    }
}

struct Fanout {
    results: Mutex<Vec<BidderCallResult>>,
    request_end: Mutex<Probes>,
    response_start: Mutex<Probes>,
    suspended: bool,
    dispatched: AtomicUsize,
}

#[async_trait::async_trait(?Send)]
impl BidderFanout for Fanout {
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        _: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
        *self.request_end.lock().unwrap() = probes();
        self.dispatched.store(calls.len(), Ordering::Relaxed);
        if self.suspended {
            pending::<()>().await;
        }
        // Keep actual outbound storage alive across suspension, just as transport does.
        drop(calls);
        *self.response_start.lock().unwrap() = probes();
        Ok(take(&mut *self.results.lock().unwrap()))
    }
}

pub struct Engine {
    requested: bool,
    account_allow: bool,
    suspended: bool,
    host: HostConfig,
    registry: BidderRegistry,
    schema_reads: Arc<AtomicUsize>,
    request: NormalizedBidRequest,
    results: Vec<BidderCallResult>,
}

pub struct EngineObserved {
    pub response: Option<Value>,
    pub peak: u64,
    pub schema_reads: usize,
    pub request_probes: Probes,
    pub response_probes: Probes,
    pub dispatched: usize,
    pub released: bool,
}

impl Engine {
    pub fn set_permissions(&mut self, requested: bool, account_allow: bool) {
        self.requested = requested;
        self.account_allow = account_allow;
    }

    pub fn suspend(&mut self) {
        self.suspended = true;
    }

    pub fn new(
        calls: usize,
        ceiling: u32,
        base_allow: bool,
        results: Vec<BidderCallResult>,
    ) -> Self {
        Self::with_request_mime(calls, ceiling, base_allow, results, None)
    }

    pub fn with_request_mime(
        calls: usize,
        ceiling: u32,
        base_allow: bool,
        results: Vec<BidderCallResult>,
        request_mime: Option<String>,
    ) -> Self {
        let alias = "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        host.resource_limits.max_outbound_requests = usize::try_from(ceiling).unwrap();
        host.resource_limits.max_outbound_raw_bytes = 64 * 1024;
        host.resource_limits.max_outbound_request_bytes = 64 * 1024;
        host.resource_limits.max_parsed_nodes = 4096;
        let request = prepare(
            serde_json::from_value(json!({
                "id": "ENGINE_REQUEST_CANARY", "site": {"id": "ENGINE_SITE_CANARY"},
                "imp": (0..calls).map(|index| json!({
                    "id": format!("imp-{index}"), "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"bidder": {alias.clone(): {}}}
                })).collect::<Vec<_>>(),
                "ext": {"prebid": {"aliases": {alias: "fake_bb"}}}
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 100,
                ..PrepareLimits::default()
            },
        )
        .unwrap();
        let schema_reads = Arc::new(AtomicUsize::new(0));
        let mut registry = BidderRegistry::new();
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: "fake_bb".into(),
                    endpoint: "https://bidder.example/private?token=ENGINE_URL_CANARY".into(),
                    debug_allow: base_allow,
                    ..BidderInfo::default()
                })),
                Box::new(Adapter {
                    schema_reads: Arc::clone(&schema_reads),
                    request_mime,
                }),
            )
            .expect("unique test bidder");
        Self {
            requested: true,
            account_allow: true,
            suspended: false,
            host,
            registry,
            schema_reads,
            request,
            results,
        }
    }

    pub fn run(self) -> EngineObserved {
        let config = AccountConfig {
            debug_allow: self.account_allow,
            ..AccountConfig::default()
        };
        let account = ResolvedAccount {
            id: "account".into(),
            source: AccountSource::DefaultFallback,
            config: &config,
        };
        let plan = AuctionPlan::new(
            resolve_aliases(
                &self.request,
                &self.registry,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap(),
            BTreeSet::new(),
            response_budgets(&self.registry, &self.host),
            BTreeMap::new(),
        );
        let fanout = Fanout {
            results: Mutex::new(self.results),
            request_end: Mutex::new(probes()),
            response_start: Mutex::new(probes()),
            suspended: self.suspended,
            dispatched: AtomicUsize::new(0),
        };
        // Darwin lazily allocates mutex backing storage; it belongs to the fixture.
        drop(fanout.request_end.lock().unwrap());
        drop(fanout.response_start.lock().unwrap());
        drop(fanout.results.lock().unwrap());
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &self.host,
            &account,
            &self.registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            self.requested,
        );
        let mut audit = DisclosureAudit::default();
        let waker = noop_waker();
        let mut task = Context::from_waker(&waker);
        let _storage_probe = CallStorageProbe::new(
            u32::try_from(self.host.resource_limits.max_outbound_requests).unwrap(),
        );
        let request_start = probes();
        let before = alloc_track::reset_peak();
        let response = if self.suspended {
            let mut future = Box::pin(run_auction(&self.request, &ctx, &plan, &mut audit));
            assert!(future.as_mut().poll(&mut task).is_pending());
            drop(future);
            None
        } else {
            Some(
                serde_json::to_value(
                    block_on(run_auction(&self.request, &ctx, &plan, &mut audit))
                        .unwrap()
                        .into_bid_response()
                        .unwrap(),
                )
                .unwrap(),
            )
        };
        let peak = alloc_track::peak_since(before);
        let released = alloc_track::live() == before;
        let start = *fanout.response_start.lock().unwrap();
        let end = probes();
        EngineObserved {
            response,
            peak,
            released,
            schema_reads: self.schema_reads.load(Ordering::Relaxed),
            request_probes: fanout.request_end.lock().unwrap().since(request_start),
            dispatched: fanout.dispatched.load(Ordering::Relaxed),
            response_probes: end.since(start),
        }
    }
}
